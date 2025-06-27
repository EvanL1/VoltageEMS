/// OpenAPI Server Example
/// 
/// This example demonstrates how to run the Communication Service
/// with OpenAPI/Swagger documentation enabled.
/// 
/// Usage: cargo run --example openapi_server
/// 
/// Then visit:
/// - http://localhost:3030/swagger - Swagger UI
/// - http://localhost:3030/openapi.json - OpenAPI JSON spec
/// - http://localhost:3030/api/health - API health check

use std::net::SocketAddr;
use tokio;
use warp::Filter;

#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::init();
    
    println!("🚀 Starting OpenAPI Communication Service Server...");
    
    // Create the combined routes
    let api_routes = comsrv::api::openapi_routes::api_routes();
    let swagger_routes = comsrv::api::swagger::swagger_routes();
    
    // Combine all routes
    let routes = api_routes
        .or(swagger_routes)
        .with(warp::log("comsrv::openapi_server"));
    
    // Define server address
    let addr: SocketAddr = "127.0.0.1:3030".parse().unwrap();
    
    println!("📡 Server starting on http://{}", addr);
    println!("📚 Swagger UI available at: http://{}/swagger", addr);
    println!("📄 OpenAPI spec available at: http://{}/openapi.json", addr);
    println!("❤️  Health check at: http://{}/api/health", addr);
    println!("📊 Service status at: http://{}/api/status", addr);
    
    // Start the server
    warp::serve(routes)
        .run(addr)
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use warp::test::request;
    
    #[tokio::test]
    async fn test_openapi_server_routes() {
        let api_routes = comsrv::api::openapi_routes::api_routes();
        let swagger_routes = comsrv::api::swagger::swagger_routes();
        let routes = api_routes.or(swagger_routes);
        
        // Test swagger UI
        let resp = request()
            .method("GET")
            .path("/swagger")
            .reply(&routes)
            .await;
        assert_eq!(resp.status(), 200);
        
        // Test OpenAPI JSON
        let resp = request()
            .method("GET")
            .path("/openapi.json")
            .reply(&routes)
            .await;
        assert_eq!(resp.status(), 200);
        
        // Test health endpoint
        let resp = request()
            .method("GET")
            .path("/api/health")
            .reply(&routes)
            .await;
        assert_eq!(resp.status(), 200);
        
        // Test status endpoint
        let resp = request()
            .method("GET")
            .path("/api/status")
            .reply(&routes)
            .await;
        assert_eq!(resp.status(), 200);
    }
} 