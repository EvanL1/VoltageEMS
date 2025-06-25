use bytes::BytesMut;
use crossbeam::queue::SegQueue;
use smallvec::SmallVec;
use std::sync::Arc;

/// Generic object pool for reusing expensive objects
pub struct ObjectPool<T> {
    queue: Arc<SegQueue<T>>,
    factory: Box<dyn Fn() -> T + Send + Sync>,
    max_size: usize,
}

impl<T> ObjectPool<T> {
    /// Create a new object pool
    ///
    /// # Arguments
    ///
    /// * `factory` - Function to create new objects
    /// * `max_size` - Maximum number of objects to keep in pool
    pub fn new<F>(factory: F, max_size: usize) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            queue: Arc::new(SegQueue::new()),
            factory: Box::new(factory),
            max_size,
        }
    }

    /// Get an object from the pool or create a new one
    pub fn get(&self) -> PooledObject<T> {
        let object = self.queue.pop().unwrap_or_else(|| (self.factory)());
        PooledObject {
            object: Some(object),
            pool: self.queue.clone(),
            max_size: self.max_size,
        }
    }

    /// Get the current size of the pool
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Check if the pool is empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

/// A pooled object that automatically returns to the pool when dropped
pub struct PooledObject<T> {
    object: Option<T>,
    pool: Arc<SegQueue<T>>,
    max_size: usize,
}

impl<T> PooledObject<T> {
    /// Get a reference to the pooled object
    pub fn as_ref(&self) -> &T {
        self.object.as_ref().unwrap()
    }

    /// Get a mutable reference to the pooled object
    pub fn as_mut(&mut self) -> &mut T {
        self.object.as_mut().unwrap()
    }

    /// Take ownership of the object (won't return to pool)
    pub fn take(mut self) -> T {
        self.object.take().unwrap()
    }
}

impl<T> std::ops::Deref for PooledObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<T> std::ops::DerefMut for PooledObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

impl<T> Drop for PooledObject<T> {
    fn drop(&mut self) {
        if let Some(object) = self.object.take() {
            // Only return to pool if not at capacity
            if self.pool.len() < self.max_size {
                self.pool.push(object);
            }
        }
    }
}

/// Specialized buffer pool for byte operations
pub struct BufferPool {
    small_buffers: ObjectPool<BytesMut>,
    large_buffers: ObjectPool<BytesMut>,
    small_capacity: usize,
    large_capacity: usize,
}

impl BufferPool {
    /// Create a new buffer pool with small and large buffer pools
    pub fn new(small_capacity: usize, large_capacity: usize, max_pool_size: usize) -> Self {
        let small_buffers = ObjectPool::new(
            move || BytesMut::with_capacity(small_capacity),
            max_pool_size,
        );

        let large_buffers = ObjectPool::new(
            move || BytesMut::with_capacity(large_capacity),
            max_pool_size / 4, // Keep fewer large buffers
        );

        Self {
            small_buffers,
            large_buffers,
            small_capacity,
            large_capacity,
        }
    }

    /// Get a buffer appropriate for the requested size
    pub fn get_buffer(&self, size_hint: usize) -> PooledObject<BytesMut> {
        if size_hint <= self.small_capacity {
            let mut buffer = self.small_buffers.get();
            buffer.clear();
            buffer
        } else {
            let mut buffer = self.large_buffers.get();
            buffer.clear();
            let buffer_capacity = buffer.capacity();
            if buffer_capacity < size_hint {
                buffer.reserve(size_hint - buffer_capacity);
            }
            buffer
        }
    }

    /// Get a small buffer (typically for protocol headers)
    pub fn get_small_buffer(&self) -> PooledObject<BytesMut> {
        let mut buffer = self.small_buffers.get();
        buffer.clear();
        buffer
    }

    /// Get a large buffer (typically for data payloads)
    pub fn get_large_buffer(&self) -> PooledObject<BytesMut> {
        let mut buffer = self.large_buffers.get();
        buffer.clear();
        buffer
    }
}

/// Global buffer pool instance
static BUFFER_POOL: once_cell::sync::Lazy<BufferPool> = once_cell::sync::Lazy::new(|| {
    BufferPool::new(
        1024, // Small buffer size (1KB)
        8192, // Large buffer size (8KB)
        100,  // Max pool size
    )
});

/// Get the global buffer pool
pub fn get_global_buffer_pool() -> &'static BufferPool {
    &BUFFER_POOL
}

/// Optimized vector pool for small collections
pub type SmallVecPool<T> = ObjectPool<SmallVec<[T; 8]>>;

/// Create a pool for small vectors
pub fn create_small_vec_pool<T: Default + Clone>() -> SmallVecPool<T> {
    ObjectPool::new(|| SmallVec::new(), 50)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_pool_basic() {
        let pool = ObjectPool::new(|| String::from("test"), 5);

        let obj1 = pool.get();
        assert_eq!(*obj1, "test");

        drop(obj1);
        assert_eq!(pool.len(), 1);

        let obj2 = pool.get();
        assert_eq!(*obj2, "test");
    }

    #[test]
    fn test_object_pool_max_size() {
        let pool = ObjectPool::new(|| String::from("test"), 2);

        // Fill pool to capacity
        let obj1 = pool.get();
        let obj2 = pool.get();
        drop(obj1);
        drop(obj2);

        assert_eq!(pool.len(), 2);

        // Try to add more objects - should not exceed max_size
        let obj3 = pool.get();
        drop(obj3);

        // Pool should still be at max capacity
        assert!(pool.len() <= 2);
    }

    #[test]
    fn test_object_pool_empty() {
        let pool = ObjectPool::new(|| 42u32, 5);

        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);

        let obj = pool.get();
        assert_eq!(*obj, 42);
        drop(obj);

        assert!(!pool.is_empty());
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn test_pooled_object_deref() {
        let pool = ObjectPool::new(|| String::from("hello"), 5);
        let mut obj = pool.get();

        // Test deref
        assert_eq!(obj.len(), 5);

        // Test deref_mut
        obj.push_str(" world");
        assert_eq!(obj.len(), 11);
        assert_eq!(*obj, "hello world");
    }

    #[test]
    fn test_pooled_object_take() {
        let pool = ObjectPool::new(|| String::from("test"), 5);
        let obj = pool.get();

        let owned = obj.take();
        assert_eq!(owned, "test");

        // Pool should remain empty since object was taken
        assert!(pool.is_empty());
    }

    #[test]
    fn test_buffer_pool_basic() {
        let buffer_pool = BufferPool::new(1024, 8192, 10);

        let small_buf = buffer_pool.get_small_buffer();
        assert!(small_buf.capacity() >= 1024);
        assert_eq!(small_buf.len(), 0);

        let large_buf = buffer_pool.get_large_buffer();
        assert!(large_buf.capacity() >= 8192);
        assert_eq!(large_buf.len(), 0);
    }

    #[test]
    fn test_buffer_pool_size_hint() {
        let buffer_pool = BufferPool::new(1024, 8192, 10);

        // Request small buffer
        let small_buf = buffer_pool.get_buffer(512);
        assert!(small_buf.capacity() >= 512);

        // Request large buffer
        let large_buf = buffer_pool.get_buffer(4096);
        assert!(large_buf.capacity() >= 4096);
    }

    #[test]
    fn test_buffer_pool_reuse() {
        let buffer_pool = BufferPool::new(1024, 8192, 10);

        {
            let mut buf = buffer_pool.get_small_buffer();
            buf.extend_from_slice(b"test data");
        }

        // Get another buffer - should be cleared
        let buf = buffer_pool.get_small_buffer();
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_global_buffer_pool() {
        let pool = get_global_buffer_pool();

        let buf1 = pool.get_small_buffer();
        let buf2 = pool.get_large_buffer();

        assert!(buf1.capacity() > 0);
        assert!(buf2.capacity() > 0);
        assert!(buf2.capacity() > buf1.capacity());
    }

    #[test]
    fn test_small_vec_pool() {
        let pool = create_small_vec_pool::<u32>();

        let mut vec = pool.get();
        vec.push(1);
        vec.push(2);
        vec.push(3);

        assert_eq!(vec.len(), 3);
        assert_eq!(vec[0], 1);

        drop(vec);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let pool = Arc::new(ObjectPool::new(|| String::from("test"), 10));

        let handles: Vec<_> = (0..5)
            .map(|_| {
                let pool_clone = Arc::clone(&pool);
                thread::spawn(move || {
                    let obj = pool_clone.get();
                    assert_eq!(*obj, "test");
                    // Simulate some work
                    thread::sleep(std::time::Duration::from_millis(1));
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // All objects should be returned to pool
        assert!(pool.len() <= 10);
    }

    #[test]
    fn test_buffer_pool_reserve() {
        let pool = BufferPool::new(1024, 8192, 10);

        // Request buffer larger than small capacity but smaller than large capacity
        let buf = pool.get_buffer(4096);

        // Should get a large buffer with at least 4096 capacity
        assert!(buf.capacity() >= 4096);

        // Return buffer to pool
        drop(buf);

        // Get another buffer and verify it has reasonable capacity
        let buf2 = pool.get_buffer(512);
        assert!(buf2.capacity() >= 512);
    }

    #[test]
    fn test_pooled_object_as_ref_mut() {
        let pool = ObjectPool::new(|| vec![1, 2, 3], 5);
        let mut obj = pool.get();

        // Test as_ref
        assert_eq!(obj.as_ref().len(), 3);

        // Test as_mut
        obj.as_mut().push(4);
        assert_eq!(obj.as_ref().len(), 4);
    }
}
