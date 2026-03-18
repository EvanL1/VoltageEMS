/// Background tasks:
/// - **collector_task** – polls Redis every `collection_interval_secs` and
///   appends data points to the shared buffer.
/// - **flush_task** – drains the buffer every `flush_interval_secs` and
///   writes to storage in batches.
/// - **cleanup_task** – runs daily at approximately 02:00 UTC and removes
///   data older than `cleanup_older_than_days`.
use std::sync::Arc;

use chrono::Utc;
use tokio::time::{self, Duration};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::collector;
use crate::state::AppState;

/// Spawn all background tasks. Each task honours the given `CancellationToken`.
pub fn spawn_all(state: Arc<AppState>, shutdown: CancellationToken) {
    {
        let s = state.clone();
        let sd = shutdown.clone();
        tokio::spawn(async move { collector_task(s, sd).await });
    }
    {
        let s = state.clone();
        let sd = shutdown.clone();
        tokio::spawn(async move { flush_task(s, sd).await });
    }
    {
        let s = state.clone();
        let sd = shutdown.clone();
        tokio::spawn(async move { cleanup_task(s, sd).await });
    }
}

async fn collector_task(state: Arc<AppState>, shutdown: CancellationToken) {
    loop {
        let interval = {
            state.config.read().await.collection_interval_secs
        };
        let enabled = {
            state.storage_settings.read().await.enabled
        };

        tokio::select! {
            _ = time::sleep(Duration::from_secs(interval)) => {}
            _ = shutdown.cancelled() => {
                info!("Collector task shutting down");
                return;
            }
        }

        if !enabled {
            continue;
        }

        let cfg_snapshot = {
            let cfg = state.config.read().await;
            cfg.clone()
        };

        let points = collector::collect(state.rtdb.as_ref(), &cfg_snapshot).await;
        if !points.is_empty() {
            let mut buf = state.buffer.lock().await;
            buf.extend(points);
        }
    }
}

async fn flush_task(state: Arc<AppState>, shutdown: CancellationToken) {
    loop {
        let interval = {
            let cfg = state.config.read().await;
            cfg.flush_interval_secs
        };

        tokio::select! {
            _ = time::sleep(Duration::from_secs(interval)) => {}
            _ = shutdown.cancelled() => {
                // Final flush before exit
                flush_buffer(&state).await;
                info!("Flush task shutting down");
                return;
            }
        }

        flush_buffer(&state).await;
    }
}

async fn flush_buffer(state: &AppState) {
    let batch_size = state.config.read().await.batch_size;
    let enabled = state.storage_settings.read().await.enabled;

    if !enabled {
        return;
    }

    let points = {
        let mut buf = state.buffer.lock().await;
        if buf.is_empty() {
            return;
        }
        std::mem::take(&mut *buf)
    };

    let backend = state.storage.read().await.clone();
    let total = points.len();
    for chunk in points.chunks(batch_size) {
        match backend.write_batch(chunk.to_vec()).await {
            Ok(n) => info!("Flushed {} data points to {}", n, backend.name()),
            Err(e) => error!("Flush failed: {}", e),
        }
    }
    info!("Flush complete: {} points written", total);
}

async fn cleanup_task(state: Arc<AppState>, shutdown: CancellationToken) {
    loop {
        // Wait until approximately 02:00 UTC next day
        let sleep_secs = secs_until_02_utc();

        tokio::select! {
            _ = time::sleep(Duration::from_secs(sleep_secs)) => {}
            _ = shutdown.cancelled() => {
                info!("Cleanup task shutting down");
                return;
            }
        }

        let (enabled, days) = {
            let cfg = state.config.read().await;
            (cfg.cleanup_enabled && state.storage_settings.read().await.enabled,
             cfg.cleanup_older_than_days)
        };

        if !enabled {
            continue;
        }

        let backend = state.storage.read().await.clone();
        match backend.cleanup_old_data(days).await {
            Ok(n) => info!("Cleanup: removed {} rows older than {} days", n, days),
            Err(e) => warn!("Cleanup failed: {}", e),
        }
    }
}

/// How many seconds until the next 02:00 UTC (minimum 60s to avoid tight loops).
fn secs_until_02_utc() -> u64 {
    let now = Utc::now();
    let today_02: chrono::DateTime<Utc> = now
        .date_naive()
        .and_hms_opt(2, 0, 0)
        .unwrap()
        .and_utc();

    let target = if now < today_02 {
        today_02
    } else {
        today_02 + chrono::Duration::days(1)
    };

    let delta = (target - now).num_seconds().max(60) as u64;
    delta
}
