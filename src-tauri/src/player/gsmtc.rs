//! Windows backend: follows a media session through the Global System Media
//! Transport Controls, the same source the volume flyout and lock screen use.
//! Spotify, browsers and most players publish to it.
//!
//! Everything runs on one background thread that polls a few times a second:
//! the API's change events fire on arbitrary threads and Spotify updates its
//! timeline only sparsely anyway, so polling is simpler and just as timely.
//! There is no volume here: the API has none, so the UI hides the control.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ::windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use ::windows::Storage::Streams::DataReader;
use tauri::{AppHandle, Manager as _};

use super::select::choose_player;
use super::{
    apply_status, apply_track, apply_volume, disconnect, position_read, PlayerState, BACKOFF_MAX,
    BACKOFF_MIN,
};
use crate::error::{AppError, AppResult};
use crate::events::{PlaybackStatus, TrackInfo};

/// What `PlayerState` keeps to talk to the followed player.
pub type Handle = Session;

const TICK: Duration = Duration::from_millis(500);
/// Position is re-published every this many ticks while nothing else changes.
const POSITION_EVERY: u32 = 2;
/// Players often publish the thumbnail a moment after the title.
const ART_RETRIES: u32 = 10;

type WinResult<T> = ::windows::core::Result<T>;

impl PlayerState {
    pub async fn set_volume(&self, _volume: f64) -> AppResult<()> {
        Err(AppError::Unsupported("volume is not available on Windows"))
    }

    pub async fn play_pause(&self) -> AppResult<()> {
        let s = self.handle()?;
        blocking(move || s.TryTogglePlayPauseAsync()?.join()).await?;
        self.repoll.notify_one();
        Ok(())
    }

    pub async fn next(&self) -> AppResult<()> {
        let s = self.handle()?;
        blocking(move || s.TrySkipNextAsync()?.join()).await?;
        Ok(())
    }

    pub async fn previous(&self) -> AppResult<()> {
        let s = self.handle()?;
        blocking(move || s.TrySkipPreviousAsync()?.join()).await?;
        Ok(())
    }

    pub async fn seek(&self, position_ms: u64) -> AppResult<()> {
        let s = self.handle()?;
        // The timeline counts in 100 ns ticks from its StartTime.
        let ticks = i64::try_from(position_ms.saturating_mul(10_000)).unwrap_or(i64::MAX);
        blocking(move || {
            let start = s.GetTimelineProperties()?.StartTime()?.Duration;
            s.TryChangePlaybackPositionAsync(start.saturating_add(ticks))?
                .join()
        })
        .await?;
        self.after_seek(position_ms);
        Ok(())
    }
}

/// Runs a blocking WinRT call off the async runtime.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> WinResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| AppError::Unsupported("media call was cancelled"))?
        .map_err(AppError::from)
}

/// Starts the background thread; it never returns and survives API errors.
pub fn spawn(app: AppHandle, state: Arc<PlayerState>) {
    let spawned = std::thread::Builder::new()
        .name("sahne-media".into())
        .spawn(move || {
            let mut backoff = BACKOFF_MIN;
            loop {
                match Manager::RequestAsync().and_then(|op| op.join()) {
                    Ok(manager) => {
                        backoff = BACKOFF_MIN;
                        if let Err(e) = run(&app, &state, &manager) {
                            tracing::warn!("media session lost: {e}");
                        }
                    }
                    Err(e) => tracing::warn!("media sessions unavailable: {e}"),
                }
                disconnect(&app, &state);
                std::thread::sleep(backoff);
                backoff = (backoff * 2).min(BACKOFF_MAX);
            }
        });
    if let Err(e) = spawned {
        tracing::error!("could not start the media thread: {e}");
    }
}

/// Short names of all current media sessions, for the settings UI.
pub async fn list_players() -> AppResult<Vec<String>> {
    blocking(|| {
        let manager = Manager::RequestAsync()?.join()?;
        let mut names: Vec<String> = sessions(&manager)?.into_iter().map(|(n, _, _)| n).collect();
        names.sort();
        names.dedup();
        Ok(names)
    })
    .await
}

/// `(short name, is playing, session)` for every session, in system order.
fn sessions(manager: &Manager) -> WinResult<Vec<(String, bool, Session)>> {
    let mut out = Vec::new();
    for session in manager.GetSessions()? {
        let id = session.SourceAppUserModelId()?.to_string_lossy();
        let playing = session
            .GetPlaybackInfo()
            .and_then(|i| i.PlaybackStatus())
            .is_ok_and(|s| s == Status::Playing);
        out.push((short_name(&id), playing, session));
    }
    Ok(out)
}

/// Turns an app id into a short player name: `Spotify.exe` and the Store's
/// `SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify` both become `spotify`,
/// `MSEdge` becomes `msedge`.
fn short_name(app_id: &str) -> String {
    let lower = app_id.to_lowercase();
    if lower.contains("spotify") {
        return "spotify".to_owned();
    }
    let tail = lower.rsplit('!').next().unwrap_or(&lower);
    let tail = tail.rsplit(['\\', '/']).next().unwrap_or(tail);
    tail.strip_suffix(".exe").unwrap_or(tail).to_owned()
}

/// The player currently followed, and what we last saw from it.
struct Followed {
    name: String,
    session: Session,
    track_key: Option<String>,
    art_url: Option<String>,
    art_tries: u32,
}

fn run(app: &AppHandle, state: &PlayerState, manager: &Manager) -> WinResult<()> {
    let art_dir = app
        .path()
        .app_cache_dir()
        .map(|d| d.join("session-art"))
        .ok();
    let mut followed: Option<Followed> = None;
    let mut tick: u32 = 0;
    loop {
        tick = tick.wrapping_add(1);
        let all = sessions(manager)?;
        let pairs: Vec<(String, bool)> = all.iter().map(|(n, p, _)| (n.clone(), *p)).collect();
        let chosen = choose_player(&pairs, state.preferred().as_deref()).map(str::to_owned);

        let Some(name) = chosen else {
            if followed.take().is_some() {
                tracing::info!("no media session, waiting");
                disconnect(app, state);
            }
            std::thread::sleep(TICK);
            continue;
        };

        let switched = followed.as_ref().is_none_or(|f| f.name != name);
        if switched {
            // `chosen` came from this list, so the lookup cannot miss; skip the tick if it does.
            let Some(session) = all
                .into_iter()
                .find(|(n, _, _)| *n == name)
                .map(|(_, _, s)| s)
            else {
                std::thread::sleep(TICK);
                continue;
            };
            tracing::info!("following {name}");
            disconnect(app, state);
            {
                let mut s = state.lock();
                s.handle = Some(session.clone());
                s.player = Some(name.clone());
            }
            apply_volume(app, state, None);
            followed = Some(Followed {
                name: name.clone(),
                session,
                track_key: None,
                art_url: None,
                art_tries: 0,
            });
        }
        let Some(f) = followed.as_mut() else {
            std::thread::sleep(TICK);
            continue;
        };

        let status = match f.session.GetPlaybackInfo().and_then(|i| i.PlaybackStatus()) {
            Ok(Status::Playing) => PlaybackStatus::Playing,
            Ok(Status::Paused) => PlaybackStatus::Paused,
            _ => PlaybackStatus::Stopped,
        };
        let was = state.lock().status;
        apply_status(app, state, status, &f.name);

        let new_track = update_track(app, state, f, art_dir.as_ref());
        let hard = switched || new_track || was != Some(status);
        // A seek from our UI asks for a fresh read; consume it without waiting.
        let asked = poll_notify(&state.repoll);
        if hard || asked || tick.is_multiple_of(POSITION_EVERY) {
            position_read(app, state, read_position(&f.session, status), hard || asked);
        }
        std::thread::sleep(TICK);
    }
}

/// Reads title/artist/album/length and the thumbnail; returns true when the
/// track itself changed.
fn update_track(
    app: &AppHandle,
    state: &PlayerState,
    f: &mut Followed,
    art_dir: Option<&PathBuf>,
) -> bool {
    let Ok(props) = f
        .session
        .TryGetMediaPropertiesAsync()
        .and_then(|op| op.join())
    else {
        return false;
    };
    let title = props
        .Title()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    if title.trim().is_empty() {
        f.track_key = None;
        return apply_track(app, state, None);
    }
    let artist = props
        .Artist()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let album = props
        .AlbumTitle()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    // No track id in this API; the length is left out because players fill
    // in the timeline a moment after the title.
    let key = format!("{artist}|{title}|{album}");
    if f.track_key.as_deref() != Some(key.as_str()) {
        f.track_key = Some(key.clone());
        f.art_url = None;
        f.art_tries = 0;
    }
    if f.art_url.is_none() && f.art_tries < ART_RETRIES {
        f.art_tries += 1;
        if let Some(dir) = art_dir {
            match save_thumbnail(&props, dir, &key) {
                Ok(url) => f.art_url = url,
                Err(e) => tracing::debug!("thumbnail not readable yet: {e}"),
            }
        }
    }
    let length_ms = f
        .session
        .GetTimelineProperties()
        .and_then(|t| Ok(t.EndTime()?.Duration - t.StartTime()?.Duration))
        .map_or(0, |ticks| u64::try_from(ticks / 10_000).unwrap_or(0));
    apply_track(
        app,
        state,
        Some(TrackInfo {
            id: key,
            title,
            artist,
            album,
            length_ms,
            art_url: f.art_url.clone(),
            player: f.name.clone(),
        }),
    )
}

/// Position in ms. The timeline is a snapshot taken at `LastUpdatedTime`, so
/// while playing the time since then is added.
fn read_position(session: &Session, status: PlaybackStatus) -> Option<u64> {
    let t = session.GetTimelineProperties().ok()?;
    let mut ticks = t.Position().ok()?.Duration - t.StartTime().ok()?.Duration;
    if status == PlaybackStatus::Playing {
        let updated = t.LastUpdatedTime().ok()?.UniversalTime;
        if updated > 0 {
            ticks += (filetime_now() - updated).max(0);
        }
    }
    u64::try_from(ticks / 10_000).ok()
}

/// Now, in 100 ns ticks since 1601-01-01 (the WinRT `DateTime` epoch).
fn filetime_now() -> i64 {
    const UNIX_TO_1601_SECS: i64 = 11_644_473_600;
    let since_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let ticks = i64::try_from(since_unix.as_nanos() / 100).unwrap_or(i64::MAX);
    ticks.saturating_add(UNIX_TO_1601_SECS * 10_000_000)
}

/// Writes the session's thumbnail to the cache and returns a `file://` URL
/// for the artwork service, or `None` if the player has no thumbnail.
fn save_thumbnail(
    props: &::windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
    dir: &PathBuf,
    key: &str,
) -> WinResult<Option<String>> {
    let Ok(reference) = props.Thumbnail() else {
        return Ok(None);
    };
    let stream = reference.OpenReadAsync()?.join()?;
    let size = u32::try_from(stream.Size()?).unwrap_or(u32::MAX);
    if size == 0 {
        return Ok(None);
    }
    let reader = DataReader::CreateDataReader(&stream)?;
    let read = reader.LoadAsync(size)?.join()?;
    let mut bytes = vec![0u8; read as usize];
    reader.ReadBytes(&mut bytes)?;

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    let path = dir.join(format!("{:016x}.img", hasher.finish()));
    if let Err(e) = std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&path, &bytes)) {
        tracing::warn!("could not cache thumbnail: {e}");
        return Ok(None);
    }
    Ok(Some(format!("file://{}", path.display())))
}

/// Takes a pending `notify_one` permit without blocking.
fn poll_notify(notify: &tokio::sync::Notify) -> bool {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    let fut = notify.notified();
    let mut fut = std::pin::pin!(fut);
    matches!(
        fut.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_names_from_app_ids() {
        assert_eq!(short_name("Spotify.exe"), "spotify");
        assert_eq!(
            short_name("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"),
            "spotify"
        );
        assert_eq!(short_name("MSEdge"), "msedge");
        assert_eq!(
            short_name("C:\\Program Files\\VideoLAN\\VLC\\vlc.exe"),
            "vlc"
        );
        assert_eq!(
            short_name("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"),
            "microsoft.zunemusic"
        );
    }

    #[test]
    fn filetime_is_after_2020() {
        // 2020-01-01 in WinRT ticks.
        assert!(filetime_now() > 132_223_104_000_000_000);
    }
}
