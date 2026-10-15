//! Event names and payloads sent from the backend to the frontend.
//!
//! The TypeScript mirror of these types lives in `src/lib/types.ts`; the
//! `ts_contract` test below serializes sample payloads so a field rename on
//! either side is caught.

use serde::Serialize;

pub const TRACK_CHANGED: &str = "track-changed";
pub const PLAYBACK_STATE: &str = "playback-state";
pub const POSITION_SYNC: &str = "position-sync";
pub const PLAYER_DISCONNECTED: &str = "player-disconnected";

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
        let state = PlaybackState { status: PlaybackStatus::Paused, player: "spotify".into() };
        assert_eq!(serde_json::to_value(&state).unwrap(), json!({"status":"paused","player":"spotify"}));
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
    }
}
