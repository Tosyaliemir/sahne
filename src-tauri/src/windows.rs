//! Switching between the main stage window and the small always-on-top mini window.

use tauri::{
    AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

use crate::settings::SettingsStore;

pub const MAIN: &str = "main";
pub const MINI: &str = "mini";
const MINI_SIZE: (f64, f64) = (460.0, 132.0);
const MINI_MARGIN: i32 = 24;

/// Wayland compositors don't let clients read or set window positions.
fn can_position_windows() -> bool {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let forced_x11 = std::env::var("GDK_BACKEND").is_ok_and(|b| b.starts_with("x11"));
    !wayland || forced_x11
}

pub fn set_mini_mode(
    app: &AppHandle,
    settings: &SettingsStore,
    enabled: bool,
) -> tauri::Result<()> {
    let main = app.get_webview_window(MAIN);
    if enabled {
        let mini = match app.get_webview_window(MINI) {
            Some(w) => w,
            None => build_mini(app)?,
        };
        if can_position_windows() {
            place_mini(&mini, main.as_ref(), settings)?;
        }
        mini.show()?;
        mini.set_focus()?;
        if let Some(main) = main {
            if main.is_fullscreen().unwrap_or(false) {
                main.set_fullscreen(false)?;
            }
            main.hide()?;
        }
    } else {
        if let Some(mini) = app.get_webview_window(MINI) {
            remember_position(&mini, settings);
            mini.destroy()?;
        }
        if let Some(main) = main {
            main.show()?;
            main.set_focus()?;
        }
    }
    Ok(())
}

fn build_mini(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let mini = WebviewWindowBuilder::new(app, MINI, WebviewUrl::App("index.html".into()))
        .title("Sahne Mini")
        .inner_size(MINI_SIZE.0, MINI_SIZE.1)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()?;
    // If the mini window is closed some other way (e.g. Alt+F4), bring the
    // main window back instead of leaving the app running invisibly.
    let handle = app.clone();
    mini.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            if let Some(main) = handle.get_webview_window(MAIN) {
                if !main.is_visible().unwrap_or(true) {
                    let _ = main.show();
                    let _ = main.set_focus();
                }
            }
        }
    });
    Ok(mini)
}

/// Restores the saved position, or defaults to the bottom-right corner.
fn place_mini(
    mini: &WebviewWindow,
    main: Option<&WebviewWindow>,
    settings: &SettingsStore,
) -> tauri::Result<()> {
    if let Some((x, y)) = settings.get().mini_position {
        return mini.set_position(PhysicalPosition::new(x, y));
    }
    let monitor = main
        .and_then(|m| m.current_monitor().ok().flatten())
        .or(mini.primary_monitor()?);
    if let Some(m) = monitor {
        let size = mini.outer_size()?;
        let x = m.position().x + m.size().width as i32 - size.width as i32 - MINI_MARGIN;
        let y = m.position().y + m.size().height as i32 - size.height as i32 - MINI_MARGIN;
        mini.set_position(PhysicalPosition::new(x, y))?;
    }
    Ok(())
}

fn remember_position(mini: &WebviewWindow, settings: &SettingsStore) {
    if !can_position_windows() {
        return;
    }
    if let Ok(pos) = mini.outer_position() {
        settings.update(|s| s.mini_position = Some((pos.x, pos.y)));
    }
}
