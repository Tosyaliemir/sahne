//! MPRIS D-Bus proxies and pure helpers for metadata parsing.

use std::collections::HashMap;

use zbus::{OwnedVariant, Variant};

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
    fn set_position(&self, track_id: &zbus::ObjectPath<'_>, position_us: i64) -> zbus::Result<()>;

    #[zbus(signal)]
    fn seeked(&self, position_us: i64) -> zbus::Result<()>;

    #[zbus(property)]
    fn metadata(&self) -> zbus::Result<HashMap<String, OwnedVariant>>;
    #[zbus(property)]
    fn playback_status(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn rate(&self) -> zbus::Result<f64>;
    /// 0.0..=1.0; not every player implements it.
    #[zbus(property)]
    fn volume(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn set_volume(&self, value: f64) -> zbus::Result<()>;
    // Spotify does not emit PropertiesChanged for Position, so never cache it.
    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> zbus::Result<i64>;
}

/// Strips the MPRIS prefix: `org.mpris.MediaPlayer2.spotify` -> `spotify`.
pub fn short_name(bus_name: &str) -> &str {
    bus_name.strip_prefix(MPRIS_PREFIX).unwrap_or(bus_name)
}

/// Builds a [`TrackInfo`] from an MPRIS metadata map. Returns `None` when
/// there is no title (players report empty metadata while idle).
pub fn parse_metadata(meta: &HashMap<String, OwnedVariant>, player: &str) -> Option<TrackInfo> {
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

fn as_string(v: &Variant<'_>) -> Option<String> {
    match v {
        Variant::Str(s) => Some(s.to_string()),
        Variant::ObjectPath(p) => Some(p.to_string()),
        Variant::Variant(inner) => as_string(inner),
        _ => None,
    }
}

fn as_string_list(v: &Variant<'_>) -> Vec<String> {
    match v {
        Variant::Array(arr) => arr.iter().filter_map(as_string).collect(),
        Variant::Variant(inner) => as_string_list(inner),
        other => as_string(other).into_iter().collect(),
    }
}

fn as_u64(v: &Variant<'_>) -> Option<u64> {
    match v {
        Variant::U64(n) => Some(*n),
        Variant::I64(n) => u64::try_from(*n).ok(),
        Variant::U32(n) => Some(u64::from(*n)),
        Variant::I32(n) => u64::try_from(*n).ok(),
        Variant::F64(n) if *n >= 0.0 => Some(*n as u64),
        Variant::Variant(inner) => as_u64(inner),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::ObjectPath;

    fn ov<'a>(v: impl Into<Variant<'a>>) -> OwnedVariant {
        v.into().try_to_owned().unwrap()
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
    fn short_names() {
        assert_eq!(short_name("org.mpris.MediaPlayer2.spotify"), "spotify");
        assert_eq!(short_name("spotify"), "spotify");
    }
}
