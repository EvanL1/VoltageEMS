//! History Service (HisSrv)
//! Collect the data and storage into InfluxDB

use anyhow::Result;
use axum::{extract::State, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{net::SocketAddr, sync::Arc};
use tokio::time::{interval, Duration};
use tracing::{error, info};
use voltage_libs::{config::ConfigLoader, influxdb::InfluxClient};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct Config {
    #[serde(default)]
    service: ServiceConfig,
    redis: RedisConfig,
    #[serde(default)]
    influxdb: InfluxDbConfig,
    #[serde(default)]
    collection: CollectionConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct ServiceConfig {
    #[serde(default = "default_service_name")]
    name: String,
    #[serde(default = "default_port")]
    port: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RedisConfig {
    #[serde(default = "default_redis_url")]
    url: String,
}

fn default_redis_url() -> String {
    "redis://127.0.0.1:6379".to_string()
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: default_redis_url(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct InfluxDbConfig {
    #[serde(default = "default_influxdb_enabled")]
    enabled: bool,
    #[serde(default = "default_influxdb_url")]
    url: String,
    #[serde(default = "default_influxdb_org")]
    org: String,
    #[serde(default = "default_influxdb_bucket")]
    bucket: String,
    #[serde(default)]
    token: Option<String>,
}

fn default_influxdb_enabled() -> bool {
    true
}

fn default_influxdb_url() -> String {
    "http://localhost:8086".to_string()
}

fn default_influxdb_org() -> String {
    "voltage".to_string()
}

fn default_influxdb_bucket() -> String {
    "voltage".to_string()
}

impl Default for InfluxDbConfig {
    fn default() -> Self {
        Self {
            enabled: default_influxdb_enabled(),
            url: default_influxdb_url(),
            org: default_influxdb_org(),
            bucket: default_influxdb_bucket(),
            token: None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct CollectionConfig {
    #[serde(default = "default_interval")]
    interval_seconds: u64,
    #[serde(default = "default_batch_size")]
    batch_size: usize,
    #[serde(default)]
    sources: Vec<String>,
}

fn default_service_name() -> String {
    "hissrv".to_string()
}

fn default_port() -> u16 {
    6004
}

fn default_interval() -> u64 {
    60
}

fn default_batch_size() -> usize {
    1000
}

struct AppState {
    redis_client: redis::Client,
    influx_client: Option<Arc<InfluxClient>>,
    config: Config,
    stats: Arc<tokio::sync::RwLock<Stats>>,
}

#[derive(Default, Clone, Serialize)]
struct Stats {
    total_points_collected: u64,
    total_batches_sent: u64,
    last_collection_time: Option<String>,
    last_error: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // init the logging system
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting History Service...");

    // load the configure
    let config: Config = ConfigLoader::new()
        .with_yaml_file("config/hissrv.yaml")
        .with_env_prefix("HISSRV")
        .build()?;

    // Connect to Redis
    let redis_client = redis::Client::open(config.redis.url.clone())?;
    info!("Connected to Redis");

    // Connect to InfluxDB
    let influx_client = if config.influxdb.enabled {
        match InfluxClient::new(
            &config.influxdb.url,
            &config.influxdb.org,
            &config.influxdb.bucket,
            config.influxdb.token.as_deref().unwrap_or(""),
        ) {
            Ok(client) => {
                info!(
                    "✅ InfluxDB connected: {} (org: {}, bucket: {})",
                    config.influxdb.url, config.influxdb.org, config.influxdb.bucket
                );
                Some(Arc::new(client))
            },
            Err(e) => {
                error!("❌ Failed to connect to InfluxDB: {}", e);
                error!(
                    "   URL: {}, Org: {}, Bucket: {}",
                    config.influxdb.url, config.influxdb.org, config.influxdb.bucket
                );
                None
            },
        }
    } else {
        info!("InfluxDB disabled in configuration, data will only be collected in Redis");
        None
    };

    // init the status
    let state = Arc::new(AppState {
        redis_client,
        influx_client,
        config: config.clone(),
        stats: Arc::new(tokio::sync::RwLock::new(Stats::default())),
    });

    // Start Collect
    let collection_state = state.clone();
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(
            collection_state.config.collection.interval_seconds,
        ));
        let mut batch_id = 0u64;

        loop {
            interval.tick().await;

            if let Err(e) = collect_and_store(&collection_state, batch_id).await {
                error!("Collection error: {}", e);
                let mut stats = collection_state.stats.write().await;
                stats.last_error = Some(e.to_string());
            }

            batch_id += 1;
        }
    });

    // 创建API路由
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/stats", get(get_stats))
        .with_state(state);

    // 启动HTTP服务
    let addr = SocketAddr::from(([0, 0, 0, 0], config.service.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("History Service started on {}", addr);
    info!("API endpoints:");
    info!("  GET /health - Health check");
    info!("  GET /api/stats - Get statistics");

    axum::serve(listener, app).await?;
    Ok(())
}

async fn collect_and_store(state: &AppState, batch_id: u64) -> Result<()> {
    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await?;

    // 调用Lua函数收集数据
    let sources_json = serde_json::to_string(&state.config.collection.sources)?;
    let result: String = redis::cmd("FCALL")
        .arg("hissrv_collect_batch")
        .arg(1)
        .arg(format!("batch_{}", batch_id))
        .arg(sources_json)
        .query_async(&mut conn)
        .await?;

    let batch_info: serde_json::Value = serde_json::from_str(&result)?;
    let point_count = batch_info["point_count"].as_u64().unwrap_or(0);

    if point_count > 0 {
        // Write into InfluxDB
        if let Some(influx) = &state.influx_client {
            let lines = batch_info["lines"].as_str().unwrap_or("");
            match influx.write_line_protocol(lines).await {
                Ok(_) => {
                    info!(
                        "📊 Wrote {} points to InfluxDB (batch {})",
                        point_count, batch_id
                    );

                    // 更新统计
                    let mut stats = state.stats.write().await;
                    stats.total_points_collected += point_count;
                    stats.total_batches_sent += 1;
                    stats.last_collection_time = Some(chrono::Utc::now().to_rfc3339());
                },
                Err(e) => {
                    error!("Failed to write to InfluxDB: {}", e);
                    // 记录错误但不中断服务
                    let mut stats = state.stats.write().await;
                    stats.last_error = Some(format!("InfluxDB write error: {}", e));
                },
            }
        } else {
            info!(
                "📦 Collected {} points in batch {} (InfluxDB disabled)",
                point_count, batch_id
            );
        }
    }

    Ok(())
}

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "service": "hissrv",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

async fn get_stats(State(state): State<Arc<AppState>>) -> Json<Stats> {
    let stats = state.stats.read().await;
    Json((*stats).clone())
}
