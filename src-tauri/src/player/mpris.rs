//! MPRIS D-Bus proxies and pure helpers for metadata parsing and player selection.

use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Value};

use crate::events::TrackInfo;

pub const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";

#[zbus::proxy(
    interface = "org.mpris.MediaPlayer2.Player",
    default_path = "/org/mpris/MediaPlayer2"
)]
pub trait Player {
    fn play_pause(&self) -> zbus::Result<()>;
    fn next(&self) -> zbus::Result<()>;
    fn previous(&self) -> zbus::Result<()>;
    fn seek(&self, offset_us: i64) -> zbus::Result<()>;
    fn set_position(
        &self,
        track_id: &zbus::zvariant::ObjectPath<'_>,
        position_us: i64,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    fn seeked(&self, position_us: i64) -> zbus::Result<()>;

    #[zbus(property)]
    fn metadata(&self) -> zbus::Result<HashMap<String, OwnedValue>>;
    #[zbus(property)]
    fn playback_status(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn rate(&self) -> zbus::Result<f64>;
    // Spotify does not emit PropertiesChanged for Position, so never cache it.
    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> zbus::Result<i64>;
}

/// Strips the MPRIS prefix: `org.mpris.MediaPlayer2.spotify` -> `spotify`.
pub fn short_name(bus_name: &str) -> &str {
    bus_name.strip_prefix(MPRIS_PREFIX).unwrap_or(bus_name)
}

/// Picks which player to follow.
///
/// Order: an explicit override (env/settings), then Spotify, then the first
/// player that is currently playing, then any player. `players` holds
/// `(short_name, is_playing)` pairs in bus order.
pub fn choose_player<'a>(
    players: &'a [(String, bool)],
    preferred: Option<&str>,
) -> Option<&'a str> {
    let find = |want: &str| {
        players
            .iter()
            .find(|(name, _)| name == want || is_instance_of(name, want))
            .map(|(name, _)| name.as_str())
    };
    preferred
        .and_then(find)
        .or_else(|| find("spotify"))
        .or_else(|| {
            players
                .iter()
                .find(|(_, playing)| *playing)
                .map(|(n, _)| n.as_str())
        })
        .or_else(|| players.first().map(|(n, _)| n.as_str()))
}

/// Matches multi-instance names such as `chromium.instance1234` against `chromium`.
fn is_instance_of(name: &str, base: &str) -> bool {
    name.strip_prefix(base)
        .is_some_and(|rest| rest.starts_with(".instance"))
}

/// Builds a [`TrackInfo`] from an MPRIS metadata map. Returns `None` when
/// there is no title (players report empty metadata while idle).
pub fn parse_metadata(meta: &HashMap<String, OwnedValue>, player: &str) -> Option<TrackInfo> {
    let title = meta.get("xesam:title").and_then(|v| as_string(v))?;
    if title.trim().is_empty() {
        return None;
    }
    let artist = meta
        .get("xesam:artist")
        .map(|v| as_string_list(v).join(", "))
        .unwrap_or_default();
    let album = meta
        .get("xesam:album")
        .and_then(|v| as_string(v))
        .unwrap_or_default();
    let length_us = meta
        .get("mpris:length")
        .and_then(|v| as_u64(v))
        .unwrap_or(0);
    let art_url = meta
        .get("mpris:artUrl")
        .and_then(|v| as_string(v))
        .filter(|s| !s.is_empty());
    let id = meta
        .get("mpris:trackid")
        .and_then(|v| as_string(v))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{artist}|{title}|{length_us}"));

    Some(TrackInfo {
        id,
        title,
        artist,
        album,
        length_ms: length_us / 1000,
        art_url,
        player: player.to_owned(),
    })
}

// Players disagree on variant types (Spotify sends trackid as `s` and length
// as `t`; the spec says `o` and `x`), so accept every reasonable encoding.

fn as_string(v: &Value<'_>) -> Option<String> {
    match v {
        Value::Str(s) => Some(s.to_string()),
        Value::ObjectPath(p) => Some(p.to_string()),
        Value::Value(inner) => as_string(inner),
        _ => None,
    }
}

fn as_string_list(v: &Value<'_>) -> Vec<String> {
    match v {
        Value::Array(arr) => arr.iter().filter_map(as_string).collect(),
        Value::Value(inner) => as_string_list(inner),
        other => as_string(other).into_iter().collect(),
    }
}

fn as_u64(v: &Value<'_>) -> Option<u64> {
    match v {
        Value::U64(n) => Some(*n),
        Value::I64(n) => u64::try_from(*n).ok(),
        Value::U32(n) => Some(u64::from(*n)),
        Value::I32(n) => u64::try_from(*n).ok(),
        Value::F64(n) if *n >= 0.0 => Some(*n as u64),
        Value::Value(inner) => as_u64(inner),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::ObjectPath;

    fn ov<'a>(v: impl Into<Value<'a>>) -> OwnedValue {
        v.into().try_to_owned().unwrap()
    }

    fn players(list: &[(&str, bool)]) -> Vec<(String, bool)> {
        list.iter().map(|(n, p)| (n.to_string(), *p)).collect()
    }

    #[test]
    fn parses_spotify_metadata() {
        let mut m = HashMap::new();
        m.insert("mpris:trackid".into(), ov("/com/spotify/track/abc"));
        m.insert("mpris:length".into(), ov(275_387_000u64));
        m.insert("mpris:artUrl".into(), ov("https://i.scdn.co/image/x"));
        m.insert("xesam:album".into(), ov("All Eyez On Me"));
        m.insert("xesam:artist".into(), ov(vec!["2Pac", "Dr. Dre"]));
        m.insert("xesam:title".into(), ov("Ambitionz Az A Ridah"));
        let t = parse_metadata(&m, "spotify").unwrap();
        assert_eq!(t.id, "/com/spotify/track/abc");
        assert_eq!(t.artist, "2Pac, Dr. Dre");
        assert_eq!(t.length_ms, 275_387);
        assert_eq!(t.art_url.as_deref(), Some("https://i.scdn.co/image/x"));
    }

    #[test]
    fn parses_spec_encodings_and_missing_fields() {
        let mut m = HashMap::new();
        m.insert(
            "mpris:trackid".into(),
            ov(ObjectPath::try_from("/org/x/1").unwrap()),
        );
        m.insert("mpris:length".into(), ov(-5i64));
        m.insert("xesam:artist".into(), ov("Solo"));
        m.insert("xesam:title".into(), ov("Song"));
        let t = parse_metadata(&m, "vlc").unwrap();
        assert_eq!(t.id, "/org/x/1");
        assert_eq!(t.artist, "Solo");
        assert_eq!(t.length_ms, 0);
        assert_eq!(t.album, "");
        assert_eq!(t.art_url, None);
    }

    #[test]
    fn empty_title_means_no_track() {
        let mut m = HashMap::new();
        m.insert("xesam:title".into(), ov(""));
        assert!(parse_metadata(&m, "spotify").is_none());
        assert!(parse_metadata(&HashMap::new(), "spotify").is_none());
    }

    #[test]
    fn missing_trackid_gets_stable_synthetic_id() {
        let mut m = HashMap::new();
        m.insert("xesam:title".into(), ov("Song"));
        m.insert("xesam:artist".into(), ov(vec!["A"]));
        let a = parse_metadata(&m, "firefox").unwrap();
        let b = parse_metadata(&m, "firefox").unwrap();
        assert_eq!(a.id, b.id);
    }

    #[test]
    fn player_selection_order() {
        let p = players(&[
            ("chromium.instance1", true),
            ("spotifast", false),
            ("spotify", false),
        ]);
        assert_eq!(choose_player(&p, None), Some("spotify"));
        assert_eq!(choose_player(&p, Some("spotifast")), Some("spotifast"));
        assert_eq!(
            choose_player(&p, Some("chromium")),
            Some("chromium.instance1")
        );
        // Unknown preference falls back to Spotify.
        assert_eq!(choose_player(&p, Some("vlc")), Some("spotify"));

        let p = players(&[("vlc", false), ("firefox.instance9", true)]);
        assert_eq!(choose_player(&p, None), Some("firefox.instance9"));
        let p = players(&[("vlc", false), ("mpv", false)]);
        assert_eq!(choose_player(&p, None), Some("vlc"));
        assert_eq!(choose_player(&[], None), None);
    }

    #[test]
    fn short_names() {
        assert_eq!(short_name("org.mpris.MediaPlayer2.spotify"), "spotify");
        assert_eq!(short_name("spotify"), "spotify");
    }
}
