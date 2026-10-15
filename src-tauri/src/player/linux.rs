//! Linux backend: follows an MPRIS player on the D-Bus session bus.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use tauri::AppHandle;
use tokio::time::MissedTickBehavior;
use zbus::fdo::DBusProxy;
use zbus::zvariant::ObjectPath;
use zbus::Connection;

use super::mpris::{parse_metadata, short_name, PlayerProxy, MPRIS_PREFIX};
use super::select::choose_player;
use super::{
    apply_status, apply_track, apply_volume, disconnect, position_read, PlayerState, BACKOFF_MAX,
    BACKOFF_MIN,
};
use crate::error::AppResult;
use crate::events::PlaybackStatus;

/// What `PlayerState` keeps to talk to the followed player.
pub type Handle = PlayerProxy<'static>;

const POLL_INTERVAL: Duration = Duration::from_millis(1000);
/// Delay before re-reading Position after a command, giving the player time to apply it.
const REPOLL_DELAY: Duration = Duration::from_millis(150);

impl PlayerState {
    pub async fn set_volume(&self, volume: f64) -> AppResult<()> {
        self.handle()?.set_volume(volume.clamp(0.0, 1.0)).await?;
        Ok(())
    }

    pub async fn play_pause(&self) -> AppResult<()> {
        self.handle()?.play_pause().await?;
        Ok(())
    }

    pub async fn next(&self) -> AppResult<()> {
        self.handle()?.next().await?;
        Ok(())
    }

    pub async fn previous(&self) -> AppResult<()> {
        self.handle()?.previous().await?;
        Ok(())
    }

    pub async fn seek(&self, position_ms: u64) -> AppResult<()> {
        let proxy = self.handle()?;
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
        self.after_seek(position_ms);
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
    let volume = proxy.volume().await.ok();
    {
        let mut s = state.lock();
        s.handle = Some(proxy.clone());
        s.player = Some(short.clone());
        s.rate = rate;
        s.volume = None;
        s.track = None;
        s.status = None;
    }
    let status = PlaybackStatus::parse(&proxy.playback_status().await?);
    apply_status(app, state, status, &short);
    apply_volume(app, state, volume);
    apply_track(app, state, parse_metadata(&proxy.metadata().await?, &short));
    poll_position(app, state, &proxy, true).await;

    let mut tick = tokio::time::interval(POLL_INTERVAL);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            Some(change) = metadata_changes.next() => {
                if let Ok(meta) = change.get().await {
                    if apply_track(app, state, parse_metadata(&meta, &short)) {
                        poll_position(app, state, &proxy, true).await;
                    }
                }
            }
            Some(change) = volume_changes.next() => {
                if let Ok(v) = change.get().await {
                    apply_volume(app, state, Some(v));
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
                    super::publish_position(app, state, pos_ms, true);
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

async fn poll_position(app: &AppHandle, state: &PlayerState, proxy: &PlayerProxy<'_>, hard: bool) {
    let read = match proxy.position().await {
        Ok(us) => Some(u64::try_from(us / 1000).unwrap_or(0)),
        Err(e) => {
            // Fall back to our estimate rather than skipping the update.
            tracing::debug!("Position read failed: {e}");
            None
        }
    };
    position_read(app, state, read, hard);
}
