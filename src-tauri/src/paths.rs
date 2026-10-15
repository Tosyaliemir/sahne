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
    /// Fallback when there is no home directory. Never a predictable shared
    /// path such as `/tmp/sahne` (open to symlink and pre-creation attacks on
    /// multi-user systems): the per-user runtime dir if there is one, else a
    /// fresh private (0700) temporary directory.
    pub fn temporary() -> Self {
        let base = private_runtime_dir().unwrap_or_else(|| {
            tempfile::Builder::new()
                .prefix("sahne-")
                .tempdir()
                .map(tempfile::TempDir::keep)
                .unwrap_or_else(|e| {
                    tracing::error!("no private temporary directory ({e}); settings won't persist");
                    std::env::temp_dir().join(format!("sahne-{}", std::process::id()))
                })
        });
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
/// `$XDG_RUNTIME_DIR/sahne`, created 0700. The runtime dir itself is
/// guaranteed to be owned by the user and 0700.
#[cfg(unix)]
fn private_runtime_dir() -> Option<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let base = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?);
    if !base.is_absolute() {
        return None;
    }
    let dir = base.join("sahne");
    match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(_) => return None,
    }
    let meta = std::fs::symlink_metadata(&dir).ok()?;
    (meta.is_dir() && meta.permissions().mode() & 0o077 == 0).then_some(dir)
}

#[cfg(not(unix))]
fn private_runtime_dir() -> Option<PathBuf> {
    None
}

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

/// Writes through a uniquely named temporary file in the same directory,
/// fsyncs it and renames it over `path`: readers see the old or the new
/// content, never a mix, and concurrent writers don't clobber each other's
/// temporary file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
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

    #[test]
    fn concurrent_writers_never_tear_or_fail() {
        let dir = std::env::temp_dir().join(format!("sahne-atomic-{}", std::process::id()));
        let path = dir.join("shared.json");
        let writers: Vec<_> = (0..8u8)
            .map(|n| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let body = vec![b'a' + n; 64 * 1024];
                    for _ in 0..20 {
                        write_atomic(&path, &body).unwrap();
                    }
                })
            })
            .collect();
        for w in writers {
            w.join().unwrap();
        }
        let content = std::fs::read(&path).unwrap();
        assert_eq!(content.len(), 64 * 1024);
        assert!(content.iter().all(|&b| b == content[0]), "torn write");
        // No temporary files left behind.
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn fallback_dir_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let paths = Paths::temporary();
        let base = paths.config.parent().unwrap();
        let mode = std::fs::metadata(base).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "fallback dir is {:o}", mode & 0o777);
        assert_ne!(base, std::env::temp_dir().join("sahne"));
    }
}
