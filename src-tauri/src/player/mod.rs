//! Follows an MPRIS player on the session bus and forwards its state to the UI.

mod mpris;
mod tracker;

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};
use tokio::sync::{watch, Notify};
use tokio::time::MissedTickBehavior;
use zbus::fdo::DBusProxy;
use zbus::zvariant::ObjectPath;
use zbus::Connection;

use crate::error::{AppError, AppResult};
use crate::events::{self, PlaybackState, PlaybackStatus, PlayerSnapshot, PositionSync, TrackInfo};
use mpris::{choose_player, parse_metadata, short_name, PlayerProxy, MPRIS_PREFIX};
use tracker::PositionTracker;

const POLL_INTERVAL: Duration = Duration::from_millis(1000);
/// Delay before re-reading Position after a command, giving the player time to apply it.
const REPOLL_DELAY: Duration = Duration::from_millis(150);
const BACKOFF_MIN: Duration = Duration::from_millis(500);
const BACKOFF_MAX: Duration = Duration::from_secs(30);

#[derive(Default)]
struct Shared {
    proxy: Option<PlayerProxy<'static>>,
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

    pub async fn set_volume(&self, volume: f64) -> AppResult<()> {
        self.proxy()?.set_volume(volume.clamp(0.0, 1.0)).await?;
        Ok(())
    }

    fn proxy(&self) -> AppResult<PlayerProxy<'static>> {
        self.lock().proxy.clone().ok_or(AppError::NoPlayer)
    }

    pub async fn play_pause(&self) -> AppResult<()> {
        self.proxy()?.play_pause().await?;
        Ok(())
    }

    pub async fn next(&self) -> AppResult<()> {
        self.proxy()?.next().await?;
        Ok(())
    }

    pub async fn previous(&self) -> AppResult<()> {
        self.proxy()?.previous().await?;
        Ok(())
    }

    pub async fn seek(&self, position_ms: u64) -> AppResult<()> {
        let proxy = self.proxy()?;
        let (track_id, current_ms) = {
            let s = self.lock();
            (
                s.track.as_ref().map(|t| t.id.clone()),
                s.tracker.estimate(self.now_ms()),
            )
        };
        let target_us = i64::try_from(position_ms.saturating_mul(1000)).unwrap_or(i64::MAX);
        let path = track_id
            .as_deref()
            .and_then(|id| ObjectPath::try_from(id).ok());
        let absolute = match &path {
            Some(p) => proxy.set_position(p, target_us).await,
            None => Err(zbus::Error::Failure(
                "track id is not an object path".into(),
            )),
        };
        if let Err(e) = absolute {
            // Players without a usable track id still support relative Seek.
            tracing::debug!("SetPosition failed ({e}), falling back to Seek");
            let current_us = i64::try_from(current_ms.saturating_mul(1000)).unwrap_or(i64::MAX);
            proxy.seek(target_us - current_us).await?;
        }
        // Keeps the estimate right for players whose Position is broken.
        let now = self.now_ms();
        self.lock().tracker.seeked(position_ms, now);
        self.repoll.notify_one();
        Ok(())
    }
}

/// Starts the background loop; it never returns and survives bus errors.
pub fn spawn(app: AppHandle, state: Arc<PlayerState>) {
    tauri::async_runtime::spawn(async move {
        let mut backoff = BACKOFF_MIN;
        loop {
            if let Err(e) = run_session(&app, &state, &mut backoff).await {
                tracing::warn!("D-Bus session lost: {e}");
            }
            disconnect(&app, &state);
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(BACKOFF_MAX);
        }
    });
}

enum Exit {
    /// The followed player went away or a better one appeared.
    Reselect,
}

async fn run_session(
    app: &AppHandle,
    state: &PlayerState,
    backoff: &mut Duration,
) -> AppResult<()> {
    let conn = Connection::session().await?;
    let dbus = DBusProxy::new(&conn).await?;
    let mut owners = dbus.receive_name_owner_changed().await?;
    *backoff = BACKOFF_MIN;
    tracing::info!("connected to session bus");

    loop {
        let Some(name) = select_player(&conn, &dbus, state.preferred().as_deref()).await? else {
            disconnect(app, state);
            tracing::info!("no MPRIS player found, waiting");
            // Wait until some MPRIS player appears.
            loop {
                let signal = owners.next().await.ok_or(zbus::Error::InputOutput(
                    std::io::Error::other("NameOwnerChanged stream ended").into(),
                ))?;
                let args = signal.args()?;
                if args.name().starts_with(MPRIS_PREFIX) && args.new_owner().is_some() {
                    break;
                }
            }
            continue;
        };

        tracing::info!("following {name}");
        match follow(app, state, &conn, &dbus, &name, &mut owners).await {
            Ok(Exit::Reselect) => {}
            Err(e) => {
                tracing::warn!("lost {name}: {e}");
                disconnect(app, state);
                tokio::time::sleep(*backoff).await;
                *backoff = (*backoff * 2).min(BACKOFF_MAX);
            }
        }
    }
}

/// Short names of all MPRIS players currently on the bus, for the settings UI.
pub async fn list_players() -> AppResult<Vec<String>> {
    let conn = Connection::session().await?;
    let dbus = DBusProxy::new(&conn).await?;
    let mut names: Vec<String> = dbus
        .list_names()
        .await?
        .iter()
        .filter_map(|n| n.as_str().strip_prefix(MPRIS_PREFIX).map(str::to_owned))
        .collect();
    names.sort();
    Ok(names)
}

/// Lists MPRIS players with their playing state and applies the selection rules.
async fn select_player(
    conn: &Connection,
    dbus: &DBusProxy<'_>,
    preferred: Option<&str>,
) -> AppResult<Option<String>> {
    let mut players = Vec::new();
    for name in dbus.list_names().await? {
        let Some(short) = name.as_str().strip_prefix(MPRIS_PREFIX) else {
            continue;
        };
        let playing = match PlayerProxy::builder(conn)
            .destination(name.to_string())?
            .build()
            .await
        {
            Ok(p) => p
                .playback_status()
                .await
                .map(|s| s == "Playing")
                .unwrap_or(false),
            Err(_) => false,
        };
        players.push((short.to_owned(), playing));
    }
    players.sort();
    Ok(choose_player(&players, preferred).map(|s| format!("{MPRIS_PREFIX}{s}")))
}

async fn follow(
    app: &AppHandle,
    state: &PlayerState,
    conn: &Connection,
    dbus: &DBusProxy<'_>,
    bus_name: &str,
    owners: &mut zbus::fdo::NameOwnerChangedStream,
) -> AppResult<Exit> {
    let proxy = PlayerProxy::builder(conn)
        .destination(bus_name.to_owned())?
        .build()
        .await?;
    let short = short_name(bus_name).to_owned();
    let mut metadata_changes = proxy.receive_metadata_changed().await;
    let mut status_changes = proxy.receive_playback_status_changed().await;
    let mut seeks = proxy.receive_seeked().await?;
    let mut volume_changes = proxy.receive_volume_changed().await;

    let rate = proxy.rate().await.unwrap_or(1.0);
    let volume = proxy.volume().await.ok().filter(|v| v.is_finite());
    {
        let mut s = state.lock();
        s.proxy = Some(proxy.clone());
        s.player = Some(short.clone());
        s.rate = rate;
        s.volume = volume;
        s.track = None;
        s.status = None;
    }
    let status = PlaybackStatus::parse(&proxy.playback_status().await?);
    apply_status(app, state, status, &short);
    emit(app, events::VOLUME_CHANGED, &volume);
    apply_metadata(app, state, &proxy.metadata().await?, &short);
    poll_position(app, state, &proxy, true).await;

    let mut tick = tokio::time::interval(POLL_INTERVAL);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            Some(change) = metadata_changes.next() => {
                if let Ok(meta) = change.get().await {
                    if apply_metadata(app, state, &meta, &short) {
                        poll_position(app, state, &proxy, true).await;
                    }
                }
            }
            Some(change) = volume_changes.next() => {
                if let Ok(v) = change.get().await {
                    let v = Some(v.clamp(0.0, 1.0));
                    state.lock().volume = v;
                    emit(app, events::VOLUME_CHANGED, &v);
                }
            }
            Some(change) = status_changes.next() => {
                if let Ok(status) = change.get().await {
                    apply_status(app, state, PlaybackStatus::parse(&status), &short);
                    poll_position(app, state, &proxy, true).await;
                }
            }
            Some(signal) = seeks.next() => {
                if let Ok(args) = signal.args() {
                    let pos_ms = u64::try_from(args.position_us / 1000).unwrap_or(0);
                    let now = state.now_ms();
                    state.lock().tracker.seeked(pos_ms, now);
                    publish_position(app, state, pos_ms, true);
                }
            }
            _ = tick.tick() => {
                let playing = state.lock().status == Some(PlaybackStatus::Playing);
                if playing {
                    poll_position(app, state, &proxy, false).await;
                }
            }
            _ = state.repoll.notified() => {
                tokio::time::sleep(REPOLL_DELAY).await;
                poll_position(app, state, &proxy, true).await;
            }
            _ = state.reselect.notified() => {
                let best = select_player(conn, dbus, state.preferred().as_deref()).await?;
                if best.as_deref() != Some(bus_name) {
                    return Ok(Exit::Reselect);
                }
            }
            signal = owners.next() => {
                let signal = signal.ok_or_else(|| zbus::Error::InputOutput(
                    std::io::Error::other("NameOwnerChanged stream ended").into(),
                ))?;
                let args = signal.args()?;
                if !args.name().starts_with(MPRIS_PREFIX) {
                    continue;
                }
                if args.name().as_str() == bus_name && args.new_owner().is_none() {
                    tracing::info!("{bus_name} quit");
                    return Ok(Exit::Reselect);
                }
                if args.new_owner().is_some() {
                    let best = select_player(conn, dbus, state.preferred().as_deref()).await?;
                    if best.as_deref() != Some(bus_name) {
                        return Ok(Exit::Reselect);
                    }
                }
            }
        }
    }
}

/// Updates the current track. Returns true when the track itself changed
/// (not just e.g. late-arriving artwork for the same track).
fn apply_metadata(
    app: &AppHandle,
    state: &PlayerState,
    meta: &std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
    player: &str,
) -> bool {
    let track = parse_metadata(meta, player);
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

async fn poll_position(app: &AppHandle, state: &PlayerState, proxy: &PlayerProxy<'_>, hard: bool) {
    let read = match proxy.position().await {
        Ok(us) => u64::try_from(us / 1000).unwrap_or(0),
        Err(e) => {
            tracing::debug!("Position read failed: {e}");
            // Fall back to our estimate rather than skipping the update.
            let now = state.now_ms();
            let pos = state.lock().tracker.estimate(now);
            publish_position(app, state, pos, hard);
            return;
        }
    };
    let now = state.now_ms();
    let pos = state.lock().tracker.polled(read, now);
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
        let was = s.proxy.is_some();
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
