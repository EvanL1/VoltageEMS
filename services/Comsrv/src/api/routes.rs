use std::sync::Arc;
use tokio::sync::RwLock;
use warp::{Filter, Rejection, Reply};
use chrono::Utc;

use crate::api::handlers;
use crate::api::models::ChannelOperation;
use crate::core::protocol_factory::ProtocolFactory;

/// create API routes
pub fn api_routes(
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
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
    
    // channels related routes
    let channels = v1.and(warp::path("channels"));
    
    // get all channels
    let all_channels = channels
        .and(warp::get())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(handlers::get_all_channels);
    
    // get single channel status
    let channel_status = channels
        .and(warp::path::param::<String>())
        .and(warp::get())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(handlers::get_channel_status);
    
    // control channel operation
    let channel_control = channels
        .and(warp::path::param::<String>())
        .and(warp::path("control"))
        .and(warp::post())
        .and(warp::body::json::<ChannelOperation>())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(handlers::control_channel);
    
    // get all points for a channel
    let get_channel_points = channels
        .and(warp::path::param::<String>())
        .and(warp::path("points"))
        .and(warp::get())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(handlers::get_channel_points);
    
    // read point value
    let read_point = channels
        .and(warp::path::param::<String>())
        .and(warp::path("points"))
        .and(warp::path::param::<String>())  // point_table
        .and(warp::path::param::<String>())  // point_name
        .and(warp::get())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(handlers::read_point);
    
    // write point value
    let write_point = channels
        .and(warp::path::param::<String>())
        .and(warp::path("points"))
        .and(warp::path::param::<String>())  // point_table
        .and(warp::path::param::<String>())  // point_name
        .and(warp::put())
        .and(warp::body::json())
        .and(with_protocol_factory(protocol_factory.clone()))
        .and_then(handlers::write_point);
    
    // combine all routes
    health
        .or(status)
        .or(all_channels)
        .or(channel_status)
        .or(channel_control)
        .or(get_channel_points)
        .or(read_point)
        .or(write_point)
        .with(warp::cors().allow_any_origin())
}

/// add ProtocolFactory to request context
fn with_protocol_factory(
    factory: Arc<RwLock<ProtocolFactory>>,
) -> impl Filter<Extract = (Arc<RwLock<ProtocolFactory>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || factory.clone())
}

/// add start time to request context
fn with_start_time(
    start_time: Arc<chrono::DateTime<Utc>>,
) -> impl Filter<Extract = (Arc<chrono::DateTime<Utc>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || start_time.clone())
} 