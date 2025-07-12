use crate::models::point_table::*;
use crate::services::point_table_service::PointTableService;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_point_tables(
    service: State<'_, Arc<PointTableService>>,
) -> std::result::Result<Vec<PointTableMetadata>, String> {
    service
        .get_all_tables()
        .await
        .map_err(|e| format!("Failed to get point tables: {}", e))
}

#[tauri::command]
pub async fn get_point_table(
    service: State<'_, Arc<PointTableService>>,
    id: String,
) -> std::result::Result<PointTable, String> {
    service
        .get_table(&id)
        .await
        .map_err(|e| format!("Failed to get point table: {}", e))
}

#[tauri::command]
pub async fn create_point_table(
    service: State<'_, Arc<PointTableService>>,
    name: String,
    protocol_type: String,
) -> std::result::Result<PointTableMetadata, String> {
    service
        .create_table(name, protocol_type)
        .await
        .map_err(|e| format!("Failed to create point table: {}", e))
}

#[tauri::command]
pub async fn delete_point_table(
    service: State<'_, Arc<PointTableService>>,
    id: String,
) -> std::result::Result<(), String> {
    service
        .delete_table(&id)
        .await
        .map_err(|e| format!("Failed to delete point table: {}", e))
}

#[tauri::command]
pub async fn upload_csv_file(
    service: State<'_, Arc<PointTableService>>,
    table_id: String,
    csv_type: CsvType,
    content: String,
) -> std::result::Result<ValidationResult, String> {
    service
        .upload_csv(&table_id, csv_type, content)
        .await
        .map_err(|e| format!("Failed to upload CSV: {}", e))
}

#[tauri::command]
pub async fn export_csv_file(
    service: State<'_, Arc<PointTableService>>,
    table_id: String,
    csv_type: CsvType,
) -> std::result::Result<String, String> {
    service
        .export_csv(&table_id, csv_type)
        .await
        .map_err(|e| format!("Failed to export CSV: {}", e))
}

#[tauri::command]
pub async fn validate_point_table(
    service: State<'_, Arc<PointTableService>>,
    table_id: String,
) -> std::result::Result<ValidationResult, String> {
    service
        .validate_table(&table_id)
        .await
        .map_err(|e| format!("Failed to validate point table: {}", e))
}

#[tauri::command]
pub async fn update_point(
    service: State<'_, Arc<PointTableService>>,
    table_id: String,
    point_type: String,
    point_id: u32,
    point_data: serde_json::Value,
) -> std::result::Result<(), String> {
    service
        .update_point(&table_id, &point_type, point_id, point_data)
        .await
        .map_err(|e| format!("Failed to update point: {}", e))
}

#[tauri::command]
pub async fn delete_point(
    service: State<'_, Arc<PointTableService>>,
    table_id: String,
    point_type: String,
    point_id: u32,
) -> std::result::Result<(), String> {
    service
        .delete_point(&table_id, &point_type, point_id)
        .await
        .map_err(|e| format!("Failed to delete point: {}", e))
}

#[tauri::command]
pub async fn export_to_comsrv_format(
    service: State<'_, Arc<PointTableService>>,
    table_id: String,
) -> std::result::Result<String, String> {
    service
        .export_to_comsrv_format(&table_id)
        .await
        .map_err(|e| format!("Failed to export to comsrv format: {}", e))
}

#[tauri::command]
pub async fn get_protocol_csv_template(
    service: State<'_, Arc<PointTableService>>,
    protocol_type: String,
    csv_type: CsvType,
) -> std::result::Result<String, String> {
    service
        .get_protocol_csv_template(&protocol_type, csv_type)
        .await
        .map_err(|e| format!("Failed to get CSV template: {}", e))
}