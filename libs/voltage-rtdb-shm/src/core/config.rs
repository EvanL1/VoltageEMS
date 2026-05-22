//! Pure-infra constants, path resolution, and time helpers.
//!
//! These items have no dependency on `voltage-model` (`PointType`) or any
//! other business concept — they speak only in terms of files, paths, and
//! wall-clock time. They must stay in `core::` so they can be promoted to
//! the future `voltage-shm-slots` crate untouched.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Magic number for validation: "VOLTAGE_" in ASCII.
pub const SHARED_MAGIC: u64 = 0x564F4C544147455F;

/// Default snapshot interval in seconds (5 minutes).
pub const DEFAULT_SNAPSHOT_INTERVAL_SECS: u64 = 300;

/// Default shared memory file path (Docker tmpfs mount point).
///
/// Kept for backward compatibility — prefer [`default_shm_path`] for
/// intelligent path selection.
pub const DEFAULT_SHM_PATH: &str = "/shm/rtdb/voltage-rtdb.shm";

/// Get the default shared memory path with intelligent fallback.
///
/// Priority:
/// 1. `VOLTAGE_SHM_PATH` environment variable (if set)
/// 2. Docker tmpfs mount point `/shm/rtdb/voltage-rtdb.shm` (if exists)
/// 3. Linux RAM-backed tmpfs `/dev/shm/voltage-rtdb.shm`
/// 4. Fallback to `/tmp/voltage-rtdb.shm` (macOS or other platforms)
pub fn default_shm_path() -> PathBuf {
    if let Ok(path) = std::env::var("VOLTAGE_SHM_PATH") {
        return PathBuf::from(path);
    }

    let docker_path = Path::new("/shm/rtdb");
    if docker_path.exists() {
        return PathBuf::from(DEFAULT_SHM_PATH);
    }

    #[cfg(target_os = "linux")]
    {
        let dev_shm = Path::new("/dev/shm");
        if dev_shm.exists() {
            return dev_shm.join("voltage-rtdb.shm");
        }
    }

    PathBuf::from("/tmp/voltage-rtdb.shm")
}

/// Check whether the parent directory of a SHM file path exists.
///
/// This is the only generic "is the SHM mountpoint available" check that
/// makes sense without knowing anything about the business config. The
/// business-config-aware version lives in `shared_config::is_shm_available`.
pub fn parent_dir_exists(path: &Path) -> bool {
    path.parent().map(|p| p.exists()).unwrap_or(false)
}

/// Get current timestamp in milliseconds since UNIX epoch.
#[inline]
pub fn timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Compute the path for a per-generation SHM file given the canonical
/// "current" path and a generation number.
///
/// Convention: a base path of `/dev/shm/voltage-rtdb.shm` with generation
/// 42 yields `/dev/shm/voltage-rtdb-42.shm`. The base path itself
/// continues to refer to the "current" generation — atomic swaps land a
/// freshly-created per-generation file at the base path via
/// `rename(2)`, preserving the open file inode for any reader that
/// already mmap'd the previous generation.
///
/// If the base path has no extension, the generation suffix is appended
/// directly (no `.shm`). Multiple-extension paths (`foo.shm.bak`) get
/// the generation injected before the final extension.
pub fn generation_file_path(base: &Path, generation: u64) -> PathBuf {
    let parent = base.parent();
    let file_name = base.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let new_name = match file_name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem}-{generation}.{ext}"),
        _ => format!("{file_name}-{generation}"),
    };
    match parent {
        Some(p) if !p.as_os_str().is_empty() => p.join(new_name),
        _ => PathBuf::from(new_name),
    }
}

/// Atomically rename a freshly-created per-generation SHM file into the
/// canonical "current" path.
///
/// POSIX `rename(2)` is atomic on the same filesystem: any process opening
/// `canonical_path` after the call sees either the old or the new file,
/// never a partial state. Existing readers that already mmap'd the
/// previous file at `canonical_path` continue to operate on the previous
/// inode (kept alive by the mmap reference), so they never see torn data
/// either. New readers that re-open `canonical_path` pick up the new
/// generation.
///
/// The two paths must be on the same filesystem; otherwise `rename(2)`
/// degrades to copy+unlink and loses atomicity. Both per-generation files
/// and the canonical "current" file should live in the same SHM mount
/// (typically `/dev/shm` or `/shm/rtdb`).
pub fn commit_generation_swap(staging_path: &Path, canonical_path: &Path) -> Result<()> {
    std::fs::rename(staging_path, canonical_path)
        .with_context(|| format!("rename {staging_path:?} → {canonical_path:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_path_appends_before_extension() {
        let p = generation_file_path(Path::new("/dev/shm/voltage-rtdb.shm"), 42);
        assert_eq!(p, PathBuf::from("/dev/shm/voltage-rtdb-42.shm"));
    }

    #[test]
    fn generation_path_handles_no_extension() {
        let p = generation_file_path(Path::new("/tmp/voltage"), 7);
        assert_eq!(p, PathBuf::from("/tmp/voltage-7"));
    }

    #[test]
    fn generation_path_handles_multi_extension() {
        // foo.shm.bak → foo.shm-N.bak (suffix injected before final ext)
        let p = generation_file_path(Path::new("/tmp/voltage.shm.bak"), 3);
        assert_eq!(p, PathBuf::from("/tmp/voltage.shm-3.bak"));
    }

    #[test]
    fn generation_path_handles_relative_path() {
        let p = generation_file_path(Path::new("voltage.shm"), 1);
        assert_eq!(p, PathBuf::from("voltage-1.shm"));
    }

    #[test]
    fn generation_path_handles_hidden_file() {
        // Leading-dot file: ".voltage" has empty stem before the dot — we
        // treat the whole name as the stem and append the generation.
        let p = generation_file_path(Path::new("/tmp/.voltage"), 5);
        assert_eq!(p, PathBuf::from("/tmp/.voltage-5"));
    }

    #[test]
    fn commit_swap_renames_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let canonical = dir.path().join("voltage-rtdb.shm");
        let staging = generation_file_path(&canonical, 1);

        // Pre-populate canonical to simulate a "current" file already in
        // place — the swap should overwrite it.
        std::fs::write(&canonical, b"OLD").unwrap();
        std::fs::write(&staging, b"NEW").unwrap();

        commit_generation_swap(&staging, &canonical).unwrap();

        assert_eq!(std::fs::read(&canonical).unwrap(), b"NEW");
        assert!(
            !staging.exists(),
            "staging file should be gone after rename"
        );
    }

    #[test]
    fn commit_swap_works_when_canonical_missing() {
        let dir = tempfile::tempdir().unwrap();
        let canonical = dir.path().join("voltage-rtdb.shm");
        let staging = generation_file_path(&canonical, 1);

        std::fs::write(&staging, b"NEW").unwrap();
        // canonical does NOT exist — first-create case.
        commit_generation_swap(&staging, &canonical).unwrap();

        assert_eq!(std::fs::read(&canonical).unwrap(), b"NEW");
    }
}
