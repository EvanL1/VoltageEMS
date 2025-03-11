//! Rule Service (RuleSrv)
//! Rule service - responsible for managing rule configuration and execution.

use common::service_bootstrap::ServiceInfo;
use rulesrv::Result;
use rulesrv::{create_app_state, create_routes};
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};
#[cfg(feature = "swagger-ui")]
use utoipa::OpenApi;
#[cfg(feature = "swagger-ui")]
use utoipa_swagger_ui::SwaggerUi;

#[tokio::main]
async fn main() -> Result<()> {
    // Define service info for unified bootstrap
    let service_info = ServiceInfo::new(
        "rulesrv",
        "Rule Engine Service - Intelligent Rule Processing & Automation",
        6003,
    );

    // Load environment variables from .env file
    common::service_bootstrap::load_development_env();

    // Initialize logging using service_bootstrap (API logging enabled by default)
    common::service_bootstrap::init_logging(&service_info).map_err(|e| {
        rulesrv::RuleSrvError::ConfigError(format!("Failed to initialize logging: {}", e))
    })?;
    // Enable SIGHUP-triggered log reopen for runtime log management
    common::logging::enable_sighup_log_reopen();

    // Print startup banner using service_bootstrap
    common::service_bootstrap::print_startup_banner(&service_info);

    info!("Starting Rule Service...");
    debug!("Log configuration initialized with file level: DEBUG, console level: INFO");

    // Create application state
    let state = create_app_state(&service_info).await?;

    // Create cancellation token for graceful shutdown
    let token = CancellationToken::new();

    // Start the rule execution task
    let task_token = token.clone();
    let rule_task = rulesrv::routes::start_rule_execution_task(state.clone(), task_token);

    // Get the port before moving state
    let service_port = state.config.api.port;

    // Create API routes
    let app = create_routes(state);

    #[cfg(feature = "swagger-ui")]
    let app = {
        info!("Swagger UI feature ENABLED - initializing at /docs");
        let openapi = rulesrv::routes::RulesrvApiDoc::openapi();
        let merged = app.merge(SwaggerUi::new("/docs").url("/openapi.json", openapi));
        info!("Swagger UI configured successfully");
        merged
    };

    #[cfg(not(feature = "swagger-ui"))]
    info!("Swagger UI feature DISABLED");

    // Start the HTTP service
    let addr = SocketAddr::from(([0, 0, 0, 0], service_port));

    // Create socket with SO_REUSEADDR to allow quick restart
    let socket = tokio::net::TcpSocket::new_v4()?;
    socket.set_reuseaddr(true)?;
    socket.bind(addr)?;
    let listener = socket.listen(1024)?;

    let actual_addr = listener.local_addr()?;

    info!("Rule Service started on {}", actual_addr);
    debug!("API endpoints:");
    debug!("  GET /health - Health check");
    debug!("  GET/POST /api/rules - Rule management");
    debug!("  POST /api/rules/:id/enable - Enable rule");
    debug!("  POST /api/rules/:id/disable - Disable rule");
    debug!("  GET /api/executions - Execution history");
    debug!("  GET /api/statistics - Rule statistics");

    // Setup graceful shutdown
    let shutdown_token = token.clone();
    let shutdown_signal = async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            error!("Failed to install CTRL+C signal handler: {}", e);
            return;
        }
        info!("Shutdown signal received, stopping rule service...");
        shutdown_token.cancel();
    };

    // Start server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await?;

    // Wait for background task to complete
    rule_task.abort();
    info!("Rule Service shutdown complete");

    Ok(())
}
