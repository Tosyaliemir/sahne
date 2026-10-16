mod artwork;
mod cache;
mod error;
mod events;
mod idle;
mod lyrics;
mod net;
mod paths;
mod player;
mod screensaver;
mod settings;
mod sync;
mod windows;

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, State};

use artwork::ArtworkService;
use error::{AppError, AppResult};
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
async fn set_volume(player: Player<'_>, volume: f64) -> AppResult<()> {
    player.set_volume(volume).await
}

#[tauri::command]
fn get_lyrics(lyrics: Lyrics<'_>) -> Option<LyricsLoaded> {
    lyrics.current()
}

#[tauri::command]
fn get_palette(artwork: Artwork<'_>) -> Option<PaletteChanged> {
    artwork.current()
}

/// Updates the offset in memory at once; the file write is debounced.
#[tauri::command]
async fn set_lyrics_offset(lyrics: Lyrics<'_>, track_id: String, offset_ms: i64) -> AppResult<()> {
    lyrics.set_offset(&track_id, offset_ms);
    Ok(())
}

#[tauri::command]
async fn retry_lyrics(app: AppHandle, lyrics: Lyrics<'_>) -> AppResult<()> {
    let lyrics = Arc::clone(&lyrics);
    tauri::async_runtime::spawn_blocking(move || lyrics.retry(&app))
        .await
        .map_err(|_| AppError::Cancelled)?;
    Ok(())
}

#[tauri::command]
fn get_settings(settings: SettingsState<'_>) -> Settings {
    settings.get()
}

/// Merges a partial settings object onto the current settings, persists
/// them, applies the backend-relevant parts and tells every window. Only the
/// changed fields are sent, so a stale copy in one window can't undo changes
/// made elsewhere (the other window, or the backend's mini position).
#[tauri::command]
async fn update_settings(
    app: AppHandle,
    settings: SettingsState<'_>,
    player: Player<'_>,
    idle: State<'_, Arc<IdleMonitor>>,
    patch: serde_json::Map<String, serde_json::Value>,
) -> AppResult<Settings> {
    let store = Arc::clone(&settings);
    let saved = tauri::async_runtime::spawn_blocking(move || store.apply_patch(&patch))
        .await
        .map_err(|_| AppError::Cancelled)?;
    player.set_preferred(effective_preferred_player(&saved));
    screensaver::apply_settings(&idle, &settings);
    screensaver::settings_changed(&app, &saved);
    if let Err(e) = app.emit(SETTINGS_CHANGED, saved.clone()) {
        tracing::warn!("failed to emit settings: {e}");
    }
    Ok(saved)
}

/// Where the caches live, for maintenance and the "clear cache" button.
struct CacheDirs {
    paths: Paths,
    /// Platform extras, e.g. Windows media-session thumbnails: (dir, cap).
    extra: Vec<(std::path::PathBuf, u64)>,
}

/// Bytes currently used by all caches.
#[tauri::command]
async fn cache_size(dirs: State<'_, Arc<CacheDirs>>) -> AppResult<u64> {
    let dirs = Arc::clone(&dirs);
    tauri::async_runtime::spawn_blocking(move || cache::size_all(&dirs.paths, &dirs.extra))
        .await
        .map_err(|_| AppError::Cancelled)
}

/// Deletes cached lyrics and artwork (not settings or offsets); returns bytes freed.
#[tauri::command]
async fn clear_cache(dirs: State<'_, Arc<CacheDirs>>) -> AppResult<u64> {
    let dirs = Arc::clone(&dirs);
    tauri::async_runtime::spawn_blocking(move || cache::clear_all(&dirs.paths, &dirs.extra))
        .await
        .map_err(|_| AppError::Cancelled)
}

#[tauri::command]
async fn list_players() -> AppResult<Vec<String>> {
    player::list_players().await
}

#[tauri::command]
fn set_mini_mode(app: AppHandle, settings: SettingsState<'_>, enabled: bool) -> AppResult<()> {
    Ok(windows::set_mini_mode(&app, &settings, enabled)?)
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
    let message = sanitize_log(&message);
    match level.as_str() {
        "error" => tracing::error!(target: "frontend", "{message}"),
        "warn" => tracing::warn!(target: "frontend", "{message}"),
        _ => tracing::info!(target: "frontend", "{message}"),
    }
}

/// Release builds abort on panic (`panic = "abort"`), so the reason would be
/// lost with the process. Record it in `<cache>/crash.log` first.
fn install_panic_log(paths: &Paths) {
    let log = paths.cache.join("crash.log");
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        use std::io::Write;
        let backtrace = std::backtrace::Backtrace::force_capture();
        let thread = std::thread::current();
        let entry = format!(
            "[{}] Sahne {} panicked on thread {}: {info}\n{backtrace}\n",
            paths::unix_now(),
            env!("CARGO_PKG_VERSION"),
            thread.name().unwrap_or("?"),
        );
        let _ = std::fs::create_dir_all(log.parent().unwrap_or(std::path::Path::new(".")));
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
        {
            let _ = f.write_all(entry.as_bytes());
        }
        default(info);
    }));
}

/// Longest frontend log message kept, in bytes.
const MAX_LOG_BYTES: usize = 4096;

/// Keeps frontend messages to one bounded log line: control characters
/// (newlines, ANSI escapes) are escaped so they cannot forge or hide lines.
fn sanitize_log(message: &str) -> String {
    let mut out = String::with_capacity(message.len().min(MAX_LOG_BYTES));
    for c in message.chars() {
        if out.len() >= MAX_LOG_BYTES {
            out.push_str("…[truncated]");
            break;
        }
        if c.is_control() {
            out.extend(c.escape_default());
        } else {
            out.push(c);
        }
    }
    out
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

/// Bundle identifier up to 0.1; see the migration in `run`.
const LEGACY_IDENTIFIER: &str = "dev.sahne.app";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();
    #[cfg(target_os = "linux")]
    apply_webkit_workarounds();

    let paths = Paths::from_xdg().unwrap_or_else(|| {
        tracing::warn!("no home directory; using a temporary location for settings and caches");
        Paths::temporary()
    });
    install_panic_log(&paths);
    let settings = Arc::new(SettingsStore::load(&paths));
    let initial = settings.get();
    let player = Arc::new(PlayerState::new(effective_preferred_player(&initial)));
    let lyrics = Arc::new(LyricsService::new(&paths));
    let artwork = Arc::new(ArtworkService::new(&paths));
    let idle = Arc::new(IdleMonitor::start(std::time::Duration::from_secs(180)));
    screensaver::apply_settings(&idle, &settings);
    let cache_paths = paths.clone();

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
            let mut extra = Vec::new();
            let mut legacy_cache = None;
            if let Ok(dir) = app.path().app_cache_dir() {
                if cfg!(windows) {
                    extra.push((dir.join("session-art"), 50 * 1024 * 1024));
                }
                // 0.2 changed the bundle identifier; the old one's cache
                // directory (Windows session art, WebView cache) is orphaned.
                legacy_cache = dir
                    .parent()
                    .map(|p| p.join(LEGACY_IDENTIFIER))
                    .filter(|old| *old != dir);
            }
            let dirs = Arc::new(CacheDirs {
                paths: cache_paths.clone(),
                extra,
            });
            app.manage(Arc::clone(&dirs));
            tauri::async_runtime::spawn_blocking(move || {
                if let Some(old) = legacy_cache.filter(|old| old.is_dir()) {
                    match std::fs::remove_dir_all(&old) {
                        Ok(()) => tracing::info!("removed the 0.1 cache at {}", old.display()),
                        Err(e) => tracing::warn!("could not remove {}: {e}", old.display()),
                    }
                }
                cache::prune_all(&dirs.paths, &dirs.extra)
            });
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
            set_volume,
            get_lyrics,
            get_palette,
            set_lyrics_offset,
            retry_lyrics,
            cache_size,
            clear_cache,
            get_settings,
            update_settings,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontend_log_is_one_bounded_line() {
        assert_eq!(sanitize_log("a\nb\x1b[31mc"), "a\\nb\\u{1b}[31mc");
        let long = sanitize_log(&"x".repeat(100_000));
        assert!(long.len() < MAX_LOG_BYTES + 32);
        assert!(long.ends_with("[truncated]"));
        assert_eq!(sanitize_log("şarkı ✓"), "şarkı ✓");
    }
}
