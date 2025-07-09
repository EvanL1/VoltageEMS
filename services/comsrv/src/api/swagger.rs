use axum::{
    response::{Html, Json},
    routing::get,
    Router,
};
use utoipa::OpenApi;

/// OpenAPI specification for Communication Service
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Communication Service API",
        version = "0.1.0",
        description = "Industrial communication service providing protocol abstraction and data access"
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "status", description = "Service status endpoints"),
        (name = "channels", description = "Channel management endpoints"),
        (name = "points", description = "Point data endpoints"),
        (name = "point-tables", description = "Point table management endpoints")
    )
)]
pub struct ApiDoc;

/// Generate OpenAPI JSON specification
pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// Swagger UI HTML page
pub async fn swagger_ui() -> Html<&'static str> {
    Html(include_str!("swagger_ui.html"))
}

/// Create Swagger routes
pub fn swagger_routes() -> Router {
    Router::new()
        // SwaggerUi integration removed due to compatibility issues
        .route("/api-docs/openapi.json", get(openapi_json))
        .route("/swagger", get(swagger_ui))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_generation() {
        let openapi = ApiDoc::openapi();
        assert_eq!(openapi.info.title, "Communication Service API");
        assert_eq!(openapi.info.version, "0.1.0");
    }
}
