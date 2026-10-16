//! Dev tooling (feature `dev-fixture`): builds preview fixtures from the
//! local caches with the app's own lyrics and palette code, so screenshots
//! match the app exactly. Not part of any shipped build.

use std::path::Path;

use crate::events::TrackInfo;
use crate::paths::{self, Paths};

/// What the preview harness needs to show `track` as the app would.
pub struct Fixture {
    /// `LyricsLoaded` as sent to the frontend.
    pub lyrics: serde_json::Value,
    /// `Palette` as sent to the frontend.
    pub palette: serde_json::Value,
    /// The cached artwork bytes.
    pub cover: Vec<u8>,
}

/// Reads `track` from the lyrics and artwork caches. The blurred backdrop is
/// rendered to `background`.
pub fn build(track: &TrackInfo, background: &Path) -> Result<Fixture, String> {
    let dirs = Paths::from_xdg().ok_or("no cache directory")?;
    let lyrics_file = dirs
        .lyrics_cache()
        .join(crate::lyrics::cache_file_name(track));
    let bytes = std::fs::read(&lyrics_file).map_err(|e| {
        format!(
            "{}: {e} (play the track in Sahne first)",
            lyrics_file.display()
        )
    })?;
    let lyrics = crate::lyrics::loaded_from_cache(track, &bytes)
        .ok_or_else(|| format!("{}: not a lyrics cache entry", lyrics_file.display()))?;

    let url = track
        .art_url
        .as_deref()
        .ok_or("the track has no artwork URL")?;
    let art_file = dirs
        .art_cache()
        .join(format!("{}.img", paths::stable_hash(url)));
    let cover = std::fs::read(&art_file).map_err(|e| {
        format!(
            "{}: {e} (play the track in Sahne first)",
            art_file.display()
        )
    })?;
    let palette = crate::artwork::analyse(&cover, Some(background)).map_err(|e| e.to_string())?;

    Ok(Fixture {
        lyrics: serde_json::to_value(lyrics).map_err(|e| e.to_string())?,
        palette: serde_json::to_value(palette).map_err(|e| e.to_string())?,
        cover,
    })
}

/// A `TrackInfo` from plain fields.
pub fn track(
    id: &str,
    title: &str,
    artist: &str,
    album: &str,
    length_ms: u64,
    art_url: &str,
) -> TrackInfo {
    TrackInfo {
        id: id.into(),
        title: title.into(),
        artist: artist.into(),
        album: album.into(),
        length_ms,
        art_url: Some(art_url.into()),
        player: "spotify".into(),
    }
}
