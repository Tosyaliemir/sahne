//! Finds lyrics for the current track (LRCLIB + disk cache) and tracks
//! per-track timing offsets.

pub mod lrc;
mod lrclib;
mod normalize;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::watch;

use crate::events::{self, LyricsLoaded, LyricsState, TrackInfo};
use crate::paths::{self, Paths};
use crate::sync::{lock, Inflight};
use lrclib::{FetchError, Record};

/// Negative results are retried after a day; positive ones are kept forever.
const NOT_FOUND_TTL_S: u64 = 24 * 60 * 60;
const OFFLINE_RETRY: Duration = Duration::from_secs(20);
/// When LRCLIB stays busy (429/5xx) beyond our in-request retries, wait this long
/// (doubling, capped) and try again while the track is still playing.
const RATE_LIMIT_RETRY: Duration = Duration::from_secs(3);
const RATE_LIMIT_RETRY_MAX: Duration = Duration::from_secs(60);

/// Bumped whenever lookup rules change so stale (possibly junk) entries are refetched.
const CACHE_VERSION: u32 = 2;
/// Offsets are saved this long after the last `[`/`]` press.
const OFFSET_SAVE_DELAY: Duration = Duration::from_millis(500);

/// What we cache per track: the raw LRCLIB fields, or `None` for "not found".
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    #[serde(default)]
    version: u32,
    fetched_at: u64,
    found: Option<Found>,
}

/// The state a lookup result is shown as.
fn state_of(found: Option<&Found>) -> LyricsState {
    match found {
        Some(f) if f.synced.is_some() => LyricsState::Synced,
        Some(f) if f.plain.is_some() => LyricsState::Plain,
        Some(f) if f.instrumental => LyricsState::Instrumental,
        _ => LyricsState::NotFound,
    }
}

/// The payload sent to the frontend for a lookup result.
fn build_loaded(
    track: &TrackInfo,
    state: LyricsState,
    found: Option<&Found>,
    offset_ms: i64,
) -> LyricsLoaded {
    let lines = found
        .and_then(|f| f.synced.as_deref())
        .map(|s| lrc::parse(s, track.length_ms))
        .unwrap_or_default();
    // A synced body that parses to nothing is no better than plain text.
    let state = if state == LyricsState::Synced && lines.is_empty() {
        if found.and_then(|f| f.plain.as_ref()).is_some() {
            LyricsState::Plain
        } else {
            LyricsState::NotFound
        }
    } else {
        state
    };
    LyricsLoaded {
        track_id: track.id.clone(),
        state,
        lines,
        plain: found.and_then(|f| f.plain.clone()),
        offset_ms,
    }
}

/// Dev tooling: the payload the app would show for a cache file's content.
#[cfg(feature = "dev-fixture")]
pub(crate) fn loaded_from_cache(track: &TrackInfo, bytes: &[u8]) -> Option<LyricsLoaded> {
    let entry: CacheEntry = serde_json::from_slice(bytes).ok()?;
    let found = entry.found.as_ref();
    Some(build_loaded(track, state_of(found), found, 0))
}

/// File name of a track's entry in the lyrics cache.
pub(crate) fn cache_file_name(track: &TrackInfo) -> String {
    let key = format!(
        "{}|{}|{}",
        track.artist,
        track.title,
        track.length_ms / 1000
    );
    format!("{}.json", paths::stable_hash(&key))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Found {
    synced: Option<String>,
    plain: Option<String>,
    instrumental: bool,
}

impl From<&Record> for Found {
    fn from(r: &Record) -> Self {
        Self {
            synced: r.has_synced().then(|| r.synced_lyrics.clone()).flatten(),
            plain: r.has_plain().then(|| r.plain_lyrics.clone()).flatten(),
            instrumental: r.instrumental,
        }
    }
}

pub struct LyricsService {
    client: Option<lrclib::Client>,
    cache_dir: PathBuf,
    offsets_path: PathBuf,
    offsets: Mutex<HashMap<String, i64>>,
    /// Bumped on every offset change; a pending save only writes if it is
    /// still the latest (holding `]` causes one write, not one per press).
    offsets_rev: AtomicU64,
    current: Mutex<Option<LyricsLoaded>>,
    /// Bumped on every track change so stale fetches drop their results.
    generation: AtomicU64,
    track: Mutex<Option<TrackInfo>>,
    /// The running lookup; aborted when the track changes so skipped songs
    /// don't keep LRCLIB's (strictly rate-limited) request queue busy.
    inflight: Inflight,
}

impl LyricsService {
    pub fn new(paths: &Paths) -> Self {
        let client = lrclib::Client::new()
            .map_err(|e| tracing::error!("HTTP client unavailable: {e}"))
            .ok();
        let offsets_path = paths.config.join("offsets.json");
        let offsets = paths::read_json(&offsets_path).unwrap_or_default();
        Self {
            client,
            cache_dir: paths.lyrics_cache(),
            offsets_path,
            offsets: Mutex::new(offsets),
            offsets_rev: AtomicU64::new(0),
            current: Mutex::new(None),
            generation: AtomicU64::new(0),
            track: Mutex::new(None),
            inflight: Inflight::default(),
        }
    }

    pub fn current(&self) -> Option<LyricsLoaded> {
        lock(&self.current).clone()
    }

    /// Changes a track's offset in memory immediately and saves the offsets
    /// file [`OFFSET_SAVE_DELAY`] after the last change, off the main thread.
    pub fn set_offset(self: &Arc<Self>, track_id: &str, offset_ms: i64) {
        {
            let mut offsets = lock(&self.offsets);
            if offset_ms == 0 {
                offsets.remove(track_id);
            } else {
                offsets.insert(track_id.to_owned(), offset_ms);
            }
        }
        if let Some(cur) = lock(&self.current)
            .as_mut()
            .filter(|c| c.track_id == track_id)
        {
            cur.offset_ms = offset_ms;
        }
        let rev = self.offsets_rev.fetch_add(1, Ordering::SeqCst) + 1;
        let this = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(OFFSET_SAVE_DELAY).await;
            if this.offsets_rev.load(Ordering::SeqCst) != rev {
                return; // a newer change will save
            }
            let snapshot = lock(&this.offsets).clone();
            let path = this.offsets_path.clone();
            let saved =
                tokio::task::spawn_blocking(move || paths::write_json(&path, &snapshot)).await;
            match saved {
                Ok(Ok(())) => {}
                Ok(Err(e)) => tracing::warn!("could not save offsets: {e}"),
                Err(_) => tracing::warn!("offset save task failed"),
            }
        });
    }

    fn offset_for(&self, track_id: &str) -> i64 {
        lock(&self.offsets).get(track_id).copied().unwrap_or(0)
    }

    /// Drops the cached result for the current track and fetches again.
    pub fn retry(self: &Arc<Self>, app: &AppHandle) {
        let track = lock(&self.track).clone();
        if let Some(track) = track {
            let _ = std::fs::remove_file(self.cache_path(&track));
            self.load(app.clone(), track);
        }
    }

    /// Follows track changes from the player for the lifetime of the app.
    pub fn spawn(self: Arc<Self>, app: AppHandle, mut tracks: watch::Receiver<Option<TrackInfo>>) {
        tauri::async_runtime::spawn(async move {
            loop {
                let track = tracks.borrow_and_update().clone();
                let changed =
                    lock(&self.track).as_ref().map(|t| &t.id) != track.as_ref().map(|t| &t.id);
                if changed {
                    *lock(&self.track) = track.clone();
                    match track {
                        Some(t) => self.load(app.clone(), t),
                        None => {
                            self.generation.fetch_add(1, Ordering::SeqCst);
                            self.replace_inflight(None);
                            *lock(&self.current) = None;
                        }
                    }
                }
                if tracks.changed().await.is_err() {
                    break;
                }
            }
        });
    }

    fn load(self: &Arc<Self>, app: AppHandle, track: TrackInfo) {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.publish(&app, generation, &track, LyricsState::Loading, None);
        let this = Arc::clone(self);
        let task = tauri::async_runtime::spawn(async move {
            let mut rate_limit_wait = RATE_LIMIT_RETRY;
            loop {
                let result = this.resolve(&track).await;
                if this.generation.load(Ordering::SeqCst) != generation {
                    return;
                }
                match result {
                    Ok(found) => {
                        let state = state_of(found.as_ref());
                        this.publish(&app, generation, &track, state, found.as_ref());
                        return;
                    }
                    Err(FetchError::Offline(e)) => {
                        tracing::info!("offline while fetching lyrics: {e}");
                        this.publish(&app, generation, &track, LyricsState::Offline, None);
                        tokio::time::sleep(OFFLINE_RETRY).await;
                        if this.generation.load(Ordering::SeqCst) != generation {
                            return;
                        }
                    }
                    Err(FetchError::Busy) => {
                        // Stay in "loading" rather than showing an error for a transient condition.
                        tracing::info!("LRCLIB busy; retrying in {rate_limit_wait:?}");
                        tokio::time::sleep(rate_limit_wait).await;
                        rate_limit_wait = (rate_limit_wait * 2).min(RATE_LIMIT_RETRY_MAX);
                        if this.generation.load(Ordering::SeqCst) != generation {
                            return;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("lyrics lookup failed: {e}");
                        this.publish(&app, generation, &track, LyricsState::Error, None);
                        return;
                    }
                }
            }
        });
        self.replace_inflight(Some(task));
    }

    fn replace_inflight(&self, task: Option<tauri::async_runtime::JoinHandle<()>>) {
        self.inflight.replace(task);
    }

    fn publish(
        &self,
        app: &AppHandle,
        generation: u64,
        track: &TrackInfo,
        state: LyricsState,
        found: Option<&Found>,
    ) {
        let payload = build_loaded(track, state, found, self.offset_for(&track.id));
        {
            let mut cur = lock(&self.current);
            if self.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            *cur = Some(payload.clone());
        }
        tracing::info!(
            "lyrics for {}: {:?} ({} lines)",
            track.title,
            payload.state,
            payload.lines.len()
        );
        if let Err(e) = app.emit(events::LYRICS_LOADED, payload) {
            tracing::warn!("failed to emit lyrics: {e}");
        }
    }

    fn cache_path(&self, track: &TrackInfo) -> PathBuf {
        self.cache_dir.join(cache_file_name(track))
    }

    /// Cache first, then LRCLIB. `Ok(None)` means there are no lyrics.
    async fn resolve(&self, track: &TrackInfo) -> Result<Option<Found>, FetchError> {
        let path = self.cache_path(track);
        // Entries from older lookup rules may hold junk; refetch those.
        if let Ok(entry) = paths::read_json::<CacheEntry>(&path) {
            let fresh = entry.found.is_some()
                || paths::unix_now().saturating_sub(entry.fetched_at) < NOT_FOUND_TTL_S;
            if entry.version == CACHE_VERSION && fresh {
                tracing::debug!("lyrics cache hit for {}", track.title);
                crate::cache::touch(&path);
                return Ok(entry.found);
            }
        }
        let Some(client) = &self.client else {
            return Ok(None);
        };
        let found = fetch(client, track).await?;
        let entry = CacheEntry {
            version: CACHE_VERSION,
            fetched_at: paths::unix_now(),
            found: found.clone(),
        };
        if let Err(e) = paths::write_json(&path, &entry) {
            tracing::warn!("could not cache lyrics: {e}");
        }
        Ok(found)
    }
}

/// Cache entries written under an older [`CACHE_VERSION`] (or unreadable).
pub fn is_obsolete_cache_file(path: &std::path::Path) -> bool {
    #[derive(serde::Deserialize)]
    struct Version {
        version: u32,
    }
    if path.extension().and_then(|e| e.to_str()) != Some("json") {
        return true;
    }
    paths::read_json::<Version>(path).map_or(true, |v| v.version != CACHE_VERSION)
}

/// Lookup strategy: exact match with the original metadata, then with a
/// cleaned title and the primary artist, then a fuzzy search.
async fn fetch(client: &lrclib::Client, track: &TrackInfo) -> Result<Option<Found>, FetchError> {
    let duration_s = (track.length_ms as f64 / 1000.0).round() as u64;
    let clean = normalize::clean_title(&track.title);
    let primary = normalize::primary_artist(&track.artist);
    let mut fallback: Option<Found> = None;
    let mut consider = |record: &lrclib::Record| {
        if record.is_useful() && fallback.is_none() {
            fallback = Some(Found::from(record));
        }
        record.has_synced()
    };

    // Fastest first: LRCLIB's own database with the exact metadata (~0.3 s).
    if let Some(r) = client
        .get_cached(&track.artist, &track.title, &track.album, duration_s)
        .await?
    {
        if consider(&r) {
            return Ok(Some(Found::from(&r)));
        }
    }
    // A fuzzy search catches "feat."/"Remastered" variants and other albums.
    let results = client.search(primary, &clean).await?;
    if let Some(best) = lrclib::pick_best(&results, track.length_ms as f64 / 1000.0) {
        if consider(best) {
            return Ok(Some(Found::from(best)));
        }
    }
    // Last resort, slow: lets LRCLIB look in external sources too.
    if let Some(r) = client
        .get(primary, &clean, &track.album, duration_s)
        .await?
    {
        if consider(&r) {
            return Ok(Some(Found::from(&r)));
        }
    }
    Ok(fallback)
}
