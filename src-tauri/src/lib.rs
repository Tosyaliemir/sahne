mod artwork;
mod error;
mod events;
mod lyrics;
mod paths;
mod player;

use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use artwork::ArtworkService;
use error::AppResult;
use events::{LyricsLoaded, PaletteChanged, PlayerSnapshot};
use lyrics::LyricsService;
use paths::Paths;
use player::PlayerState;

type Player<'a> = State<'a, Arc<PlayerState>>;
type Lyrics<'a> = State<'a, Arc<LyricsService>>;
type Artwork<'a> = State<'a, Arc<ArtworkService>>;

#[tauri::command]
fn get_state(player: Player<'_>) -> PlayerSnapshot {
    player.snapshot()
}

#[tauri::command]
async fn play_pause(player: Player<'_>) -> AppResult<()> {
    player.play_pause().await
}

#[tauri::command]
async fn next(player: Player<'_>) -> AppResult<()> {
    player.next().await
}

#[tauri::command]
async fn previous(player: Player<'_>) -> AppResult<()> {
    player.previous().await
}

#[tauri::command]
async fn seek(player: Player<'_>, position_ms: u64) -> AppResult<()> {
    player.seek(position_ms).await
}

#[tauri::command]
fn get_lyrics(lyrics: Lyrics<'_>) -> Option<LyricsLoaded> {
    lyrics.current()
}

#[tauri::command]
fn get_palette(artwork: Artwork<'_>) -> Option<PaletteChanged> {
    artwork.current()
}

#[tauri::command]
fn set_lyrics_offset(lyrics: Lyrics<'_>, track_id: String, offset_ms: i64) {
    lyrics.set_offset(&track_id, offset_ms);
}

#[tauri::command]
fn retry_lyrics(app: AppHandle, lyrics: Lyrics<'_>) {
    lyrics.retry(&app);
}

/// Lets the webview report errors into our log, where they are visible.
#[tauri::command]
fn frontend_log(level: String, message: String) {
    match level.as_str() {
        "error" => tracing::error!(target: "frontend", "{message}"),
        "warn" => tracing::warn!(target: "frontend", "{message}"),
        _ => tracing::info!(target: "frontend", "{message}"),
    }
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("sahne_lib=info,frontend=info,warn")
    });
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

/// WebKitGTK's DMA-BUF renderer dies with "Error 71 (Protocol error)" on
/// Wayland with the NVIDIA driver. Fall back to the older path there unless
/// the user has made their own choice.
#[cfg(target_os = "linux")]
fn apply_webkit_workarounds() {
    const VAR: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";
    if std::env::var_os(VAR).is_none()
        && std::path::Path::new("/proc/driver/nvidia/version").exists()
    {
        tracing::info!("NVIDIA driver detected, setting {VAR}=1");
        std::env::set_var(VAR, "1");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();
    #[cfg(target_os = "linux")]
    apply_webkit_workarounds();

    let paths = Paths::from_xdg().unwrap_or_else(|| {
        tracing::warn!("no home directory; using a temporary location for settings and caches");
        Paths::temporary()
    });
    // Until the settings panel exists, SAHNE_PLAYER selects a non-Spotify player
    // (e.g. `SAHNE_PLAYER=sahnemock` for the mock player).
    let preferred = std::env::var("SAHNE_PLAYER").ok().filter(|s| !s.is_empty());
    let player = Arc::new(PlayerState::new(preferred));
    let lyrics = Arc::new(LyricsService::new(&paths));
    let artwork = Arc::new(ArtworkService::new(&paths));

    let result = tauri::Builder::default()
        .manage(player.clone())
        .manage(lyrics.clone())
        .manage(artwork.clone())
        .setup(move |app| {
            // Development aid: tell test instances apart from the everyday one.
            if let (Ok(title), Some(window)) =
                (std::env::var("SAHNE_TITLE"), app.get_webview_window("main"))
            {
                let _ = window.set_title(&title);
            }
            let handle = app.handle().clone();
            lyrics.spawn(handle.clone(), player.subscribe_tracks());
            artwork.spawn(handle.clone(), player.subscribe_tracks());
            player::spawn(handle, player);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            play_pause,
            next,
            previous,
            seek,
            get_lyrics,
            get_palette,
            set_lyrics_offset,
            retry_lyrics,
            frontend_log
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        tracing::error!("failed to start Sahne: {e}");
        std::process::exit(1);
    }
}
