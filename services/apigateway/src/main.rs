use axum::{
    http::{Method, StatusCode},
    middleware,
    response::Response,
    routing::{get, post},
    Router,
};
use env_logger::Env;
use log::info;
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

mod auth;
mod config;
mod config_client;
mod data_access;
mod error;
mod handlers;
mod influxdb_client;
mod redis_client;
mod response;
mod websocket;

use auth::middleware::jwt_auth_layer;
use config::Config;
use config_client::ConfigClient;
use data_access::{hybrid::HybridDataAccess, sync::ConfigSyncService, DataAccessLayer};
use influxdb_client::InfluxDbClient;
use redis_client::RedisClient;

// CORS OPTIONS 处理器
async fn handle_options() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .body(Default::default())
        .unwrap()
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Initialize logger
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    // Initialize configuration client
    let config_service_url =
        std::env::var("CONFIG_SERVICE_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());
    let service_name = "apigateway";

    let config_client = Arc::new(ConfigClient::new(
        config_service_url,
        service_name.to_string(),
    ));

    // Fetch initial configuration from config service
    let config = match config_client.fetch_config().await {
        Ok(cfg) => Arc::new(cfg),
        Err(e) => {
            log::warn!(
                "Failed to fetch config from service: {}, falling back to local config",
                e
            );
            // Fallback to local configuration, use default if that fails too
            Arc::new(Config::load().unwrap_or_else(|e| {
                log::warn!("Failed to load local configuration: {}, using default config", e);
                Config::default()
            }))
        }
    };

    let bind_addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .expect("Failed to parse bind address");
    info!("Starting API Gateway on {}", bind_addr);

    // Start configuration watch loop
    let update_interval = std::time::Duration::from_secs(30);
    config_client.start_watch_loop(update_interval).await;

    // Initialize Redis client
    let redis_client = Arc::new(
        RedisClient::new(&config.redis.url)
            .await
            .expect("Failed to connect to Redis"),
    );

    // Initialize InfluxDB client
    let influxdb_client = Arc::new(InfluxDbClient::new(
        &config.influxdb.url,
        &config.influxdb.database,
    ));

    // Create HTTP client for backend services
    let http_client = Arc::new(reqwest::Client::new());

    // Create WebSocket Hub
    let ws_hub = websocket::hub::Hub::new(redis_client.clone());
    let ws_hub = Arc::new(tokio::sync::RwLock::new(ws_hub));
    
    // Start Redis subscriber for real-time data
    websocket::handlers::realtime::start_redis_subscriber(
        ws_hub.clone(),
        redis_client.clone(),
    );

    // Initialize data access layer
    let mut service_urls = std::collections::HashMap::new();
    if let Some(comsrv_url) = config.get_service_url("comsrv") {
        service_urls.insert("comsrv".to_string(), comsrv_url.to_string());
    }
    if let Some(modsrv_url) = config.get_service_url("modsrv") {
        service_urls.insert("modsrv".to_string(), modsrv_url.to_string());
    }
    if let Some(hissrv_url) = config.get_service_url("hissrv") {
        service_urls.insert("hissrv".to_string(), hissrv_url.to_string());
    }
    if let Some(netsrv_url) = config.get_service_url("netsrv") {
        service_urls.insert("netsrv".to_string(), netsrv_url.to_string());
    }
    if let Some(alarmsrv_url) = config.get_service_url("alarmsrv") {
        service_urls.insert("alarmsrv".to_string(), alarmsrv_url.to_string());
    }

    let data_access_layer = Arc::new(HybridDataAccess::new(
        redis_client.clone(),
        http_client.clone(),
        service_urls.clone(),
        influxdb_client.clone(),
    )) as Arc<dyn DataAccessLayer>;

    // Initialize configuration sync service
    let config_sync_service = Arc::new(ConfigSyncService::new(
        redis_client.clone(),
        http_client.clone(),
    ));

    // Register services for configuration sync
    for (service_name, service_url) in service_urls.iter() {
        let service_config = data_access::sync::ServiceConfig::new(
            service_name.clone(),
            service_url.to_string(),
        );
        config_sync_service.register_service(service_config).await;
    }

    // Start configuration sync tasks
    config_sync_service.start_sync_tasks().await;

    info!("Data access layer and configuration sync service initialized");

    // Setup CORS
    let cors = if config.cors.allowed_origins.contains(&"*".to_string()) {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
            .expose_headers(Any)
            .max_age(std::time::Duration::from_secs(config.cors.max_age))
    } else {
        let mut cors_layer = CorsLayer::new();
        
        for origin in &config.cors.allowed_origins {
            cors_layer = cors_layer.allow_origin(origin.parse::<http::HeaderValue>().unwrap());
        }
        
        let methods: Vec<Method> = config.cors.allowed_methods
            .iter()
            .filter_map(|m| m.parse().ok())
            .collect();
            
        cors_layer
            .allow_methods(methods)
            .allow_headers(Any)
            .expose_headers(Any)
            .max_age(std::time::Duration::from_secs(config.cors.max_age))
    };

    // Build application routes
    let app = Router::new()
        // WebSocket endpoint (no auth required)
        .route("/ws", get(websocket::ws_handler))
        // Health check endpoint (no auth required)
        .route("/health", get(handlers::health::simple_health))
        // Public endpoints (no auth required)
        .route("/auth/login", post(handlers::auth::login))
        .route("/auth/refresh", post(handlers::auth::refresh_token))
        .route("/health/check", get(handlers::health::health_check))
        .route("/health/detailed", get(handlers::health::detailed_health))
        // OPTIONS preflight requests for CORS (bypass auth)
        .route("/api/*path", axum::routing::options(handle_options))
        // Protected API routes
        .nest(
            "/api",
            Router::new()
                .route("/auth/logout", post(handlers::auth::logout))
                .route("/auth/me", get(handlers::auth::current_user))
                // Channel management
                .route("/channels", get(handlers::channels::list_channels))
                .route("/channels/:id", get(handlers::channels::get_channel))
                .route("/channels/:id/telemetry", get(handlers::data::get_telemetry))
                .route("/channels/:id/signals", get(handlers::data::get_signals))
                .route("/channels/:id/control", post(handlers::data::send_control))
                .route("/channels/:id/adjustment", post(handlers::data::send_adjustment))
                .route("/channels/:id/points/:point_id/history", get(handlers::data::get_point_history))
                // Configuration management
                .route("/configs", get(handlers::config::list_configs))
                .route("/configs/:key", get(handlers::config::get_config))
                .route("/configs/:key", axum::routing::put(handlers::config::update_config))
                .route("/configs/:key", axum::routing::delete(handlers::config::delete_config))
                .route("/configs/sync/:service", post(handlers::config::trigger_sync))
                .route("/configs/sync/status", get(handlers::config::get_sync_status))
                .route("/configs/cache/clear", post(handlers::config::clear_cache))
                // Alarms
                .route("/alarms", get(handlers::data::get_alarms))
                .route("/alarms/active", get(handlers::data::get_active_alarms))
                .route("/alarms/:id/acknowledge", post(handlers::data::acknowledge_alarm))
                // Historical data
                .route("/historical", get(handlers::data::get_historical))
                // System info
                .route("/system/info", get(handlers::system::get_info))
                .route("/device-models", get(handlers::system::get_device_models))
                // Service proxies
                .nest("/comsrv", handlers::comsrv::proxy_routes())
                .nest("/modsrv", handlers::modsrv::proxy_routes())
                .nest("/hissrv", handlers::hissrv::proxy_routes())
                .nest("/netsrv", handlers::netsrv::proxy_routes())
                .nest("/alarmsrv", handlers::alarmsrv::proxy_routes())
                .nest("/rulesrv", handlers::rulesrv::proxy_routes())
                .layer(jwt_auth_layer())
        )
        // Global middleware
        .layer(
            ServiceBuilder::new()
                .layer(cors)
                .layer(middleware::from_fn(logging_middleware))
        )
        // Shared state
        .with_state(AppState {
            config: config.clone(),
            redis_client: redis_client.clone(),
            http_client: http_client.clone(),
            influxdb_client: influxdb_client.clone(),
            ws_hub: ws_hub.clone(),
            data_access_layer: data_access_layer.clone(),
            config_sync_service: config_sync_service.clone(),
        });

    // Create TCP listener
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    
    info!("API Gateway listening on {}", bind_addr);

    // Run server
    axum::serve(listener, app)
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

// Application state
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub redis_client: Arc<RedisClient>,
    pub http_client: Arc<reqwest::Client>,
    pub influxdb_client: Arc<InfluxDbClient>,
    pub ws_hub: Arc<tokio::sync::RwLock<websocket::hub::Hub>>,
    pub data_access_layer: Arc<dyn DataAccessLayer>,
    pub config_sync_service: Arc<ConfigSyncService>,
}

// Logging middleware
async fn logging_middleware(
    req: axum::extract::Request,
    next: middleware::Next,
) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    
    let response = next.run(req).await;
    
    let status = response.status();
    log::info!("{} {} -> {}", method, uri, status);
    
    response
}