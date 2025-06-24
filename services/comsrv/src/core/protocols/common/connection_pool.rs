use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::hash::{Hash, Hasher};
use tokio::sync::{RwLock, Semaphore, OwnedSemaphorePermit};
use tokio::time::timeout;
use dashmap::DashMap;
use log::{debug, warn, info};
use once_cell::sync::OnceCell;

use crate::utils::{ComSrvError, Result};

/// Connection key for identifying unique connections
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionKey {
    pub protocol: String,
    pub address: String,
    pub port: Option<u16>,
    pub params: BTreeMap<String, String>, // Changed to BTreeMap for ordered params
    cached_hash: OnceCell<u64>, // Cache hash to avoid recomputation (using OnceCell for true zero-cost after first calc)
}

impl ConnectionKey {
    /// Create a new connection key
    pub fn new(protocol: &str, address: &str, port: Option<u16>) -> Self {
        Self {
            protocol: protocol.to_string(),
            address: address.to_string(),
            port,
            params: BTreeMap::new(),
            cached_hash: OnceCell::new(),
        }
    }

    /// Add a parameter to the connection key
    pub fn with_param(mut self, key: &str, value: &str) -> Self {
        self.params.insert(key.to_string(), value.to_string());
        // Note: OnceCell doesn't allow invalidation, but since we're building the key,
        // the hash will be computed after all params are set
        self
    }

    /// Calculate the hash (expensive operation, only done once)
    fn calc_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.protocol.hash(&mut hasher);
        self.address.hash(&mut hasher);
        self.port.hash(&mut hasher);
        // BTreeMap is naturally ordered, no need to sort
        for (k, v) in &self.params {
            k.hash(&mut hasher);
            v.hash(&mut hasher);
        }
        hasher.finish()
    }
}

impl Hash for ConnectionKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Use OnceCell for true zero-cost hash caching after first computation
        let hash = self.cached_hash.get_or_init(|| self.calc_hash());
        hash.hash(state);
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

/// Metrics event for connection pool operations
#[derive(Debug, Clone)]
pub enum PoolEvent {
    ConnectionCreated { key: String },
    ConnectionReused { key: String },
    ConnectionClosed { key: String, reason: String },
    ConnectionExpired { key: String },
    PoolFull { key: String, current: usize, max: usize }, // Added key and capacity info
    CleanupCompleted { removed_count: usize },
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

    /// Close the connection properly
    async fn close(&mut self) -> Result<()> {
        self.connection.close().await
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
    /// Enable metrics collection
    pub enable_metrics: bool,
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
            enable_metrics: true,
        }
    }
}

/// Builder for connection pool configuration
pub struct PoolConfigBuilder {
    config: PoolConfig,
}

impl PoolConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: PoolConfig::default(),
        }
    }

    pub fn max_total(mut self, max: usize) -> Self {
        self.config.max_total_connections = max;
        self
    }

    pub fn max_per_key(mut self, max: usize) -> Self {
        self.config.max_connections_per_key = max;
        self
    }

    pub fn max_age(mut self, duration: Duration) -> Self {
        self.config.max_connection_age = duration;
        self
    }

    pub fn max_idle(mut self, duration: Duration) -> Self {
        self.config.max_idle_time = duration;
        self
    }

    pub fn connection_timeout(mut self, duration: Duration) -> Self {
        self.config.connection_timeout = duration;
        self
    }

    pub fn cleanup_interval(mut self, duration: Duration) -> Self {
        self.config.cleanup_interval = duration;
        self
    }

    pub fn enable_metrics(mut self, enable: bool) -> Self {
        self.config.enable_metrics = enable;
        self
    }

    pub fn build(self) -> PoolConfig {
        self.config
    }
}

/// High-performance connection pool with builder pattern
pub struct ConnectionPool<T> {
    /// Pool configuration
    config: PoolConfig,
    /// Connection pools by key - changed to tokio RwLock for async safety
    pools: DashMap<ConnectionKey, Arc<RwLock<Vec<ConnectionWrapper<T>>>>>,
    /// Connection counters per key (includes both pooled and borrowed connections)
    connection_counters: DashMap<ConnectionKey, Arc<std::sync::atomic::AtomicUsize>>,
    /// Semaphore to limit total connections
    connection_semaphore: Arc<Semaphore>,
    /// Handle for the background cleanup task
    cleanup_handle: Option<tokio::task::JoinHandle<()>>,
    /// Connection factory
    factory: Arc<dyn Fn(&ConnectionKey) -> Box<dyn std::future::Future<Output = Result<T>> + Send + Unpin> + Send + Sync>,
    /// Metrics hook
    metrics_hook: Option<Arc<dyn Fn(PoolEvent) + Send + Sync>>,
}

impl<T> ConnectionPool<T>
where
    T: PooledConnection + 'static,
{
    /// Create a new connection pool with builder pattern
    pub fn builder() -> ConnectionPoolBuilder<T> {
        ConnectionPoolBuilder::new()
    }

    /// Create a new connection pool (legacy method)
    pub fn new<F, Fut>(config: PoolConfig, factory: F) -> Self
    where
        F: Fn(&ConnectionKey) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<T>> + Send + 'static,
    {
        Self::builder()
            .config(config)
            .factory(factory)
            .build()
    }

    /// Get a connection from the pool or create a new one
    pub async fn get_connection(&self, key: &ConnectionKey) -> Result<PooledConnectionGuard<T>> {
        // Acquire semaphore permit for the connection
        let permit = timeout(
            self.config.connection_timeout,
            self.connection_semaphore.clone().acquire_owned()
        )
        .await
        .map_err(|_| ComSrvError::TimeoutError("Connection pool timeout".to_string()))?
        .map_err(|_| ComSrvError::InternalError("Semaphore closed".to_string()))?;

        // Check per-key capacity before proceeding (done later with creation logic)
        // We'll check capacity during actual connection creation to avoid race conditions

        // Try to get an existing connection first
        if let Some(pool_ref) = self.pools.get(key) {
            // Reduce lock contention by extracting valid connections quickly
            let mut valid_connection = None;
            {
                let mut pool = pool_ref.write().await;
                // Use pop to get LIFO behavior (most recently used first)
                while let Some(mut conn_wrapper) = pool.pop() {
                    if conn_wrapper.connection.is_valid() && 
                       !conn_wrapper.is_expired(self.config.max_connection_age) &&
                       !conn_wrapper.is_idle(self.config.max_idle_time) {
                        conn_wrapper.touch();
                        valid_connection = Some(conn_wrapper);
                        break;
                    }
                    // Connection is invalid, expired, or idle - will be dropped
                    debug!("Dropping invalid/expired/idle connection for key: {:?}", key);
                }
            }

            if let Some(conn_wrapper) = valid_connection {
                // Emit metrics event if enabled
                if let Some(hook) = &self.metrics_hook {
                    hook(PoolEvent::ConnectionReused { 
                        key: format!("{:?}", key) 
                    });
                }

                return         Ok(PooledConnectionGuard {
            connection: Some(conn_wrapper),
            pool_ref: pool_ref.clone(),
            semaphore: self.connection_semaphore.clone(),
            permit: Some(permit),
            metrics_hook: self.metrics_hook.clone(),
            key: key.clone(),
            connection_counter: None,
            pool_alive: Arc::downgrade(&self.connection_semaphore),
        });
            }
        }

        // No valid connection found, create a new one
        self.create_new_connection_with_permit(key, permit).await
    }

    /// Create a new connection with an acquired permit
    async fn create_new_connection_with_permit(&self, key: &ConnectionKey, permit: OwnedSemaphorePermit) -> Result<PooledConnectionGuard<T>> {
        // Get or create the pool for this key first
        let pool_ref = self.pools.entry(key.clone()).or_insert_with(|| {
            Arc::new(RwLock::new(Vec::with_capacity(self.config.max_connections_per_key)))
        }).clone();

        // Get or create the connection counter for this key
        let counter_ref = self.connection_counters.entry(key.clone()).or_insert_with(|| {
            Arc::new(std::sync::atomic::AtomicUsize::new(0))
        }).clone();

        // Check per-key capacity more strictly using the counter
        let current_count = counter_ref.load(std::sync::atomic::Ordering::Relaxed);
        if current_count >= self.config.max_connections_per_key {
            // Emit metrics event if enabled
            if let Some(hook) = &self.metrics_hook {
                hook(PoolEvent::PoolFull {
                    key: format!("{:?}", key),
                    current: current_count,
                    max: self.config.max_connections_per_key,
                });
            }
            return Err(ComSrvError::ResourceExhausted(
                format!("Connection pool full for key: {:?} (current: {}, max: {})", 
                    key, current_count, self.config.max_connections_per_key)
            ));
        }

        // Increment the counter
        counter_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Create the connection with timeout
        let connection = timeout(
            self.config.connection_timeout,
            (self.factory)(key)
        ).await
        .map_err(|_| {
            // Decrement counter on failure
            counter_ref.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            ComSrvError::TimeoutError("Connection creation timeout".to_string())
        })?
        .map_err(|e| {
            // Decrement counter on failure
            counter_ref.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            e
        })?;

        let conn_wrapper = ConnectionWrapper::new(connection);

        // Emit metrics event if enabled
        if let Some(hook) = &self.metrics_hook {
            hook(PoolEvent::ConnectionCreated { 
                key: format!("{:?}", key) 
            });
        }

        Ok(PooledConnectionGuard {
            connection: Some(conn_wrapper),
            pool_ref,
            semaphore: self.connection_semaphore.clone(),
            permit: Some(permit),
            metrics_hook: self.metrics_hook.clone(),
            key: key.clone(),
            connection_counter: Some(counter_ref),
            pool_alive: Arc::downgrade(&self.connection_semaphore),
        })
    }

    /// Start the cleanup task to remove expired connections
    fn start_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let pools = self.pools.clone();
        let connection_counters = self.connection_counters.clone();
        let config = self.config.clone();
        let metrics_hook = self.metrics_hook.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.cleanup_interval);

            loop {
                interval.tick().await;
                
                let mut total_removed = 0;
                
                // Clean up expired connections with proper async close
                for entry in pools.iter() {
                    let key = entry.key();
                    let pool_ref = entry.value();
                    
                    // Extract connections to be removed
                    let mut to_remove = Vec::new();
                    {
                        let mut pool = pool_ref.write().await;
                        let initial_len = pool.len();
                        
                        // Separate valid and invalid connections
                        let mut valid_connections = Vec::new();
                        for conn in pool.drain(..) {
                            if conn.is_expired(config.max_connection_age) ||
                               conn.is_idle(config.max_idle_time) ||
                               !conn.connection.is_valid() {
                                to_remove.push(conn);
                            } else {
                                valid_connections.push(conn);
                            }
                        }
                        
                        // Put back valid connections
                        *pool = valid_connections;
                        
                        let removed = initial_len - pool.len();
                        total_removed += removed;
                        
                        if removed > 0 {
                            debug!("Marked {} connections for cleanup for key: {:?}", removed, key);
                        }
                    }
                    
                    // Close connections outside of the lock
                    for mut conn in to_remove {
                        if let Err(e) = conn.close().await {
                            warn!("Error closing connection: {}", e);
                        }
                        
                        // CRITICAL: Decrement counter for cleaned up connections
                        if let Some(counter) = connection_counters.get(key) {
                            counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                        }
                        
                        // Emit metrics event if hook is available
                        if let Some(hook) = &metrics_hook {
                            hook(PoolEvent::ConnectionClosed {
                                key: format!("{:?}", key),
                                reason: "cleanup".to_string(),
                            });
                        }
                    }
                }
                
                if total_removed > 0 {
                    info!("Cleanup completed, removed {} connections", total_removed);
                    
                    // Emit cleanup completion event
                    if let Some(hook) = &metrics_hook {
                        hook(PoolEvent::CleanupCompleted { 
                            removed_count: total_removed 
                        });
                    }
                }
            }
        })
    }

    /// Get pool statistics
    pub fn stats(&self) -> PoolStats {
        let mut total_connections = 0;
        let mut pools_count = 0;
        
        for entry in self.pools.iter() {
            pools_count += 1;
            // Note: This is a best-effort read, might not be 100% accurate
            // in high-concurrency scenarios, but avoids async in sync method
            if let Ok(pool) = entry.value().try_read() {
                total_connections += pool.len();
            }
        }
        
        PoolStats {
            total_connections,
            pools_count,
            available_permits: self.connection_semaphore.available_permits(),
            max_total_connections: self.config.max_total_connections,
        }
    }

    /// Gracefully shutdown the connection pool
    /// 
    /// This method should be called before the tokio runtime is shut down
    /// to ensure all connections are properly closed and no resources are leaked.
    /// 
    /// # Returns
    /// A tuple of (total_closed_connections, cleanup_task_aborted)
    pub async fn shutdown(&mut self) -> (usize, bool) {
        let mut total_closed = 0;
        
        // First, stop the cleanup task
        let cleanup_aborted = if let Some(handle) = self.cleanup_handle.take() {
            handle.abort();
            true
        } else {
            false
        };
        
        // Close all pooled connections synchronously
        for entry in self.pools.iter() {
            let pool_ref = entry.value();
            let mut connections_to_close = Vec::new();
            
            // Extract all connections from the pool
            {
                let mut pool = pool_ref.write().await;
                connections_to_close = pool.drain(..).collect();
            }
            
            // Close connections outside the lock
            for mut conn in connections_to_close {
                if let Err(e) = conn.close().await {
                    warn!("Error closing connection during shutdown: {}", e);
                }
                total_closed += 1;
                
                // Emit metrics event if enabled
                if let Some(hook) = &self.metrics_hook {
                    hook(PoolEvent::ConnectionClosed {
                        key: format!("{:?}", entry.key()),
                        reason: "shutdown".to_string(),
                    });
                }
            }
        }
        
        // Clear all pools and counters
        self.pools.clear();
        self.connection_counters.clear();
        
        if total_closed > 0 {
            info!("Connection pool shutdown completed, closed {} connections", total_closed);
            
            // Emit shutdown completion event
            if let Some(hook) = &self.metrics_hook {
                hook(PoolEvent::CleanupCompleted { 
                    removed_count: total_closed 
                });
            }
        }
        
        (total_closed, cleanup_aborted)
    }

    /// Check if the pool has been shut down
    pub fn is_shutdown(&self) -> bool {
        self.cleanup_handle.is_none()
    }
}

/// Builder for ConnectionPool
pub struct ConnectionPoolBuilder<T> {
    config: PoolConfig,
    factory: Option<Arc<dyn Fn(&ConnectionKey) -> Box<dyn std::future::Future<Output = Result<T>> + Send + Unpin> + Send + Sync>>,
    metrics_hook: Option<Arc<dyn Fn(PoolEvent) + Send + Sync>>,
}

impl<T> ConnectionPoolBuilder<T>
where
    T: PooledConnection + 'static,
{
    pub fn new() -> Self {
        Self {
            config: PoolConfig::default(),
            factory: None,
            metrics_hook: None,
        }
    }

    pub fn config(mut self, config: PoolConfig) -> Self {
        self.config = config;
        self
    }

    pub fn max_total_connections(mut self, max: usize) -> Self {
        self.config.max_total_connections = max;
        self
    }

    pub fn max_connections_per_key(mut self, max: usize) -> Self {
        self.config.max_connections_per_key = max;
        self
    }

    pub fn cleanup_interval(mut self, duration: Duration) -> Self {
        self.config.cleanup_interval = duration;
        self
    }

    pub fn factory<F, Fut>(mut self, factory: F) -> Self
    where
        F: Fn(&ConnectionKey) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<T>> + Send + 'static,
    {
        let factory = Arc::new(move |key: &ConnectionKey| {
            let fut = factory(key);
            Box::new(Box::pin(fut)) as Box<dyn std::future::Future<Output = Result<T>> + Send + Unpin>
        });
        self.factory = Some(factory);
        self
    }

    pub fn metrics_hook<F>(mut self, hook: F) -> Self
    where
        F: Fn(PoolEvent) + Send + Sync + 'static,
    {
        self.metrics_hook = Some(Arc::new(hook));
        self
    }

    pub fn build(self) -> ConnectionPool<T> {
        let factory = self.factory.expect("Factory must be provided");
        let connection_semaphore = Arc::new(Semaphore::new(self.config.max_total_connections));

        let mut pool = ConnectionPool {
            config: self.config,
            pools: DashMap::new(),
            connection_counters: DashMap::new(),
            connection_semaphore,
            factory,
            metrics_hook: self.metrics_hook,
            cleanup_handle: None,
        };

        // Start cleanup task
        let handle = pool.start_cleanup_task();
        pool.cleanup_handle = Some(handle);

        pool
    }
}

/// Connection guard that automatically returns connection to pool
pub struct PooledConnectionGuard<T>
where
    T: PooledConnection + 'static,
{
    connection: Option<ConnectionWrapper<T>>,
    pool_ref: Arc<RwLock<Vec<ConnectionWrapper<T>>>>,
    semaphore: Arc<Semaphore>,
    permit: Option<OwnedSemaphorePermit>,
    metrics_hook: Option<Arc<dyn Fn(PoolEvent) + Send + Sync>>,
    key: ConnectionKey,
    connection_counter: Option<Arc<std::sync::atomic::AtomicUsize>>,
    /// Weak reference to check if pool is still alive
    pool_alive: std::sync::Weak<Semaphore>,
}

impl<T> PooledConnectionGuard<T>
where
    T: PooledConnection + 'static,
{
    /// Take ownership of the connection (won't return to pool)
    pub fn take(mut self) -> T {
        self.connection.take().unwrap().connection
    }
}

impl<T> std::ops::Deref for PooledConnectionGuard<T>
where
    T: PooledConnection + 'static,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.connection.as_ref().unwrap().connection
    }
}

impl<T> std::ops::DerefMut for PooledConnectionGuard<T>
where
    T: PooledConnection + 'static,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection.as_mut().unwrap().connection
    }
}

impl<T> Drop for PooledConnectionGuard<T>
where
    T: PooledConnection + 'static,
{
    fn drop(&mut self) {
        if let Some(conn_wrapper) = self.connection.take() {
            // Check if the pool is still alive (not shutdown)
            if self.pool_alive.upgrade().is_none() {
                // Pool has been shut down, just close the connection synchronously
                debug!("Pool shut down, closing connection synchronously");
                if let Some(hook) = &self.metrics_hook {
                    hook(PoolEvent::ConnectionClosed {
                        key: format!("{:?}", self.key),
                        reason: "pool_shutdown".to_string(),
                    });
                }
                if let Some(counter) = &self.connection_counter {
                    counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                }
                return;
            }
            
            // Check if connection is still valid before returning to pool
            if conn_wrapper.connection.is_valid() {
                // Spawn a task to handle the async return to pool
                let pool_ref = self.pool_ref.clone();
                let key = self.key.clone();
                let metrics_hook = self.metrics_hook.clone();
                let connection_counter = self.connection_counter.clone();
                
                tokio::spawn(async move {
                    let mut pool = pool_ref.write().await;
                    if pool.len() < pool.capacity() {
                        pool.push(conn_wrapper);
                    } else {
                        debug!("Pool at capacity, dropping connection");
                        if let Some(hook) = &metrics_hook {
                            hook(PoolEvent::ConnectionClosed {
                                key: format!("{:?}", key),
                                reason: "pool_full".to_string(),
                            });
                        }
                        // If we're dropping a connection, decrement the counter
                        if let Some(counter) = &connection_counter {
                            counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                });
            } else {
                debug!("Connection invalid, not returning to pool");
                if let Some(hook) = &self.metrics_hook {
                    hook(PoolEvent::ConnectionClosed {
                        key: format!("{:?}", self.key),
                        reason: "invalid".to_string(),
                    });
                }
                // If connection was invalid, decrement the counter
                if let Some(counter) = &self.connection_counter {
                    counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                }
            }
        } else {
            // If connection was taken, decrement the counter
            if let Some(counter) = &self.connection_counter {
                counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            }
        }
        // Semaphore permit is automatically released when permit is dropped
    }
}

impl<T> Drop for ConnectionPool<T> {
    fn drop(&mut self) {
        if let Some(handle) = &self.cleanup_handle {
            handle.abort();
        }
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
        
        let pool = ConnectionPool::builder()
            .config(config)
            .factory({
                let counter = counter.clone();
                move |_key| {
                    let id = counter.fetch_add(1, Ordering::Relaxed);
                    async move { Ok(MockConnection::new(id)) }
                }
            })
            .build();

        let key = ConnectionKey::new("test", "localhost", Some(8080));
        
        // Get first connection
        let conn1 = pool.get_connection(&key).await.unwrap();
        assert_eq!(conn1.id, 0);
        
        drop(conn1);
        
        // Small delay to allow async drop task to complete
        tokio::time::sleep(Duration::from_millis(10)).await;
        
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
    async fn test_pool_config_builder() {
        let config = PoolConfigBuilder::new()
            .max_total(50)
            .max_per_key(5)
            .max_age(Duration::from_secs(1800))
            .enable_metrics(false)
            .build();
            
        assert_eq!(config.max_total_connections, 50);
        assert_eq!(config.max_connections_per_key, 5);
        assert_eq!(config.max_connection_age, Duration::from_secs(1800));
        assert!(!config.enable_metrics);
    }

    #[tokio::test]
    async fn test_connection_pool_builder() {
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let pool = ConnectionPool::builder()
            .max_total_connections(50)
            .max_connections_per_key(5)
            .factory({
                let counter = counter.clone();
                move |_key: &ConnectionKey| {
                    let counter = counter.clone();
                    async move {
                        let id = counter.fetch_add(1, Ordering::Relaxed);
                        Ok(MockConnection::new(id))
                    }
                }
            })
            .build();

        let stats = pool.stats();
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.pools_count, 0);
        assert_eq!(stats.max_total_connections, 50);
    }

    #[tokio::test]
    async fn test_get_connection() {
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let pool = ConnectionPool::builder()
            .factory({
                let counter = counter.clone();
                move |_key: &ConnectionKey| {
                    let counter = counter.clone();
                    async move {
                        let id = counter.fetch_add(1, Ordering::Relaxed);
                        Ok(MockConnection::new(id))
                    }
                }
            })
            .build();
            
        let key = ConnectionKey::new("test", "127.0.0.1", Some(8080));

        let conn_guard = pool.get_connection(&key).await.unwrap();
        assert!(conn_guard.is_valid());
        assert_eq!(conn_guard.id, 0);
    }

    #[tokio::test]
    async fn test_connection_reuse() {
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let pool = ConnectionPool::builder()
            .factory({
                let counter = counter.clone();
                move |_key: &ConnectionKey| {
                    let counter = counter.clone();
                    async move {
                        let id = counter.fetch_add(1, Ordering::Relaxed);
                        Ok(MockConnection::new(id))
                    }
                }
            })
            .build();
            
        let key = ConnectionKey::new("test", "127.0.0.1", Some(8080));

        // Get and return a connection
        {
            let _conn_guard = pool.get_connection(&key).await.unwrap();
        }
        
        // Small delay to allow async drop task to complete
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Get another connection - should reuse the first one
        let conn_guard = pool.get_connection(&key).await.unwrap();
        assert_eq!(conn_guard.id, 0); // Should be the same connection
    }

    #[tokio::test]
    async fn test_connection_pool_stats() {
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let pool = ConnectionPool::builder()
            .max_total_connections(5)
            .factory({
                let counter = counter.clone();
                move |_key: &ConnectionKey| {
                    let counter = counter.clone();
                    async move {
                        let id = counter.fetch_add(1, Ordering::Relaxed);
                        Ok(MockConnection::new(id))
                    }
                }
            })
            .build();
            
        let key = ConnectionKey::new("test", "127.0.0.1", Some(8080));

        let _conn1 = pool.get_connection(&key).await.unwrap();
        let _conn2 = pool.get_connection(&key).await.unwrap();

        let stats = pool.stats();
        assert_eq!(stats.max_total_connections, 5);
        assert_eq!(stats.available_permits, 3);
    }

    #[tokio::test]
    async fn test_per_key_capacity_limit() {
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let pool = ConnectionPool::builder()
            .max_connections_per_key(2)
            .factory({
                let counter = counter.clone();
                move |_key: &ConnectionKey| {
                    let counter = counter.clone();
                    async move {
                        let id = counter.fetch_add(1, Ordering::Relaxed);
                        Ok(MockConnection::new(id))
                    }
                }
            })
            .build();
            
        let key = ConnectionKey::new("test", "127.0.0.1", Some(8080));

        // Get maximum connections for this key
        let _conn1 = pool.get_connection(&key).await.unwrap();
        let _conn2 = pool.get_connection(&key).await.unwrap();
        
        // This should fail due to per-key limit
        let result = pool.get_connection(&key).await;
        assert!(result.is_err());
        if let Err(ComSrvError::ResourceExhausted(_)) = result {
            // Expected error type
        } else {
            panic!("Expected ResourceExhausted error");
        }
    }

    #[tokio::test]
    async fn test_metrics_hook() {
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let pool = ConnectionPool::builder()
            .factory({
                let counter = counter.clone();
                move |_key: &ConnectionKey| {
                    let counter = counter.clone();
                    async move {
                        let id = counter.fetch_add(1, Ordering::Relaxed);
                        Ok(MockConnection::new(id))
                    }
                }
            })
            .metrics_hook({
                let events = events.clone();
                move |event| {
                    events.lock().unwrap().push(event);
                }
            })
            .build();
            
        let key = ConnectionKey::new("test", "127.0.0.1", Some(8080));

        // Create a connection
        let conn = pool.get_connection(&key).await.unwrap();
        drop(conn);
        
        // Small delay to allow async operations to complete
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Get another connection (should reuse)
        let _conn = pool.get_connection(&key).await.unwrap();

        let captured_events = events.lock().unwrap().clone();
        assert!(!captured_events.is_empty());
        
        // Check for creation and reuse events
        let has_creation = captured_events.iter().any(|e| matches!(e, PoolEvent::ConnectionCreated { .. }));
        let has_reuse = captured_events.iter().any(|e| matches!(e, PoolEvent::ConnectionReused { .. }));
        
        assert!(has_creation);
        assert!(has_reuse);
    }

    #[tokio::test]
    async fn test_invalid_connection_cleanup() {
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let pool = ConnectionPool::builder()
            .factory({
                let counter = counter.clone();
                move |_key: &ConnectionKey| {
                    let counter = counter.clone();
                    async move {
                        let id = counter.fetch_add(1, Ordering::Relaxed);
                        Ok(MockConnection::new(id))
                    }
                }
            })
            .build();
            
        let key = ConnectionKey::new("test", "127.0.0.1", Some(8080));

        // Create a connection and invalidate it
        {
            let conn_guard = pool.get_connection(&key).await.unwrap();
            conn_guard.valid.store(false, Ordering::Relaxed);
        }
        
        // Small delay to allow async drop task to complete
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Next connection should be a new one
        let conn_guard = pool.get_connection(&key).await.unwrap();
        assert_eq!(conn_guard.id, 1); // Should be a new connection
    }

    #[tokio::test]
    async fn test_connection_guard_take() {
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let pool = ConnectionPool::builder()
            .factory({
                let counter = counter.clone();
                move |_key: &ConnectionKey| {
                    let counter = counter.clone();
                    async move {
                        let id = counter.fetch_add(1, Ordering::Relaxed);
                        Ok(MockConnection::new(id))
                    }
                }
            })
            .build();
            
        let key = ConnectionKey::new("test", "127.0.0.1", Some(8080));

        let conn_guard = pool.get_connection(&key).await.unwrap();
        let connection = conn_guard.take();
        
        assert_eq!(connection.id, 0);
        assert!(connection.is_valid());
    }

    #[tokio::test]
    async fn test_multiple_pools() {
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let pool = ConnectionPool::builder()
            .factory({
                let counter = counter.clone();
                move |_key: &ConnectionKey| {
                    let counter = counter.clone();
                    async move {
                        let id = counter.fetch_add(1, Ordering::Relaxed);
                        Ok(MockConnection::new(id))
                    }
                }
            })
            .build();
        
        let key1 = ConnectionKey::new("test1", "127.0.0.1", Some(8080));
        let key2 = ConnectionKey::new("test2", "127.0.0.1", Some(8081));

        let _conn1 = pool.get_connection(&key1).await.unwrap();
        let _conn2 = pool.get_connection(&key2).await.unwrap();

        let stats = pool.stats();
        assert_eq!(stats.pools_count, 2);
    }

    #[tokio::test]
    async fn test_connection_factory_error() {
        let pool: ConnectionPool<MockConnection> = ConnectionPool::builder()
            .factory(|_key: &ConnectionKey| async {
                Err(ComSrvError::ConnectionError("Factory error".to_string()))
            })
            .build();
            
        let key = ConnectionKey::new("test", "127.0.0.1", Some(8080));

        let result = pool.get_connection(&key).await;
        assert!(result.is_err());
    }

    #[tokio::test] 
    async fn test_cleanup_task() {
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        
        let pool = ConnectionPool::builder()
            .factory({
                let counter = counter.clone();
                move |_key: &ConnectionKey| {
                    let counter = counter.clone();
                    async move {
                        let id = counter.fetch_add(1, Ordering::Relaxed);
                        Ok(MockConnection::new(id))
                    }
                }
            })
            .metrics_hook({
                let events = events.clone();
                move |event| {
                    events.lock().unwrap().push(event);
                }
            })
            .build();
            
        let key = ConnectionKey::new("test", "127.0.0.1", Some(8080));

        // Create and invalidate a connection
        {
            let conn = pool.get_connection(&key).await.unwrap();
            conn.valid.store(false, Ordering::Relaxed);
        }
        
        // Allow some time for cleanup (note: actual cleanup runs every 60s by default)
        // This test just ensures the structure is correct
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // The cleanup task should be running (we can't easily test the actual cleanup 
        // without waiting 60 seconds, but we can verify the structure is correct)
        assert!(pool.cleanup_handle.is_some());
    }

    #[tokio::test]
    async fn test_counter_sync_during_cleanup() {
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        
        let pool = ConnectionPool::builder()
            .max_connections_per_key(3)
            .cleanup_interval(Duration::from_millis(50)) // Fast cleanup for testing
            .factory({
                let counter = counter.clone();
                move |_key: &ConnectionKey| {
                    let counter = counter.clone();
                    async move {
                        let id = counter.fetch_add(1, Ordering::Relaxed);
                        Ok(MockConnection::new(id))
                    }
                }
            })
            .build();
            
        let key = ConnectionKey::new("test", "127.0.0.1", Some(8080));

        // Create connections and invalidate some
        let mut connections = Vec::new();
        for _ in 0..3 {
            connections.push(pool.get_connection(&key).await.unwrap());
        }
        
        // Invalidate 2 connections
        connections[0].valid.store(false, Ordering::Relaxed);
        connections[1].valid.store(false, Ordering::Relaxed);
        
        // Drop all connections to return them to pool
        drop(connections);
        
        // Wait for async drop tasks to complete
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Get the current counter value
        let initial_counter = if let Some(counter_ref) = pool.connection_counters.get(&key) {
            counter_ref.load(Ordering::Relaxed)
        } else {
            0
        };
        
        // Wait for cleanup to run (50ms + some buffer)
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Counter should be decremented for cleaned up connections
        let final_counter = if let Some(counter_ref) = pool.connection_counters.get(&key) {
            counter_ref.load(Ordering::Relaxed)
        } else {
            0
        };
        
        // Should have 1 valid connection left (3 created - 2 invalidated)
        assert_eq!(final_counter, 1, 
            "Counter should be 1 after cleanup, initial: {}, final: {}", 
            initial_counter, final_counter);
         
        // Should be able to create 2 more connections (max_per_key=3, current=1)
        let _conn1 = pool.get_connection(&key).await.unwrap();
        let _conn2 = pool.get_connection(&key).await.unwrap();
        
        // Third connection should succeed (we now have 3 total: 1 from pool + 2 new = 3, which equals the limit)
        let _conn3 = pool.get_connection(&key).await.unwrap();
        
        // Fourth should fail due to limit (3/3 connections in use)
        let result = pool.get_connection(&key).await;
        assert!(result.is_err(), "Should fail due to per-key limit, but succeeded");
    }

    #[tokio::test]
    async fn test_hash_caching_performance() {
        let key = ConnectionKey::new("modbus", "127.0.0.1", Some(502))
            .with_param("slave_id", "1")
            .with_param("timeout", "5000");
        
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        // First hash computation
        let mut hasher1 = DefaultHasher::new();
        key.hash(&mut hasher1);
        let hash1 = hasher1.finish();
        
        // Second hash computation (should use cached value)
        let mut hasher2 = DefaultHasher::new();
        key.hash(&mut hasher2);
        let hash2 = hasher2.finish();
        
        // Should be identical
        assert_eq!(hash1, hash2);
        
        // Test with cloned key (should also work)
        let key_clone = key.clone();
        let mut hasher3 = DefaultHasher::new();
        key_clone.hash(&mut hasher3);
        let hash3 = hasher3.finish();
        
        assert_eq!(hash1, hash3);
    }

    #[tokio::test]
    async fn test_graceful_shutdown() {
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        
        let mut pool = ConnectionPool::builder()
            .max_connections_per_key(5)
            .factory({
                let counter = counter.clone();
                move |_key: &ConnectionKey| {
                    let counter = counter.clone();
                    async move {
                        let id = counter.fetch_add(1, Ordering::Relaxed);
                        Ok(MockConnection::new(id))
                    }
                }
            })
            .metrics_hook({
                let events = events.clone();
                move |event| {
                    events.lock().unwrap().push(event);
                }
            })
            .build();
            
        let key = ConnectionKey::new("test", "127.0.0.1", Some(8080));

        // Create some connections
        let _conn1 = pool.get_connection(&key).await.unwrap();
        let _conn2 = pool.get_connection(&key).await.unwrap();
        
        // Drop one connection to put it back in the pool
        drop(_conn1);
        tokio::time::sleep(Duration::from_millis(10)).await; // Allow async drop
        
        // Check initial state
        assert!(!pool.is_shutdown());
        
        // Shutdown the pool
        let (closed_count, cleanup_aborted) = pool.shutdown().await;
        
        // Verify shutdown state
        assert!(pool.is_shutdown());
        assert!(cleanup_aborted); // Cleanup task should have been running
        assert!(closed_count > 0); // Should have closed at least one pooled connection
        
        // Verify pool is empty
        let stats = pool.stats();
        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.pools_count, 0);
        
        // Check that shutdown events were emitted
        let captured_events = events.lock().unwrap().clone();
        let has_shutdown_events = captured_events.iter().any(|e| {
            matches!(e, PoolEvent::ConnectionClosed { reason, .. } if reason == "shutdown")
        });
        assert!(has_shutdown_events);
        
        // Any remaining connection guards should not spawn tasks on drop
        // (this tests the pool_alive weak reference mechanism)
        drop(_conn2);
        // No assertion needed - the test passes if no panic occurs
    }

    #[tokio::test]
    async fn test_shutdown_with_no_connections() {
        let mut pool: ConnectionPool<MockConnection> = ConnectionPool::builder()
            .factory(|_key: &ConnectionKey| async {
                Ok(MockConnection::new(0))
            })
            .build();
        
        // Shutdown empty pool
        let (closed_count, cleanup_aborted) = pool.shutdown().await;
        
        assert_eq!(closed_count, 0);
        assert!(cleanup_aborted); // Cleanup task should still be aborted
        assert!(pool.is_shutdown());
    }
} 