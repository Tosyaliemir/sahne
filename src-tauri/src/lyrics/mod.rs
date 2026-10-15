//! Finds lyrics for the current track (LRCLIB + disk cache) and tracks
//! per-track timing offsets.

pub mod lrc;
mod lrclib;
mod normalize;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::watch;

use crate::events::{self, LyricsLoaded, LyricsState, TrackInfo};
use crate::paths::{self, Paths};
use lrclib::{FetchError, Record};

/// Negative results are retried after a day; positive ones are kept forever.
const NOT_FOUND_TTL_S: u64 = 24 * 60 * 60;
const OFFLINE_RETRY: Duration = Duration::from_secs(20);
/// After LRCLIB's rate limit outlasts our in-request retries, wait this long
/// (doubling, capped) and try again while the track is still playing.
const RATE_LIMIT_RETRY: Duration = Duration::from_secs(15);
const RATE_LIMIT_RETRY_MAX: Duration = Duration::from_secs(120);

/// What we cache per track: the raw LRCLIB fields, or `None` for "not found".
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    fetched_at: u64,
    found: Option<Found>,
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
    current: Mutex<Option<LyricsLoaded>>,
    /// Bumped on every track change so stale fetches drop their results.
    generation: AtomicU64,
    track: Mutex<Option<TrackInfo>>,
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
            current: Mutex::new(None),
            generation: AtomicU64::new(0),
            track: Mutex::new(None),
        }
    }

    pub fn current(&self) -> Option<LyricsLoaded> {
        lock(&self.current).clone()
    }

    pub fn set_offset(&self, track_id: &str, offset_ms: i64) {
        let snapshot = {
            let mut offsets = lock(&self.offsets);
            if offset_ms == 0 {
                offsets.remove(track_id);
            } else {
                offsets.insert(track_id.to_owned(), offset_ms);
            }
            offsets.clone()
        };
        if let Some(cur) = lock(&self.current)
            .as_mut()
            .filter(|c| c.track_id == track_id)
        {
            cur.offset_ms = offset_ms;
        }
        if let Err(e) = paths::write_json(&self.offsets_path, &snapshot) {
            tracing::warn!("could not save offsets: {e}");
        }
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
        tauri::async_runtime::spawn(async move {
            let mut rate_limit_wait = RATE_LIMIT_RETRY;
            loop {
                let result = this.resolve(&track).await;
                if this.generation.load(Ordering::SeqCst) != generation {
                    return;
                }
                match result {
                    Ok(found) => {
                        let state = match &found {
                            Some(f) if f.synced.is_some() => LyricsState::Synced,
                            Some(f) if f.plain.is_some() => LyricsState::Plain,
                            Some(f) if f.instrumental => LyricsState::Instrumental,
                            _ => LyricsState::NotFound,
                        };
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
                    Err(FetchError::RateLimited) => {
                        // Stay in "loading" rather than showing an error for a transient condition.
                        tracing::info!("LRCLIB rate limit; retrying in {rate_limit_wait:?}");
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
    }

    fn publish(
        &self,
        app: &AppHandle,
        generation: u64,
        track: &TrackInfo,
        state: LyricsState,
        found: Option<&Found>,
    ) {
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
        let payload = LyricsLoaded {
            track_id: track.id.clone(),
            state,
            lines,
            plain: found.and_then(|f| f.plain.clone()),
            offset_ms: self.offset_for(&track.id),
        };
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
        let key = format!(
            "{}|{}|{}",
            track.artist,
            track.title,
            track.length_ms / 1000
        );
        self.cache_dir
            .join(format!("{}.json", paths::stable_hash(&key)))
    }

    /// Cache first, then LRCLIB. `Ok(None)` means there are no lyrics.
    async fn resolve(&self, track: &TrackInfo) -> Result<Option<Found>, FetchError> {
        let path = self.cache_path(track);
        if let Ok(entry) = paths::read_json::<CacheEntry>(&path) {
            let fresh = entry.found.is_some()
                || paths::unix_now().saturating_sub(entry.fetched_at) < NOT_FOUND_TTL_S;
            if fresh {
                tracing::debug!("lyrics cache hit for {}", track.title);
                return Ok(entry.found);
            }
        }
        let Some(client) = &self.client else {
            return Ok(None);
        };
        let found = fetch(client, track).await?;
        let entry = CacheEntry {
            fetched_at: paths::unix_now(),
            found: found.clone(),
        };
        if let Err(e) = paths::write_json(&path, &entry) {
            tracing::warn!("could not cache lyrics: {e}");
        }
        Ok(found)
    }
}

/// Lookup strategy: exact match with the original metadata, then with a
/// cleaned title and the primary artist, then a fuzzy search.
async fn fetch(client: &lrclib::Client, track: &TrackInfo) -> Result<Option<Found>, FetchError> {
    let duration_s = (track.length_ms as f64 / 1000.0).round() as u64;
    let clean = normalize::clean_title(&track.title);
    let primary = normalize::primary_artist(&track.artist);

    let mut attempts = vec![(track.artist.as_str(), track.title.as_str())];
    if (primary, clean.as_str()) != attempts[0] {
        attempts.push((primary, clean.as_str()));
    }

    let mut fallback: Option<Found> = None;
    for (artist, title) in &attempts {
        if let Some(record) = client.get(artist, title, &track.album, duration_s).await? {
            if record.has_synced() {
                return Ok(Some(Found::from(&record)));
            }
            fallback.get_or_insert_with(|| Found::from(&record));
        }
    }

    let results = client.search(primary, &clean).await?;
    if let Some(best) = lrclib::pick_best(&results, track.length_ms as f64 / 1000.0) {
        if best.has_synced() || fallback.is_none() {
            return Ok(Some(Found::from(best)));
        }
    }
    Ok(fallback)
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
