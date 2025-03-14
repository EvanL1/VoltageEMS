use std::sync::Arc;
use log::{info, error};
use crate::error::Result;
use crate::config::Config;
use crate::storage::{DataStore, SyncMode};
use crate::storage::hybrid_store::{HybridStore, SyncService};

/// Storage agent, responsible for managing storage operations and synchronization
pub struct StorageAgent {
    store: Arc<HybridStore>,
    sync_service: Option<SyncService>,
    config: Config,
}

impl StorageAgent {
    /// Create a new storage agent
    pub fn new(config: Config) -> Result<Self> {
        let sync_mode = config.get_sync_mode();
        let store = Arc::new(HybridStore::new(&config, sync_mode.clone())?);
        
        // If using Redis, load initial data from Redis
        if config.use_redis {
            info!("Loading initial data from Redis...");
            store.load_from_redis(&format!("{}*", config.redis.key_prefix))?;
        }
        
        // If using WriteBack mode, create and start sync service
        let sync_service = if let SyncMode::WriteBack(interval) = sync_mode {
            let service = SyncService::new(
                store.clone(),
                interval,
                vec![format!("{}*", config.redis.key_prefix)]
            );
            
            if let Err(e) = service.start() {
                error!("Failed to start sync service: {}", e);
            } else {
                info!("Sync service started with interval {:?}", interval);
            }
            
            Some(service)
        } else {
            None
        };
        
        Ok(Self {
            store,
            sync_service,
            config,
        })
    }
    
    /// Get storage instance
    pub fn store(&self) -> Arc<HybridStore> {
        self.store.clone()
    }
    
    /// Manually sync to Redis
    pub fn sync_to_redis(&self) -> Result<()> {
        if self.config.use_redis {
            info!("Manually syncing data to Redis...");
            self.store.sync_to_redis(&format!("{}*", self.config.redis.key_prefix))?;
        }
        Ok(())
    }
    
    /// Shutdown the agent, stop sync service
    pub fn shutdown(&self) -> Result<()> {
        if let Some(sync_service) = &self.sync_service {
            info!("Shutting down sync service...");
            sync_service.stop()?;
        }
        
        // If using OnDemand mode, perform one final sync before shutdown
        if let SyncMode::OnDemand = self.config.get_sync_mode() {
            if self.config.use_redis {
                info!("Final sync to Redis before shutdown...");
                self.store.sync_to_redis(&format!("{}*", self.config.redis.key_prefix))?;
            }
        }
        
        Ok(())
    }
}

impl Drop for StorageAgent {
    fn drop(&mut self) {
        if let Err(e) = self.shutdown() {
            error!("Error during StorageAgent shutdown: {}", e);
        }
    }
} 