//! XDG locations and small file helpers shared by the caches and settings.

use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Paths {
    /// `~/.config/sahne`
    pub config: PathBuf,
    /// `~/.cache/sahne`
    pub cache: PathBuf,
}

impl Paths {
    pub fn from_xdg() -> Option<Self> {
        let dirs = directories::ProjectDirs::from("", "", "sahne")?;
        Some(Self {
            config: dirs.config_dir().to_path_buf(),
            cache: dirs.cache_dir().to_path_buf(),
        })
    }

    /// Fallback when no home directory can be resolved: keep everything in /tmp.
    pub fn temporary() -> Self {
        let base = std::env::temp_dir().join("sahne");
        Self {
            config: base.join("config"),
            cache: base.join("cache"),
        }
    }

    pub fn lyrics_cache(&self) -> PathBuf {
        self.cache.join("lyrics")
    }

    pub fn art_cache(&self) -> PathBuf {
        self.cache.join("art")
    }
}

/// Stable 64-bit FNV-1a hash, used for cache file names. (std's hasher is not
/// guaranteed stable across Rust releases.)
pub fn stable_hash(data: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in data.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> io::Result<T> {
    let bytes = std::fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Writes via a temporary file and rename so readers never see half a file.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    write_atomic(path, &bytes)
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable() {
        assert_eq!(stable_hash(""), "cbf29ce484222325");
        assert_eq!(stable_hash("a"), "af63dc4c8601ec8c");
        assert_ne!(stable_hash("a|b|1"), stable_hash("a|b|2"));
    }

    #[test]
    fn json_roundtrip() {
        let dir = std::env::temp_dir().join(format!("sahne-test-{}", std::process::id()));
        let path = dir.join("x/y.json");
        write_json(&path, &vec![1, 2, 3]).unwrap();
        let back: Vec<i32> = read_json(&path).unwrap();
        assert_eq!(back, vec![1, 2, 3]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
