use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

#[cfg(test)]
mod tests {
    use super::*;
    
    /// test basic functionality of a generic object pool
    #[tokio::test]
    async fn test_generic_object_pool() {
        use crossbeam::queue::SegQueue;
        use bytes::BytesMut;
        
        // create the pool
        let pool = Arc::new(SegQueue::new());
        
        // preallocate some objects
        for i in 0..5 {
            let mut buffer = BytesMut::with_capacity(1024);
            buffer.extend_from_slice(format!("Initial buffer {}", i).as_bytes());
            pool.push(buffer);
        }
        
        // test acquiring and returning
        if let Some(mut buffer) = pool.pop() {
            // use the buffer
            buffer.clear();
            buffer.extend_from_slice(b"Modified content");
            
            // return to the pool
            pool.push(buffer);
        }
        
        // verify objects remain in the pool
        assert!(!pool.is_empty());
        
        // test concurrent access
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
    
    /// test the basic concept of a connection pool
    #[tokio::test]
    async fn test_connection_pool_concept() {
        // simulate a connection pool
        let (tx, mut rx) = mpsc::channel::<String>(10);
        
        // pre-create some connections
        for i in 0..3 {
            let conn_id = format!("connection_{}", i);
            tx.send(conn_id).await.unwrap();
        }
        
        // test acquiring a connection
        let connection = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("Should get connection within timeout")
            .expect("Should have connection available");
        
        println!("Got connection: {}", connection);
        
        // return the connection after use
        tx.send(connection).await.unwrap();
        
        // verify the connection was returned
        let returned_connection = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("Should get returned connection")
            .expect("Should have returned connection");
        
        println!("Retrieved returned connection: {}", returned_connection);
        
        println!("✅ Connection pool concept test passed");
    }
    
    /// test the performance advantages of pooling
    #[tokio::test]
    async fn test_pool_performance_benefit() {
        use crossbeam::queue::SegQueue;
        use bytes::BytesMut;
        use std::time::Instant;
        
        let pool = Arc::new(SegQueue::new());
        
        // preallocate objects into the pool
        for _ in 0..100 {
            pool.push(BytesMut::with_capacity(1024));
        }
        
        // measure performance of getting objects from the pool
        let start = Instant::now();
        for _ in 0..1000 {
            if let Some(mut buffer) = pool.pop() {
                // simulate usage
                buffer.clear();
                buffer.extend_from_slice(b"test data");
                // return
                pool.push(buffer);
            } else {
                // create a new object if the pool is empty
                let buffer = BytesMut::with_capacity(1024);
                pool.push(buffer);
            }
        }
        let pool_duration = start.elapsed();
        
        // measure performance when creating new objects each time
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
        
        // pooling should be faster (or at least not much slower)
        assert!(pool_duration <= new_duration * 2, 
                "Pool should not be significantly slower than direct allocation");
        
        println!("✅ Pool performance test passed");
    }
    
    /// test resource management and cleanup
    #[tokio::test]
    async fn test_resource_management() {
        use std::sync::atomic::{AtomicU32, Ordering};
        
        // track creation and destruction of resources
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
            
            // create some resources
            for i in 0..5 {
                created_count.fetch_add(1, Ordering::SeqCst);
                let resource = TrackedResource {
                    _id: i,
                    dropped_count: dropped_count.clone(),
                };
                pool.push(resource);
            }
            
            // use some resources
            if let Some(_resource) = pool.pop() {
                // resource will be automatically released at end of scope
            }
            
            // empty the pool
            while pool.pop().is_some() {}
        } // pool is dropped here
        
        // wait a bit to ensure all destructors have run
        tokio::time::sleep(Duration::from_millis(1)).await;
        
        // verify all resources were correctly released
        assert_eq!(created_count.load(Ordering::SeqCst), 5);
        assert_eq!(dropped_count.load(Ordering::SeqCst), 5);
        
        println!("✅ Resource management test passed");
    }
}

// Mock connection used for timeout and lifecycle management tests
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
        
        // newly created connection should be valid
        assert!(conn.is_valid());
        
        // use the connection
        conn.use_connection();
        
        // should not be idle shortly after use
        assert!(!conn.is_idle(Duration::from_millis(1)));
        
        // wait for a while
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // now it should be idle
        assert!(conn.is_idle(Duration::from_millis(5)));
        
        // close the connection
        conn.close();
        assert!(!conn.is_valid());
        
        println!("✅ Connection lifecycle test passed");
    }
    
    #[tokio::test] 
    async fn test_connection_pool_with_timeout() {
        use std::collections::HashMap;
        use tokio::sync::RwLock;
        
        // simplified connection pool implementation
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
                
                // remove expired connections
                pool.retain(|conn| conn.is_valid() && !conn.is_idle(self.max_idle_time));
                
                // return an available connection or create a new one
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
        
        // obtain a connection
        let conn = pool.get_connection(key).await.unwrap();
        assert!(conn.is_valid());
        
        // return the connection
        pool.return_connection(key, conn).await;
        
        // getting again should return the same or a new connection
        let conn2 = pool.get_connection(key).await.unwrap();
        assert!(conn2.is_valid());
        
        // wait for the connection to expire
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // clean up expired connections
        pool.cleanup_expired().await;
        
        // verify expired connections were cleaned up
        {
            let pools = pool.connections.read().await;
            if let Some(conns) = pools.get(key) {
                // since conn2 wasn't returned, the pool should be empty or contain only valid connections
                for conn in conns {
                    assert!(conn.is_valid());
                }
            }
        }
        
        println!("✅ Connection pool with timeout test passed");
    }
} 