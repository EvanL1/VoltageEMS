//! 高频环形缓冲区 - T-Box 风格数据记录
//!
//! 支持微秒级时间戳、Lock-free 写入、SharedMemory 跨进程访问。

use std::fs::{File, OpenOptions};
use std::path::Path;
use std::ptr::NonNull;
#[cfg(debug_assertions)]
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU64, Ordering};

use memmap2::MmapMut;

/// 魔数标识 "VOLTRING"
const MAGIC: u64 = 0x564F4C54_52494E47;
const VERSION: u32 = 1;

// ============================================================================
// DataPoint - 数据点结构 (32 字节对齐)
// ============================================================================

/// 高频采集数据点
///
/// 显式 32 字节对齐，确保：
/// - 跨进程内存布局一致
/// - 缓存行友好（两个点填满一个 64 字节缓存行）
/// - 原子操作高效
///
/// 内存布局 (32 bytes total):
/// ```text
/// offset 0:  timestamp_us (u64)  8 bytes
/// offset 8:  value (f64)         8 bytes  <- f64 需要 8 字节对齐
/// offset 16: channel_id (u32)    4 bytes  <- 协议通道 ID
/// offset 20: instance_id (u32)   4 bytes  <- 业务设备 ID (通过路由映射)
/// offset 24: point_id (u32)      4 bytes
/// offset 28: point_type (u8)     1 byte
/// offset 29: quality (u8)        1 byte
/// offset 30: _reserved ([u8;2])  2 bytes  <- 填充到 32 字节
/// ```
#[repr(C, align(32))]
#[derive(Clone, Copy, Debug, Default)]
pub struct DataPoint {
    /// 微秒时间戳 (Unix epoch)
    pub timestamp_us: u64,
    /// 数值（放在 offset 8 确保 8 字节对齐）
    pub value: f64,
    /// 通道 ID (协议层面，如 Modbus 连接 ID)
    pub channel_id: u32,
    /// 实例 ID (业务层面，如设备 ID，通过 RoutingCache C2M 映射获取)
    pub instance_id: u32,
    /// 点位 ID
    pub point_id: u32,
    /// 点位类型: 0=T(遥测), 1=S(信号), 2=C(控制), 3=A(调节)
    pub point_type: u8,
    /// 质量码: 0=Good, 其他见协议定义
    pub quality: u8,
    /// 预留扩展空间
    pub _reserved: [u8; 2],
}

impl DataPoint {
    /// 创建新的数据点（仅 channel 级别，instance_id = 0）
    #[inline]
    pub fn new(channel_id: u32, point_type: u8, point_id: u32, value: f64, quality: u8) -> Self {
        Self {
            timestamp_us: current_timestamp_us(),
            value,
            channel_id,
            instance_id: 0, // 需要通过路由映射填充
            point_id,
            point_type,
            quality,
            _reserved: [0; 2],
        }
    }

    /// 创建完整数据点（包含 channel 和 instance）
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

    /// 使用指定时间戳创建
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
// RingBufferHeader - 缓冲区头部元数据 (64 字节)
// ============================================================================

/// 环形缓冲区头部
///
/// 位于共享内存文件开头，包含元数据和原子写指针。
#[repr(C)]
pub struct RingBufferHeader {
    /// 魔数 "VOLTRING" (0x564F4C54_52494E47)
    pub magic: u64,
    /// 版本号
    pub version: u32,
    /// 缓冲区容量（数据点数量）
    pub capacity: u32,
    /// 写指针（原子，单调递增）
    pub head: AtomicU64,
    /// 总写入次数
    pub total_writes: AtomicU64,
    /// 数据保留时长（微秒）
    pub retention_us: u64,
    /// 创建时间（微秒时间戳）
    pub created_at: u64,
    /// 预留空间
    _reserved: [u8; 16],
}

impl RingBufferHeader {
    /// 初始化头部
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

    /// 验证头部有效性
    fn validate(&self) -> bool {
        self.magic == MAGIC && self.version == VERSION && self.capacity > 0
    }
}

// ============================================================================
// HighFreqRingBuffer - Lock-free 环形缓冲区
// ============================================================================

/// 高频环形缓冲区
///
/// **单生产者**、多读者的 Lock-free 实现，支持：
/// - O(1) 写入（原子 fetch_add 分配 slot）
/// - 时间范围查询
/// - 零拷贝内存映射
///
/// # Safety Contract
///
/// **写入端（`push`/`push_batch`）必须限制在单一线程调用。**
/// 虽然 `fetch_add` 保证每次写入获得唯一 slot，但 `write_volatile`
/// 对 32 字节的 `DataPoint` 不是原子操作。多线程同时 push 时，
/// 读者可能观察到撕裂数据（torn read）。
///
/// 读取方法（`read_range`/`read_latest` 等）可安全从任意线程调用。
///
/// Debug 构建中，`push()` 会通过 `AtomicBool` 守卫检测并发调用违规。
pub struct HighFreqRingBuffer {
    /// 内存起始指针（保留用于 Debug 和未来扩展）
    #[allow(dead_code)]
    data: NonNull<u8>,
    /// 头部指针
    header: *mut RingBufferHeader,
    /// 数据区指针
    points: *mut DataPoint,
    /// 容量
    capacity: usize,
    /// 是否拥有内存（保留用于未来独立内存分配场景）
    #[allow(dead_code)]
    owned: bool,
    /// Debug 模式下检测 push() 并发调用
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
// - 读取方法仅使用 atomic load + read_volatile，多读者安全
// - push() 的 slot 分配通过 atomic fetch_add 保证唯一性
// - **前提条件**：push() 必须限制在单一生产者线程调用。
//   write_volatile 对 32 字节 DataPoint 非原子，多生产者会导致读者观察到撕裂数据。
//   Debug 构建中通过 AtomicBool 守卫检测违规。
unsafe impl Sync for HighFreqRingBuffer {}

impl HighFreqRingBuffer {
    /// 从原始内存创建（内部使用）
    ///
    /// # Safety
    /// - `data` 必须指向有效的、足够大的内存区域
    /// - 内存必须已正确初始化或将被初始化
    unsafe fn from_raw(data: NonNull<u8>, capacity: usize, init: bool, retention_us: u64) -> Self {
        let header = data.as_ptr() as *mut RingBufferHeader;
        let points = data.as_ptr().add(std::mem::size_of::<RingBufferHeader>()) as *mut DataPoint;

        if init {
            (*header).init(capacity as u32, retention_us);
            // 清零数据区
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

    /// 写入单个数据点 (Lock-free, 单生产者)
    ///
    /// 使用原子 `fetch_add` 获取唯一写位置，然后 `write_volatile` 写入数据。
    ///
    /// # Safety Contract
    ///
    /// **此方法必须限制在单一线程调用。** `write_volatile` 对 32 字节
    /// `DataPoint` 不是原子操作，多线程同时调用会导致读者观察到撕裂数据。
    ///
    /// Debug 构建中会通过 `AtomicBool` 守卫在运行时检测并发调用违规。
    #[inline]
    pub fn push(&self, point: DataPoint) {
        // Debug 模式：检测是否有另一个线程正在 push
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                !self.push_guard.swap(true, Ordering::Acquire),
                "HighFreqRingBuffer::push() called concurrently from multiple threads! \
                 This is a single-producer buffer — concurrent push causes torn reads."
            );
        }

        let pos =
            unsafe { (*self.header).head.fetch_add(1, Ordering::Relaxed) as usize % self.capacity };
        unsafe {
            std::ptr::write_volatile(self.points.add(pos), point);
        }
        unsafe {
            (*self.header).total_writes.fetch_add(1, Ordering::Relaxed);
        }

        #[cfg(debug_assertions)]
        {
            self.push_guard.store(false, Ordering::Release);
        }
    }

    /// 批量写入
    #[inline]
    pub fn push_batch(&self, points: &[DataPoint]) {
        for point in points {
            self.push(*point);
        }
    }

    /// 获取当前写位置
    #[inline]
    pub fn head(&self) -> u64 {
        unsafe { (*self.header).head.load(Ordering::Relaxed) }
    }

    /// 获取总写入次数
    #[inline]
    pub fn total_writes(&self) -> u64 {
        unsafe { (*self.header).total_writes.load(Ordering::Relaxed) }
    }

    /// 获取容量
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// 读取指定时间范围内的数据点
    ///
    /// 从最新数据向后遍历，返回 [start_us, end_us] 范围内的点。
    pub fn read_range(&self, start_us: u64, end_us: u64) -> Vec<DataPoint> {
        let mut result = Vec::new();
        let head = self.head() as usize;
        let cap = self.capacity;

        // 从最新位置向后遍历
        for i in 0..cap {
            let idx = (head.wrapping_sub(1).wrapping_sub(i)) % cap;
            let point = unsafe { std::ptr::read_volatile(self.points.add(idx)) };

            // 跳过未写入的槽位
            if point.timestamp_us == 0 {
                continue;
            }

            // 时间范围检查
            if point.timestamp_us < start_us {
                break; // 已超出范围，停止
            }
            if point.timestamp_us <= end_us {
                result.push(point);
            }
        }

        result.reverse(); // 按时间升序
        result
    }

    /// 读取最近 N 个数据点
    pub fn read_latest(&self, count: usize) -> Vec<DataPoint> {
        let mut result = Vec::with_capacity(count.min(self.capacity));
        let head = self.head() as usize;
        let cap = self.capacity;

        for i in 0..count.min(cap) {
            let idx = (head.wrapping_sub(1).wrapping_sub(i)) % cap;
            let point = unsafe { std::ptr::read_volatile(self.points.add(idx)) };

            if point.timestamp_us == 0 {
                break;
            }
            result.push(point);
        }

        result.reverse();
        result
    }

    /// 导出事件快照（碰撞检测风格）
    ///
    /// 返回事件时间前后指定微秒范围内的数据。
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

    /// 按通道过滤读取
    pub fn read_channel(&self, channel_id: u32, count: usize) -> Vec<DataPoint> {
        let mut result = Vec::with_capacity(count);
        let head = self.head() as usize;
        let cap = self.capacity;

        for i in 0..cap {
            if result.len() >= count {
                break;
            }
            let idx = (head.wrapping_sub(1).wrapping_sub(i)) % cap;
            let point = unsafe { std::ptr::read_volatile(self.points.add(idx)) };

            if point.timestamp_us == 0 {
                break;
            }
            if point.channel_id == channel_id {
                result.push(point);
            }
        }

        result.reverse();
        result
    }

    /// 按实例 ID 过滤读取（业务设备视角）
    pub fn read_instance(&self, instance_id: u32, count: usize) -> Vec<DataPoint> {
        let mut result = Vec::with_capacity(count);
        let head = self.head() as usize;
        let cap = self.capacity;

        for i in 0..cap {
            if result.len() >= count {
                break;
            }
            let idx = (head.wrapping_sub(1).wrapping_sub(i)) % cap;
            let point = unsafe { std::ptr::read_volatile(self.points.add(idx)) };

            if point.timestamp_us == 0 {
                break;
            }
            if point.instance_id == instance_id {
                result.push(point);
            }
        }

        result.reverse();
        result
    }
}

// ============================================================================
// SharedRingBuffer - 共享内存封装
// ============================================================================

/// 共享内存环形缓冲区
///
/// 使用 mmap 映射文件，支持跨进程访问。
pub struct SharedRingBuffer {
    inner: HighFreqRingBuffer,
    _mmap: MmapMut,
    _file: File,
}

impl SharedRingBuffer {
    /// 创建或打开共享内存环形缓冲区
    ///
    /// - 如果文件不存在，创建并初始化
    /// - 如果文件存在，验证头部后直接映射
    pub fn create_or_open(
        path: impl AsRef<Path>,
        capacity: usize,
        retention_us: u64,
    ) -> std::io::Result<Self> {
        let path = path.as_ref();
        let size = Self::calculate_size(capacity);

        // 确保父目录存在
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let exists = path.exists();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false) // 保留现有数据，不截断
            .open(path)?;

        // 设置文件大小
        file.set_len(size as u64)?;

        // 创建内存映射
        let mut mmap = unsafe { MmapMut::map_mut(&file)? };
        let data = NonNull::new(mmap.as_mut_ptr())
            .ok_or_else(|| std::io::Error::other("mmap returned null pointer"))?;

        let need_init = if exists {
            // 验证现有文件
            let header = unsafe { &*(data.as_ptr() as *const RingBufferHeader) };
            !header.validate() || header.capacity as usize != capacity
        } else {
            true
        };

        let inner =
            unsafe { HighFreqRingBuffer::from_raw(data, capacity, need_init, retention_us) };

        Ok(Self {
            inner,
            _mmap: mmap,
            _file: file,
        })
    }

    /// 只读打开（用于读取端）
    pub fn open_readonly(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;

        let mut mmap = unsafe { MmapMut::map_mut(&file)? };
        let data = NonNull::new(mmap.as_mut_ptr())
            .ok_or_else(|| std::io::Error::other("mmap returned null pointer"))?;

        // 验证头部
        let header = unsafe { &*(data.as_ptr() as *const RingBufferHeader) };
        if !header.validate() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid ring buffer header",
            ));
        }

        let capacity = header.capacity as usize;
        let inner = unsafe { HighFreqRingBuffer::from_raw(data, capacity, false, 0) };

        Ok(Self {
            inner,
            _mmap: mmap,
            _file: file,
        })
    }

    /// 计算所需内存大小
    fn calculate_size(capacity: usize) -> usize {
        std::mem::size_of::<RingBufferHeader>() + capacity * std::mem::size_of::<DataPoint>()
    }

    /// 获取内部缓冲区引用
    #[inline]
    pub fn buffer(&self) -> &HighFreqRingBuffer {
        &self.inner
    }

    // 代理方法
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

    /// 按实例 ID 过滤读取（业务设备视角）
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
// RingBufferConfig - 配置
// ============================================================================

/// 环形缓冲区配置
#[derive(Debug, Clone)]
pub struct RingBufferConfig {
    /// 共享内存文件路径
    pub path: std::path::PathBuf,
    /// 缓冲区容量（数据点数量）
    pub capacity: usize,
    /// 数据保留时长（微秒）
    pub retention_us: u64,
    /// 是否启用
    pub enabled: bool,
}

impl Default for RingBufferConfig {
    fn default() -> Self {
        Self {
            path: std::path::PathBuf::from("/shm/rtdb/ring.bin"),
            capacity: 1_000_000,      // 100 万点 ≈ 32MB
            retention_us: 60_000_000, // 60 秒
            enabled: true,
        }
    }
}

impl RingBufferConfig {
    /// 从环境变量加载
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
// 辅助函数
// ============================================================================

/// 获取当前微秒时间戳
#[inline]
pub fn current_timestamp_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_data_point_size() {
        // 确保结构体正好是 32 字节
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

        // 写入测试数据
        for i in 0..100 {
            buffer.push(DataPoint::new(1, 0, i, i as f64 * 1.5, 0));
        }

        assert_eq!(buffer.head(), 100);
        assert_eq!(buffer.total_writes(), 100);

        // 读取最近数据
        let latest = buffer.read_latest(10);
        assert_eq!(latest.len(), 10);
        assert_eq!(latest[9].point_id, 99);
    }

    #[test]
    fn test_ring_buffer_wrap_around() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wrap_ring.bin");

        let buffer = SharedRingBuffer::create_or_open(&path, 100, 60_000_000).unwrap();

        // 写入超过容量的数据
        for i in 0..250 {
            buffer.push(DataPoint::new(1, 0, i, i as f64, 0));
        }

        assert_eq!(buffer.head(), 250);
        assert_eq!(buffer.total_writes(), 250);

        // 读取最近数据（应该是 150-249）
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

        // 写入多个通道的数据
        for i in 0u32..100 {
            let channel = (i % 3) + 1; // 通道 1, 2, 3
            buffer.push(DataPoint::new(channel, 0, i, i as f64, 0));
        }

        // 按通道过滤
        let ch1_data = buffer.read_channel(1, 50);
        assert!(!ch1_data.is_empty());
        for p in &ch1_data {
            assert_eq!(p.channel_id, 1);
        }
    }

    /// 测试多线程写入的原子计数正确性。
    ///
    /// 注意：HighFreqRingBuffer 设计为单生产者，debug 构建中 push() 守卫
    /// 会检测并发调用。此测试仅在 release 构建中运行，验证 fetch_add
    /// 计数器在极端情况下的正确性。
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

        // 创建并写入
        {
            let buffer = SharedRingBuffer::create_or_open(&path, 1000, 60_000_000).unwrap();
            for i in 0..50 {
                buffer.push(DataPoint::new(1, 0, i, i as f64, 0));
            }
        }

        // 重新打开
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

        // 写入多个实例的数据（使用 new_full 包含 instance_id）
        for i in 0..100u32 {
            let channel_id = 1;
            let instance_id = (i % 3) + 1; // 实例 1, 2, 3
            buffer.push(DataPoint::new_full(
                channel_id,
                instance_id,
                0,
                i,
                i as f64,
                0,
            ));
        }

        // 按实例过滤
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
