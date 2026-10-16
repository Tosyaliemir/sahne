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

#[cfg(target_os = "linux")]
#[zbus::proxy(
    interface = "org.freedesktop.ScreenSaver",
    default_service = "org.freedesktop.ScreenSaver",
    default_path = "/org/freedesktop/ScreenSaver"
)]
trait Inhibitor {
    fn inhibit(&self, application: &str, reason: &str) -> zbus::Result<u32>;
    fn un_inhibit(&self, cookie: u32) -> zbus::Result<()>;
}

/// Lets [`settings_changed`] wake the screensaver loop.
struct Control(tokio::sync::Notify);

/// Call after settings change: turning the screensaver off while it is
/// showing restores the windows right away instead of on the next input.
pub fn settings_changed(app: &AppHandle, settings: &crate::settings::Settings) {
    if !settings.screensaver_enabled {
        if let Some(control) = app.try_state::<Control>() {
            control.0.notify_one();
        }
    }
}

pub fn spawn(
    app: AppHandle,
    settings: Arc<SettingsStore>,
    player: Arc<PlayerState>,
    idle: Arc<IdleMonitor>,
) {
    app.manage(Control(tokio::sync::Notify::new()));
    tauri::async_runtime::spawn(async move {
        let mut idle_rx = idle.subscribe();
        let mut before: Option<Before> = None;
        let mut awake: Option<KeepAwake> = None;
        loop {
            let disabled = {
                let control = app.state::<Control>();
                tokio::select! {
                    changed = idle_rx.changed() => {
                        if changed.is_err() {
                            return;
                        }
                        false
                    }
                    () = control.0.notified() => true,
                }
            };
            let is_idle = !disabled && *idle_rx.borrow_and_update();
            let s = settings.get();
            if is_idle && before.is_none() && s.screensaver_enabled && player.is_playing() {
                tracing::info!("screensaver: showing");
                before = activate(&app, &settings);
                let _ = app.emit(SCREENSAVER, true);
                if s.screensaver_keep_awake {
                    awake = KeepAwake::start().await;
                }
            } else if !is_idle {
                if let Some(b) = before.take() {
                    tracing::info!("screensaver: user is back");
                    let _ = app.emit(SCREENSAVER, false);
                    restore(&app, &settings, b);
                    if let Some(a) = awake.take() {
                        a.stop().await;
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

/// Keeps the screen from blanking or locking while it exists (until `stop`).
#[cfg(target_os = "linux")]
struct KeepAwake {
    /// The inhibition lives as long as the D-Bus connection that asked for it.
    conn: zbus::Connection,
    cookie: u32,
}

#[cfg(target_os = "linux")]
impl KeepAwake {
    async fn start() -> Option<Self> {
        let conn = zbus::Connection::session().await.ok()?;
        let proxy = InhibitorProxy::new(&conn).await.ok()?;
        match proxy.inhibit("Sahne", "Screensaver mode").await {
            Ok(cookie) => Some(Self { conn, cookie }),
            Err(e) => {
                tracing::warn!("could not keep the screen awake: {e}");
                None
            }
        }
    }

    async fn stop(self) {
        if let Ok(proxy) = InhibitorProxy::new(&self.conn).await {
            let _ = proxy.un_inhibit(self.cookie).await;
        }
    }
}

/// Keeps the display on while it exists (until `stop`): a Windows power
/// request, which unlike `SetThreadExecutionState` isn't tied to a thread.
#[cfg(windows)]
struct KeepAwake(::windows::Win32::Foundation::HANDLE);

// SAFETY: a power request handle may be used and closed from any thread.
#[cfg(windows)]
unsafe impl Send for KeepAwake {}

#[cfg(windows)]
impl KeepAwake {
    async fn start() -> Option<Self> {
        use ::windows::core::PWSTR;
        use ::windows::Win32::System::Power::{
            PowerCreateRequest, PowerRequestDisplayRequired, PowerSetRequest,
        };
        use ::windows::Win32::System::SystemServices::POWER_REQUEST_CONTEXT_VERSION;
        use ::windows::Win32::System::Threading::{
            POWER_REQUEST_CONTEXT_SIMPLE_STRING, REASON_CONTEXT, REASON_CONTEXT_0,
        };
        let mut reason: Vec<u16> = "Sahne screensaver mode\0".encode_utf16().collect();
        let context = REASON_CONTEXT {
            Version: POWER_REQUEST_CONTEXT_VERSION,
            Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
            Reason: REASON_CONTEXT_0 {
                SimpleReasonString: PWSTR(reason.as_mut_ptr()),
            },
        };
        // SAFETY: `context` and the string it points to outlive the call,
        // which copies the reason.
        let result = unsafe { PowerCreateRequest(&context) }.and_then(|handle| {
            // SAFETY: `handle` is the request just created.
            unsafe { PowerSetRequest(handle, PowerRequestDisplayRequired) }.map(|()| handle)
        });
        match result {
            Ok(handle) => Some(Self(handle)),
            Err(e) => {
                tracing::warn!("could not keep the screen awake: {e}");
                None
            }
        }
    }

    async fn stop(self) {
        use ::windows::Win32::Foundation::CloseHandle;
        use ::windows::Win32::System::Power::{PowerClearRequest, PowerRequestDisplayRequired};
        // SAFETY: `self.0` is our live power request; it is not used after this.
        unsafe {
            let _ = PowerClearRequest(self.0, PowerRequestDisplayRequired);
            let _ = CloseHandle(self.0);
        }
    }
}
