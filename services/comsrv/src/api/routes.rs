use std::sync::Arc;
use tokio::sync::RwLock;
use warp::{Filter, Rejection, Reply};
use chrono::Utc;

use crate::api::handlers;
use crate::api::models::ChannelOperation;
use crate::core::protocols::common::ProtocolFactory;
use crate::core::config::ConfigManager;

/// create API routes
pub fn api_routes(
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
    config_manager: Arc<RwLock<ConfigManager>>,
    start_time: Arc<chrono::DateTime<Utc>>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let api_base = warp::path("api");
    let v1 = api_base.and(warp::path("v1"));
    
    // health check route
    let health = warp::path("health")
        .and(with_start_time(start_time.clone()))
        .and_then(handlers::health_check);

    // service status route    
    let status = v1.and(warp::path("status"))
        .and(warp::get())
        .and(with_start_time(start_time.clone()))
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(handlers::get_service_status);
    
    // channels related routes - detailed channel routes
    let channels_base = v1.and(warp::path("channels"));
    
    // most specific channel routes with multiple path parameters
    let read_point = channels_base
        .and(warp::path::param::<String>())  // channel_id
        .and(warp::path("points"))
        .and(warp::path::param::<String>())  // point_table
        .and(warp::path::param::<String>())  // point_name
        .and(warp::path::end())
        .and(warp::get())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(handlers::read_point);
    
    let write_point = channels_base
        .and(warp::path::param::<String>())  // channel_id
        .and(warp::path("points"))
        .and(warp::path::param::<String>())  // point_table
        .and(warp::path::param::<String>())  // point_name
        .and(warp::path::end())
        .and(warp::put())
        .and(warp::body::json())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(handlers::write_point);

    // moderately specific channel routes
    let get_channel_points = channels_base
        .and(warp::path::param::<String>())  // channel_id
        .and(warp::path("points"))
        .and(warp::path::end())
        .and(warp::get())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(handlers::get_channel_points);
    
    let channel_control = channels_base
        .and(warp::path::param::<String>())  // channel_id
        .and(warp::path("control"))
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json::<ChannelOperation>())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(handlers::control_channel);
    
    // single channel status route
    let channel_status = channels_base
        .and(warp::path::param::<String>())  // channel_id
        .and(warp::path::end())
        .and(warp::get())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(handlers::get_channel_status);
    
    // route for all channels
    let all_channels = channels_base
        .and(warp::path::end())
        .and(warp::get())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(handlers::get_all_channels);

    // Point tables management routes
    let point_tables_base = v1.and(warp::path("point-tables"));
    
    // most specific point-table routes
    let point_from_table = point_tables_base
        .and(warp::path::param::<String>())  // table_name
        .and(warp::path("points"))
        .and(warp::path::param::<String>())  // point_id
        .and(warp::path::end())
        .and(warp::get())
        .and(with_config_manager(config_manager.clone()))
        .and_then(handlers::get_point_from_table);
    
    let upsert_point = point_tables_base
        .and(warp::path::param::<String>())  // table_name
        .and(warp::path("points"))
        .and(warp::path::end())
        .and(warp::put())
        .and(warp::body::json())
        .and(with_config_manager(config_manager.clone()))
        .and_then(handlers::upsert_point_in_table);
    
    let delete_point = point_tables_base
        .and(warp::path::param::<String>())  // table_name
        .and(warp::path("points"))
        .and(warp::path::param::<String>())  // point_id
        .and(warp::path::end())
        .and(warp::delete())
        .and(with_config_manager(config_manager.clone()))
        .and_then(handlers::delete_point_from_table);
    
    let export_table = point_tables_base
        .and(warp::path::param::<String>())  // table_name
        .and(warp::path("export"))
        .and(warp::path::end())
        .and(warp::get())
        .and(with_config_manager(config_manager.clone()))
        .and_then(handlers::export_point_table);
    
    let reload_tables = point_tables_base
        .and(warp::path("reload"))
        .and(warp::path::end())
        .and(warp::post())
        .and(with_config_manager(config_manager.clone()))
        .and_then(handlers::reload_point_tables);
    
    // single point-table route
    let point_table_details = point_tables_base
        .and(warp::path::param::<String>())  // table_name
        .and(warp::path::end())
        .and(warp::get())
        .and(with_config_manager(config_manager.clone()))
        .and_then(handlers::get_point_table);
    
    // route for all point tables
    let all_point_tables = point_tables_base
        .and(warp::path::end())
        .and(warp::get())
        .and(with_config_manager(config_manager.clone()))
        .and_then(handlers::get_point_tables);
    
    // combine all routes ordered by specificity
    health
        .or(status)
        .or(read_point)           // /channels/{id}/points/{table}/{point}
        .or(write_point)          // /channels/{id}/points/{table}/{point}
        .or(get_channel_points)   // /channels/{id}/points
        .or(channel_control)      // /channels/{id}/control
        .or(channel_status)       // /channels/{id}
        .or(all_channels)         // /channels
        .or(point_from_table)     // /point-tables/{table}/points/{point}
        .or(upsert_point)         // /point-tables/{table}/points
        .or(delete_point)         // /point-tables/{table}/points/{point}
        .or(export_table)         // /point-tables/{table}/export
        .or(reload_tables)        // /point-tables/reload
        .or(point_table_details)  // /point-tables/{table}
        .or(all_point_tables)     // /point-tables
        .with(warp::cors().allow_any_origin())
}

/// add ProtocolFactory to request context
fn with_protocol_factory(
    factory: Arc<RwLock<ProtocolFactory>>,
) -> impl Filter<Extract = (Arc<RwLock<ProtocolFactory>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || factory.clone())
}

/// add ConfigManager to request context
fn with_config_manager(
    config_manager: Arc<RwLock<ConfigManager>>,
) -> impl Filter<Extract = (Arc<RwLock<ConfigManager>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || config_manager.clone())
}

/// add start time to request context
fn with_start_time(
    start_time: Arc<chrono::DateTime<Utc>>,
) -> impl Filter<Extract = (Arc<chrono::DateTime<Utc>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || start_time.clone())
} 