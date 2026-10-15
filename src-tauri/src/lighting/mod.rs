//! Ambient lighting: paints OpenRGB devices in the album-art palette.
//!
//! The OpenRGB client is GPL-2.0, so it is only compiled with the optional
//! `lighting` cargo feature. Without it the service reports itself as
//! unavailable and the rest of the app is unaffected.

#[cfg_attr(not(feature = "lighting"), allow(dead_code))]
pub mod frames;
#[cfg(feature = "lighting")]
mod openrgb;

use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::watch;

use crate::artwork::palette::Palette;
use crate::settings::Settings;

pub const LIGHTING_STATUS: &str = "lighting-status";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(not(feature = "lighting"), allow(dead_code))]
pub enum LightingState {
    /// Turned off in settings (or not compiled in).
    Disabled,
    Connecting,
    Connected,
    /// OpenRGB's SDK server is not reachable.
    Offline,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeviceInfo {
    pub name: String,
    pub leds: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LightingStatus {
    /// Whether this build includes OpenRGB support at all.
    pub available: bool,
    pub state: LightingState,
    pub devices: Vec<DeviceInfo>,
}

pub struct LightingService {
    status: Mutex<LightingStatus>,
    #[cfg(feature = "lighting")]
    shutdown: tokio::sync::mpsc::Sender<tokio::sync::oneshot::Sender<()>>,
}

impl LightingService {
    pub fn status(&self) -> LightingStatus {
        lock(&self.status).clone()
    }

    #[cfg_attr(not(feature = "lighting"), allow(dead_code))]
    fn set_status(&self, app: &AppHandle, status: LightingStatus) {
        let changed = {
            let mut cur = lock(&self.status);
            let changed = *cur != status;
            *cur = status.clone();
            changed
        };
        if changed {
            if let Err(e) = app.emit(LIGHTING_STATUS, status) {
                tracing::warn!("failed to emit lighting status: {e}");
            }
        }
    }

    /// Starts the background worker (a no-op without the `lighting` feature).
    pub fn spawn(
        app: AppHandle,
        settings: watch::Receiver<Settings>,
        palette: watch::Receiver<Palette>,
    ) -> Arc<Self> {
        #[cfg(feature = "lighting")]
        {
            let (shutdown, shutdown_rx) = tokio::sync::mpsc::channel(1);
            let service = Arc::new(Self {
                status: Mutex::new(LightingStatus {
                    available: true,
                    state: LightingState::Disabled,
                    devices: Vec::new(),
                }),
                shutdown,
            });
            let worker = Arc::clone(&service);
            tauri::async_runtime::spawn(openrgb::run(worker, app, settings, palette, shutdown_rx));
            service
        }
        #[cfg(not(feature = "lighting"))]
        {
            let _ = (app, settings, palette);
            Arc::new(Self {
                status: Mutex::new(LightingStatus {
                    available: false,
                    state: LightingState::Disabled,
                    devices: Vec::new(),
                }),
            })
        }
    }

    /// Restores the devices' previous colors before the app exits.
    pub fn shutdown(&self) {
        #[cfg(feature = "lighting")]
        {
            let (done_tx, done_rx) = tokio::sync::oneshot::channel();
            if self.shutdown.try_send(done_tx).is_ok() {
                tauri::async_runtime::block_on(async {
                    let _ =
                        tokio::time::timeout(std::time::Duration::from_millis(1500), done_rx).await;
                });
            }
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
