//! OpenRGB SDK driver (only built with the `lighting` feature).

use std::sync::Arc;
use std::time::{Duration, Instant};

use openrgb2::{Color, Controller, OpenRgbClient};
use tauri::AppHandle;
use tokio::sync::{mpsc, oneshot, watch};

use super::frames::{blend, target_frame, Rgb};
use super::{DeviceInfo, LightingService, LightingState, LightingStatus};
use crate::artwork::palette::Palette;
use crate::settings::{LightingSettings, Settings};

const FRAME: Duration = Duration::from_millis(33); // ~30 fps
const FADE: Duration = Duration::from_secs(1);
const RECONNECT: Duration = Duration::from_secs(10);

type Shutdown = mpsc::Receiver<oneshot::Sender<()>>;

/// What a device showed before we took it over, so it can be put back.
struct Saved {
    mode_name: Option<String>,
    colors: Vec<Color>,
}

struct Driven {
    /// Index into the controller list (controllers aren't `Clone`).
    idx: usize,
    saved: Saved,
    /// Last frame actually sent; identical frames are not re-sent.
    sent: Vec<Rgb>,
    /// Frame at the moment the palette last changed, for cross-fading.
    fade_from: Vec<Rgb>,
}

enum Exit {
    /// Settings changed in a way that needs a fresh connection.
    Reconfigure,
    Lost,
    Shutdown(oneshot::Sender<()>),
}

pub async fn run(
    service: Arc<LightingService>,
    app: AppHandle,
    mut settings: watch::Receiver<Settings>,
    mut palette: watch::Receiver<Palette>,
    mut shutdown: Shutdown,
) {
    let status = |state, devices| LightingStatus {
        available: true,
        state,
        devices,
    };
    loop {
        let wanted = settings.borrow_and_update().lighting.clone();
        if !wanted.enabled {
            service.set_status(&app, status(LightingState::Disabled, Vec::new()));
            tokio::select! {
                r = settings.changed() => if r.is_err() { return },
                Some(done) = shutdown.recv() => { let _ = done.send(()); return; }
            }
            continue;
        }

        service.set_status(&app, status(LightingState::Connecting, Vec::new()));
        let client = match OpenRgbClient::connect().await {
            Ok(c) => c,
            Err(e) => {
                tracing::info!("OpenRGB not reachable: {e}");
                service.set_status(&app, status(LightingState::Offline, Vec::new()));
                tokio::select! {
                    _ = tokio::time::sleep(RECONNECT) => {}
                    r = settings.changed() => if r.is_err() { return },
                    Some(done) = shutdown.recv() => { let _ = done.send(()); return; }
                }
                continue;
            }
        };

        let controllers = match client.get_all_controllers().await {
            Ok(group) => group,
            Err(e) => {
                tracing::warn!("OpenRGB device list failed: {e}");
                service.set_status(&app, status(LightingState::Offline, Vec::new()));
                tokio::time::sleep(RECONNECT).await;
                continue;
            }
        };
        let devices: Vec<DeviceInfo> = controllers
            .iter()
            .map(|c| DeviceInfo {
                name: c.name().to_owned(),
                leds: c.num_leds(),
            })
            .collect();
        service.set_status(&app, status(LightingState::Connected, devices));

        let list = controllers.controllers();
        let mut driven = take_over(list, &wanted).await;
        let exit = drive(
            list,
            &mut driven,
            &mut settings,
            &mut palette,
            &mut shutdown,
            &wanted,
        )
        .await;
        restore(list, &driven).await;
        match exit {
            Exit::Reconfigure => {}
            Exit::Lost => {
                service.set_status(&app, status(LightingState::Offline, Vec::new()));
                tokio::time::sleep(RECONNECT).await;
            }
            Exit::Shutdown(done) => {
                let _ = done.send(());
                return;
            }
        }
    }
}

/// Puts the selected devices into direct-control mode, remembering their state.
async fn take_over(controllers: &[Controller], wanted: &LightingSettings) -> Vec<Driven> {
    let mut driven = Vec::new();
    for (idx, c) in controllers
        .iter()
        .enumerate()
        .filter(|(_, c)| wanted.devices.iter().any(|d| d == c.name()))
    {
        // openrgb2 panics on controllers without modes; skip those.
        if c.modes().is_empty() || c.num_leds() == 0 {
            tracing::info!("skipping {}: no controllable modes or LEDs", c.name());
            continue;
        }
        let saved = Saved {
            mode_name: c
                .mode_iter()
                .find(|m| m.is_active())
                .map(|m| m.name().to_owned()),
            colors: c.colors().to_vec(),
        };
        if let Err(e) = c.set_controllable_mode().await {
            tracing::warn!("{} can't be controlled: {e}", c.name());
            continue;
        }
        let shown: Vec<Rgb> = saved.colors.iter().map(|c| Rgb(c.r, c.g, c.b)).collect();
        driven.push(Driven {
            idx,
            saved,
            sent: Vec::new(),
            fade_from: shown,
        });
    }
    driven
}

async fn drive(
    controllers: &[Controller],
    driven: &mut [Driven],
    settings: &mut watch::Receiver<Settings>,
    palette: &mut watch::Receiver<Palette>,
    shutdown: &mut Shutdown,
    wanted: &LightingSettings,
) -> Exit {
    let start = Instant::now();
    let mut fade_start = Instant::now();
    let mut mode = wanted.mode;
    let mut tick = tokio::time::interval(FRAME);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            r = settings.changed() => {
                if r.is_err() { return Exit::Reconfigure; }
                let new = settings.borrow().lighting.clone();
                if !new.enabled || new.devices != wanted.devices {
                    return Exit::Reconfigure;
                }
                if new.mode != mode {
                    mode = new.mode;
                    start_fade(driven, &mut fade_start);
                }
            }
            r = palette.changed() => {
                if r.is_err() { return Exit::Reconfigure; }
                start_fade(driven, &mut fade_start);
            }
            Some(done) = shutdown.recv() => return Exit::Shutdown(done),
            _ = tick.tick() => {
                let current = palette.borrow().clone();
                let t = start.elapsed().as_secs_f64();
                let k = fade_start.elapsed().as_secs_f64() / FADE.as_secs_f64();
                for d in driven.iter_mut() {
                    let c = &controllers[d.idx];
                    let target = target_frame(mode, &current, c.num_leds(), t);
                    let frame = blend(&d.fade_from, &target, k);
                    if frame == d.sent {
                        continue; // nothing changed: no traffic
                    }
                    let colors = frame.iter().map(|c| Color::new(c.0, c.1, c.2));
                    if let Err(e) = c.set_leds(colors).await {
                        tracing::warn!("OpenRGB write failed: {e}");
                        return Exit::Lost;
                    }
                    d.sent = frame;
                }
            }
        }
    }
}

/// Cross-fade from whatever is showing now.
fn start_fade(driven: &mut [Driven], fade_start: &mut Instant) {
    for d in driven.iter_mut() {
        d.fade_from = d.sent.clone();
    }
    *fade_start = Instant::now();
}

async fn restore(controllers: &[Controller], driven: &[Driven]) {
    for d in driven {
        let c = &controllers[d.idx];
        if let Some(name) = &d.saved.mode_name {
            if let Some(mode) = c.mode_iter().find(|m| m.name() == name) {
                if let Err(e) = mode.builder().execute(c).await {
                    tracing::debug!("could not restore mode on {}: {e}", c.name());
                }
            }
        }
        if let Err(e) = c.set_leds(d.saved.colors.iter().copied()).await {
            tracing::debug!("could not restore colors on {}: {e}", c.name());
        }
    }
}
