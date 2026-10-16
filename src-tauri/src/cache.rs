//! Keeps the on-disk caches bounded: superseded files are deleted and each
//! directory is trimmed to a size cap, least recently used first. "Used"
//! is the file's modification time, which the services refresh on every
//! cache hit ([`touch`]), so it works on `noatime` mounts too.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::paths::Paths;

pub const ART_CAP_BYTES: u64 = 200 * 1024 * 1024;
pub const LYRICS_CAP_BYTES: u64 = 20 * 1024 * 1024;

/// Marks a cache file as just used, for LRU trimming. Best effort.
pub fn touch(path: &Path) {
    if let Ok(f) = std::fs::File::options().append(true).open(path) {
        let _ = f.set_modified(SystemTime::now());
    }
}

struct Entry {
    path: PathBuf,
    len: u64,
    used: SystemTime,
}

fn entries(dir: &Path) -> Vec<Entry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    read.filter_map(Result::ok)
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            meta.is_file().then(|| Entry {
                path: e.path(),
                len: meta.len(),
                used: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            })
        })
        .collect()
}

/// Deletes files for which `obsolete` is true, then the least recently used
/// ones until the directory fits in `cap` bytes. Returns bytes freed.
pub fn prune(dir: &Path, cap: u64, obsolete: impl Fn(&Path) -> bool) -> u64 {
    let mut freed = 0;
    let mut keep = Vec::new();
    for e in entries(dir) {
        if obsolete(&e.path) {
            if std::fs::remove_file(&e.path).is_ok() {
                freed += e.len;
            }
        } else {
            keep.push(e);
        }
    }
    let mut total: u64 = keep.iter().map(|e| e.len).sum();
    keep.sort_by_key(|e| e.used);
    for e in keep {
        if total <= cap {
            break;
        }
        if std::fs::remove_file(&e.path).is_ok() {
            total -= e.len;
            freed += e.len;
        }
    }
    freed
}

/// Total size of the regular files in `dir`.
pub fn dir_size(dir: &Path) -> u64 {
    entries(dir).iter().map(|e| e.len).sum()
}

/// Art cache files from older renderer versions (`<key>-bg.png` predates
/// the current `-bg2`) and interrupted writes.
fn obsolete_art(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.ends_with("-bg.png") || name.starts_with(".tmp")
}

/// Startup maintenance for every cache. Blocking; run it off the main thread.
pub fn prune_all(paths: &Paths, extra_dirs: &[(PathBuf, u64)]) {
    let mut freed = prune(&paths.art_cache(), ART_CAP_BYTES, obsolete_art);
    freed += prune(&paths.lyrics_cache(), LYRICS_CAP_BYTES, |p| {
        crate::lyrics::is_obsolete_cache_file(p)
    });
    for (dir, cap) in extra_dirs {
        freed += prune(dir, *cap, |_| false);
    }
    if freed > 0 {
        tracing::info!("cache: freed {} KiB", freed / 1024);
    }
}

/// Bytes used by all caches.
pub fn size_all(paths: &Paths, extra_dirs: &[(PathBuf, u64)]) -> u64 {
    dir_size(&paths.art_cache())
        + dir_size(&paths.lyrics_cache())
        + extra_dirs.iter().map(|(d, _)| dir_size(d)).sum::<u64>()
}

/// Empties all caches. Settings and lyric offsets live elsewhere and stay.
pub fn clear_all(paths: &Paths, extra_dirs: &[(PathBuf, u64)]) -> u64 {
    let mut freed = prune(&paths.art_cache(), 0, |_| false);
    freed += prune(&paths.lyrics_cache(), 0, |_| false);
    for (dir, _) in extra_dirs {
        freed += prune(dir, 0, |_| false);
    }
    freed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn file(dir: &Path, name: &str, len: usize, age_s: u64) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, vec![0u8; len]).unwrap();
        let f = std::fs::File::options().append(true).open(&p).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(age_s))
            .unwrap();
        p
    }

    #[test]
    fn prunes_obsolete_then_least_recently_used() {
        let dir = std::env::temp_dir().join(format!("sahne-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let old = file(&dir, "a.img", 400, 300);
        let mid = file(&dir, "b.img", 400, 200);
        let new = file(&dir, "c.img", 400, 100);
        let legacy = file(&dir, "a-bg.png", 50, 10);
        let current = file(&dir, "a-bg2.png", 50, 400);
        // Touching makes the oldest file the most recently used one.
        touch(&current);

        let freed = prune(&dir, 500, obsolete_art);
        assert!(!legacy.exists(), "superseded version is removed");
        assert!(!old.exists() && !mid.exists(), "oldest go first");
        assert!(new.exists() && current.exists());
        assert_eq!(freed, 50 + 400 + 400);
        assert!(dir_size(&dir) <= 500);

        assert_eq!(prune(&dir, 0, |_| false), 450);
        assert_eq!(dir_size(&dir), 0);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
