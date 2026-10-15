//! Screensaver mode: after the computer has been idle for a while with music
//! playing, Sahne comes forward in fullscreen; on the first input it steps
//! back to how the windows were before.

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::idle::IdleMonitor;
use crate::player::PlayerState;
use crate::settings::SettingsStore;
use crate::windows::{self, MAIN, MINI};

pub const SCREENSAVER: &str = "screensaver";

/// How the windows looked before the screensaver took over.
#[derive(Debug, Clone, Copy)]
struct Before {
    visible: bool,
    minimized: bool,
    fullscreen: bool,
    mini: bool,
}

#[zbus::proxy(
    interface = "org.freedesktop.ScreenSaver",
    default_service = "org.freedesktop.ScreenSaver",
    default_path = "/org/freedesktop/ScreenSaver"
)]
trait Inhibitor {
    fn inhibit(&self, application: &str, reason: &str) -> zbus::Result<u32>;
    fn un_inhibit(&self, cookie: u32) -> zbus::Result<()>;
}

pub fn spawn(
    app: AppHandle,
    settings: Arc<SettingsStore>,
    player: Arc<PlayerState>,
    idle: Arc<IdleMonitor>,
) {
    tauri::async_runtime::spawn(async move {
        let mut idle_rx = idle.subscribe();
        let mut before: Option<Before> = None;
        // The inhibition lives as long as the D-Bus connection that asked for it.
        let mut cookie: Option<(zbus::Connection, u32)> = None;
        loop {
            if idle_rx.changed().await.is_err() {
                return;
            }
            let is_idle = *idle_rx.borrow_and_update();
            let s = settings.get();
            if is_idle && before.is_none() && s.screensaver_enabled && player.is_playing() {
                tracing::info!("screensaver: showing");
                before = activate(&app, &settings);
                let _ = app.emit(SCREENSAVER, true);
                if s.screensaver_keep_awake {
                    cookie = inhibit().await;
                }
            } else if !is_idle {
                if let Some(b) = before.take() {
                    tracing::info!("screensaver: user is back");
                    let _ = app.emit(SCREENSAVER, false);
                    restore(&app, &settings, b);
                    if let Some((conn, c)) = cookie.take() {
                        uninhibit(&conn, c).await;
                    }
                }
            }
        }
    });
}

/// Applies the idle timeout from settings (call after settings change).
pub fn apply_settings(idle: &IdleMonitor, settings: &SettingsStore) {
    let s = settings.get();
    // Development aid: `SAHNE_SCREENSAVER_SECS=15` to try it without waiting minutes.
    let secs = std::env::var("SAHNE_SCREENSAVER_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(u64::from(s.screensaver_minutes) * 60);
    idle.set_timeout(s.screensaver_enabled.then(|| Duration::from_secs(secs)));
}

fn activate(app: &AppHandle, settings: &SettingsStore) -> Option<Before> {
    let mini = app
        .get_webview_window(MINI)
        .is_some_and(|w| w.is_visible().unwrap_or(false));
    if mini {
        if let Err(e) = windows::set_mini_mode(app, settings, false) {
            tracing::warn!("screensaver: leaving mini mode failed: {e}");
        }
    }
    let main = app.get_webview_window(MAIN)?;
    let before = Before {
        visible: main.is_visible().unwrap_or(true) || mini,
        minimized: main.is_minimized().unwrap_or(false),
        fullscreen: main.is_fullscreen().unwrap_or(false),
        mini,
    };
    let result = (|| -> tauri::Result<()> {
        main.show()?;
        main.unminimize()?;
        main.set_fullscreen(true)?;
        // Compositors may refuse to focus a background window; staying on top
        // still puts the stage in front.
        main.set_always_on_top(true)?;
        let _ = main.set_focus();
        Ok(())
    })();
    if let Err(e) = result {
        tracing::warn!("screensaver: could not bring Sahne forward: {e}");
    }
    Some(before)
}

fn restore(app: &AppHandle, settings: &SettingsStore, b: Before) {
    let Some(main) = app.get_webview_window(MAIN) else {
        return;
    };
    let result = (|| -> tauri::Result<()> {
        main.set_always_on_top(false)?;
        if !b.fullscreen {
            main.set_fullscreen(false)?;
        }
        if b.minimized {
            main.minimize()?;
        } else if !b.visible {
            main.hide()?;
        }
        Ok(())
    })();
    if let Err(e) = result {
        tracing::warn!("screensaver: restoring windows failed: {e}");
    }
    if b.mini {
        if let Err(e) = windows::set_mini_mode(app, settings, true) {
            tracing::warn!("screensaver: returning to mini mode failed: {e}");
        }
    }
}

async fn inhibit() -> Option<(zbus::Connection, u32)> {
    let conn = zbus::Connection::session().await.ok()?;
    let proxy = InhibitorProxy::new(&conn).await.ok()?;
    match proxy.inhibit("Sahne", "Ekran koruyucu modu").await {
        Ok(c) => Some((conn, c)),
        Err(e) => {
            tracing::warn!("could not keep the screen awake: {e}");
            None
        }
    }
}

async fn uninhibit(conn: &zbus::Connection, cookie: u32) {
    if let Ok(proxy) = InhibitorProxy::new(conn).await {
        let _ = proxy.un_inhibit(cookie).await;
    }
}
