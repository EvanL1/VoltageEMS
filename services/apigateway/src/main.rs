//! apigateway – API Gateway service (Rust)
//!
//! Unified entry for the VoltageEMS front-end:
//! - JWT auth (users, roles)
//! - WebSocket real-time data push with subscriptions
//! - POST /broadcast – push any JSON to all WebSocket clients
//! - GET /api/v1/homepage – calculated points CRUD
//! - GET/PUT /api/v1/network – systemd-networkd config
//! - GET/POST /api/v1/config – config export/import, restart, upgrade

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::{Query, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::{delete, get, post, put},
    Router,
};
use dashmap::DashMap;
use serde::Deserialize;
use tower_http::cors::{Any, CorsLayer};
use tokio_util::sync::CancellationToken;
use tracing::info;
use utoipa::OpenApi;
use utoipa_swagger_ui::{Config, SwaggerUi};

mod auth;
mod config;
mod db;
mod models;
mod routes_auth;
mod routes_broadcast;
mod routes_config;
mod routes_homepage;
mod routes_network;
mod state;
mod ws;

use crate::config::GatewayConfig;
use crate::state::AppState;
use crate::ws::WsHub;

// ── OpenAPI / Swagger UI ──────────────────────────────────────────────────────

#[derive(OpenApi)]
#[openapi(
    paths(
        routes_auth::register,
        routes_auth::login,
        routes_auth::refresh_token,
        routes_auth::logout,
        routes_auth::get_me,
        routes_auth::update_me,
        routes_auth::change_password,
        routes_auth::get_roles,
        routes_auth::get_all_users,
        routes_auth::admin_get_user,
        routes_auth::admin_update_user,
        routes_auth::admin_delete_user,
        routes_auth::get_auth_stats,
        routes_auth::cleanup_tokens,
        routes_broadcast::broadcast_message,
        routes_broadcast::broadcast_status,
        routes_homepage::list_points,
        routes_homepage::get_point,
        routes_homepage::update_point,
        routes_homepage::reset_points,
        routes_network::get_network_config,
        routes_network::update_network_config,
        routes_network::apply_network_config,
        routes_config::check_config,
        routes_config::export_config,
        routes_config::import_config,
        routes_config::restart_services,
        routes_config::start_upgrade,
        routes_config::abort_upgrade,
        routes_config::upgrade_status,
    ),
    components(schemas(
        models::UserCreate,
        models::UserLogin,
        models::UserUpdate,
        models::PasswordChange,
        models::RefreshTokenRequest,
        models::TokenResponse,
        models::Role,
        models::RoleInfo,
        models::UserWithRole,
        models::CalculatedPoint,
        models::CalculatedPointUpdate,
        models::NetworkConfig,
        models::NetworkUpdateRequest,
    )),
    tags(
        (name = "Auth", description = "认证与用户管理"),
        (name = "Homepage", description = "首页计算点位 CRUD"),
        (name = "Network", description = "网络接口配置"),
        (name = "Config", description = "系统配置导出/导入/升级"),
        (name = "WebSocket", description = "WebSocket 广播与状态"),
    ),
    modifiers(&SecurityAddon),
    info(title = "VoltageEMS API Gateway", version = "1.0.0", description = "VoltageEMS 统一 API 网关")
)]
struct ApiDoc;

struct SecurityAddon;
impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::HttpBuilder::new()
                        .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        }
    }
}

// ── WebSocket endpoint ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct WsParams {
    client_id: Option<String>,
    data_type: Option<String>,
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsParams>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let client_id = params
        .client_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let data_type = params.data_type.unwrap_or_else(|| "general".to_string());
    let hub = Arc::clone(&state.ws_hub);

    ws.on_upgrade(move |socket| ws::handle_socket(socket, client_id, data_type, hub))
}

// ── Router ────────────────────────────────────────────────────────────────────

fn build_router(state: Arc<AppState>) -> Router {
    let auth_routes = Router::new()
        .route("/register", post(routes_auth::register))
        .route("/login", post(routes_auth::login))
        .route("/refresh", post(routes_auth::refresh_token))
        .route("/logout", post(routes_auth::logout))
        .route("/me", get(routes_auth::get_me).put(routes_auth::update_me))
        .route("/me/password", put(routes_auth::change_password))
        .route("/roles", get(routes_auth::get_roles))
        .route("/users", get(routes_auth::get_all_users))
        .route("/users/{id}", get(routes_auth::admin_get_user))
        .route("/users/{id}", put(routes_auth::admin_update_user))
        .route("/users/{id}", delete(routes_auth::admin_delete_user))
        .route("/stats", get(routes_auth::get_auth_stats))
        .route("/cleanup-tokens", post(routes_auth::cleanup_tokens));

    let homepage_routes = Router::new()
        .route("/", get(routes_homepage::list_points))
        .route("/reset", post(routes_homepage::reset_points))
        .route("/{id}", get(routes_homepage::get_point))
        .route("/{id}", put(routes_homepage::update_point));

    let network_routes = Router::new()
        .route("/", get(routes_network::get_network_config))
        .route("/", put(routes_network::update_network_config))
        .route("/apply", post(routes_network::apply_network_config));

    let config_routes = Router::new()
        .route("/check", get(routes_config::check_config))
        .route("/export", get(routes_config::export_config))
        .route("/import", post(routes_config::import_config))
        .route("/restart-services", post(routes_config::restart_services))
        .route("/upgrade", post(routes_config::start_upgrade))
        .route("/upgrade/abort", post(routes_config::abort_upgrade))
        .route("/upgrade/status", get(routes_config::upgrade_status));

    let api_v1 = Router::new()
        .route("/broadcast", post(routes_broadcast::broadcast_message))
        .route("/broadcast/status", get(routes_broadcast::broadcast_status))
        .nest("/auth", auth_routes)
        .nest("/homepage", homepage_routes)
        .nest("/network", network_routes)
        .nest("/config", config_routes);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", get(|| async { "VoltageEMS API Gateway" }))
        .route("/health", get(|| async { "ok" }))
        .route("/ws", get(ws_handler))
        .nest("/api/v1", api_v1)
        .merge(
            SwaggerUi::new("/docs")
                .url("/openapi.json", ApiDoc::openapi())
                .config(
                    Config::default()
                        .default_model_rendering("model")
                        .default_models_expand_depth(1),
                ),
        )
        .with_state(state)
        .layer(cors)
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = GatewayConfig::default();

    // ── Logging ───────────────────────────────────────────────────────────────
    common::logging::init_log_root(Some(&cfg.log_dir));
    common::logging::init_with_config(common::logging::LogConfig {
        service_name: "apigateway".to_string(),
        log_dir: std::path::PathBuf::from(&cfg.log_dir),
        console_level: tracing::Level::INFO,
        file_level: tracing::Level::DEBUG,
        ..Default::default()
    })
    .map_err(|e| anyhow::anyhow!("Failed to init logging: {}", e))?;

    info!("apigateway starting on port {}", cfg.api_port);
    info!("Redis: {}", cfg.redis_url);
    info!("DB:    {}", cfg.db_path);

    // ── SQLite ────────────────────────────────────────────────────────────────
    let db_dir = std::path::Path::new(&cfg.db_path)
        .parent()
        .unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(db_dir)?;

    let db_pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&format!("sqlite:{}?mode=rwc", cfg.db_path))
        .await
        .map_err(|e| anyhow::anyhow!("SQLite connect failed: {} path={}", e, cfg.db_path))?;

    db::create_tables(&db_pool).await?;
    db::init_roles(&db_pool).await?;
    db::init_calculated_points(&db_pool).await?;

    // ── Default admin user ────────────────────────────────────────────────────
    if db::get_user_by_username(&db_pool, "admin").await?.is_none() {
        // Default password is MD5 of "admin123" = "0192023a7bbd73250516f069df18b500"
        let default_md5 = "0192023a7bbd73250516f069df18b500";
        let hash = auth::hash_password(default_md5)?;
        db::create_user(&db_pool, "admin", &hash, 1).await?;
        info!("Created default admin user (password: admin123)");
    }

    // ── Redis ─────────────────────────────────────────────────────────────────
    let (_, redis_client) =
        common::bootstrap_database::setup_redis_connection(Some(cfg.redis_url.clone()))
            .await
            .map_err(|e| anyhow::anyhow!("Redis connect failed: {}", e))?;

    let rtdb = Arc::new(voltage_rtdb::RedisRtdb::from_client(redis_client));

    // ── App State ─────────────────────────────────────────────────────────────
    let ws_hub = WsHub::new(Arc::clone(&rtdb));

    let state = Arc::new(AppState {
        db: db_pool,
        rtdb,
        config: Arc::new(cfg),
        ws_hub: Arc::clone(&ws_hub),
        refresh_tokens: DashMap::new(),
    });

    // ── Background tasks ──────────────────────────────────────────────────────
    let shutdown = CancellationToken::new();

    let hb_hub = Arc::clone(&ws_hub);
    let hb_shutdown = shutdown.clone();
    tokio::spawn(async move {
        ws::run_heartbeat(hb_hub, hb_shutdown).await;
    });

    let push_hub = Arc::clone(&ws_hub);
    let push_shutdown = shutdown.clone();
    let push_interval = state.config.data_fetch_interval_secs;
    tokio::spawn(async move {
        ws::run_data_push(push_hub, push_shutdown, push_interval).await;
    });

    // ── HTTP server ───────────────────────────────────────────────────────────
    let app = build_router(Arc::clone(&state))
        .layer(axum::middleware::from_fn(
            common::logging::http_request_logger,
        ));

    let bind_addr: SocketAddr = format!("{}:{}", state.config.api_host, state.config.api_port)
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid bind address: {}", e))?;

    let socket = tokio::net::TcpSocket::new_v4()?;
    socket.set_reuseaddr(true)?;
    socket.bind(bind_addr)?;
    let listener = socket.listen(1024)?;

    info!("Listening on {}", bind_addr);

    let serve_shutdown = shutdown.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move { serve_shutdown.cancelled().await })
        .await?;

    shutdown.cancel();
    common::logging::shutdown_logging_tasks().await;
    info!("apigateway stopped");
    Ok(())
}
