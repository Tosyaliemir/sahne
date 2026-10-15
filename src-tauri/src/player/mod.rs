//! Follows the system's media player and forwards its state to the UI.
//!
//! The platform backends only talk to the player: MPRIS over D-Bus on Linux,
//! the Global System Media Transport Controls on Windows. State, position
//! bookkeeping and events to the frontend are shared and live here.

#[cfg(windows)]
mod gsmtc;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod mpris;
mod select;
mod tracker;

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};
use tokio::sync::{watch, Notify};

use crate::error::{AppError, AppResult};
use crate::events::{self, PlaybackState, PlaybackStatus, PlayerSnapshot, PositionSync, TrackInfo};
#[cfg(windows)]
use gsmtc::Handle;
#[cfg(target_os = "linux")]
use linux::Handle;
use tracker::PositionTracker;

#[cfg(windows)]
pub use gsmtc::{list_players, spawn};
#[cfg(target_os = "linux")]
pub use linux::{list_players, spawn};

const BACKOFF_MIN: Duration = Duration::from_millis(500);
const BACKOFF_MAX: Duration = Duration::from_secs(30);

#[derive(Default)]
struct Shared {
    /// The backend's connection to the followed player, if any.
    handle: Option<Handle>,
    player: Option<String>,
    track: Option<TrackInfo>,
    status: Option<PlaybackStatus>,
    rate: f64,
    volume: Option<f64>,
    tracker: PositionTracker,
    last_position: Option<PositionSync>,
}

/// State shared between the background loop and Tauri commands.
pub struct PlayerState {
    shared: Mutex<Shared>,
    repoll: Notify,
    /// Wakes the follow loop to re-run player selection (preference changed).
    reselect: Notify,
    started: Instant,
    preferred: Mutex<Option<String>>,
    /// Current track for the lyrics and artwork services.
    track_tx: watch::Sender<Option<TrackInfo>>,
}

impl PlayerState {
    pub fn new(preferred: Option<String>) -> Self {
        Self {
            shared: Mutex::new(Shared {
                rate: 1.0,
                ..Default::default()
            }),
            repoll: Notify::new(),
            reselect: Notify::new(),
            started: Instant::now(),
            preferred: Mutex::new(preferred),
            track_tx: watch::Sender::new(None),
        }
    }

    fn preferred(&self) -> Option<String> {
        self.preferred
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Changes which player to prefer and re-runs selection right away.
    pub fn set_preferred(&self, preferred: Option<String>) {
        let mut cur = self.preferred.lock().unwrap_or_else(|e| e.into_inner());
        if *cur != preferred {
            *cur = preferred;
            self.reselect.notify_one();
        }
    }

    pub fn subscribe_tracks(&self) -> watch::Receiver<Option<TrackInfo>> {
        self.track_tx.subscribe()
    }

    fn lock(&self) -> MutexGuard<'_, Shared> {
        // A panic while holding the lock leaves plain data behind; keep going.
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn now_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn handle(&self) -> AppResult<Handle> {
        self.lock().handle.clone().ok_or(AppError::NoPlayer)
    }

    pub fn is_playing(&self) -> bool {
        self.lock().status == Some(PlaybackStatus::Playing)
    }

    pub fn snapshot(&self) -> PlayerSnapshot {
        let s = self.lock();
        PlayerSnapshot {
            track: s.track.clone(),
            playback: s
                .status
                .zip(s.player.clone())
                .map(|(status, player)| PlaybackState { status, player }),
            position: s.last_position.clone(),
            volume: s.volume,
        }
    }

    /// Records a seek for players whose reported position is broken, and
    /// asks the follow loop to re-read the real position shortly.
    fn after_seek(&self, position_ms: u64) {
        let now = self.now_ms();
        self.lock().tracker.seeked(position_ms, now);
        self.repoll.notify_one();
    }
}

/// Updates the current track. Returns true when the track itself changed
/// (not just e.g. late-arriving artwork for the same track).
fn apply_track(app: &AppHandle, state: &PlayerState, track: Option<TrackInfo>) -> bool {
    let now = state.now_ms();
    let mut s = state.lock();
    if s.track == track {
        return false;
    }
    let new_track = s.track.as_ref().map(|t| &t.id) != track.as_ref().map(|t| &t.id);
    if new_track {
        s.tracker.track_changed(0, now);
    }
    s.track = track.clone();
    drop(s);
    state.track_tx.send_replace(track.clone());
    match &track {
        Some(t) => tracing::info!("track: {} – {}", t.artist, t.title),
        None => tracing::info!("no track"),
    }
    emit(app, events::TRACK_CHANGED, &track);
    new_track
}

fn apply_status(app: &AppHandle, state: &PlayerState, status: PlaybackStatus, player: &str) {
    let now = state.now_ms();
    let mut s = state.lock();
    s.tracker
        .set_playing(status == PlaybackStatus::Playing, now);
    if s.status == Some(status) {
        return;
    }
    s.status = Some(status);
    drop(s);
    emit(
        app,
        events::PLAYBACK_STATE,
        &PlaybackState {
            status,
            player: player.to_owned(),
        },
    );
}

fn apply_volume(app: &AppHandle, state: &PlayerState, volume: Option<f64>) {
    let volume = volume.filter(|v| v.is_finite()).map(|v| v.clamp(0.0, 1.0));
    let mut s = state.lock();
    if s.volume == volume {
        return;
    }
    s.volume = volume;
    drop(s);
    emit(app, events::VOLUME_CHANGED, &volume);
}

/// Feeds a position read from the player (or `None` if reading failed, in
/// which case our own estimate is published) into the tracker and the UI.
fn position_read(app: &AppHandle, state: &PlayerState, read_ms: Option<u64>, hard: bool) {
    let now = state.now_ms();
    let pos = {
        let mut s = state.lock();
        match read_ms {
            Some(ms) => s.tracker.polled(ms, now),
            None => s.tracker.estimate(now),
        }
    };
    publish_position(app, state, pos, hard);
}

fn publish_position(app: &AppHandle, state: &PlayerState, pos_ms: u64, hard: bool) {
    let now = state.now_ms();
    let mut s = state.lock();
    let length = s.track.as_ref().map_or(0, |t| t.length_ms);
    let sync = PositionSync {
        position_ms: if length > 0 {
            pos_ms.min(length)
        } else {
            pos_ms
        },
        monotonic_ms: now,
        playing: s.status == Some(PlaybackStatus::Playing),
        rate: s.rate,
        source: s.tracker.source(),
        hard,
    };
    s.last_position = Some(sync.clone());
    drop(s);
    tracing::debug!(
        "position {}ms playing={} source={:?} hard={}",
        sync.position_ms,
        sync.playing,
        sync.source,
        sync.hard
    );
    emit(app, events::POSITION_SYNC, &sync);
}

fn disconnect(app: &AppHandle, state: &PlayerState) {
    let was_connected = {
        let mut s = state.lock();
        let was = s.handle.is_some();
        *s = Shared {
            rate: 1.0,
            ..Default::default()
        };
        was
    };
    state.track_tx.send_replace(None);
    if was_connected {
        emit(app, events::PLAYER_DISCONNECTED, &());
    }
}

fn emit<T: serde::Serialize + Clone>(app: &AppHandle, event: &str, payload: &T) {
    if let Err(e) = app.emit(event, payload.clone()) {
        tracing::warn!("failed to emit {event}: {e}");
    }
}
