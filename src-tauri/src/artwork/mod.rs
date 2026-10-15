//! Downloads and caches album art, extracts a palette and renders a small
//! pre-blurred background image so the UI never blurs full-screen per frame.

pub mod palette;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use image::DynamicImage;
use tauri::{AppHandle, Emitter};
use tokio::sync::watch;

use crate::events::{self, PaletteChanged, TrackInfo};
use crate::paths::{self, Paths};
use palette::Palette;

const MAX_ART_BYTES: usize = 15 * 1024 * 1024;
/// Side of the pre-blurred background; the UI scales it up, which smooths it
/// further. Large enough to keep the artwork's color regions recognisable.
const BG_SIZE: u32 = 192;
const BG_SIGMA: f32 = 9.0;
/// While offline, try the download again this often (as long as the track plays).
/// Retry delays after a network failure: quick at first (usually a blip),
/// then settling at the maximum.
const OFFLINE_RETRY_MIN: Duration = Duration::from_secs(1);
const OFFLINE_RETRY_MAX: Duration = Duration::from_secs(20);

#[derive(Debug, thiserror::Error)]
enum ArtError {
    #[error("download failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("artwork too large")]
    TooLarge,
    #[error("unsupported artwork URL {0}")]
    Unsupported(String),
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
}

impl ArtworkService {
    pub fn new(paths: &Paths) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            // Covers come from the same CDN track after track; reuse the connection.
            .pool_idle_timeout(Duration::from_secs(600))
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .map_err(|e| tracing::error!("HTTP client unavailable: {e}"))
            .ok();
        Self {
            http,
            dir: paths.art_cache(),
            current: Mutex::new(None),
            generation: AtomicU64::new(0),
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
                        tauri::async_runtime::spawn(async move {
                            this.load(&app, generation, track).await
                        });
                    }
                }
                if tracks.changed().await.is_err() {
                    break;
                }
            }
        });
    }

    async fn load(&self, app: &AppHandle, generation: u64, track: TrackInfo) {
        let mut retry = OFFLINE_RETRY_MIN;
        let mut failures = 0u32;
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
            // A first network blip is retried before giving up on the colors,
            // so the screen doesn't flash gray for a moment.
            if offline {
                failures += 1;
            }
            if offline && failures == 1 {
                tokio::time::sleep(retry).await;
                retry = (retry * 2).min(OFFLINE_RETRY_MAX);
                if self.generation.load(Ordering::SeqCst) != generation {
                    return;
                }
                continue;
            }
            *lock(&self.current) = Some(payload.clone());
            tracing::debug!("palette for {}: {:?}", track.title, payload.palette);
            if let Err(e) = app.emit(events::PALETTE_CHANGED, payload) {
                tracing::warn!("failed to emit palette: {e}");
            }
            if !offline {
                return;
            }
            tokio::time::sleep(retry).await;
            retry = (retry * 2).min(OFFLINE_RETRY_MAX);
            if self.generation.load(Ordering::SeqCst) != generation {
                return;
            }
        }
    }

    /// Returns (artwork path, blurred background path, palette).
    async fn prepare(&self, url: &str) -> Result<(String, String, Palette), ArtError> {
        let key = paths::stable_hash(url);
        let art_path = self.dir.join(format!("{key}.img"));
        // `-bg2`: regenerated when the backdrop rendering changed.
        let bg_path = self.dir.join(format!("{key}-bg2.png"));

        let bytes = if let Some(local) = url.strip_prefix("file://") {
            let path = percent_decode(local);
            tokio::fs::read(&path).await?
        } else if let Ok(cached) = tokio::fs::read(&art_path).await {
            cached
        } else if url.starts_with("https://") || url.starts_with("http://") {
            let bytes = self.download(url).await?;
            paths::write_atomic(&art_path, &bytes)?;
            bytes
        } else {
            return Err(ArtError::Unsupported(url.to_owned()));
        };
        // Local files are served from the cache too, so one asset scope covers everything.
        if url.starts_with("file://") && !art_path.exists() {
            paths::write_atomic(&art_path, &bytes)?;
        }

        let bg_exists = bg_path.exists();
        let bg_for_task = bg_path.clone();
        let palette = tokio::task::spawn_blocking(move || -> Result<Palette, ArtError> {
            let img = image::load_from_memory(&bytes)?;
            if !bg_exists {
                write_background(&img, &bg_for_task)?;
            }
            Ok(palette::extract(&img))
        })
        .await
        .map_err(|_| ArtError::Join)??;

        Ok((path_string(&art_path), path_string(&bg_path), palette))
    }

    async fn download(&self, url: &str) -> Result<Vec<u8>, ArtError> {
        let http = self
            .http
            .as_ref()
            .ok_or_else(|| ArtError::Unsupported(url.to_owned()))?;
        let resp = http.get(url).send().await?.error_for_status()?;
        if resp
            .content_length()
            .is_some_and(|n| n as usize > MAX_ART_BYTES)
        {
            return Err(ArtError::TooLarge);
        }
        let bytes = resp.bytes().await?;
        if bytes.len() > MAX_ART_BYTES {
            return Err(ArtError::TooLarge);
        }
        Ok(bytes.to_vec())
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

/// Decodes `%XX` escapes in file:// URLs (players escape spaces and non-ASCII).
fn percent_decode(s: &str) -> PathBuf {
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h << 4 | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    bytes_to_path(out)
}

/// Unix paths are arbitrary bytes; Windows paths here are always UTF-8
/// (we build those URLs ourselves from the cache directory).
#[cfg(unix)]
fn bytes_to_path(bytes: Vec<u8>) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    PathBuf::from(std::ffi::OsString::from_vec(bytes))
}

#[cfg(not(unix))]
fn bytes_to_path(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(&bytes).into_owned())
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_file_urls() {
        assert_eq!(
            percent_decode("/home/k/M%C3%BCzik/a%20b.jpg"),
            PathBuf::from("/home/k/Müzik/a b.jpg")
        );
        assert_eq!(percent_decode("/tmp/100%"), PathBuf::from("/tmp/100%"));
        assert_eq!(percent_decode("/tmp/%zz"), PathBuf::from("/tmp/%zz"));
    }
}
