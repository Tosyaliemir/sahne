//! User settings stored in `~/.config/sahne/settings.json`.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use crate::paths::{self, Paths};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LyricsStyle {
    Klasik,
    Kinetik,
    Sade,
    /// Classic layout; words rise into place as they are sung.
    Dalga,
    /// Classic, switching to kinetic for fast passages.
    Harman,
    /// Recent lines stacked into a wall that drifts in 3D.
    Duvar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackdropStyle {
    /// WebGL: the artwork itself, slowly flowing and swirling.
    Akiskan,
    /// Pre-blurred artwork with drifting color fields.
    Bulanik,
    /// The sharp, original cover, slowly zooming, darkened for readability.
    Kapak,
    /// A calm gradient made from the cover's colors.
    Renk,
}

/// What the cover in the split view (and the no-lyrics screen) wears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoverEffect {
    /// Cover set into a brushed metal plate that mirrors it.
    Metal,
    /// Crackling light running around the cover's edge.
    Elektrik,
    /// No effect; also what a removed effect (e.g. an old "holo") reads as.
    /// `serde(other)` must be on the last variant.
    #[serde(other)]
    Yok,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontChoice {
    /// Bundled Manrope.
    Embedded,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Tr,
    En,
}

/// Deserialized leniently (see [`Settings::from_value`]): an invalid or
/// unknown value only resets that one field.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Settings {
    /// MPRIS short name to follow instead of Spotify, e.g. `vlc`.
    pub preferred_player: Option<String>,
    pub lyrics_style: LyricsStyle,
    pub font: FontChoice,
    /// Multiplier for lyric text size.
    pub font_scale: f64,
    /// Extra CSS blur on the (already pre-blurred) backdrop, in px. Costly
    /// on animated layers, so off by default.
    pub backdrop_blur: u32,
    /// Opacity of the darkening layer, 0..1.
    pub backdrop_dim: f64,
    /// Added to every track's own offset; positive shows lyrics later.
    pub global_offset_ms: i64,
    /// Last mini-window position (physical pixels), if the user moved it.
    pub mini_position: Option<(i32, i32)>,
    pub start_fullscreen: bool,
    /// Seconds without mouse movement before the bottom bar hides; 0 = never.
    pub controls_hide_secs: u32,
    pub language: Language,
    pub backdrop_style: BackdropStyle,
    /// Light flowing along the screen edges in the cover's colors.
    pub edge_glow: bool,
    /// Go fullscreen when the computer has been idle this long while music plays.
    pub screensaver_enabled: bool,
    pub screensaver_minutes: u32,
    /// While the screensaver shows, keep the screen from blanking/locking.
    /// Off by default: it delays the system lock screen.
    pub screensaver_keep_awake: bool,
    pub cover_effect: CoverEffect,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            preferred_player: None,
            lyrics_style: LyricsStyle::Klasik,
            font: FontChoice::Embedded,
            font_scale: 1.0,
            backdrop_blur: 0,
            backdrop_dim: 0.35,
            global_offset_ms: 0,
            mini_position: None,
            start_fullscreen: false,
            controls_hide_secs: 2,
            language: Language::Tr,
            backdrop_style: BackdropStyle::Akiskan,
            edge_glow: false,
            screensaver_enabled: true,
            screensaver_minutes: 3,
            screensaver_keep_awake: false,
            cover_effect: CoverEffect::Yok,
        }
    }
}

/// Applies `$field` from `map` onto `$s` when present and valid; otherwise
/// records it in `$bad` and keeps the default.
macro_rules! lenient_fields {
    ($s:ident, $map:ident, $bad:ident; $($field:ident),+ $(,)?) => {
        $(
            if let Some(v) = $map.get(stringify!($field)) {
                match serde_json::from_value(v.clone()) {
                    Ok(x) => $s.$field = x,
                    Err(_) => $bad.push(stringify!($field)),
                }
            }
        )+
    };
}

impl Settings {
    /// Builds settings from JSON, field by field: a typo, a value from a
    /// newer version or a downgrade resets only the affected fields, never
    /// the whole file. Returns the names of fields that were discarded.
    pub fn from_value(value: &serde_json::Value) -> (Self, Vec<&'static str>) {
        let mut s = Self::default();
        let mut bad = Vec::new();
        let Some(map) = value.as_object() else {
            return (s, vec!["<root>"]);
        };
        lenient_fields!(s, map, bad; preferred_player, lyrics_style, font, font_scale, backdrop_blur, backdrop_dim, global_offset_ms, mini_position, start_fullscreen, controls_hide_secs, language, backdrop_style, edge_glow, screensaver_enabled, screensaver_minutes, screensaver_keep_awake, cover_effect);
        (s, bad)
    }

    /// Clamps values a hand-edited file might have pushed out of range.
    pub fn sanitized(mut self) -> Self {
        self.screensaver_minutes = self.screensaver_minutes.clamp(1, 120);
        self.controls_hide_secs = self.controls_hide_secs.min(60);
        self.font_scale = if self.font_scale.is_finite() {
            self.font_scale.clamp(0.5, 2.0)
        } else {
            1.0
        };
        self.backdrop_blur = self.backdrop_blur.min(120);
        self.backdrop_dim = if self.backdrop_dim.is_finite() {
            self.backdrop_dim.clamp(0.0, 0.9)
        } else {
            0.35
        };
        self.global_offset_ms = self.global_offset_ms.clamp(-10_000, 10_000);
        self.preferred_player = self.preferred_player.filter(|p| !p.trim().is_empty());
        self
    }
}

impl<'de> Deserialize<'de> for Settings {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        Ok(Self::from_value(&value).0)
    }
}

pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    pub fn load(paths: &Paths) -> Self {
        let path = paths.config.join("settings.json");
        let current = match std::fs::read(&path) {
            Ok(bytes) => {
                let (settings, bad) = match serde_json::from_slice::<serde_json::Value>(&bytes) {
                    Ok(value) => Settings::from_value(&value),
                    Err(_) => (Settings::default(), vec!["<file>"]),
                };
                if !bad.is_empty() {
                    // Keep the user's file before anything overwrites it.
                    let backup = path.with_extension("json.bak");
                    tracing::warn!(
                        "settings: ignoring invalid {} (original kept as {})",
                        bad.join(", "),
                        backup.display()
                    );
                    if let Err(e) = std::fs::write(&backup, &bytes) {
                        tracing::warn!("could not back up settings: {e}");
                    }
                }
                settings.sanitized()
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Settings::default(),
            Err(e) => {
                tracing::warn!("ignoring unreadable settings ({e}); using defaults");
                Settings::default()
            }
        };
        Self {
            path,
            current: Mutex::new(current),
        }
    }

    pub fn get(&self) -> Settings {
        self.lock().clone()
    }

    /// Stores and persists new settings; returns the sanitized version.
    pub fn set(&self, settings: Settings) -> Settings {
        let settings = settings.sanitized();
        *self.lock() = settings.clone();
        if let Err(e) = paths::write_json(&self.path, &settings) {
            tracing::warn!("could not save settings: {e}");
        }
        settings
    }

    /// Merges `patch` (a JSON object with some settings fields) onto the
    /// current settings and persists the result. Fields not in the patch keep
    /// their current value, so concurrent edits from the other window or from
    /// the backend (e.g. the mini window's position) are not lost.
    pub fn apply_patch(&self, patch: &serde_json::Map<String, serde_json::Value>) -> Settings {
        let merged = {
            let cur = self.lock();
            let mut value = serde_json::to_value(&*cur).unwrap_or_default();
            if let Some(obj) = value.as_object_mut() {
                for (k, v) in patch {
                    obj.insert(k.clone(), v.clone());
                }
            }
            let (merged, bad) = Settings::from_value(&value);
            if !bad.is_empty() {
                tracing::warn!("settings patch: ignored invalid {}", bad.join(", "));
            }
            merged
        };
        self.set(merged)
    }

    pub fn update(&self, f: impl FnOnce(&mut Settings)) -> Settings {
        let mut s = self.get();
        f(&mut s);
        self.set(s)
    }

    fn lock(&self) -> MutexGuard<'_, Settings> {
        self.current.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: Settings =
            serde_json::from_value(json!({"lyrics_style": "kinetik", "language": "en"})).unwrap();
        assert_eq!(s.lyrics_style, LyricsStyle::Kinetik);
        assert_eq!(s.language, Language::En);
        assert_eq!(s.font_scale, 1.0);
    }

    #[test]
    fn sanitizes_out_of_range_values() {
        let s = Settings {
            font_scale: 9.0,
            backdrop_dim: -1.0,
            backdrop_blur: 999,
            global_offset_ms: 1_000_000,
            preferred_player: Some("  ".into()),
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.font_scale, 2.0);
        assert_eq!(s.backdrop_dim, 0.0);
        assert_eq!(s.backdrop_blur, 120);
        assert_eq!(s.global_offset_ms, 10_000);
        assert_eq!(s.preferred_player, None);
    }

    #[test]
    fn persists_and_reloads() {
        let base = std::env::temp_dir().join(format!("sahne-settings-{}", std::process::id()));
        let paths = Paths {
            config: base.join("config"),
            cache: base.join("cache"),
        };
        let store = SettingsStore::load(&paths);
        assert_eq!(store.get(), Settings::default());
        store.update(|s| s.lyrics_style = LyricsStyle::Sade);
        assert_eq!(
            SettingsStore::load(&paths).get().lyrics_style,
            LyricsStyle::Sade
        );
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn removed_cover_effect_falls_back_to_none() {
        let s: Settings =
            serde_json::from_value(json!({"cover_effect": "holo", "language": "en"})).unwrap();
        assert_eq!(s.cover_effect, CoverEffect::Yok);
        assert_eq!(s.language, Language::En);
    }

    // C2: one bad field must not discard the others.
    #[test]
    fn invalid_field_keeps_the_rest() {
        let (s, bad) = Settings::from_value(&json!({"lyrics_style": "disco", "font_scale": 1.4}));
        assert_eq!(s.lyrics_style, LyricsStyle::Klasik);
        assert_eq!(s.font_scale, 1.4);
        assert_eq!(bad, vec!["lyrics_style"]);
        let s: Settings =
            serde_json::from_value(json!({"font_scale": "big", "language": "en"})).unwrap();
        assert_eq!(s.language, Language::En);
        assert_eq!(s.font_scale, 1.0);
    }

    #[test]
    fn unreadable_file_is_backed_up_before_overwrite() {
        let base = std::env::temp_dir().join(format!("sahne-settings-bak-{}", std::process::id()));
        let paths = Paths {
            config: base.join("config"),
            cache: base.join("cache"),
        };
        std::fs::create_dir_all(&paths.config).unwrap();
        let file = paths.config.join("settings.json");
        std::fs::write(&file, br#"{"lyrics_style": "disco", "font_scale": 1.4}"#).unwrap();
        let store = SettingsStore::load(&paths);
        assert_eq!(store.get().font_scale, 1.4);
        let backup = paths.config.join("settings.json.bak");
        assert!(std::fs::read_to_string(&backup).unwrap().contains("disco"));
        std::fs::write(&file, b"{not json").unwrap();
        let store = SettingsStore::load(&paths);
        assert_eq!(store.get(), Settings::default());
        assert_eq!(std::fs::read(&backup).unwrap(), b"{not json");
        std::fs::remove_dir_all(base).unwrap();
    }

    // C3: a patch from one window must not undo changes made elsewhere.
    #[test]
    fn patch_merges_onto_current_state() {
        let base =
            std::env::temp_dir().join(format!("sahne-settings-patch-{}", std::process::id()));
        let paths = Paths {
            config: base.join("config"),
            cache: base.join("cache"),
        };
        let store = SettingsStore::load(&paths);
        store.update(|s| s.mini_position = Some((40, 60)));
        let patch = json!({"lyrics_style": "dalga", "edge_glow": true});
        let saved = store.apply_patch(patch.as_object().unwrap());
        assert_eq!(saved.mini_position, Some((40, 60)));
        assert_eq!(saved.lyrics_style, LyricsStyle::Dalga);
        assert!(saved.edge_glow);
        let bad = json!({"font_scale": "huge"});
        assert_eq!(store.apply_patch(bad.as_object().unwrap()).font_scale, 1.0);
        std::fs::remove_dir_all(base).unwrap();
    }
}
