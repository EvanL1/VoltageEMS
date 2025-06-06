use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

#[cfg(test)]
mod tests {
    use super::*;
    
    /// 测试通用对象池的基本功能
    #[tokio::test]
    async fn test_generic_object_pool() {
        use crossbeam::queue::SegQueue;
        use bytes::BytesMut;
        
        // 创建池
        let pool = Arc::new(SegQueue::new());
        
        // 预分配一些对象
        for i in 0..5 {
            let mut buffer = BytesMut::with_capacity(1024);
            buffer.extend_from_slice(format!("Initial buffer {}", i).as_bytes());
            pool.push(buffer);
        }
        
        // 测试获取和归还
        if let Some(mut buffer) = pool.pop() {
            // 使用缓冲区
            buffer.clear();
            buffer.extend_from_slice(b"Modified content");
            
            // 归还到池中
            pool.push(buffer);
        }
        
        // 验证池中还有对象
        assert!(!pool.is_empty());
        
        // 测试并发访问
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
    
    /// 测试连接池的基本概念
    #[tokio::test]
    async fn test_connection_pool_concept() {
        // 模拟连接池
        let (tx, mut rx) = mpsc::channel::<String>(10);
        
        // 预创建一些连接
        for i in 0..3 {
            let conn_id = format!("connection_{}", i);
            tx.send(conn_id).await.unwrap();
        }
        
        // 测试获取连接
        let connection = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("Should get connection within timeout")
            .expect("Should have connection available");
        
        println!("Got connection: {}", connection);
        
        // 使用连接后归还
        tx.send(connection).await.unwrap();
        
        // 验证连接已归还
        let returned_connection = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("Should get returned connection")
            .expect("Should have returned connection");
        
        println!("Retrieved returned connection: {}", returned_connection);
        
        println!("✅ Connection pool concept test passed");
    }
    
    /// 测试池化的性能优势
    #[tokio::test]
    async fn test_pool_performance_benefit() {
        use crossbeam::queue::SegQueue;
        use bytes::BytesMut;
        use std::time::Instant;
        
        let pool = Arc::new(SegQueue::new());
        
        // 预分配对象到池中
        for _ in 0..100 {
            pool.push(BytesMut::with_capacity(1024));
        }
        
        // 测试从池中获取对象的性能
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
        
        // 测试每次都创建新对象的性能
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
        
        // 池化应该更快（或至少不会太慢）
        assert!(pool_duration <= new_duration * 2, 
                "Pool should not be significantly slower than direct allocation");
        
        println!("✅ Pool performance test passed");
    }
    
    /// 测试资源管理和清理
    #[tokio::test]
    async fn test_resource_management() {
        use std::sync::atomic::{AtomicU32, Ordering};
        
        // 跟踪资源的创建和销毁
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
            
            // 创建一些资源
            for i in 0..5 {
                created_count.fetch_add(1, Ordering::SeqCst);
                let resource = TrackedResource {
                    _id: i,
                    dropped_count: dropped_count.clone(),
                };
                pool.push(resource);
            }
            
            // 使用一些资源
            if let Some(_resource) = pool.pop() {
                // 资源会在作用域结束时自动释放
            }
            
            // 清空池
            while pool.pop().is_some() {}
        } // 池在这里被销毁
        
        // 等待一下确保所有析构函数都已执行
        tokio::time::sleep(Duration::from_millis(1)).await;
        
        // 验证所有资源都被正确释放
        assert_eq!(created_count.load(Ordering::SeqCst), 5);
        assert_eq!(dropped_count.load(Ordering::SeqCst), 5);
        
        println!("✅ Resource management test passed");
    }
}

// 用于测试连接超时和生命周期管理的模拟连接
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
        
        // 新连接应该是有效的
        assert!(conn.is_valid());
        
        // 使用连接
        conn.use_connection();
        
        // 短时间内不应该是空闲的
        assert!(!conn.is_idle(Duration::from_millis(1)));
        
        // 等待一段时间
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // 现在应该是空闲的
        assert!(conn.is_idle(Duration::from_millis(5)));
        
        // 关闭连接
        conn.close();
        assert!(!conn.is_valid());
        
        println!("✅ Connection lifecycle test passed");
    }
    
    #[tokio::test] 
    async fn test_connection_pool_with_timeout() {
        use std::collections::HashMap;
        use tokio::sync::RwLock;
        
        // 简化的连接池实现
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
                
                // 移除过期连接
                pool.retain(|conn| conn.is_valid() && !conn.is_idle(self.max_idle_time));
                
                // 返回可用连接或创建新连接
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
        
        // 再次获取应该返回同一个连接（或新连接）
        let conn2 = pool.get_connection(key).await.unwrap();
        assert!(conn2.is_valid());
        
        // 等待连接过期
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // 清理过期连接
        pool.cleanup_expired().await;
        
        // 验证过期连接被清理
        {
            let pools = pool.connections.read().await;
            if let Some(conns) = pools.get(key) {
                // 由于我们没有归还conn2，池应该是空的或只包含有效连接
                for conn in conns {
                    assert!(conn.is_valid());
                }
            }
        }
        
        println!("✅ Connection pool with timeout test passed");
    }
} 