//! PointWatch subscription bitmap — mmap-backed `[AtomicU64; N]`
//!
//! Tracks which SHM slots modsrv wants to watch. comsrv (the writer) reads
//! this bitmap in the hot path via a single Relaxed atomic load + bit test.
//! modsrv (the watcher) sets bits when it loads or reloads rules.
//!
//! # File ownership
//!
//! - **comsrv** creates the file (zero-fills) at startup.
//! - **modsrv** opens the existing file and writes subscription bits.
//!
//! This mirrors the main SHM ownership model (comsrv creates, modsrv opens).
//!
//! # Sizing
//!
//! 1563 words × 64 bits = 100,032 slots, covering
//! `DEFAULT_MAX_SLOTS` (100,000). File size: 12,504 bytes.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use memmap2::MmapMut;
use tracing::debug;

/// Number of AtomicU64 words in the subscription bitmap.
///
/// Covers DEFAULT_MAX_SLOTS (100_000): ceil(100_000 / 64) = 1563.
pub const WATCH_WORDS_COUNT: usize = 1563;

/// Bitmap file size in bytes (1563 × 8).
pub const WATCH_BITMAP_SIZE: usize = WATCH_WORDS_COUNT * 8; // 12,504

/// Default bitmap file name, placed next to the main SHM file.
pub const WATCH_BITMAP_SUFFIX: &str = "-point-watch-subs";

/// Derive the subscription bitmap path from the main SHM path.
///
/// `/shm/rtdb/voltage-rtdb.shm` →
/// `/shm/rtdb/voltage-rtdb-point-watch-subs.shm`
pub fn bitmap_path_from_shm(shm_path: &Path) -> PathBuf {
    let parent = shm_path.parent().unwrap_or(Path::new(""));
    let file_name = shm_path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let new_name = match file_name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => {
            format!("{stem}{WATCH_BITMAP_SUFFIX}.{ext}")
        },
        _ => format!("{file_name}{WATCH_BITMAP_SUFFIX}"),
    };
    if parent.as_os_str().is_empty() {
        PathBuf::from(new_name)
    } else {
        parent.join(new_name)
    }
}

/// mmap-backed subscription bitmap for PointWatch.
///
/// Both the comsrv (read-only) and modsrv (read-write) side use this type;
/// the difference is in *who* calls which methods:
///
/// - comsrv calls [`is_watched`] exclusively (hot path).
/// - modsrv calls [`set_watched`] / [`clear_all`] during rule load.
///
/// # Safety
///
/// The pointer cast to `*const AtomicU64` is safe because:
/// 1. `MmapMut` is page-aligned (≥ 4096 bytes), which is ≥ 8-byte alignment
///    required by `AtomicU64`.
/// 2. The file is `WATCH_BITMAP_SIZE` bytes, matching exactly
///    `WATCH_WORDS_COUNT * size_of::<AtomicU64>()`.
/// 3. Concurrent atomic operations across processes are well-defined on x86-64
///    and AArch64 for naturally-aligned loads/stores.
pub struct SubscriptionBitmap {
    mmap: MmapMut,
    /// Path kept for diagnostics.
    path: PathBuf,
    /// Set to true when opened in read-only mode (comsrv).
    read_only: bool,
}

impl SubscriptionBitmap {
    /// Create a new zero-filled bitmap file (comsrv side).
    ///
    /// Overwrites any existing file at `path`.
    pub fn create(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create parent dir for bitmap {:?}", path))?;
        }

        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .with_context(|| format!("open/create bitmap {:?}", path))?;

        file.set_len(WATCH_BITMAP_SIZE as u64)
            .with_context(|| "set bitmap file length")?;

        // SAFETY: file just created with WATCH_BITMAP_SIZE bytes, page-aligned.
        let mmap = unsafe {
            memmap2::MmapOptions::new()
                .len(WATCH_BITMAP_SIZE)
                .map_mut(&file)
                .with_context(|| format!("mmap bitmap {:?}", path))?
        };

        debug!("SubscriptionBitmap created at {:?}", path);
        Ok(Self {
            mmap,
            path: path.to_owned(),
            read_only: false,
        })
    }

    /// Open an existing bitmap file (modsrv side).
    ///
    /// Fails if the file does not exist or has the wrong size.
    pub fn open(path: &Path) -> Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .with_context(|| format!("open bitmap {:?}", path))?;

        let file_len = file.metadata().with_context(|| "stat bitmap file")?.len();
        if file_len as usize != WATCH_BITMAP_SIZE {
            anyhow::bail!(
                "Bitmap {:?} has wrong size: {} (expected {})",
                path,
                file_len,
                WATCH_BITMAP_SIZE
            );
        }

        // SAFETY: existing file with correct size, page-aligned mmap.
        let mmap = unsafe {
            memmap2::MmapOptions::new()
                .len(WATCH_BITMAP_SIZE)
                .map_mut(&file)
                .with_context(|| format!("mmap bitmap {:?}", path))?
        };

        debug!("SubscriptionBitmap opened at {:?}", path);
        Ok(Self {
            mmap,
            path: path.to_owned(),
            read_only: false,
        })
    }

    /// Create an in-memory bitmap backed by an anonymous mmap.
    ///
    /// Used in unit tests so that no real file is required.
    pub fn new_in_memory() -> Result<Self> {
        // Anonymous mmap: just allocate zeroed memory
        let mmap = memmap2::MmapOptions::new()
            .len(WATCH_BITMAP_SIZE)
            .map_anon()
            .context("anonymous mmap for in-memory bitmap")?;

        // MmapMut from anon is writeable; wrap in a MmapMut.
        // memmap2::MmapMut::map_anon returns MmapMut directly.
        Ok(Self {
            mmap,
            path: PathBuf::from("<memory>"),
            read_only: false,
        })
    }

    /// Check whether a slot is subscribed (comsrv hot path).
    ///
    /// Returns `false` for out-of-range slots. Single Relaxed load + bit test
    /// — about 1–2 ns on modern hardware.
    #[inline]
    pub fn is_watched(&self, slot: usize) -> bool {
        let word_idx = slot / 64;
        let bit_idx = slot % 64;
        if word_idx >= WATCH_WORDS_COUNT {
            return false;
        }
        // SAFETY: mmap is WATCH_BITMAP_SIZE bytes, 8-byte aligned (page-aligned).
        let words = unsafe {
            std::slice::from_raw_parts(self.mmap.as_ptr() as *const AtomicU64, WATCH_WORDS_COUNT)
        };
        words[word_idx].load(Ordering::Relaxed) & (1u64 << bit_idx) != 0
    }

    /// Subscribe a slot (modsrv side, called during rule load).
    ///
    /// Silently ignores out-of-range slots.
    #[inline]
    pub fn set_watched(&self, slot: usize) {
        let word_idx = slot / 64;
        let bit_idx = slot % 64;
        if word_idx >= WATCH_WORDS_COUNT {
            return;
        }
        // SAFETY: same as is_watched; word_idx bounds-checked above.
        let words = unsafe {
            std::slice::from_raw_parts(self.mmap.as_ptr() as *const AtomicU64, WATCH_WORDS_COUNT)
        };
        words[word_idx].fetch_or(1u64 << bit_idx, Ordering::Release);
    }

    /// Unsubscribe a slot (modsrv side).
    ///
    /// Silently ignores out-of-range slots.
    #[inline]
    pub fn clear_watched(&self, slot: usize) {
        let word_idx = slot / 64;
        let bit_idx = slot % 64;
        if word_idx >= WATCH_WORDS_COUNT {
            return;
        }
        // SAFETY: same invariant as set_watched.
        let words = unsafe {
            std::slice::from_raw_parts(self.mmap.as_ptr() as *const AtomicU64, WATCH_WORDS_COUNT)
        };
        words[word_idx].fetch_and(!(1u64 << bit_idx), Ordering::Release);
    }

    /// Clear all subscriptions (modsrv side, before re-loading rules).
    pub fn clear_all(&self) {
        // SAFETY: same invariant.
        let words = unsafe {
            std::slice::from_raw_parts(self.mmap.as_ptr() as *const AtomicU64, WATCH_WORDS_COUNT)
        };
        for word in words {
            word.store(0, Ordering::Release);
        }
    }

    /// Count the number of subscribed slots (diagnostic helper).
    pub fn subscription_count(&self) -> usize {
        // SAFETY: same invariant.
        let words = unsafe {
            std::slice::from_raw_parts(self.mmap.as_ptr() as *const AtomicU64, WATCH_WORDS_COUNT)
        };
        words
            .iter()
            .map(|w| w.load(Ordering::Relaxed).count_ones() as usize)
            .sum()
    }

    /// Bitmap file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether opened read-only (currently unused — both sides open r/w).
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    #[test]
    fn basic_subscribe_and_check() {
        let bm = SubscriptionBitmap::new_in_memory().unwrap();
        assert!(!bm.is_watched(0));
        assert!(!bm.is_watched(63));
        assert!(!bm.is_watched(64));

        bm.set_watched(63);
        assert!(bm.is_watched(63));
        assert!(!bm.is_watched(62));
        assert!(!bm.is_watched(64));

        bm.set_watched(99_999);
        assert!(bm.is_watched(99_999));
        assert!(!bm.is_watched(99_998));
    }

    #[test]
    fn out_of_range_slot_ignored() {
        let bm = SubscriptionBitmap::new_in_memory().unwrap();
        // Slot 100_032 is beyond WATCH_WORDS_COUNT * 64 = 100,032
        // (exactly at boundary; slot 100_031 is last valid)
        bm.set_watched(100_032); // out-of-range, silently ignored
        assert!(!bm.is_watched(100_032));
    }

    #[test]
    fn clear_all_resets_bits() {
        let bm = SubscriptionBitmap::new_in_memory().unwrap();
        bm.set_watched(0);
        bm.set_watched(127);
        bm.set_watched(99_999);
        assert_eq!(bm.subscription_count(), 3);

        bm.clear_all();
        assert_eq!(bm.subscription_count(), 0);
        assert!(!bm.is_watched(0));
        assert!(!bm.is_watched(127));
    }

    #[test]
    fn clear_watched_individual() {
        let bm = SubscriptionBitmap::new_in_memory().unwrap();
        bm.set_watched(10);
        bm.set_watched(11);
        assert!(bm.is_watched(10));
        assert!(bm.is_watched(11));

        bm.clear_watched(10);
        assert!(!bm.is_watched(10));
        assert!(bm.is_watched(11)); // unaffected
    }

    #[test]
    fn subscription_count() {
        let bm = SubscriptionBitmap::new_in_memory().unwrap();
        for slot in [0, 63, 64, 128, 1000] {
            bm.set_watched(slot);
        }
        assert_eq!(bm.subscription_count(), 5);
    }

    #[test]
    fn create_and_open_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-subs.shm");

        {
            let bm = SubscriptionBitmap::create(&path).unwrap();
            bm.set_watched(42);
            bm.set_watched(100);
        }
        // Re-open (simulates modsrv opening what comsrv created)
        {
            let bm = SubscriptionBitmap::open(&path).unwrap();
            // Bits set by previous instance persist in the file
            assert!(bm.is_watched(42));
            assert!(bm.is_watched(100));
            assert!(!bm.is_watched(41));
        }
    }

    #[test]
    fn bitmap_path_from_shm_derives_correct_path() {
        let shm = Path::new("/shm/rtdb/voltage-rtdb.shm");
        let bm = bitmap_path_from_shm(shm);
        assert_eq!(
            bm,
            PathBuf::from("/shm/rtdb/voltage-rtdb-point-watch-subs.shm")
        );
    }
}
