//! REST API Module
//!
//! This module provides a comprehensive REST API for the communication service,
//! built with axum for high performance and utoipa for `OpenAPI` documentation.
//!
//! # Architecture
//!
//! The API is structured around the following components:
//!
//! - **Routes** (`routes`): API endpoint definitions with axum handlers
//! - **Models** (`models`): Request/response models with `OpenAPI` schemas  
//! - **Documentation**: API documentation and endpoints
//!
//! # Features
//!
//! - **High Performance**: Built on axum for async request handling
//! - **`OpenAPI` 3.0**: Auto-generated documentation via utoipa
//! - **Type Safety**: Comprehensive request/response validation
//! - **CORS Support**: Cross-origin resource sharing for web clients
//! - **Error Handling**: Standardized error responses
//!
//! # API Structure
//!
//! ```text
//! /api/v1/
//! ├── status              - Service health and status
//! ├── channels/           - Channel management
//! │   ├── {id}/points     - Point data access
//! │   ├── {id}/start      - Start channel
//! │   └── {id}/stop       - Stop channel
//! ├── factory/            - Protocol factory information
//! └── openapi.json        - OpenAPI specification
//! ```
//!
//! # Usage
//!
//! ```rust,no_run
//! use comsrv::api::routes::create_api_routes;
//! use axum::Server;
//! use std::net::SocketAddr;
//!
//! #[tokio::main]
//! async fn main() {
//!     let app = create_api_routes();
//!     let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
//!     
//!     Server::bind(&addr)
//!         .serve(app.into_make_service())
//!         .await
//!         .unwrap();
//! }
//! ```
//!
//! # Request/Response Examples
//!
//! ## Get Service Status
//!
//! ```http
//! GET /api/v1/status
//! ```
//!
//! Response:
//! ```json
//! {
//!   "name": "ComsrvRust",
//!   "version": "0.1.0",
//!   "uptime": 3600,
//!   "channels": 5,
//!   "active_channels": 3
//! }
//! ```
//!
//! ## List Channels
//!
//! ```http
//! GET /api/v1/channels
//! ```
//!
//! Response:
//! ```json
//! [
//!   {
//!     "id": 1,
//!     "name": "Modbus Device 1",
//!     "protocol": "ModbusTcp",
//!     "connected": true,
//!     "running": true,
//!     "error_count": 0
//!   }
//! ]
//! ```
//!
//! # Error Responses
//!
//! All endpoints return standardized error responses:
//!
//! ```json
//! {
//!   "status": 404,
//!   "message": "Channel not found"
//! }
//! ```

// Legacy API modules - temporarily disabled
// pub mod handlers;
// pub mod routes;

// Active API modules
pub mod models;
pub mod routes;

// Future helper functions can be added here as needed.

#[cfg(test)]
mod tests {

    #[test]
    fn test_api_module_structure() {
        // Test that all API modules are accessible
        // This serves as a compilation check for the module structure
        // API module structure is valid if this compiles
    }

    #[test]
    fn test_models_module_access() {
        // Test that we can access models from the API module
        use crate::api::models::*;

        // Test ApiResponse creation
        let success_response: ApiResponse<String> = ApiResponse::success("test data".to_string());
        assert!(success_response.success);
        assert_eq!(success_response.data, Some("test data".to_string()));
        assert!(success_response.error.is_none());

        let error_response: ApiResponse<String> = ApiResponse::error("test error".to_string());
        assert!(!error_response.success);
        assert!(error_response.data.is_none());
        assert_eq!(error_response.error, Some("test error".to_string()));
    }

    #[test]
    fn test_service_status_model() {
        use crate::api::models::ServiceStatus;
        use chrono::Utc;

        let start_time = Utc::now();
        let status = ServiceStatus {
            name: "test_service".to_string(),
            version: "1.0.0".to_string(),
            uptime: 3600,
            start_time,
            channels: 5,
            active_channels: 3,
        };

        assert_eq!(status.name, "test_service");
        assert_eq!(status.version, "1.0.0");
        assert_eq!(status.uptime, 3600);
        assert_eq!(status.channels, 5);
        assert_eq!(status.active_channels, 3);
    }

    #[test]
    fn test_channel_status_model() {
        use crate::api::models::ChannelStatus;
        use chrono::Utc;
        use serde_json::json;
        use std::collections::HashMap;

        let mut statistics = HashMap::new();
        statistics.insert("host".to_string(), json!("127.0.0.1"));
        statistics.insert("port".to_string(), json!(502));

        let now = Utc::now();
        let status = ChannelStatus {
            id: 1,
            name: "Test Channel".to_string(),
            protocol: "ModbusTcp".to_string(),
            connected: true,
            running: true,
            last_update: now,
            error_count: 0,
            last_error: None,
            statistics,
        };

        assert_eq!(status.id, 1);
        assert_eq!(status.name, "Test Channel");
        assert_eq!(status.protocol, "ModbusTcp");
        assert!(status.connected);
        assert!(status.running);
        assert_eq!(status.error_count, 0);
        assert_eq!(status.statistics.len(), 2);
    }

    #[tokio::test]
    async fn test_api_integration() {
        // Test that API components can work together
        use crate::api::models::*;

        // Create test service status
        let service_status = ServiceStatus {
            name: "integration_test".to_string(),
            version: "1.0.0".to_string(),
            uptime: 1000,
            start_time: chrono::Utc::now(),
            channels: 2,
            active_channels: 2,
        };

        // Wrap it in an API response
        let response = ApiResponse::success(service_status);

        // Verify the integration
        assert!(response.success);
        assert!(response.data.is_some());
        assert!(response.error.is_none());

        let status = response.data.unwrap();
        assert_eq!(status.name, "integration_test");
        assert_eq!(status.channels, 2);
    }

    #[test]
    fn test_error_handling_in_api() {
        use crate::api::models::ApiResponse;

        // Test error response creation
        let error_response: ApiResponse<i32> =
            ApiResponse::error("Something went wrong".to_string());

        assert!(!error_response.success);
        assert!(error_response.data.is_none());
        assert_eq!(
            error_response.error,
            Some("Something went wrong".to_string())
        );
    }

    #[test]
    fn test_api_serialization() {
        use crate::api::models::*;
        use chrono::Utc;

        // Test that API models can be serialized
        let service_status = ServiceStatus {
            name: "serialization_test".to_string(),
            version: "1.0.0".to_string(),
            uptime: 500,
            start_time: Utc::now(),
            channels: 1,
            active_channels: 1,
        };

        let response = ApiResponse::success(service_status);

        // Test JSON serialization
        let json_result = serde_json::to_string(&response);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();
        assert!(json_str.contains("serialization_test"));
        assert!(json_str.contains("success"));
        assert!(json_str.contains("true"));
    }

    // #[test]
    // fn test_openapi_generation() {
    //     let openapi = crate::api::routes::ApiDoc::openapi();
    //     assert_eq!(openapi.info.title, "Communication Service API");
    //     assert_eq!(openapi.info.version, "0.1.0");
    // }
}
