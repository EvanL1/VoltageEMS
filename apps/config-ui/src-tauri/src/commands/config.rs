use crate::error::Result;
use crate::models::*;
use crate::services::ConfigService;
use tauri::State;
use std::sync::Arc;

#[tauri::command]
pub async fn get_all_services(
    config_service: State<'_, Arc<ConfigService>>,
) -> Result<Vec<ServiceInfo>> {
    config_service.get_all_services().await
}

#[tauri::command]
pub async fn get_service_config(
    service: String,
    config_service: State<'_, Arc<ConfigService>>,
) -> Result<ServiceConfig> {
    config_service.get_service_config(&service).await
}

#[tauri::command]
pub async fn update_service_config(
    service: String,
    config: serde_json::Value,
    config_service: State<'_, Arc<ConfigService>>,
) -> Result<()> {
    config_service.update_service_config(&service, config).await
}

#[tauri::command]
pub async fn validate_config(
    service: String,
    config: serde_json::Value,
    config_service: State<'_, Arc<ConfigService>>,
) -> Result<ValidationResult> {
    config_service.validate_config(&service, &config).await
}

#[tauri::command]
pub async fn get_service_status(
    service: String,
    config_service: State<'_, Arc<ConfigService>>,
) -> Result<ServiceInfo> {
    config_service.get_service_info(&service).await
}

#[tauri::command]
pub async fn get_config_diff(
    service: String,
    version1: String,
    version2: String,
    config_service: State<'_, Arc<ConfigService>>,
) -> Result<DiffResult> {
    config_service
        .get_config_diff(&service, &version1, &version2)
        .await
}

#[tauri::command]
pub async fn import_config(
    _service: String,
    file_path: String,
) -> Result<()> {
    // TODO: 实现配置导入
    let config_data = std::fs::read_to_string(file_path)?;
    let _config: serde_json::Value = serde_json::from_str(&config_data)?;
    
    Ok(())
}

#[tauri::command]
pub async fn export_config(
    service: String,
    file_path: String,
    config_service: State<'_, Arc<ConfigService>>,
) -> Result<()> {
    let config = config_service.get_service_config(&service).await?;
    let config_json = serde_json::to_string_pretty(&config)?;
    std::fs::write(file_path, config_json)?;
    
    Ok(())
}