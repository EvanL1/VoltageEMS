use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpServer, HttpResponse};
use env_logger::Env;
use log::info;
use std::sync::Arc;

mod auth;
mod config;
mod config_client;
mod error;
mod handlers;
mod redis_client;
mod response;
mod websocket;

use auth::middleware::JwtAuthMiddleware;
use config::Config;
use config_client::ConfigClient;
use redis_client::RedisClient;

// CORS OPTIONS 处理器
async fn handle_options() -> actix_web::Result<HttpResponse> {
    Ok(HttpResponse::Ok().finish())
}

#[actix_web::main]
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
            // Fallback to local configuration
            Arc::new(Config::load().expect("Failed to load local configuration"))
        }
    };

    let bind_addr = format!("{}:{}", config.server.host, config.server.port);
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

    // Create HTTP client for backend services
    let http_client = Arc::new(reqwest::Client::new());

    let workers = config.server.workers;

    // Start HTTP server
    HttpServer::new(move || {
        // Create WebSocket Hub
        let ws_hub = websocket::server::create_hub(redis_client.clone());
        
        // Start Redis subscriber for real-time data
        let _redis_subscriber = websocket::handlers::realtime::RedisSubscriber::start(
            ws_hub.clone(),
            redis_client.clone(),
        );
        let cors = if config.cors.allowed_origins.contains(&"*".to_string()) {
            // Allow all origins - 明确设置所有需要的头部
            Cors::default()
                .allow_any_origin()
                .allow_any_method()
                .allow_any_header()
                .expose_any_header()
                .max_age(config.cors.max_age as usize)
        } else {
            // Configure specific origins
            let mut cors = Cors::default();
            for origin in &config.cors.allowed_origins {
                cors = cors.allowed_origin(origin);
            }
            
            // Convert string methods to HttpMethod
            let methods: Vec<actix_web::http::Method> = config.cors.allowed_methods
                .iter()
                .filter_map(|m| match m.as_str() {
                    "GET" => Some(actix_web::http::Method::GET),
                    "POST" => Some(actix_web::http::Method::POST),
                    "PUT" => Some(actix_web::http::Method::PUT),
                    "DELETE" => Some(actix_web::http::Method::DELETE),
                    "OPTIONS" => Some(actix_web::http::Method::OPTIONS),
                    "HEAD" => Some(actix_web::http::Method::HEAD),
                    "PATCH" => Some(actix_web::http::Method::PATCH),
                    _ => None,
                })
                .collect();
                
            cors.allowed_methods(methods)
                .allowed_headers(&config.cors.allowed_headers)
                .expose_any_header()
                .supports_credentials()
                .max_age(config.cors.max_age as usize)
        };

        App::new()
            .app_data(web::Data::new(config.clone()))
            .app_data(web::Data::new(redis_client.clone()))
            .app_data(web::Data::new(http_client.clone()))
            .app_data(web::Data::new(ws_hub.clone()))
            .wrap(cors)
            .wrap(middleware::Logger::default())
            // WebSocket endpoint (no auth required, must be registered first)
            .route("/ws", web::get().to(websocket::ws_handler))
            // Health check endpoint (no auth required)
            .route("/health", web::get().to(handlers::health::simple_health))
            // Public endpoints (no auth required)
            .route("/auth/login", web::post().to(handlers::auth::login))
            .route("/auth/refresh", web::post().to(handlers::auth::refresh_token))
            .service(handlers::health::health_check)
            .service(handlers::health::detailed_health)
            // OPTIONS preflight requests for CORS (bypass auth)
            .route("/api/{path:.*}", web::method(actix_web::http::Method::OPTIONS).to(handle_options))
            // Protected endpoints (auth required)
            .service(
                web::scope("/api")
                    .wrap(JwtAuthMiddleware)
                    .route("/auth/logout", web::post().to(handlers::auth::logout))
                    .route("/auth/me", web::get().to(handlers::auth::current_user))
                    // Channel management
                    .route("/channels", web::get().to(handlers::channels::list_channels))
                    .route("/channels/{id}", web::get().to(handlers::channels::get_channel))
                    .route("/channels/{id}/telemetry", web::get().to(handlers::data::get_telemetry))
                    .route("/channels/{id}/signals", web::get().to(handlers::data::get_signals))
                    .route("/channels/{id}/control", web::post().to(handlers::data::send_control))
                    .route("/channels/{id}/adjustment", web::post().to(handlers::data::send_adjustment))
                    .route("/channels/{id}/points/{point_id}/history", web::get().to(handlers::data::get_point_history))
                    // Alarms
                    .route("/alarms", web::get().to(handlers::data::get_alarms))
                    .route("/alarms/active", web::get().to(handlers::data::get_active_alarms))
                    .route("/alarms/{id}/acknowledge", web::post().to(handlers::data::acknowledge_alarm))
                    // Historical data
                    .route("/historical", web::get().to(handlers::data::get_historical))
                    // System info
                    .route("/system/info", web::get().to(handlers::system::get_info))
                    .route("/device-models", web::get().to(handlers::system::get_device_models))
                    // Service proxies
                    .service(web::scope("/comsrv").service(handlers::comsrv::proxy_handler))
                    .service(web::scope("/modsrv").service(handlers::modsrv::proxy_handler))
                    .service(web::scope("/hissrv").service(handlers::hissrv::proxy_handler))
                    .service(web::scope("/netsrv").service(handlers::netsrv::proxy_handler))
                    .service(web::scope("/alarmsrv").service(handlers::alarmsrv::proxy_handler))
                    .service(web::scope("/rulesrv").service(handlers::rulesrv::proxy_handler))
            )
    })
    .workers(workers)
    .bind(&bind_addr)?
    .run()
    .await
}
