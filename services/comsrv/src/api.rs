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
//! ```rust,ignore
//! use comsrv::api::routes::create_api_routes;
//! use std::sync::Arc;
//! use tokio::sync::RwLock;
//! use std::net::SocketAddr;
//!
//! #[tokio::main]
//! async fn main() {
//!     let app = create_api_routes(factory);
//!     let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
//!     
//!     let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
//!     axum::serve(listener, app).await.unwrap();
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
//!     "protocol": "modbus_tcp",
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

// Active API modules
pub mod routes;

// Handler modules
pub mod health_handlers;

// DTO definitions
pub mod dto;

// Handler modules in subdirectory
pub mod handlers {
    pub mod channel_handlers;
    pub mod channel_management_handlers;
    pub mod control_handlers;
    pub mod mapping_handlers;
    pub mod point_handlers;
}

// Future helper functions can be added here as needed.

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
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
        use crate::dto::*;

        // Test SuccessResponse creation
        let success_response = SuccessResponse::new("test data".to_string());
        assert_eq!(success_response.data, "test data");
        assert!(success_response.metadata.is_empty());

        // Test ErrorResponse creation
        let error_info = ErrorInfo::new("test error");
        let error_response = ErrorResponse {
            success: false,
            error: error_info,
        };
        assert_eq!(error_response.error.message, "test error");
        assert!(!error_response.success);
    }

    #[test]
    fn test_service_status_model() {
        use crate::dto::ServiceStatus;
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
        use crate::dto::ChannelStatus;
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
            protocol: "modbus_tcp".to_string(),
            connected: true,
            running: true,
            last_update: now,
            error_count: 0,
            last_error: None,
            statistics,
        };

        assert_eq!(status.id, 1);
        assert_eq!(status.name, "Test Channel");
        assert_eq!(status.protocol, "modbus_tcp");
        assert!(status.connected);
        assert!(status.running);
        assert_eq!(status.error_count, 0);
        assert_eq!(status.statistics.len(), 2);
    }

    #[tokio::test]
    async fn test_api_integration() {
        // Test that API components can work together
        use crate::dto::*;

        // Create test service status
        let service_status = ServiceStatus {
            name: "integration_test".to_string(),
            version: "1.0.0".to_string(),
            uptime: 1000,
            start_time: chrono::Utc::now(),
            channels: 2,
            active_channels: 2,
        };

        // Wrap it in a success response
        let response = SuccessResponse::new(service_status);

        // Verify the integration
        assert_eq!(response.data.name, "integration_test");
        assert_eq!(response.data.channels, 2);
        assert!(response.metadata.is_empty());
    }

    #[test]
    fn test_error_handling_in_api() {
        use crate::dto::{ErrorInfo, ErrorResponse};

        // Test error response creation
        let error_info = ErrorInfo::new("Something went wrong");
        let error_response = ErrorResponse {
            success: false,
            error: error_info,
        };

        assert_eq!(error_response.error.message, "Something went wrong");
        assert!(!error_response.success);
    }

    #[test]
    fn test_api_serialization() {
        use crate::dto::*;
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

        let response = SuccessResponse::new(service_status);

        // Test JSON serialization
        let json_result = serde_json::to_string(&response);
        assert!(json_result.is_ok());

        let json_str = json_result.unwrap();
        assert!(json_str.contains("serialization_test"));
        assert!(json_str.contains("data"));
    }

    // #[test]
    // fn test_openapi_generation() {
    //     let openapi = crate::api::routes::ApiDoc::openapi();
    //     assert_eq!(openapi.info.title, "Communication Service API");
    //     assert_eq!(openapi.info.version, "0.1.0");
    // }
}
