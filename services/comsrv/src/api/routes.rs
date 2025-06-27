use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::{Filter, Rejection, Reply};

use crate::core::config::ConfigManager;
use crate::core::protocols::common::protocol_factory::ProtocolFactory;
use crate::api::handlers::{
    get_service_status, get_all_channels, get_channel_status, 
    control_channel, health_check, read_point, write_point,
    get_channel_points, get_point_tables, get_point_table
};

/// API routes configuration
pub fn api_routes(
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
    config_manager: Arc<RwLock<ConfigManager>>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let start_time = Arc::new(Utc::now());
    
    // Status routes
    let status_route = warp::path("status")
        .and(warp::get())
        .and(with_start_time(start_time.clone()))
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(get_service_status);

    // Health check route
    let health_route = warp::path("health")
        .and(warp::get())
        .and_then(health_check);

    // Channels routes
    let channels_list_route = warp::path("channels")
        .and(warp::get())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(get_all_channels);

    let channel_status_route = warp::path("channels")
        .and(warp::path::param())
        .and(warp::path("status"))
        .and(warp::get())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(get_channel_status);

    let channel_control_route = warp::path("channels")
        .and(warp::path::param())
        .and(warp::path("control"))
        .and(warp::post())
        .and(warp::body::json())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(control_channel);

    // Point data routes
    let read_point_route = warp::path("channels")
        .and(warp::path::param())
        .and(warp::path("points"))
        .and(warp::path::param())
        .and(warp::path::param())
        .and(warp::get())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(read_point);

    let write_point_route = warp::path("channels")
        .and(warp::path::param())
        .and(warp::path("points"))
        .and(warp::path::param())
        .and(warp::path::param())
        .and(warp::post())
        .and(warp::body::json())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(write_point);

    let channel_points_route = warp::path("channels")
        .and(warp::path::param())
        .and(warp::path("points"))
        .and(warp::get())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(get_channel_points);

    // Point table management routes
    let point_tables_route = warp::path("point-tables")
        .and(warp::get())
        .and(warp::path::end())
        .and(with_config_manager(config_manager.clone()))
        .and_then(get_point_tables);

    let point_table_route = warp::path("point-tables")
        .and(warp::path::param())
        .and(warp::get())
        .and(warp::path::end())
        .and(with_config_manager(config_manager.clone()))
        .and_then(get_point_table);

    // Combine all routes
    let api_routes = status_route
        .or(health_route)
        .or(channels_list_route)
        .or(channel_status_route)
        .or(channel_control_route)
        .or(read_point_route)
        .or(write_point_route)
        .or(channel_points_route)
        .or(point_tables_route)
        .or(point_table_route);

    // Add CORS and common middleware
    api_routes
        .with(warp::cors()
            .allow_any_origin()
            .allow_headers(vec!["content-type"])
            .allow_methods(vec!["GET", "POST", "PUT", "DELETE"]))
        .with(warp::log("comsrv::api"))
}

// Helper functions to inject dependencies
fn with_protocol_factory(
    factory: Arc<RwLock<ProtocolFactory>>
) -> impl Filter<Extract = (Arc<RwLock<ProtocolFactory>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || factory.clone())
}

fn with_config_manager(
    manager: Arc<RwLock<ConfigManager>>
) -> impl Filter<Extract = (Arc<RwLock<ConfigManager>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || manager.clone())
}

fn with_start_time(
    start_time: Arc<DateTime<Utc>>
) -> impl Filter<Extract = (Arc<DateTime<Utc>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || start_time.clone())
}
