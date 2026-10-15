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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
            language: Language::Tr,
            backdrop_style: BackdropStyle::Akiskan,
            edge_glow: false,
            screensaver_enabled: true,
            screensaver_minutes: 3,
            screensaver_keep_awake: false,
        }
    }
}

impl Settings {
    /// Clamps values a hand-edited file might have pushed out of range.
    pub fn sanitized(mut self) -> Self {
        self.screensaver_minutes = self.screensaver_minutes.clamp(1, 120);
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

pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    pub fn load(paths: &Paths) -> Self {
        let path = paths.config.join("settings.json");
        let current = match paths::read_json::<Settings>(&path) {
            Ok(s) => s.sanitized(),
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
    fn unknown_enum_value_is_rejected_not_panicking() {
        assert!(serde_json::from_value::<Settings>(json!({"lyrics_style": "disco"})).is_err());
    }
}
