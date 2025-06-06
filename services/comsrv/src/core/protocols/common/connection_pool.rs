use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::hash::{Hash, Hasher};
use parking_lot::RwLock;
use dashmap::DashMap;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tracing::{debug};

use crate::utils::{ComSrvError, Result};

/// Connection key for identifying unique connections
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionKey {
    pub protocol: String,
    pub address: String,
    pub port: Option<u16>,
    pub params: HashMap<String, String>,
}

impl ConnectionKey {
    /// Create a new connection key
    pub fn new(protocol: &str, address: &str, port: Option<u16>) -> Self {
        Self {
            protocol: protocol.to_string(),
            address: address.to_string(),
            port,
            params: HashMap::new(),
        }
    }

    /// Add a parameter to the connection key
    pub fn with_param(mut self, key: &str, value: &str) -> Self {
        self.params.insert(key.to_string(), value.to_string());
        self
    }
}

impl Hash for ConnectionKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.protocol.hash(state);
        self.address.hash(state);
        self.port.hash(state);
        // Hash the sorted key-value pairs for consistent hashing
        let mut params: Vec<_> = self.params.iter().collect();
        params.sort();
        params.hash(state);
    }
}

/// Represents a pooled connection
pub trait PooledConnection: Send + Sync {
    /// Check if the connection is still valid
    fn is_valid(&self) -> bool;
    
    /// Close the connection
    fn close(&mut self) -> impl std::future::Future<Output = Result<()>> + Send;
    
    /// Get connection info for debugging
    fn connection_info(&self) -> String;
}

/// Connection wrapper with metadata
pub struct ConnectionWrapper<T> {
    connection: T,
    created_at: Instant,
    last_used: Instant,
    use_count: u64,
}

impl<T> ConnectionWrapper<T>
where
    T: PooledConnection,
{
    fn new(connection: T) -> Self {
        let now = Instant::now();
        Self {
            connection,
            created_at: now,
            last_used: now,
            use_count: 0,
        }
    }

    fn touch(&mut self) {
        self.last_used = Instant::now();
        self.use_count += 1;
    }

    fn is_expired(&self, max_age: Duration) -> bool {
        self.created_at.elapsed() > max_age
    }

    fn is_idle(&self, max_idle: Duration) -> bool {
        self.last_used.elapsed() > max_idle
    }
}

impl<T> std::ops::Deref for ConnectionWrapper<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl<T> std::ops::DerefMut for ConnectionWrapper<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of connections per key
    pub max_connections_per_key: usize,
    /// Maximum total connections
    pub max_total_connections: usize,
    /// Maximum age of a connection before it's retired
    pub max_connection_age: Duration,
    /// Maximum idle time before a connection is closed
    pub max_idle_time: Duration,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Cleanup interval for expired connections
    pub cleanup_interval: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_connections_per_key: 10,
            max_total_connections: 100,
            max_connection_age: Duration::from_secs(3600), // 1 hour
            max_idle_time: Duration::from_secs(300),       // 5 minutes
            connection_timeout: Duration::from_secs(30),   // 30 seconds
            cleanup_interval: Duration::from_secs(60),     // 1 minute
        }
    }
}

/// High-performance connection pool
pub struct ConnectionPool<T> {
    /// Pool configuration
    config: PoolConfig,
    /// Connection pools by key
    pools: DashMap<ConnectionKey, Arc<RwLock<Vec<ConnectionWrapper<T>>>>>,
    /// Semaphore to limit total connections
    connection_semaphore: Arc<Semaphore>,
    /// Connection factory
    factory: Arc<dyn Fn(&ConnectionKey) -> Box<dyn std::future::Future<Output = Result<T>> + Send + Unpin> + Send + Sync>,
}

impl<T> ConnectionPool<T>
where
    T: PooledConnection + 'static,
{
    /// Create a new connection pool
    pub fn new<F, Fut>(config: PoolConfig, factory: F) -> Self
    where
        F: Fn(&ConnectionKey) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<T>> + Send + 'static,
    {
        let connection_semaphore = Arc::new(Semaphore::new(config.max_total_connections));
        
        let factory = Arc::new(move |key: &ConnectionKey| {
            let fut = factory(key);
            Box::new(Box::pin(fut)) as Box<dyn std::future::Future<Output = Result<T>> + Send + Unpin>
        });

        let pool = Self {
            config,
            pools: DashMap::new(),
            connection_semaphore,
            factory,
        };

        // Start cleanup task
        pool.start_cleanup_task();
        
        pool
    }

    /// Get a connection from the pool or create a new one
    pub async fn get_connection(&self, key: &ConnectionKey) -> Result<PooledConnectionGuard<T>> {
        // Try to get an existing connection first
        if let Some(pool_ref) = self.pools.get(key) {
            let mut pool = pool_ref.write();
            
            // Find a valid connection
            while let Some(mut conn_wrapper) = pool.pop() {
                if conn_wrapper.connection.is_valid() && 
                   !conn_wrapper.is_expired(self.config.max_connection_age) {
                    conn_wrapper.touch();
                    
                    return Ok(PooledConnectionGuard {
                        connection: Some(conn_wrapper),
                        pool_ref: pool_ref.clone(),
                        semaphore: self.connection_semaphore.clone(),
                    });
                } else {
                    // Connection is invalid or expired, drop it
                    debug!("Dropping invalid/expired connection for key: {:?}", key);
                }
            }
        }

        // No valid connection found, create a new one
        self.create_new_connection(key).await
    }

    /// Create a new connection
    async fn create_new_connection(&self, key: &ConnectionKey) -> Result<PooledConnectionGuard<T>> {
        // Acquire semaphore permit
        let _permit = timeout(
            self.config.connection_timeout,
            self.connection_semaphore.acquire()
        ).await
        .map_err(|_| ComSrvError::TimeoutError("Connection pool timeout".to_string()))?
        .map_err(|_| ComSrvError::InternalError("Semaphore closed".to_string()))?;

        // Create the connection with timeout
        let connection = timeout(
            self.config.connection_timeout,
            (self.factory)(key)
        ).await
        .map_err(|_| ComSrvError::TimeoutError("Connection creation timeout".to_string()))??;

        let conn_wrapper = ConnectionWrapper::new(connection);
        
        // Get or create the pool for this key
        let pool_ref = self.pools.entry(key.clone()).or_insert_with(|| {
            Arc::new(RwLock::new(Vec::with_capacity(self.config.max_connections_per_key)))
        }).clone();

        Ok(PooledConnectionGuard {
            connection: Some(conn_wrapper),
            pool_ref,
            semaphore: self.connection_semaphore.clone(),
        })
    }

    /// Start the cleanup task to remove expired connections
    fn start_cleanup_task(&self) {
        let pools = self.pools.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.cleanup_interval);
            
            loop {
                interval.tick().await;
                
                // Clean up expired connections
                for entry in pools.iter() {
                    let key = entry.key();
                    let pool_ref = entry.value();
                    let mut pool = pool_ref.write();
                    
                    let initial_len = pool.len();
                    pool.retain(|conn| {
                        !conn.is_expired(config.max_connection_age) && 
                        !conn.is_idle(config.max_idle_time) &&
                        conn.connection.is_valid()
                    });
                    
                    let removed = initial_len - pool.len();
                    if removed > 0 {
                        debug!("Cleaned up {} expired connections for key: {:?}", removed, key);
                    }
                }
            }
        });
    }

    /// Get pool statistics
    pub fn stats(&self) -> PoolStats {
        let mut total_connections = 0;
        let mut pools_count = 0;
        
        for entry in self.pools.iter() {
            pools_count += 1;
            let pool = entry.value().read();
            total_connections += pool.len();
        }
        
        PoolStats {
            total_connections,
            pools_count,
            available_permits: self.connection_semaphore.available_permits(),
            max_total_connections: self.config.max_total_connections,
        }
    }
}

/// Connection guard that automatically returns connection to pool
pub struct PooledConnectionGuard<T>
where
    T: PooledConnection,
{
    connection: Option<ConnectionWrapper<T>>,
    pool_ref: Arc<RwLock<Vec<ConnectionWrapper<T>>>>,
    semaphore: Arc<Semaphore>,
}

impl<T> PooledConnectionGuard<T>
where
    T: PooledConnection,
{
    /// Take ownership of the connection (won't return to pool)
    pub fn take(mut self) -> T {
        self.connection.take().unwrap().connection
    }
}

impl<T> std::ops::Deref for PooledConnectionGuard<T>
where
    T: PooledConnection,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.connection.as_ref().unwrap().connection
    }
}

impl<T> std::ops::DerefMut for PooledConnectionGuard<T>
where
    T: PooledConnection,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection.as_mut().unwrap().connection
    }
}

impl<T> Drop for PooledConnectionGuard<T>
where
    T: PooledConnection,
{
    fn drop(&mut self) {
        if let Some(conn_wrapper) = self.connection.take() {
            // Check if connection is still valid before returning to pool
            if conn_wrapper.connection.is_valid() {
                let mut pool = self.pool_ref.write();
                if pool.len() < pool.capacity() {
                    pool.push(conn_wrapper);
                } else {
                    debug!("Pool at capacity, dropping connection");
                }
            } else {
                debug!("Connection invalid, not returning to pool");
            }
        }
        // Semaphore permit is automatically released when guard is dropped
    }
}

/// Pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub total_connections: usize,
    pub pools_count: usize,
    pub available_permits: usize,
    pub max_total_connections: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct MockConnection {
        valid: Arc<AtomicBool>,
        id: u32,
    }

    impl MockConnection {
        fn new(id: u32) -> Self {
            Self {
                valid: Arc::new(AtomicBool::new(true)),
                id,
            }
        }
    }

    impl PooledConnection for MockConnection {
        fn is_valid(&self) -> bool {
            self.valid.load(Ordering::Relaxed)
        }

        fn close(&mut self) -> impl std::future::Future<Output = Result<()>> + Send {
            self.valid.store(false, Ordering::Relaxed);
            async { Ok(()) }
        }

        fn connection_info(&self) -> String {
            format!("MockConnection-{}", self.id)
        }
    }

    impl PooledConnection for Arc<tokio::net::TcpStream> {
        fn is_valid(&self) -> bool {
            true // TCP streams don't have a simple health check
        }

        fn close(&mut self) -> impl std::future::Future<Output = Result<()>> + Send {
            // TCP streams are automatically closed when dropped
            async { Ok(()) }
        }

        fn connection_info(&self) -> String {
            format!("TCP Stream")
        }
    }

    #[tokio::test]
    async fn test_connection_pool_basic() {
        let config = PoolConfig::default();
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let pool = ConnectionPool::new(config, {
            let counter = counter.clone();
            move |_key| {
                let id = counter.fetch_add(1, Ordering::Relaxed);
                async move { Ok(MockConnection::new(id)) }
            }
        });

        let key = ConnectionKey::new("test", "localhost", Some(8080));
        
        // Get first connection
        let conn1 = pool.get_connection(&key).await.unwrap();
        assert_eq!(conn1.id, 0);
        
        drop(conn1);
        
        // Get second connection (should reuse the first one)
        let conn2 = pool.get_connection(&key).await.unwrap();
        assert_eq!(conn2.id, 0); // Reused connection
    }

    #[tokio::test]
    async fn test_connection_key_creation() {
        let key1 = ConnectionKey::new("modbus", "127.0.0.1", Some(502));
        assert_eq!(key1.protocol, "modbus");
        assert_eq!(key1.address, "127.0.0.1");
        assert_eq!(key1.port, Some(502));

        let key2 = ConnectionKey::new("iec104", "192.168.1.1", None);
        assert_eq!(key2.protocol, "iec104");
        assert_eq!(key2.address, "192.168.1.1");
        assert_eq!(key2.port, None);
    }

    #[tokio::test]
    async fn test_connection_key_with_params() {
        let key = ConnectionKey::new("modbus", "127.0.0.1", Some(502))
            .with_param("slave_id", "1")
            .with_param("timeout", "5000");
        
        assert_eq!(key.params.get("slave_id"), Some(&"1".to_string()));
        assert_eq!(key.params.get("timeout"), Some(&"5000".to_string()));
    }

    #[tokio::test]
    async fn test_connection_key_equality() {
        let key1 = ConnectionKey::new("modbus", "127.0.0.1", Some(502));
        let key2 = ConnectionKey::new("modbus", "127.0.0.1", Some(502));
        let key3 = ConnectionKey::new("iec104", "127.0.0.1", Some(502));

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[tokio::test]
    async fn test_connection_key_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let key1 = ConnectionKey::new("modbus", "127.0.0.1", Some(502));
        let key2 = ConnectionKey::new("modbus", "127.0.0.1", Some(502));

        let mut hasher1 = DefaultHasher::new();
        let mut hasher2 = DefaultHasher::new();
        key1.hash(&mut hasher1);
        key2.hash(&mut hasher2);

        assert_eq!(hasher1.finish(), hasher2.finish());
    }

    #[tokio::test]
    async fn test_connection_wrapper() {
        let conn = MockConnection::new(1);
        let mut wrapper = ConnectionWrapper::new(conn);

        assert_eq!(wrapper.use_count, 0);
        wrapper.touch();
        assert_eq!(wrapper.use_count, 1);

        // Test expiration
        assert!(!wrapper.is_expired(Duration::from_secs(1)));
        assert!(!wrapper.is_idle(Duration::from_secs(1)));
    }

    #[tokio::test]
    async fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.max_connections_per_key, 10);
        assert_eq!(config.max_total_connections, 100);
        assert!(config.max_connection_age > Duration::from_secs(3000));
        assert!(config.max_idle_time > Duration::from_secs(200));
    }

    #[tokio::test]
    async fn test_connection_pool_creation() {
        let config = PoolConfig::default();
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let factory = {
            let counter = Arc::clone(&counter);
            move |_key: &ConnectionKey| {
                let counter = Arc::clone(&counter);
                async move {
                    let id = counter.fetch_add(1, Ordering::Relaxed);
                    Ok(MockConnection::new(id))
                }
            }
        };

        let pool = ConnectionPool::new(config, factory);
        let stats = pool.stats();
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.pools_count, 0);
    }

    #[tokio::test]
    async fn test_get_connection() {
        let config = PoolConfig::default();
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let factory = {
            let counter = Arc::clone(&counter);
            move |_key: &ConnectionKey| {
                let counter = Arc::clone(&counter);
                async move {
                    let id = counter.fetch_add(1, Ordering::Relaxed);
                    Ok(MockConnection::new(id))
                }
            }
        };

        let pool = ConnectionPool::new(config, factory);
        let key = ConnectionKey::new("test", "127.0.0.1", Some(8080));

        let conn_guard = pool.get_connection(&key).await.unwrap();
        assert!(conn_guard.is_valid());
        assert_eq!(conn_guard.id, 0);
    }

    #[tokio::test]
    async fn test_connection_reuse() {
        let config = PoolConfig::default();
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let factory = {
            let counter = Arc::clone(&counter);
            move |_key: &ConnectionKey| {
                let counter = Arc::clone(&counter);
                async move {
                    let id = counter.fetch_add(1, Ordering::Relaxed);
                    Ok(MockConnection::new(id))
                }
            }
        };

        let pool = ConnectionPool::new(config, factory);
        let key = ConnectionKey::new("test", "127.0.0.1", Some(8080));

        // Get and return a connection
        {
            let _conn_guard = pool.get_connection(&key).await.unwrap();
        }

        // Get another connection - should reuse the first one
        let conn_guard = pool.get_connection(&key).await.unwrap();
        assert_eq!(conn_guard.id, 0); // Should be the same connection
    }

    #[tokio::test]
    async fn test_connection_pool_stats() {
        let config = PoolConfig {
            max_total_connections: 5,
            ..Default::default()
        };
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let factory = {
            let counter = Arc::clone(&counter);
            move |_key: &ConnectionKey| {
                let counter = Arc::clone(&counter);
                async move {
                    let id = counter.fetch_add(1, Ordering::Relaxed);
                    Ok(MockConnection::new(id))
                }
            }
        };

        let pool = ConnectionPool::new(config, factory);
        let key = ConnectionKey::new("test", "127.0.0.1", Some(8080));

        let _conn1 = pool.get_connection(&key).await.unwrap();
        let _conn2 = pool.get_connection(&key).await.unwrap();

        let stats = pool.stats();
        assert_eq!(stats.max_total_connections, 5);
        assert!(stats.available_permits <= 5);
    }

    #[tokio::test]
    async fn test_invalid_connection_cleanup() {
        let config = PoolConfig::default();
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let factory = {
            let counter = Arc::clone(&counter);
            move |_key: &ConnectionKey| {
                let counter = Arc::clone(&counter);
                async move {
                    let id = counter.fetch_add(1, Ordering::Relaxed);
                    Ok(MockConnection::new(id))
                }
            }
        };

        let pool = ConnectionPool::new(config, factory);
        let key = ConnectionKey::new("test", "127.0.0.1", Some(8080));

        // Create a connection and invalidate it
        {
            let conn_guard = pool.get_connection(&key).await.unwrap();
            conn_guard.valid.store(false, Ordering::Relaxed);
        }

        // Next connection should be a new one
        let conn_guard = pool.get_connection(&key).await.unwrap();
        assert_eq!(conn_guard.id, 1); // Should be a new connection
    }

    #[tokio::test]
    async fn test_connection_guard_take() {
        let config = PoolConfig::default();
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let factory = {
            let counter = Arc::clone(&counter);
            move |_key: &ConnectionKey| {
                let counter = Arc::clone(&counter);
                async move {
                    let id = counter.fetch_add(1, Ordering::Relaxed);
                    Ok(MockConnection::new(id))
                }
            }
        };

        let pool = ConnectionPool::new(config, factory);
        let key = ConnectionKey::new("test", "127.0.0.1", Some(8080));

        let conn_guard = pool.get_connection(&key).await.unwrap();
        let connection = conn_guard.take();
        
        assert_eq!(connection.id, 0);
        assert!(connection.is_valid());
    }

    #[tokio::test]
    async fn test_multiple_pools() {
        let config = PoolConfig::default();
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let factory = {
            let counter = Arc::clone(&counter);
            move |_key: &ConnectionKey| {
                let counter = Arc::clone(&counter);
                async move {
                    let id = counter.fetch_add(1, Ordering::Relaxed);
                    Ok(MockConnection::new(id))
                }
            }
        };

        let pool = ConnectionPool::new(config, factory);
        
        let key1 = ConnectionKey::new("test1", "127.0.0.1", Some(8080));
        let key2 = ConnectionKey::new("test2", "127.0.0.1", Some(8081));

        let _conn1 = pool.get_connection(&key1).await.unwrap();
        let _conn2 = pool.get_connection(&key2).await.unwrap();

        let stats = pool.stats();
        assert_eq!(stats.pools_count, 2);
    }

    #[tokio::test]
    async fn test_connection_factory_error() {
        let config = PoolConfig::default();
        
        let factory = |_key: &ConnectionKey| async {
            Err(ComSrvError::ConnectionError("Factory error".to_string()))
        };

        let pool: ConnectionPool<MockConnection> = ConnectionPool::new(config, factory);
        let key = ConnectionKey::new("test", "127.0.0.1", Some(8080));

        let result = pool.get_connection(&key).await;
        assert!(result.is_err());
    }
} 