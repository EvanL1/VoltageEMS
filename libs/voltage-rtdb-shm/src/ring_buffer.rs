//! High-Frequency Ring Buffer - T-Box Style Data Recording
//!
//! Supports microsecond timestamps, lock-free writes, and cross-process access via SharedMemory.

use std::fs::{File, OpenOptions};
use std::path::Path;
use std::ptr::NonNull;
#[cfg(debug_assertions)]
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU64, Ordering};

use memmap2::MmapMut;

/// Magic number identifier "VOLTRING"
const MAGIC: u64 = 0x564F4C54_52494E47;
const VERSION: u32 = 1;

// ============================================================================
// DataPoint - Data Point Structure (32-byte aligned)
// ============================================================================

/// High-frequency acquisition data point
///
/// Explicitly 32-byte aligned to ensure:
/// - Consistent cross-process memory layout
/// - Cache-line friendly (two points fill one 64-byte cache line)
/// - Efficient atomic operations
///
/// Memory layout (32 bytes total):
/// ```text
/// offset 0:  timestamp_us (u64)  8 bytes
/// offset 8:  value (f64)         8 bytes  <- f64 requires 8-byte alignment
/// offset 16: channel_id (u32)    4 bytes  <- Protocol channel ID
/// offset 20: instance_id (u32)   4 bytes  <- Business device ID (via routing)
/// offset 24: point_id (u32)      4 bytes
/// offset 28: point_type (u8)     1 byte
/// offset 29: quality (u8)        1 byte
/// offset 30: _reserved ([u8;2])  2 bytes  <- Padded to 32 bytes
/// ```
#[repr(C, align(32))]
#[derive(Clone, Copy, Debug, Default)]
pub struct DataPoint {
    /// Microsecond timestamp (Unix epoch)
    pub timestamp_us: u64,
    /// Value (placed at offset 8 to ensure 8-byte alignment)
    pub value: f64,
    /// Channel ID (protocol level, e.g. Modbus connection ID)
    pub channel_id: u32,
    /// Instance ID (business level, e.g. device ID, obtained via RoutingCache C2M mapping)
    pub instance_id: u32,
    /// Point ID
    pub point_id: u32,
    /// Point type: 0=T(Telemetry), 1=S(Signal), 2=C(Control), 3=A(Adjustment)
    pub point_type: u8,
    /// Quality code: 0=Good, others per protocol definition
    pub quality: u8,
    /// Reserved for future expansion
    pub _reserved: [u8; 2],
}

impl DataPoint {
    /// Create a new data point (channel level only, instance_id = 0)
    #[inline]
    pub fn new(channel_id: u32, point_type: u8, point_id: u32, value: f64, quality: u8) -> Self {
        Self {
            timestamp_us: current_timestamp_us(),
            value,
            channel_id,
            instance_id: 0, // Needs to be populated via routing mapping
            point_id,
            point_type,
            quality,
            _reserved: [0; 2],
        }
    }

    /// Create a full data point (including channel and instance)
    #[inline]
    pub fn new_full(
        channel_id: u32,
        instance_id: u32,
        point_type: u8,
        point_id: u32,
        value: f64,
        quality: u8,
    ) -> Self {
        Self {
            timestamp_us: current_timestamp_us(),
            value,
            channel_id,
            instance_id,
            point_id,
            point_type,
            quality,
            _reserved: [0; 2],
        }
    }

    /// Create with a specified timestamp
    #[inline]
    pub fn with_timestamp(
        timestamp_us: u64,
        channel_id: u32,
        instance_id: u32,
        point_type: u8,
        point_id: u32,
        value: f64,
        quality: u8,
    ) -> Self {
        Self {
            timestamp_us,
            value,
            channel_id,
            instance_id,
            point_id,
            point_type,
            quality,
            _reserved: [0; 2],
        }
    }
}

// ============================================================================
// RingBufferHeader - Buffer Header Metadata (64 bytes)
// ============================================================================

/// Ring buffer header
///
/// Located at the beginning of the shared memory file, contains metadata and atomic write pointer.
#[repr(C)]
pub struct RingBufferHeader {
    /// Magic number "VOLTRING" (0x564F4C54_52494E47)
    pub magic: u64,
    /// Version number
    pub version: u32,
    /// Buffer capacity (number of data points)
    pub capacity: u32,
    /// Write pointer (atomic, monotonically increasing)
    pub head: AtomicU64,
    /// Total write count
    pub total_writes: AtomicU64,
    /// Data retention duration (microseconds)
    pub retention_us: u64,
    /// Creation time (microsecond timestamp)
    pub created_at: u64,
    /// Reserved space
    _reserved: [u8; 16],
}

impl RingBufferHeader {
    /// Initialize header
    fn init(&mut self, capacity: u32, retention_us: u64) {
        self.magic = MAGIC;
        self.version = VERSION;
        self.capacity = capacity;
        self.head = AtomicU64::new(0);
        self.total_writes = AtomicU64::new(0);
        self.retention_us = retention_us;
        self.created_at = current_timestamp_us();
        self._reserved = [0; 16];
    }

    /// Validate header integrity
    fn validate(&self) -> bool {
        self.magic == MAGIC && self.version == VERSION && self.capacity > 0
    }
}

// ============================================================================
// HighFreqRingBuffer - Lock-free Ring Buffer
// ============================================================================

/// High-frequency ring buffer
///
/// **Single-producer**, multi-reader lock-free implementation, supporting:
/// - O(1) writes (atomic fetch_add for slot allocation)
/// - Time range queries
/// - Zero-copy memory mapping
///
/// # Safety Contract
///
/// **The write side (`push`/`push_batch`) must be restricted to a single thread.**
/// While `fetch_add` guarantees a unique slot per write, `write_volatile`
/// is not atomic for 32-byte `DataPoint`. Concurrent pushes from multiple threads
/// may cause readers to observe torn reads.
///
/// Read methods (`read_range`/`read_latest` etc.) can be safely called from any thread.
///
/// In debug builds, `push()` detects concurrent call violations via an `AtomicBool` guard.
pub struct HighFreqRingBuffer {
    /// Memory start pointer (retained for Debug and future expansion)
    #[allow(dead_code)]
    data: NonNull<u8>,
    /// Header pointer
    header: *mut RingBufferHeader,
    /// Data region pointer
    points: *mut DataPoint,
    /// Capacity
    capacity: usize,
    /// Whether memory is owned (retained for future standalone memory allocation scenarios)
    #[allow(dead_code)]
    owned: bool,
    /// Detects concurrent push() calls in debug mode
    #[cfg(debug_assertions)]
    push_guard: AtomicBool,
}

// SAFETY: HighFreqRingBuffer can be safely sent across threads because:
// - All mutable state access uses atomic operations (AtomicU64 for head/total_writes)
// - The underlying NonNull<u8> points to mmap'd memory that remains valid
//   for the lifetime of the buffer
// - No thread-local state is used
unsafe impl Send for HighFreqRingBuffer {}

// SAFETY: HighFreqRingBuffer can be safely shared between threads (&self) because:
// - Read methods only use atomic load + read_volatile, safe for multiple readers
// - push() slot allocation is guaranteed unique via atomic fetch_add
// - **Precondition**: push() must be restricted to a single producer thread.
//   write_volatile is not atomic for 32-byte DataPoint; multiple producers cause torn reads.
//   Debug builds detect violations via AtomicBool guard.
unsafe impl Sync for HighFreqRingBuffer {}

impl HighFreqRingBuffer {
    /// Create from raw memory (internal use)
    ///
    /// # Safety
    /// - `data` must point to a valid, sufficiently large memory region
    /// - Memory must be properly initialized or will be initialized
    unsafe fn from_raw(data: NonNull<u8>, capacity: usize, init: bool, retention_us: u64) -> Self {
        // SAFETY (all operations below rely on the caller's contract):
        // - data points to a valid region of size >= size_of::<RingBufferHeader>() + capacity * size_of::<DataPoint>()
        // - header is at offset 0, points array starts at offset size_of::<RingBufferHeader>()
        // - if init is true, caller guarantees exclusive write access for initialization
        let header = data.as_ptr() as *mut RingBufferHeader;
        let points = data.as_ptr().add(std::mem::size_of::<RingBufferHeader>()) as *mut DataPoint;

        if init {
            (*header).init(capacity as u32, retention_us);
            // Zero out data region
            std::ptr::write_bytes(points, 0, capacity);
        }

        Self {
            data,
            header,
            points,
            capacity,
            owned: false,
            #[cfg(debug_assertions)]
            push_guard: AtomicBool::new(false),
        }
    }

    /// Write a single data point (lock-free, single producer)
    ///
    /// Uses atomic `fetch_add` to obtain a unique write position, then `write_volatile` to write data.
    ///
    /// # Safety Contract
    ///
    /// **This method must be restricted to a single thread.** `write_volatile` is not atomic
    /// for 32-byte `DataPoint`; concurrent calls from multiple threads cause torn reads.
    ///
    /// In debug builds, an `AtomicBool` guard detects concurrent call violations at runtime.
    #[inline]
    pub fn push(&self, point: DataPoint) {
        // Debug mode: detect if another thread is currently pushing
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                !self.push_guard.swap(true, Ordering::Acquire),
                "HighFreqRingBuffer::push() called concurrently from multiple threads! \
                 This is a single-producer buffer — concurrent push causes torn reads."
            );
        }

        // SAFETY: self.header was set in from_raw() from a valid NonNull pointer.
        // fetch_add is atomic and returns a unique position per call.
        let pos =
            unsafe { (*self.header).head.fetch_add(1, Ordering::Relaxed) as usize % self.capacity };
        // SAFETY: pos = head % capacity, so pos < capacity, meaning self.points.add(pos)
        // is within the data region. write_volatile ensures the write is not elided.
        // Single-producer contract guarantees no concurrent write to the same slot.
        unsafe {
            std::ptr::write_volatile(self.points.add(pos), point);
        }
        // SAFETY: self.header is valid (set in from_raw()). Atomic operation is safe.
        unsafe {
            (*self.header).total_writes.fetch_add(1, Ordering::Relaxed);
        }

        #[cfg(debug_assertions)]
        {
            self.push_guard.store(false, Ordering::Release);
        }
    }

    /// Batch write
    #[inline]
    pub fn push_batch(&self, points: &[DataPoint]) {
        for point in points {
            self.push(*point);
        }
    }

    /// Get current write position
    #[inline]
    pub fn head(&self) -> u64 {
        // SAFETY: self.header is valid (set in from_raw()). Atomic load is always safe.
        unsafe { (*self.header).head.load(Ordering::Relaxed) }
    }

    /// Get total write count
    #[inline]
    pub fn total_writes(&self) -> u64 {
        // SAFETY: self.header is valid (set in from_raw()). Atomic load is always safe.
        unsafe { (*self.header).total_writes.load(Ordering::Relaxed) }
    }

    /// Get capacity
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Read data points within a specified time range
    ///
    /// Traverses backwards from the newest data, returning points within [start_us, end_us].
    pub fn read_range(&self, start_us: u64, end_us: u64) -> Vec<DataPoint> {
        let mut result = Vec::new();
        let head = self.head() as usize;
        let cap = self.capacity;

        // Traverse backwards from the newest position
        for i in 0..cap {
            let idx = (head.wrapping_sub(1).wrapping_sub(i)) % cap;
            // SAFETY: idx = (...) % cap, so idx < capacity. self.points.add(idx) is within
            // the data region. read_volatile ensures we see the latest written value.
            let point = unsafe { std::ptr::read_volatile(self.points.add(idx)) };

            // Skip unwritten slots
            if point.timestamp_us == 0 {
                continue;
            }

            // Time range check
            if point.timestamp_us < start_us {
                break; // Out of range, stop
            }
            if point.timestamp_us <= end_us {
                result.push(point);
            }
        }

        result.reverse(); // Sort by ascending time
        result
    }

    /// Read up to `count` data points matching `predicate`, traversing backwards from head.
    ///
    /// Shared traversal logic for `read_latest`, `read_channel`, and `read_instance`.
    fn read_filtered(
        &self,
        count: usize,
        predicate: impl Fn(&DataPoint) -> bool,
    ) -> Vec<DataPoint> {
        let mut result = Vec::with_capacity(count.min(self.capacity));
        let head = self.head() as usize;
        let cap = self.capacity;

        for i in 0..cap {
            if result.len() >= count {
                break;
            }
            let idx = (head.wrapping_sub(1).wrapping_sub(i)) % cap;
            // SAFETY: idx = (...) % cap, so idx < capacity. Pointer is within data region.
            let point = unsafe { std::ptr::read_volatile(self.points.add(idx)) };

            if point.timestamp_us == 0 {
                break;
            }
            if predicate(&point) {
                result.push(point);
            }
        }

        result.reverse();
        result
    }

    /// Read the latest N data points
    #[inline]
    pub fn read_latest(&self, count: usize) -> Vec<DataPoint> {
        self.read_filtered(count, |_| true)
    }

    /// Export event snapshot (collision detection style)
    ///
    /// Returns data within the specified microsecond range before and after the event time.
    pub fn export_snapshot(
        &self,
        event_time_us: u64,
        before_us: u64,
        after_us: u64,
    ) -> Vec<DataPoint> {
        let start = event_time_us.saturating_sub(before_us);
        let end = event_time_us.saturating_add(after_us);
        self.read_range(start, end)
    }

    /// Read filtered by channel
    #[inline]
    pub fn read_channel(&self, channel_id: u32, count: usize) -> Vec<DataPoint> {
        self.read_filtered(count, |p| p.channel_id == channel_id)
    }

    /// Read filtered by instance ID (business device perspective)
    #[inline]
    pub fn read_instance(&self, instance_id: u32, count: usize) -> Vec<DataPoint> {
        self.read_filtered(count, |p| p.instance_id == instance_id)
    }
}

// ============================================================================
// SharedRingBuffer - Shared Memory Wrapper
// ============================================================================

/// Shared memory ring buffer
///
/// Uses mmap file mapping, supports cross-process access.
pub struct SharedRingBuffer {
    inner: HighFreqRingBuffer,
    _mmap: MmapMut,
    _file: File,
}

impl SharedRingBuffer {
    /// Create or open shared memory ring buffer
    ///
    /// - If file does not exist, creates and initializes it
    /// - If file exists, validates header then maps directly
    pub fn create_or_open(
        path: impl AsRef<Path>,
        capacity: usize,
        retention_us: u64,
    ) -> std::io::Result<Self> {
        let path = path.as_ref();
        let size = Self::calculate_size(capacity);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let exists = path.exists();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false) // Preserve existing data, do not truncate
            .open(path)?;

        // Set file size
        file.set_len(size as u64)?;

        // SAFETY: File is opened with read+write and sized to calculate_size(capacity).
        // mmap region is valid for the lifetime of the MmapMut.
        let mut mmap = unsafe { MmapMut::map_mut(&file)? };
        let data = NonNull::new(mmap.as_mut_ptr())
            .ok_or_else(|| std::io::Error::other("mmap returned null pointer"))?;

        let need_init = if exists {
            // SAFETY: data is non-null and the mmap region is at least size_of::<RingBufferHeader>()
            // bytes (calculate_size includes header). Alignment is satisfied by page-aligned mmap.
            let header = unsafe { &*(data.as_ptr() as *const RingBufferHeader) };
            !header.validate() || header.capacity as usize != capacity
        } else {
            true
        };

        // SAFETY: data is non-null, mmap region is sized for header + capacity * DataPoint.
        // need_init determines whether the header and data area are re-initialized.
        let inner =
            unsafe { HighFreqRingBuffer::from_raw(data, capacity, need_init, retention_us) };

        Ok(Self {
            inner,
            _mmap: mmap,
            _file: file,
        })
    }

    /// Open read-only (for the reader side)
    pub fn open_readonly(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;

        // SAFETY: File is opened with read+write. The file was previously created by
        // create_or_open() with a valid layout.
        let mut mmap = unsafe { MmapMut::map_mut(&file)? };
        let data = NonNull::new(mmap.as_mut_ptr())
            .ok_or_else(|| std::io::Error::other("mmap returned null pointer"))?;

        // Validate header
        // SAFETY: data is non-null, mmap region includes at least the header.
        // validate() is called immediately after to check magic, version, and capacity.
        let header = unsafe { &*(data.as_ptr() as *const RingBufferHeader) };
        if !header.validate() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid ring buffer header",
            ));
        }

        let capacity = header.capacity as usize;
        // SAFETY: data is non-null, header was validated above. init=false skips
        // initialization since the existing data is valid. capacity matches the header.
        let inner = unsafe { HighFreqRingBuffer::from_raw(data, capacity, false, 0) };

        Ok(Self {
            inner,
            _mmap: mmap,
            _file: file,
        })
    }

    /// Calculate required memory size
    fn calculate_size(capacity: usize) -> usize {
        std::mem::size_of::<RingBufferHeader>() + capacity * std::mem::size_of::<DataPoint>()
    }

    /// Get inner buffer reference
    #[inline]
    pub fn buffer(&self) -> &HighFreqRingBuffer {
        &self.inner
    }

    // Proxy methods
    #[inline]
    pub fn push(&self, point: DataPoint) {
        self.inner.push(point);
    }

    #[inline]
    pub fn push_batch(&self, points: &[DataPoint]) {
        self.inner.push_batch(points);
    }

    #[inline]
    pub fn read_range(&self, start_us: u64, end_us: u64) -> Vec<DataPoint> {
        self.inner.read_range(start_us, end_us)
    }

    #[inline]
    pub fn read_latest(&self, count: usize) -> Vec<DataPoint> {
        self.inner.read_latest(count)
    }

    #[inline]
    pub fn export_snapshot(
        &self,
        event_time_us: u64,
        before_us: u64,
        after_us: u64,
    ) -> Vec<DataPoint> {
        self.inner
            .export_snapshot(event_time_us, before_us, after_us)
    }

    #[inline]
    pub fn read_channel(&self, channel_id: u32, count: usize) -> Vec<DataPoint> {
        self.inner.read_channel(channel_id, count)
    }

    /// Read filtered by instance ID (business device perspective)
    #[inline]
    pub fn read_instance(&self, instance_id: u32, count: usize) -> Vec<DataPoint> {
        self.inner.read_instance(instance_id, count)
    }

    #[inline]
    pub fn head(&self) -> u64 {
        self.inner.head()
    }

    #[inline]
    pub fn total_writes(&self) -> u64 {
        self.inner.total_writes()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }
}

// ============================================================================
// RingBufferConfig - Configuration
// ============================================================================

/// Ring buffer configuration
#[derive(Debug, Clone)]
pub struct RingBufferConfig {
    /// Shared memory file path
    pub path: std::path::PathBuf,
    /// Buffer capacity (number of data points)
    pub capacity: usize,
    /// Data retention duration (microseconds)
    pub retention_us: u64,
    /// Whether enabled
    pub enabled: bool,
}

impl Default for RingBufferConfig {
    fn default() -> Self {
        Self {
            path: std::path::PathBuf::from("/shm/rtdb/ring.bin"),
            capacity: 1_000_000,      // 1M points ≈ 32MB
            retention_us: 60_000_000, // 60 seconds
            enabled: true,
        }
    }
}

impl RingBufferConfig {
    /// Load from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(path) = std::env::var("RING_BUFFER_PATH") {
            config.path = std::path::PathBuf::from(path);
        }
        if let Ok(cap) = std::env::var("RING_BUFFER_CAPACITY") {
            if let Ok(n) = cap.parse() {
                config.capacity = n;
            }
        }
        if let Ok(ret) = std::env::var("RING_BUFFER_RETENTION_US") {
            if let Ok(n) = ret.parse() {
                config.retention_us = n;
            }
        }
        if let Ok(en) = std::env::var("RING_BUFFER_ENABLED") {
            config.enabled = en != "false" && en != "0";
        }

        config
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get current microsecond timestamp
#[inline]
pub fn current_timestamp_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_data_point_size() {
        // Ensure the struct is exactly 32 bytes
        let size = std::mem::size_of::<DataPoint>();
        assert_eq!(size, 32, "DataPoint must be exactly 32 bytes, got {}", size);
    }

    #[test]
    fn test_header_size() {
        let size = std::mem::size_of::<RingBufferHeader>();
        assert_eq!(size, 64, "Header size should be 64 bytes, got {}", size);
    }

    #[test]
    fn test_shared_ring_buffer_basic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_ring.bin");

        let buffer = SharedRingBuffer::create_or_open(&path, 1000, 60_000_000).unwrap();

        // Write test data
        for i in 0..100 {
            buffer.push(DataPoint::new(1, 0, i, i as f64 * 1.5, 0));
        }

        assert_eq!(buffer.head(), 100);
        assert_eq!(buffer.total_writes(), 100);

        // Read latest data
        let latest = buffer.read_latest(10);
        assert_eq!(latest.len(), 10);
        assert_eq!(latest[9].point_id, 99);
    }

    #[test]
    fn test_ring_buffer_wrap_around() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wrap_ring.bin");

        let buffer = SharedRingBuffer::create_or_open(&path, 100, 60_000_000).unwrap();

        // Write data exceeding capacity
        for i in 0..250 {
            buffer.push(DataPoint::new(1, 0, i, i as f64, 0));
        }

        assert_eq!(buffer.head(), 250);
        assert_eq!(buffer.total_writes(), 250);

        // Read latest data (should be 150-249)
        let latest = buffer.read_latest(100);
        assert_eq!(latest.len(), 100);
        assert_eq!(latest[0].point_id, 150);
        assert_eq!(latest[99].point_id, 249);
    }

    #[test]
    fn test_channel_filter() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("channel_ring.bin");

        let buffer = SharedRingBuffer::create_or_open(&path, 1000, 60_000_000).unwrap();

        // Write data for multiple channels
        for i in 0u32..100 {
            let channel = (i % 3) + 1; // Channels 1, 2, 3
            buffer.push(DataPoint::new(channel, 0, i, i as f64, 0));
        }

        // Filter by channel
        let ch1_data = buffer.read_channel(1, 50);
        assert!(!ch1_data.is_empty());
        for p in &ch1_data {
            assert_eq!(p.channel_id, 1);
        }
    }

    /// Test atomic counter correctness under multi-threaded writes.
    ///
    /// Note: HighFreqRingBuffer is designed as single-producer; debug builds' push() guard
    /// detects concurrent calls. This test only runs in release builds, verifying fetch_add
    /// counter correctness under extreme conditions.
    #[test]
    #[cfg_attr(debug_assertions, ignore)]
    fn test_concurrent_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("concurrent_ring.bin");

        let buffer = Arc::new(SharedRingBuffer::create_or_open(&path, 10000, 60_000_000).unwrap());

        let handles: Vec<_> = (0..4)
            .map(|t| {
                let buf = Arc::clone(&buffer);
                thread::spawn(move || {
                    for i in 0..1000 {
                        buf.push(DataPoint::new(t, 0, i, i as f64, 0));
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(buffer.total_writes(), 4000);
    }

    #[test]
    fn test_reopen_existing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reopen_ring.bin");

        // Create and write
        {
            let buffer = SharedRingBuffer::create_or_open(&path, 1000, 60_000_000).unwrap();
            for i in 0..50 {
                buffer.push(DataPoint::new(1, 0, i, i as f64, 0));
            }
        }

        // Reopen
        {
            let buffer = SharedRingBuffer::open_readonly(&path).unwrap();
            assert_eq!(buffer.capacity(), 1000);
            let latest = buffer.read_latest(50);
            assert_eq!(latest.len(), 50);
        }
    }

    #[test]
    fn test_instance_filter() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("instance_ring.bin");

        let buffer = SharedRingBuffer::create_or_open(&path, 1000, 60_000_000).unwrap();

        // Write data for multiple instances (using new_full to include instance_id)
        for i in 0..100u32 {
            let channel_id = 1;
            let instance_id = (i % 3) + 1; // Instances 1, 2, 3
            buffer.push(DataPoint::new_full(
                channel_id,
                instance_id,
                0,
                i,
                i as f64,
                0,
            ));
        }

        // Filter by instance
        let inst1_data = buffer.read_instance(1, 50);
        assert!(!inst1_data.is_empty());
        for p in &inst1_data {
            assert_eq!(p.instance_id, 1);
        }

        let inst2_data = buffer.read_instance(2, 50);
        assert!(!inst2_data.is_empty());
        for p in &inst2_data {
            assert_eq!(p.instance_id, 2);
        }
    }

    // ========== Type Safety & Layout Tests (test-expert, task #2) ==========

    /// Compile-time verification that HighFreqRingBuffer implements Send + Sync.
    ///
    /// These bounds are required because comsrv shares the buffer across
    /// tokio tasks (Send) and reads from multiple poll cycles (Sync).
    /// If the unsafe impls are removed, this test will fail to compile.
    #[test]
    fn test_ring_buffer_send_sync_bounds() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<HighFreqRingBuffer>();
        assert_sync::<HighFreqRingBuffer>();
        assert_send::<SharedRingBuffer>();
        assert_sync::<SharedRingBuffer>();
    }

    /// Verify struct layout sizes for SHM binary compatibility.
    #[test]
    fn test_struct_layout_sizes() {
        assert_eq!(
            std::mem::size_of::<DataPoint>(),
            32,
            "DataPoint must be 32 bytes for SHM layout"
        );
        assert_eq!(
            std::mem::align_of::<DataPoint>(),
            32,
            "DataPoint must be 32-byte aligned"
        );
        assert_eq!(
            std::mem::size_of::<RingBufferHeader>(),
            64,
            "RingBufferHeader must be 64 bytes for SHM layout"
        );
    }

    /// Single-producer push correctness: sequential pushes maintain FIFO ordering.
    #[test]
    fn test_single_producer_push_ordering() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("order_ring.bin");

        let buffer = SharedRingBuffer::create_or_open(&path, 100, 60_000_000).unwrap();

        // Push 50 items with distinct values
        for i in 0u32..50 {
            buffer.push(DataPoint::with_timestamp(
                (i + 1) as u64 * 1000,
                1,
                0,
                0,
                i,
                i as f64 * 2.0,
                0,
            ));
        }

        let latest = buffer.read_latest(50);
        assert_eq!(latest.len(), 50);

        // Verify FIFO ordering (ascending timestamp)
        for i in 0..49 {
            assert!(
                latest[i].timestamp_us <= latest[i + 1].timestamp_us,
                "Ordering violation at index {}: {} > {}",
                i,
                latest[i].timestamp_us,
                latest[i + 1].timestamp_us
            );
        }

        // Verify each point's data integrity
        for (i, p) in latest.iter().enumerate() {
            assert_eq!(p.point_id, i as u32);
            assert_eq!(p.value, i as f64 * 2.0);
        }
    }

    /// Debug-mode push guard detects concurrent push calls via panic.
    ///
    /// This test is only meaningful in debug builds where the AtomicBool
    /// guard is compiled in. In release builds the guard is absent.
    ///
    /// Uses 200,000 iterations per thread and multiple retry rounds to
    /// maximise the chance of overlapping push() calls.
    #[test]
    #[cfg(debug_assertions)]
    fn test_push_guard_detects_concurrent_push() {
        use std::panic;
        use std::sync::atomic::{AtomicBool, Ordering as AO};

        let dir = tempfile::tempdir().unwrap();

        // Try multiple rounds — contention is probabilistic on fast CPUs
        let mut detected = false;
        for round in 0..5 {
            let buffer = Arc::new(
                SharedRingBuffer::create_or_open(
                    dir.path().join(format!("guard_ring_{round}.bin")),
                    100_000,
                    60_000_000,
                )
                .unwrap(),
            );

            let go = Arc::new(AtomicBool::new(false));
            let b1 = Arc::clone(&buffer);
            let b2 = Arc::clone(&buffer);
            let go1 = Arc::clone(&go);
            let go2 = Arc::clone(&go);

            let h1 = thread::spawn(move || {
                while !go1.load(AO::Acquire) {
                    std::hint::spin_loop();
                }
                panic::catch_unwind(panic::AssertUnwindSafe(|| {
                    for i in 0..200_000u32 {
                        b1.push(DataPoint::new(1, 0, i, i as f64, 0));
                    }
                }))
                .is_err()
            });

            let h2 = thread::spawn(move || {
                while !go2.load(AO::Acquire) {
                    std::hint::spin_loop();
                }
                panic::catch_unwind(panic::AssertUnwindSafe(|| {
                    for i in 0..200_000u32 {
                        b2.push(DataPoint::new(2, 0, i, i as f64, 0));
                    }
                }))
                .is_err()
            });

            // Tight spin-start to maximise overlap
            go.store(true, AO::Release);

            let p1 = h1.join().unwrap();
            let p2 = h2.join().unwrap();

            if p1 || p2 {
                detected = true;
                break;
            }
        }

        assert!(
            detected,
            "Push guard did not detect concurrent push after 5 rounds"
        );
    }

    /// Export snapshot returns events within the specified time window.
    #[test]
    fn test_export_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snapshot_ring.bin");

        let buffer = SharedRingBuffer::create_or_open(&path, 1000, 60_000_000).unwrap();

        // Write 100 points, timestamps 1000..100000 (step 1000)
        for i in 1u64..=100 {
            buffer.push(DataPoint::with_timestamp(
                i * 1000,
                1,
                0,
                0,
                i as u32,
                i as f64,
                0,
            ));
        }

        // Snapshot around event_time=50000, window ±5000
        let snap = buffer.export_snapshot(50000, 5000, 5000);
        assert!(!snap.is_empty());
        for p in &snap {
            assert!(p.timestamp_us >= 45000 && p.timestamp_us <= 55000);
        }
    }
}
