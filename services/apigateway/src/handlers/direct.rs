use crate::auth::Claims;
use crate::direct_reader::{check_direct_read_permission, BatchReadRequest, DirectReadType};
use crate::error::ApiGatewayError;
use crate::response::ApiResponse;
use axum::{
    extract::{Extension, Path, State},
    response::IntoResponse,
    Json,
};

/// 直接读取单个数据
pub async fn direct_read(
    State(app_state): State<crate::AppState>,
    Extension(claims): Extension<Claims>,
    Path((type_str, id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiGatewayError> {
    // 解析数据类型
    let read_type = DirectReadType::from_str(&type_str)
        .ok_or_else(|| ApiGatewayError::BadRequest(format!("Invalid read type: {}", type_str)))?;

    // 检查权限
    if !check_direct_read_permission(&claims.roles, read_type) {
        return Err(ApiGatewayError::Forbidden(
            "Insufficient permissions for this data type".to_string(),
        ));
    }

    // 读取数据
    let data = app_state.direct_reader.read(read_type, &id).await?;

    Ok(Json(ApiResponse::success(data)))
}

/// 批量读取数据
pub async fn batch_read(
    State(app_state): State<crate::AppState>,
    Extension(claims): Extension<Claims>,
    Json(request): Json<BatchReadRequest>,
) -> Result<impl IntoResponse, ApiGatewayError> {
    // 检查权限
    let can_read_measurements =
        check_direct_read_permission(&claims.roles, DirectReadType::Measurements);
    let can_read_signals = check_direct_read_permission(&claims.roles, DirectReadType::Signals);
    let can_read_models = check_direct_read_permission(&claims.roles, DirectReadType::Models);

    // 根据权限过滤请求
    let filtered_request = BatchReadRequest {
        measurements: if can_read_measurements {
            request.measurements.clone()
        } else {
            vec![]
        },
        signals: if can_read_signals {
            request.signals.clone()
        } else {
            vec![]
        },
        models: if can_read_models {
            request.models.clone()
        } else {
            vec![]
        },
    };

    // 如果没有任何可读数据，返回错误
    if filtered_request.measurements.is_empty()
        && filtered_request.signals.is_empty()
        && filtered_request.models.is_empty()
    {
        return Err(ApiGatewayError::Forbidden(
            "No permission to read any requested data".to_string(),
        ));
    }

    // 批量读取
    let response = app_state.direct_reader.batch_read(filtered_request).await?;

    Ok(Json(ApiResponse::success(response)))
}
