use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

#[cfg(test)]
mod tests {
    use super::*;
    
    /// Test basic functionality of a generic object pool
    #[tokio::test]
    async fn test_generic_object_pool() {
        use crossbeam::queue::SegQueue;
        use bytes::BytesMut;
        
        // Create the pool
        let pool = Arc::new(SegQueue::new());
        
        // Preallocate some objects
        for i in 0..5 {
            let mut buffer = BytesMut::with_capacity(1024);
            buffer.extend_from_slice(format!("Initial buffer {}", i).as_bytes());
            pool.push(buffer);
        }
        
        // Test acquire and release
        if let Some(mut buffer) = pool.pop() {
            // Use the buffer
            buffer.clear();
            buffer.extend_from_slice(b"Modified content");
            
            // Return to the pool
            pool.push(buffer);
        }
        
        // Verify pool still has objects
        assert!(!pool.is_empty());
        
        // Test concurrent access
        let pool_clone = pool.clone();
        let handle = tokio::spawn(async move {
            if let Some(buffer) = pool_clone.pop() {
                tokio::time::sleep(Duration::from_millis(10)).await;
                pool_clone.push(buffer);
            }
        });
        
        handle.await.unwrap();
        
        println!("✅ Generic object pool test passed");
    }
    
    /// Test basic connection pool concept
    #[tokio::test]
    async fn test_connection_pool_concept() {
        // Simulate a connection pool
        let (tx, mut rx) = mpsc::channel::<String>(10);
        
        // Pre-create some connections
        for i in 0..3 {
            let conn_id = format!("connection_{}", i);
            tx.send(conn_id).await.unwrap();
        }
        
        // Test acquiring a connection
        let connection = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("Should get connection within timeout")
            .expect("Should have connection available");
        
        println!("Got connection: {}", connection);
        
        // Return the connection after use
        tx.send(connection).await.unwrap();
        
        // Verify the connection was returned
        let returned_connection = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("Should get returned connection")
            .expect("Should have returned connection");
        
        println!("Retrieved returned connection: {}", returned_connection);
        
        println!("✅ Connection pool concept test passed");
    }
    
    /// Test performance benefits of pooling
    #[tokio::test]
    async fn test_pool_performance_benefit() {
        use crossbeam::queue::SegQueue;
        use bytes::BytesMut;
        use std::time::Instant;
        
        let pool = Arc::new(SegQueue::new());
        
        // Preallocate objects into the pool
        for _ in 0..100 {
            pool.push(BytesMut::with_capacity(1024));
        }
        
        // Measure performance of fetching from pool
        let start = Instant::now();
        for _ in 0..1000 {
            if let Some(mut buffer) = pool.pop() {
                // 模拟使用
                buffer.clear();
                buffer.extend_from_slice(b"test data");
                // 归还
                pool.push(buffer);
            } else {
                // 如果池为空，创建新对象
                let buffer = BytesMut::with_capacity(1024);
                pool.push(buffer);
            }
        }
        let pool_duration = start.elapsed();
        
        // Measure performance of creating new objects each time
        let start = Instant::now();
        let mut buffers = Vec::new();
        for _ in 0..1000 {
            let mut buffer = BytesMut::with_capacity(1024);
            buffer.extend_from_slice(b"test data");
            buffers.push(buffer);
        }
        let new_duration = start.elapsed();
        
        println!("Pool-based allocation: {:?}", pool_duration);
        println!("New allocation each time: {:?}", new_duration);
        
        // Pooling should be faster (or at least not much slower)
        assert!(pool_duration <= new_duration * 2, 
                "Pool should not be significantly slower than direct allocation");
        
        println!("✅ Pool performance test passed");
    }
    
    /// Test resource management and cleanup
    #[tokio::test]
    async fn test_resource_management() {
        use std::sync::atomic::{AtomicU32, Ordering};
        
        // Track resource creation and drop
        let created_count = Arc::new(AtomicU32::new(0));
        let dropped_count = Arc::new(AtomicU32::new(0));
        
        struct TrackedResource {
            _id: u32,
            dropped_count: Arc<AtomicU32>,
        }
        
        impl Drop for TrackedResource {
            fn drop(&mut self) {
                self.dropped_count.fetch_add(1, Ordering::SeqCst);
            }
        }
        
        {
            let pool = crossbeam::queue::SegQueue::new();
            
            // Create some resources
            for i in 0..5 {
                created_count.fetch_add(1, Ordering::SeqCst);
                let resource = TrackedResource {
                    _id: i,
                    dropped_count: dropped_count.clone(),
                };
                pool.push(resource);
            }
            
            // Use some resources
            if let Some(_resource) = pool.pop() {
                // Resource drops when scope ends
            }
            
            // Empty the pool
            while pool.pop().is_some() {}
        } // 池在这里被销毁
        
        // Wait to ensure destructors have run
        tokio::time::sleep(Duration::from_millis(1)).await;
        
        // Verify all resources were released
        assert_eq!(created_count.load(Ordering::SeqCst), 5);
        assert_eq!(dropped_count.load(Ordering::SeqCst), 5);
        
        println!("✅ Resource management test passed");
    }
}

// Mock connection for timeout and lifecycle testing
#[derive(Debug, Clone)]
struct MockConnection {
    id: String,
    created_at: std::time::Instant,
    last_used: std::time::Instant,
    valid: bool,
}

impl MockConnection {
    fn new(id: &str) -> Self {
        let now = std::time::Instant::now();
        Self {
            id: id.to_string(),
            created_at: now,
            last_used: now,
            valid: true,
        }
    }
    
    fn is_valid(&self) -> bool {
        self.valid
    }
    
    fn is_idle(&self, duration: Duration) -> bool {
        self.last_used.elapsed() > duration
    }
    
    fn use_connection(&mut self) {
        self.last_used = std::time::Instant::now();
    }
    
    fn close(&mut self) {
        self.valid = false;
    }
}

#[cfg(test)]
mod connection_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_connection_lifecycle() {
        let mut conn = MockConnection::new("test_conn");
        
        // Newly created connection should be valid
        assert!(conn.is_valid());
        
        // Use the connection
        conn.use_connection();
        
        // Should not be idle shortly after use
        assert!(!conn.is_idle(Duration::from_millis(1)));
        
        // Wait for a short period
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Now it should be idle
        assert!(conn.is_idle(Duration::from_millis(5)));
        
        // Close the connection
        conn.close();
        assert!(!conn.is_valid());
        
        println!("✅ Connection lifecycle test passed");
    }
    
    #[tokio::test] 
    async fn test_connection_pool_with_timeout() {
        use std::collections::HashMap;
        use tokio::sync::RwLock;
        
        // Simplified connection pool implementation
        struct SimpleConnectionPool {
            connections: RwLock<HashMap<String, Vec<MockConnection>>>,
            max_idle_time: Duration,
        }
        
        impl SimpleConnectionPool {
            fn new() -> Self {
                Self {
                    connections: RwLock::new(HashMap::new()),
                    max_idle_time: Duration::from_millis(100),
                }
            }
            
            async fn get_connection(&self, key: &str) -> Option<MockConnection> {
                let mut pools = self.connections.write().await;
                let pool = pools.entry(key.to_string()).or_insert_with(Vec::new);
                
                // Remove expired connections
                pool.retain(|conn| conn.is_valid() && !conn.is_idle(self.max_idle_time));
                
                // Return available connection or create a new one
                pool.pop().or_else(|| Some(MockConnection::new(&format!("{}_{}", key, pools.len()))))
            }
            
            async fn return_connection(&self, key: &str, mut conn: MockConnection) {
                if conn.is_valid() {
                    conn.use_connection();
                    let mut pools = self.connections.write().await;
                    let pool = pools.entry(key.to_string()).or_insert_with(Vec::new);
                    pool.push(conn);
                }
            }
            
            async fn cleanup_expired(&self) {
                let mut pools = self.connections.write().await;
                for pool in pools.values_mut() {
                    pool.retain(|conn| conn.is_valid() && !conn.is_idle(self.max_idle_time));
                }
            }
        }
        
        let pool = SimpleConnectionPool::new();
        let key = "test_pool";
        
        // 获取连接
        let conn = pool.get_connection(key).await.unwrap();
        assert!(conn.is_valid());
        
        // 归还连接
        pool.return_connection(key, conn).await;
        
        // Acquire again; should return the same or a new connection
        let conn2 = pool.get_connection(key).await.unwrap();
        assert!(conn2.is_valid());
        
        // Wait for the connection to expire
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Clean up expired connections
        pool.cleanup_expired().await;
        
        // Verify expired connections were removed
        {
            let pools = pool.connections.read().await;
            if let Some(conns) = pools.get(key) {
                // Because conn2 wasn't returned the pool should be empty or only contain valid connections
                for conn in conns {
                    assert!(conn.is_valid());
                }
            }
        }
        
        println!("✅ Connection pool with timeout test passed");
    }
} 