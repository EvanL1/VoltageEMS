use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio::time;
use tracing::{debug, error, info};
use walkdir::WalkDir;

use crate::{ConfigError, ConfigLoader, Configurable, Result};

#[derive(Debug, Clone)]
pub enum WatchEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
    Reloaded,
}

pub struct ConfigWatcher {
    loader: Arc<Mutex<ConfigLoader>>,
    watch_paths: Vec<PathBuf>,
    interval: Duration,
    sender: mpsc::Sender<WatchEvent>,
    receiver: Arc<Mutex<mpsc::Receiver<WatchEvent>>>,
    file_states: Arc<Mutex<HashMap<PathBuf, std::time::SystemTime>>>,
}

impl ConfigWatcher {
    pub fn new(loader: ConfigLoader, watch_paths: Vec<PathBuf>) -> Self {
        let (sender, receiver) = mpsc::channel(100);
        
        Self {
            loader: Arc::new(Mutex::new(loader)),
            watch_paths,
            interval: Duration::from_secs(5),
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
            file_states: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    pub async fn start(&self) -> Result<()> {
        self.initialize_file_states().await?;
        
        let _loader = Arc::clone(&self.loader);
        let watch_paths = self.watch_paths.clone();
        let interval = self.interval;
        let sender = self.sender.clone();
        let file_states = Arc::clone(&self.file_states);
        
        tokio::spawn(async move {
            let mut interval_timer = time::interval(interval);
            
            loop {
                interval_timer.tick().await;
                
                if let Err(e) = check_for_changes(&watch_paths, &file_states, &sender).await {
                    error!("Error checking for file changes: {}", e);
                }
            }
        });
        
        info!("Configuration watcher started with interval: {:?}", self.interval);
        Ok(())
    }

    pub async fn wait_for_change(&self) -> Option<WatchEvent> {
        let mut receiver = self.receiver.lock().await;
        receiver.recv().await
    }

    pub async fn reload<T>(&self) -> Result<T>
    where
        T: Configurable + for<'de> serde::Deserialize<'de> + 'static,
    {
        let mut loader = self.loader.lock().await;
        let config = loader.reload()?;
        
        self.sender
            .send(WatchEvent::Reloaded)
            .await
            .map_err(|e| ConfigError::Watch(format!("Failed to send reload event: {}", e)))?;
        
        Ok(config)
    }

    async fn initialize_file_states(&self) -> Result<()> {
        let mut states = self.file_states.lock().await;
        
        for watch_path in &self.watch_paths {
            if watch_path.is_dir() {
                for entry in WalkDir::new(watch_path)
                    .follow_links(true)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if entry.file_type().is_file() {
                        if let Ok(metadata) = entry.metadata() {
                            if let Ok(modified) = metadata.modified() {
                                states.insert(entry.path().to_path_buf(), modified);
                            }
                        }
                    }
                }
            } else if watch_path.is_file() {
                if let Ok(metadata) = tokio::fs::metadata(watch_path).await {
                    if let Ok(modified) = metadata.modified() {
                        states.insert(watch_path.clone(), modified);
                    }
                }
            }
        }
        
        debug!("Initialized file states for {} files", states.len());
        Ok(())
    }
}

async fn check_for_changes(
    watch_paths: &[PathBuf],
    file_states: &Arc<Mutex<HashMap<PathBuf, std::time::SystemTime>>>,
    sender: &mpsc::Sender<WatchEvent>,
) -> Result<()> {
    let mut states = file_states.lock().await;
    let mut changes = Vec::new();
    
    for watch_path in watch_paths {
        if watch_path.is_dir() {
            for entry in WalkDir::new(watch_path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    let path = entry.path().to_path_buf();
                    
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            match states.get(&path) {
                                Some(last_modified) if modified != *last_modified => {
                                    states.insert(path.clone(), modified);
                                    changes.push(WatchEvent::Modified(path));
                                }
                                None => {
                                    states.insert(path.clone(), modified);
                                    changes.push(WatchEvent::Created(path));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        } else if watch_path.is_file() {
            if let Ok(metadata) = tokio::fs::metadata(watch_path).await {
                if let Ok(modified) = metadata.modified() {
                    match states.get(watch_path) {
                        Some(last_modified) if modified != *last_modified => {
                            states.insert(watch_path.clone(), modified);
                            changes.push(WatchEvent::Modified(watch_path.clone()));
                        }
                        None => {
                            states.insert(watch_path.clone(), modified);
                            changes.push(WatchEvent::Created(watch_path.clone()));
                        }
                        _ => {}
                    }
                }
            } else if states.contains_key(watch_path) {
                states.remove(watch_path);
                changes.push(WatchEvent::Deleted(watch_path.clone()));
            }
        }
    }
    
    let to_remove: Vec<_> = states
        .keys()
        .filter(|path| !path.exists())
        .cloned()
        .collect();
    
    for path in to_remove {
        states.remove(&path);
        changes.push(WatchEvent::Deleted(path));
    }
    
    for change in changes {
        debug!("Detected change: {:?}", change);
        sender
            .send(change)
            .await
            .map_err(|e| ConfigError::Watch(format!("Failed to send change event: {}", e)))?;
    }
    
    Ok(())
}