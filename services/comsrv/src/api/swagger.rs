use rweb::Filter;
use serde_json;
use warp::{Reply, Rejection};

/// Generate OpenAPI JSON specification
pub async fn openapi_json() -> Result<impl Reply, Rejection> {
    let json = crate::api::openapi_routes::get_openapi_spec();
    
    Ok(warp::reply::with_header(
        json,
        "content-type",
        "application/json"
    ))
}

/// Swagger UI HTML page
pub async fn swagger_ui() -> Result<impl Reply, Rejection> {
    let html = include_str!("swagger_ui.html");
    Ok(warp::reply::html(html))
}

/// OpenAPI and Swagger routes
pub fn swagger_routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let openapi_json_route = warp::path("openapi.json")
        .and(warp::get())
        .and_then(openapi_json);
    
    let swagger_ui_route = warp::path("swagger")
        .and(warp::get())
        .and_then(swagger_ui);
    
    openapi_json_route.or(swagger_ui_route)
}

#[derive(Debug)]
struct OpenApiError;
impl warp::reject::Reject for OpenApiError {}

#[cfg(test)]
mod tests {
    use super::*;
    use warp::test::request;
    
    #[tokio::test]
    async fn test_openapi_json_endpoint() {
        let routes = swagger_routes();
        
        let resp = request()
            .method("GET")
            .path("/openapi.json")
            .reply(&routes)
            .await;
        
        assert_eq!(resp.status(), 200);
        assert!(resp.headers().get("content-type").is_some());
    }
    
    #[tokio::test]
    async fn test_swagger_ui_endpoint() {
        let routes = swagger_routes();
        
        let resp = request()
            .method("GET")
            .path("/swagger")
            .reply(&routes)
            .await;
        
        assert_eq!(resp.status(), 200);
    }
} 