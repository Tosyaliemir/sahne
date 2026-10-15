mod error;
mod events;
mod player;

use std::sync::Arc;

use tauri::State;

use error::AppResult;
use events::PlayerSnapshot;
use player::PlayerState;

type Player<'a> = State<'a, Arc<PlayerState>>;

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

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("sahne_lib=info,warn"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

/// WebKitGTK's DMA-BUF renderer dies with "Error 71 (Protocol error)" on
/// Wayland with the NVIDIA driver. Fall back to the older path there unless
/// the user has made their own choice.
#[cfg(target_os = "linux")]
fn apply_webkit_workarounds() {
    const VAR: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";
    if std::env::var_os(VAR).is_none() && std::path::Path::new("/proc/driver/nvidia/version").exists() {
        tracing::info!("NVIDIA driver detected, setting {VAR}=1");
        std::env::set_var(VAR, "1");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();
    #[cfg(target_os = "linux")]
    apply_webkit_workarounds();
    // Until the settings panel exists, SAHNE_PLAYER selects a non-Spotify player
    // (e.g. `SAHNE_PLAYER=sahnemock` for the mock player).
    let preferred = std::env::var("SAHNE_PLAYER").ok().filter(|s| !s.is_empty());
    let player = Arc::new(PlayerState::new(preferred));

    let result = tauri::Builder::default()
        .manage(player.clone())
        .setup(move |app| {
            player::spawn(app.handle().clone(), player);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_state, play_pause, next, previous, seek])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        tracing::error!("failed to start Sahne: {e}");
        std::process::exit(1);
    }
}
