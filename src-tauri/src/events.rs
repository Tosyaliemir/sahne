//! Event names and payloads sent from the backend to the frontend.
//!
//! The TypeScript mirror of these types lives in `src/lib/types.ts`; the
//! `ts_contract` test below serializes sample payloads so a field rename on
//! either side is caught.

use serde::Serialize;

use crate::artwork::palette::Palette;
use crate::lyrics::lrc::Line;

pub const TRACK_CHANGED: &str = "track-changed";
pub const PLAYBACK_STATE: &str = "playback-state";
pub const POSITION_SYNC: &str = "position-sync";
pub const PLAYER_DISCONNECTED: &str = "player-disconnected";
pub const LYRICS_LOADED: &str = "lyrics-loaded";
pub const PALETTE_CHANGED: &str = "palette-changed";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrackInfo {
    /// `mpris:trackid`, or a synthesized id when the player omits it.
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub length_ms: u64,
    pub art_url: Option<String>,
    /// Short bus name suffix, e.g. `spotify`.
    pub player: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

impl PlaybackStatus {
    pub fn parse(s: &str) -> Self {
        match s {
            "Playing" => Self::Playing,
            "Paused" => Self::Paused,
            _ => Self::Stopped,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlaybackState {
    pub status: PlaybackStatus,
    pub player: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PositionSource {
    /// Read from the player's `Position` property or `Seeked` signal.
    Player,
    /// The player reports a constant 0; we count time ourselves.
    Estimated,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PositionSync {
    pub position_ms: u64,
    /// Backend monotonic clock (ms since app start) when the position was sampled.
    pub monotonic_ms: u64,
    pub playing: bool,
    pub rate: f64,
    pub source: PositionSource,
    /// True after a seek or track change: the frontend must not smooth this jump.
    pub hard: bool,
}

/// Snapshot returned by the `get_state` command so a freshly loaded frontend
/// does not have to wait for the next event.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PlayerSnapshot {
    pub track: Option<TrackInfo>,
    pub playback: Option<PlaybackState>,
    pub position: Option<PositionSync>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricsState {
    Loading,
    /// Time-stamped lines are available.
    Synced,
    /// Only unsynced text is available.
    Plain,
    Instrumental,
    NotFound,
    /// The lookup could not reach the network; it is retried automatically.
    Offline,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct LyricsLoaded {
    pub track_id: String,
    pub state: LyricsState,
    pub lines: Vec<Line>,
    pub plain: Option<String>,
    /// User's per-track timing correction; positive shows lyrics later.
    pub offset_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaletteChanged {
    pub track_id: String,
    /// Cached artwork file, served to the webview through the asset protocol.
    pub art_path: Option<String>,
    /// Small pre-blurred version of the artwork for the backdrop.
    pub background_path: Option<String>,
    pub palette: Palette,
}

impl PaletteChanged {
    pub fn empty(track_id: &str) -> Self {
        Self {
            track_id: track_id.to_owned(),
            art_path: None,
            background_path: None,
            palette: Palette::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ts_contract() {
        let track = TrackInfo {
            id: "/t/1".into(),
            title: "T".into(),
            artist: "A".into(),
            album: "B".into(),
            length_ms: 1000,
            art_url: None,
            player: "spotify".into(),
        };
        assert_eq!(
            serde_json::to_value(&track).unwrap(),
            json!({"id":"/t/1","title":"T","artist":"A","album":"B","length_ms":1000,"art_url":null,"player":"spotify"})
        );
        let state = PlaybackState {
            status: PlaybackStatus::Paused,
            player: "spotify".into(),
        };
        assert_eq!(
            serde_json::to_value(&state).unwrap(),
            json!({"status":"paused","player":"spotify"})
        );
        let pos = PositionSync {
            position_ms: 5,
            monotonic_ms: 7,
            playing: true,
            rate: 1.0,
            source: PositionSource::Estimated,
            hard: false,
        };
        assert_eq!(
            serde_json::to_value(&pos).unwrap(),
            json!({"position_ms":5,"monotonic_ms":7,"playing":true,"rate":1.0,"source":"estimated","hard":false})
        );
        let lyrics = LyricsLoaded {
            track_id: "x".into(),
            state: LyricsState::NotFound,
            lines: crate::lyrics::lrc::parse("[00:01.00]<00:01.00>a <00:01.50>b", 2000),
            plain: None,
            offset_ms: -100,
        };
        assert_eq!(
            serde_json::to_value(&lyrics).unwrap(),
            json!({"track_id":"x","state":"not_found","plain":null,"offset_ms":-100,"lines":[
                {"start_ms":1000,"end_ms":2000,"text":"a b","words":[
                    {"start_ms":1000,"end_ms":1500,"text":"a"},{"start_ms":1500,"end_ms":2000,"text":"b"}]}]})
        );
        let palette = serde_json::to_value(PaletteChanged::empty("x")).unwrap();
        let keys: Vec<&str> = palette["palette"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys.len(), 6);
        for k in [
            "dominant",
            "vibrant",
            "muted",
            "dark_bg",
            "light_text",
            "accent",
        ] {
            assert!(keys.contains(&k), "missing {k}");
        }
        assert!(palette["art_path"].is_null() && palette["background_path"].is_null());
    }
}
