//! Downloads and caches album art, extracts a palette and renders a small
//! pre-blurred background image so the UI never blurs full-screen per frame.
//!
//! Art URLs come from whatever MPRIS client is on the session bus, so they
//! are untrusted: remote art must be https from a public host (see
//! [`crate::net`]), local files must be small regular image files outside
//! kernel pseudo-filesystems, and decoding runs under size limits.

pub mod palette;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use image::DynamicImage;
use reqwest::Url;
use tauri::{AppHandle, Emitter};
use tokio::sync::watch;

use crate::events::{self, PaletteChanged, TrackInfo};
use crate::net;
use crate::paths::{self, Paths};
use crate::sync::{lock, Inflight};
use palette::Palette;

const MAX_ART_BYTES: usize = 15 * 1024 * 1024;
/// Decoding limits against decompression bombs.
const MAX_ART_SIDE: u32 = 8192;
const MAX_DECODE_ALLOC: u64 = 128 * 1024 * 1024;
/// The decoded art is shrunk to this once; palette and background both use it.
const WORK_SIZE: u32 = 256;
/// Side of the pre-blurred background; the UI scales it up, which smooths it
/// further. Large enough to keep the artwork's color regions recognisable.
const BG_SIZE: u32 = 192;
const BG_SIGMA: f32 = 9.0;
/// While offline, try the download again this often (as long as the track plays).
const OFFLINE_RETRY: Duration = Duration::from_secs(20);

#[derive(Debug, thiserror::Error)]
enum ArtError {
    #[error("download failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("artwork too large")]
    TooLarge,
    #[error("unsupported artwork URL {0}")]
    Unsupported(String),
    #[error("artwork URL rejected: {0}")]
    Url(#[from] net::UrlError),
    #[error("local artwork rejected: {0}")]
    Rejected(&'static str),
    #[error("not a PNG, JPEG or WebP image")]
    NotImage,
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not decode artwork: {0}")]
    Decode(#[from] image::ImageError),
    #[error("background task failed")]
    Join,
}

pub struct ArtworkService {
    http: Option<reqwest::Client>,
    dir: PathBuf,
    current: Mutex<Option<PaletteChanged>>,
    generation: AtomicU64,
    /// The current track's download/decode; aborted when the track changes.
    inflight: Inflight,
}

impl ArtworkService {
    pub fn new(paths: &Paths) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(net::user_agent())
            .timeout(Duration::from_secs(15))
            .https_only(true)
            .redirect(net::redirect_policy())
            .dns_resolver(net::PublicResolver)
            .build()
            .map_err(|e| tracing::error!("HTTP client unavailable: {e}"))
            .ok();
        Self {
            http,
            dir: paths.art_cache(),
            current: Mutex::new(None),
            generation: AtomicU64::new(0),
            inflight: Inflight::default(),
        }
    }

    pub fn current(&self) -> Option<PaletteChanged> {
        lock(&self.current).clone()
    }

    pub fn spawn(self: Arc<Self>, app: AppHandle, mut tracks: watch::Receiver<Option<TrackInfo>>) {
        tauri::async_runtime::spawn(async move {
            let mut last: Option<(String, Option<String>)> = None;
            loop {
                let track = tracks.borrow_and_update().clone();
                let key = track.as_ref().map(|t| (t.id.clone(), t.art_url.clone()));
                // Spotify sometimes sends the art URL in a second metadata update.
                if key != last {
                    last = key;
                    if let Some(track) = track {
                        let this = Arc::clone(&self);
                        let app = app.clone();
                        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
                        let task = tauri::async_runtime::spawn(async move {
                            this.load(&app, generation, track).await
                        });
                        self.inflight.replace(Some(task));
                    }
                }
                if tracks.changed().await.is_err() {
                    break;
                }
            }
        });
    }

    async fn load(&self, app: &AppHandle, generation: u64, track: TrackInfo) {
        loop {
            let result = match &track.art_url {
                Some(url) => self.prepare(url).await.map(Some),
                None => Ok(None),
            };
            // Network trouble (not an HTTP error status) is worth retrying later.
            let offline = matches!(&result, Err(ArtError::Http(e)) if !e.is_status());
            let payload = match result {
                Ok(Some((art, background, palette))) => PaletteChanged {
                    track_id: track.id.clone(),
                    art_path: Some(art),
                    background_path: Some(background),
                    palette,
                },
                Ok(None) => PaletteChanged::empty(&track.id),
                Err(e) => {
                    tracing::warn!("artwork for {}: {e}", track.title);
                    PaletteChanged::empty(&track.id)
                }
            };
            if self.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            *lock(&self.current) = Some(payload.clone());
            tracing::debug!("palette for {}: {:?}", track.title, payload.palette);
            if let Err(e) = app.emit(events::PALETTE_CHANGED, payload) {
                tracing::warn!("failed to emit palette: {e}");
            }
            if !offline {
                return;
            }
            tokio::time::sleep(OFFLINE_RETRY).await;
            if self.generation.load(Ordering::SeqCst) != generation {
                return;
            }
        }
    }

    /// Returns (artwork path, blurred background path, palette).
    async fn prepare(&self, url: &str) -> Result<(String, String, Palette), ArtError> {
        let (key, bytes) = match art_source(url)? {
            ArtSource::Local(path) => {
                let max = MAX_ART_BYTES;
                tokio::task::spawn_blocking(move || read_local_art(&path, max))
                    .await
                    .map_err(|_| ArtError::Join)??
            }
            ArtSource::Remote(remote) => {
                let key = paths::stable_hash(remote.as_str());
                let cached = self.dir.join(format!("{key}.img"));
                let bytes = match tokio::fs::read(&cached).await {
                    Ok(b) => {
                        crate::cache::touch(&cached);
                        b
                    }
                    Err(_) => self.download(&remote).await?,
                };
                (key, bytes)
            }
        };
        let art_path = self.dir.join(format!("{key}.img"));
        // `-bg2`: regenerated when the backdrop rendering changed.
        let bg_path = self.dir.join(format!("{key}-bg2.png"));
        // Local files are copied into the cache too, so one asset scope covers
        // everything. The key changes with the file's content, so a player
        // reusing one temp path for every cover gets fresh art each time.
        if !art_path.exists() {
            paths::write_atomic(&art_path, &bytes)?;
        }

        let bg_exists = bg_path.exists();
        if bg_exists {
            crate::cache::touch(&bg_path);
        }
        let bg_for_task = bg_path.clone();
        let palette = tokio::task::spawn_blocking(move || -> Result<Palette, ArtError> {
            let img = decode_limited(&bytes)?;
            // One downscale feeds both the palette and the blur.
            let small = img.thumbnail(WORK_SIZE, WORK_SIZE);
            drop(img);
            if !bg_exists {
                write_background(&small, &bg_for_task)?;
            }
            Ok(palette::extract(&small))
        })
        .await
        .map_err(|_| ArtError::Join)??;

        Ok((path_string(&art_path), path_string(&bg_path), palette))
    }

    async fn download(&self, url: &Url) -> Result<Vec<u8>, ArtError> {
        let http = self
            .http
            .as_ref()
            .ok_or_else(|| ArtError::Unsupported(url.to_string()))?;
        let resp = http.get(url.clone()).send().await?.error_for_status()?;
        let bytes = net::read_capped(resp, MAX_ART_BYTES)
            .await
            .map_err(|e| match e {
                net::ReadError::TooLarge(_) => ArtError::TooLarge,
                net::ReadError::Http(e) => ArtError::Http(e),
            })?;
        if !has_image_signature(&bytes) {
            return Err(ArtError::NotImage);
        }
        Ok(bytes)
    }
}

fn write_background(img: &DynamicImage, path: &Path) -> Result<(), ArtError> {
    let small = img
        .resize_exact(BG_SIZE, BG_SIZE, image::imageops::FilterType::Triangle)
        .to_rgb8();
    let blurred = image::imageops::blur(&small, BG_SIGMA);
    let mut out = std::io::Cursor::new(Vec::new());
    blurred.write_to(&mut out, image::ImageFormat::Png)?;
    paths::write_atomic(path, out.get_ref())?;
    Ok(())
}

fn path_string(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Where a player's art URL points.
#[derive(Debug, PartialEq)]
enum ArtSource {
    Local(PathBuf),
    Remote(Url),
}

fn art_source(url: &str) -> Result<ArtSource, ArtError> {
    let parsed = Url::parse(url).map_err(|_| ArtError::Unsupported(url.to_owned()))?;
    match parsed.scheme() {
        // Handles %-escapes and, on Windows, drive letters.
        "file" => parsed
            .to_file_path()
            .map(ArtSource::Local)
            .map_err(|()| ArtError::Unsupported(url.to_owned())),
        "https" | "http" => {
            net::check_remote_url(&parsed)?;
            Ok(ArtSource::Remote(parsed))
        }
        _ => Err(ArtError::Unsupported(url.to_owned())),
    }
}

/// Reads a local art file named by an untrusted URL. Only regular image files
/// up to `max` bytes are accepted, never kernel pseudo-files. Returns a cache
/// key that changes when the file's content does, and the bytes.
fn read_local_art(path: &Path, max: usize) -> Result<(String, Vec<u8>), ArtError> {
    let canon = std::fs::canonicalize(path)?;
    if ["/proc", "/sys", "/dev"]
        .iter()
        .any(|p| canon.starts_with(p))
    {
        return Err(ArtError::Rejected("system path"));
    }
    let meta = std::fs::metadata(&canon)?;
    if !meta.is_file() {
        return Err(ArtError::Rejected("not a regular file"));
    }
    if usize::try_from(meta.len()).map_or(true, |n| n > max) {
        return Err(ArtError::TooLarge);
    }
    let bytes = std::fs::read(&canon)?;
    if bytes.len() > max {
        return Err(ArtError::TooLarge);
    }
    if !has_image_signature(&bytes) {
        return Err(ArtError::NotImage);
    }
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    let key = paths::stable_hash(&format!("{}|{}|{mtime}", canon.display(), meta.len()));
    Ok((key, bytes))
}

/// PNG, JPEG or WebP magic bytes.
fn has_image_signature(b: &[u8]) -> bool {
    b.starts_with(b"\x89PNG\r\n\x1a\n")
        || b.starts_with(&[0xFF, 0xD8, 0xFF])
        || (b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP")
}

/// Decodes with dimension and allocation limits (decompression bombs).
fn decode_limited(bytes: &[u8]) -> Result<DynamicImage, ArtError> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_ART_SIDE);
    limits.max_image_height = Some(MAX_ART_SIDE);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    Ok(reader.decode()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(width, height, image::Rgb(rgb));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sahne-art-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(unix)]
    #[test]
    fn file_urls_are_decoded() {
        assert_eq!(
            art_source("file:///home/k/M%C3%BCzik/a%20b.jpg").unwrap(),
            ArtSource::Local(PathBuf::from("/home/k/Müzik/a b.jpg"))
        );
    }

    // C7: a file URL built from a path with `%` and non-ASCII characters
    // (as the Windows backend does) decodes back to the same path.
    #[test]
    fn file_url_round_trip_with_odd_characters() {
        let dir = scratch("odd %25 ü İ");
        let path = dir.join("cover 100% şarkı.png");
        std::fs::write(&path, png(2, 2, [5, 6, 7])).unwrap();
        let url = Url::from_file_path(&path).unwrap();
        assert_eq!(
            art_source(url.as_str()).unwrap(),
            ArtSource::Local(path.clone())
        );
        assert!(read_local_art(&path, MAX_ART_BYTES).is_ok());
        std::fs::remove_dir_all(dir).unwrap();
    }

    // S4: plain http and private hosts are refused before any request.
    #[test]
    fn remote_art_must_be_https_and_public() {
        assert!(matches!(
            art_source("https://i.scdn.co/image/ab67"),
            Ok(ArtSource::Remote(_))
        ));
        assert!(matches!(
            art_source("http://i.scdn.co/image/ab67"),
            Err(ArtError::Url(net::UrlError::NotHttps))
        ));
        assert!(matches!(
            art_source("https://10.0.0.5/cover.jpg"),
            Err(ArtError::Url(net::UrlError::PrivateHost))
        ));
        assert!(matches!(
            art_source("data:image/png;base64,AAAA"),
            Err(ArtError::Unsupported(_))
        ));
    }

    // S4: a redirect may not downgrade to http (or leave the public internet).
    #[tokio::test]
    async fn redirect_to_http_is_refused() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await;
            let resp = format!(
                "HTTP/1.1 302 Found\r\nLocation: http://{addr}/next\r\nContent-Length: 0\r\n\r\n"
            );
            let _ = sock.write_all(resp.as_bytes()).await;
        });
        // Not https_only here, so the first hop to the local server is allowed;
        // the policy alone must stop the redirect.
        let client = reqwest::Client::builder()
            .redirect(net::redirect_policy())
            .build()
            .unwrap();
        let err = client
            .get(format!("http://{addr}/"))
            .send()
            .await
            .unwrap_err();
        assert!(err.is_redirect(), "{err}");
    }

    // S3: only regular, small image files are read.
    #[cfg(target_os = "linux")]
    #[test]
    fn system_paths_are_rejected() {
        assert!(matches!(
            read_local_art(Path::new("/proc/self/status"), MAX_ART_BYTES),
            Err(ArtError::Rejected(_))
        ));
        assert!(matches!(
            read_local_art(Path::new("/dev/null"), MAX_ART_BYTES),
            Err(ArtError::Rejected(_))
        ));
    }

    #[test]
    fn oversize_and_non_image_files_are_rejected() {
        let dir = scratch("reject");
        let big = dir.join("big.png");
        std::fs::File::create(&big)
            .unwrap()
            .set_len(16 * 1024 * 1024)
            .unwrap();
        assert!(matches!(
            read_local_art(&big, MAX_ART_BYTES),
            Err(ArtError::TooLarge)
        ));
        let text = dir.join("notes.jpg");
        std::fs::write(&text, b"user:secret\n").unwrap();
        assert!(matches!(
            read_local_art(&text, MAX_ART_BYTES),
            Err(ArtError::NotImage)
        ));
        assert!(matches!(
            read_local_art(&dir, MAX_ART_BYTES),
            Err(ArtError::Rejected(_))
        ));
        let ok = dir.join("ok.png");
        std::fs::write(&ok, png(4, 4, [1, 2, 3])).unwrap();
        assert!(read_local_art(&ok, MAX_ART_BYTES).is_ok());
        std::fs::remove_dir_all(dir).unwrap();
    }

    // C1: a player reusing one temp file must not get the previous cover.
    #[test]
    fn rewritten_local_file_gets_a_new_key() {
        let dir = scratch("rewrite");
        let path = dir.join("cover.png");
        std::fs::write(&path, png(4, 4, [200, 0, 0])).unwrap();
        let (first, _) = read_local_art(&path, MAX_ART_BYTES).unwrap();
        std::fs::write(&path, png(9, 9, [0, 0, 200])).unwrap();
        let (second, bytes) = read_local_art(&path, MAX_ART_BYTES).unwrap();
        assert_ne!(first, second);
        assert_eq!(bytes, png(9, 9, [0, 0, 200]));
        std::fs::remove_dir_all(dir).unwrap();
    }

    // S2: a header claiming a huge image is refused before allocating it.
    #[test]
    fn decompression_bomb_header_is_refused() {
        fn chunk(kind: &[u8], data: &[u8]) -> Vec<u8> {
            let mut c = (data.len() as u32).to_be_bytes().to_vec();
            c.extend_from_slice(kind);
            c.extend_from_slice(data);
            let mut crc_input = kind.to_vec();
            crc_input.extend_from_slice(data);
            c.extend_from_slice(&crc32(&crc_input).to_be_bytes());
            c
        }
        fn crc32(data: &[u8]) -> u32 {
            let mut crc = 0xFFFF_FFFFu32;
            for &b in data {
                crc ^= u32::from(b);
                for _ in 0..8 {
                    crc = if crc & 1 != 0 {
                        (crc >> 1) ^ 0xEDB8_8320
                    } else {
                        crc >> 1
                    };
                }
            }
            !crc
        }
        let mut ihdr = Vec::new();
        // 10000² RGBA = 400 MB: under the library's default 512 MB cap, so
        // only our own limits stop it before the buffer is allocated.
        ihdr.extend_from_slice(&10_000u32.to_be_bytes());
        ihdr.extend_from_slice(&10_000u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
        let mut bomb = b"\x89PNG\r\n\x1a\n".to_vec();
        bomb.extend(chunk(b"IHDR", &ihdr));
        bomb.extend(chunk(b"IEND", &[]));
        assert!(matches!(
            decode_limited(&bomb),
            Err(ArtError::Decode(image::ImageError::Limits(_)))
        ));
        assert!(decode_limited(&png(8, 8, [9, 9, 9])).is_ok());
    }

    #[test]
    fn image_signatures() {
        assert!(has_image_signature(&png(1, 1, [0, 0, 0])));
        assert!(has_image_signature(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(has_image_signature(b"RIFF\0\0\0\0WEBPVP8 "));
        assert!(!has_image_signature(b"<svg"));
    }
}
