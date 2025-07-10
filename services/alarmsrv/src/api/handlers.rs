//! API handlers for alarm service

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use tracing::{error, info, warn};

use crate::api::models::*;
use crate::domain::{Alarm, AlarmStatistics};
use crate::AppState;

/// Health check endpoint
pub async fn health_check() -> &'static str {
    HEALTH_OK
}

/// Get system status
pub async fn get_status(State(state): State<AppState>) -> Json<StatusResponse> {
    let alarms = state.alarms.read().await;
    let active_count = alarms.iter().filter(|a| a.is_active()).count();
    let redis_status = state.redis_client.is_connected().await;

    Json(StatusResponse {
        service: "alarmsrv".to_string(),
        status: "running".to_string(),
        total_alarms: alarms.len(),
        active_alarms: active_count,
        redis_connected: redis_status,
        classifier_rules: state.classifier.get_rule_count(),
    })
}

/// Get alarm statistics
pub async fn get_statistics(
    State(state): State<AppState>,
) -> Result<Json<AlarmStatistics>, StatusCode> {
    match state.stats_manager.get_alarm_statistics().await {
        Ok(stats) => Ok(Json(stats)),
        Err(e) => {
            error!("Failed to get alarm statistics: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get alarm list with optional filtering
pub async fn list_alarms(
    State(state): State<AppState>,
    Query(query): Query<AlarmQuery>,
) -> Result<Json<AlarmListResponse>, StatusCode> {
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(10).min(100); // Max 100 items per page

    match state
        .query_service
        .get_alarms_paginated(
            query.category,
            query.level,
            query.status,
            query.start_time,
            query.end_time,
            query.keyword,
            offset,
            limit,
        )
        .await
    {
        Ok((alarms, total)) => Ok(Json(AlarmListResponse {
            alarms,
            total,
            offset,
            limit,
        })),
        Err(e) => {
            error!("Failed to get alarms: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Create new alarm
pub async fn create_alarm(
    State(state): State<AppState>,
    Json(request): Json<CreateAlarmRequest>,
) -> Result<Json<Alarm>, StatusCode> {
    let mut alarm = Alarm::new(request.title, request.description, request.level);

    // Classify the alarm
    let classification = state.classifier.classify(&alarm).await;
    alarm.set_classification(classification);

    // Store in Redis
    if let Err(e) = state.alarm_store.store_alarm(&alarm).await {
        error!("Failed to store alarm in Redis: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Store in memory for quick access
    let mut alarms = state.alarms.write().await;
    alarms.push(alarm.clone());

    // Publish for cloud push via netsrv
    if let Err(e) = state.alarm_store.publish_alarm_for_cloud(&alarm).await {
        warn!("Failed to publish alarm for cloud push: {}", e);
    }

    info!(
        "Created new alarm: {} (Category: {})",
        alarm.title, alarm.classification.category
    );
    Ok(Json(alarm))
}

/// Acknowledge alarm
pub async fn acknowledge_alarm(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Alarm>, StatusCode> {
    // Update in Redis first
    match state
        .alarm_store
        .acknowledge_alarm(&id, "system".to_string())
        .await
    {
        Ok(alarm) => {
            // Update in memory
            let mut alarms = state.alarms.write().await;
            if let Some(mem_alarm) = alarms.iter_mut().find(|a| a.id.to_string() == id) {
                mem_alarm.acknowledge("system".to_string());
            }

            // Publish status update for cloud
            if let Err(e) = state.alarm_store.publish_alarm_for_cloud(&alarm).await {
                warn!("Failed to publish alarm update for cloud: {}", e);
            }

            info!("Alarm acknowledged: {}", alarm.title);
            Ok(Json(alarm))
        }
        Err(e) => {
            error!("Failed to acknowledge alarm: {}", e);
            Err(StatusCode::NOT_FOUND)
        }
    }
}

/// Resolve alarm
pub async fn resolve_alarm(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Alarm>, StatusCode> {
    // Update in Redis first
    match state
        .alarm_store
        .resolve_alarm(&id, "system".to_string())
        .await
    {
        Ok(alarm) => {
            // Update in memory
            let mut alarms = state.alarms.write().await;
            if let Some(mem_alarm) = alarms.iter_mut().find(|a| a.id.to_string() == id) {
                mem_alarm.resolve("system".to_string());
            }

            // Publish status update for cloud
            if let Err(e) = state.alarm_store.publish_alarm_for_cloud(&alarm).await {
                warn!("Failed to publish alarm resolution for cloud: {}", e);
            }

            info!("Alarm resolved: {}", alarm.title);
            Ok(Json(alarm))
        }
        Err(e) => {
            error!("Failed to resolve alarm: {}", e);
            Err(StatusCode::NOT_FOUND)
        }
    }
}

/// Classify existing alarms
pub async fn classify_alarms(
    State(state): State<AppState>,
) -> Result<Json<ClassificationResult>, StatusCode> {
    let mut classified_count = 0;
    let mut failed_count = 0;

    // Get unclassified alarms from Redis
    match state.query_service.get_unclassified_alarms().await {
        Ok(alarms) => {
            for mut alarm in alarms {
                let classification = state.classifier.classify(&alarm).await;
                alarm.set_classification(classification);

                match state.alarm_store.update_alarm_classification(&alarm).await {
                    Ok(_) => {
                        classified_count += 1;
                        // Publish updated classification for cloud
                        if let Err(e) = state.alarm_store.publish_alarm_for_cloud(&alarm).await {
                            warn!("Failed to publish classified alarm for cloud: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to update alarm classification: {}", e);
                        failed_count += 1;
                    }
                }
            }
        }
        Err(e) => {
            error!("Failed to get unclassified alarms: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    Ok(Json(ClassificationResult {
        classified_count,
        failed_count,
    }))
}

/// Get alarm categories
pub async fn get_alarm_categories(
    State(state): State<AppState>,
) -> Json<Vec<crate::domain::AlarmCategory>> {
    Json(state.classifier.get_categories())
}
