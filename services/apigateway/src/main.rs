use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpServer};
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

use auth::middleware::JwtAuthMiddleware;
use config::Config;
use config_client::ConfigClient;
use redis_client::RedisClient;

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
        let cors = Cors::default()
            .allowed_origin_fn(|origin, _req_head| {
                origin.as_bytes().starts_with(b"http://localhost")
            })
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
            .allowed_headers(vec!["Content-Type", "Authorization"])
            .max_age(3600);

        App::new()
            .app_data(web::Data::new(config.clone()))
            .app_data(web::Data::new(redis_client.clone()))
            .app_data(web::Data::new(http_client.clone()))
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .service(
                web::scope("/api/v1")
                    // Public endpoints (no auth required)
                    .route("/auth/login", web::post().to(handlers::auth::login))
                    .route(
                        "/auth/refresh",
                        web::post().to(handlers::auth::refresh_token),
                    )
                    .service(handlers::health::health_check)
                    .service(handlers::health::detailed_health)
                    // Protected endpoints (auth required)
                    .service(
                        web::scope("")
                            .wrap(JwtAuthMiddleware)
                            .route("/auth/logout", web::post().to(handlers::auth::logout))
                            .route("/auth/me", web::get().to(handlers::auth::current_user))
                            .service(web::scope("/comsrv").service(handlers::comsrv::proxy_handler))
                            .service(web::scope("/modsrv").service(handlers::modsrv::proxy_handler))
                            .service(web::scope("/hissrv").service(handlers::hissrv::proxy_handler))
                            .service(web::scope("/netsrv").service(handlers::netsrv::proxy_handler))
                            .service(
                                web::scope("/alarmsrv").service(handlers::alarmsrv::proxy_handler),
                            )
                            .service(
                                web::scope("/rulesrv").service(handlers::rulesrv::proxy_handler),
                            ),
                    ),
            )
            .route("/health", web::get().to(handlers::health::simple_health))
    })
    .workers(workers)
    .bind(&bind_addr)?
    .run()
    .await
}
