//! Pure-infra constants, path resolution, and time helpers.
//!
//! These items have no dependency on `voltage-model` (`PointType`) or any
//! other business concept — they speak only in terms of files, paths, and
//! wall-clock time. They must stay in `core::` so they can be promoted to
//! the future `voltage-shm-slots` crate untouched.

use std::path::{Path, PathBuf};

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
