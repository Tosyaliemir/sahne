mod artwork;
mod error;
mod events;
mod idle;
mod lyrics;
mod paths;
mod player;
mod screensaver;
mod settings;
mod windows;

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, State};

use artwork::ArtworkService;
use error::AppResult;
use events::{LyricsLoaded, PaletteChanged, PlayerSnapshot};
use idle::IdleMonitor;
use lyrics::LyricsService;
use paths::Paths;
use player::PlayerState;
use settings::{Settings, SettingsStore};

type Player<'a> = State<'a, Arc<PlayerState>>;
type Lyrics<'a> = State<'a, Arc<LyricsService>>;
type Artwork<'a> = State<'a, Arc<ArtworkService>>;
type SettingsState<'a> = State<'a, Arc<SettingsStore>>;

const SETTINGS_CHANGED: &str = "settings-changed";

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

#[tauri::command]
fn get_settings(settings: SettingsState<'_>) -> Settings {
    settings.get()
}

/// Persists settings, applies the backend-relevant parts and tells every window.
#[tauri::command]
fn save_settings(
    app: AppHandle,
    settings: SettingsState<'_>,
    player: Player<'_>,
    idle: State<'_, Arc<IdleMonitor>>,
    new_settings: Settings,
) -> Settings {
    let saved = settings.set(new_settings);
    player.set_preferred(effective_preferred_player(&saved));
    screensaver::apply_settings(&idle, &settings);
    if let Err(e) = app.emit(SETTINGS_CHANGED, saved.clone()) {
        tracing::warn!("failed to emit settings: {e}");
    }
    saved
}

#[tauri::command]
async fn list_players() -> AppResult<Vec<String>> {
    player::list_players().await
}

#[tauri::command]
fn set_mini_mode(app: AppHandle, settings: SettingsState<'_>, enabled: bool) -> Result<(), String> {
    windows::set_mini_mode(&app, &settings, enabled).map_err(|e| e.to_string())
}

/// `SAHNE_PLAYER` (handy for the mock player) overrides the saved preference.
fn effective_preferred_player(settings: &Settings) -> Option<String> {
    std::env::var("SAHNE_PLAYER")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| settings.preferred_player.clone())
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

/// With the NVIDIA driver on Wayland, WebKitGTK's GPU (DMA-BUF) renderer dies
/// with "Error 71 (Protocol error)" because of explicit sync. Turning explicit
/// sync off keeps GPU rendering; disabling the DMA-BUF renderer instead (the
/// common advice) falls back to slow CPU copies and stutters at full screen.
#[cfg(target_os = "linux")]
fn apply_webkit_workarounds() {
    const VAR: &str = "__NV_DISABLE_EXPLICIT_SYNC";
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
    let settings = Arc::new(SettingsStore::load(&paths));
    let initial = settings.get();
    let player = Arc::new(PlayerState::new(effective_preferred_player(&initial)));
    let lyrics = Arc::new(LyricsService::new(&paths));
    let artwork = Arc::new(ArtworkService::new(&paths));
    let idle = Arc::new(IdleMonitor::start(std::time::Duration::from_secs(180)));
    screensaver::apply_settings(&idle, &settings);

    let result = tauri::Builder::default()
        .manage(player.clone())
        .manage(lyrics.clone())
        .manage(artwork.clone())
        .manage(settings.clone())
        .manage(idle.clone())
        .setup(move |app| {
            // Development aid: tell test instances apart from the everyday one.
            if let (Ok(title), Some(window)) = (
                std::env::var("SAHNE_TITLE"),
                app.get_webview_window(windows::MAIN),
            ) {
                let _ = window.set_title(&title);
            }
            if initial.start_fullscreen {
                if let Some(window) = app.get_webview_window(windows::MAIN) {
                    let _ = window.set_fullscreen(true);
                }
            }
            let handle = app.handle().clone();
            screensaver::spawn(handle.clone(), settings.clone(), player.clone(), idle);
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
            get_settings,
            save_settings,
            list_players,
            set_mini_mode,
            frontend_log
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        tracing::error!("failed to start Sahne: {e}");
        std::process::exit(1);
    }
}
