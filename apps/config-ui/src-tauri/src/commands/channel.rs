use crate::models::channel::*;
use crate::models::point_table::CsvType;
use crate::services::channel_service::ChannelService;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_all_channels(
    service: State<'_, Arc<ChannelService>>,
) -> std::result::Result<Vec<ChannelInfo>, String> {
    service.get_all_channels().await
}

#[tauri::command]
pub async fn get_channel(
    service: State<'_, Arc<ChannelService>>,
    id: u32,
) -> std::result::Result<Channel, String> {
    service.get_channel(id).await
}

#[tauri::command]
pub async fn create_channel(
    service: State<'_, Arc<ChannelService>>,
    channel: Channel,
) -> std::result::Result<Channel, String> {
    service.create_channel(channel).await
}

#[tauri::command]
pub async fn update_channel(
    service: State<'_, Arc<ChannelService>>,
    id: u32,
    channel: Channel,
) -> std::result::Result<(), String> {
    service.update_channel(id, channel).await
}

#[tauri::command]
pub async fn delete_channel(
    service: State<'_, Arc<ChannelService>>,
    id: u32,
) -> std::result::Result<(), String> {
    service.delete_channel(id).await
}

#[tauri::command]
pub async fn upload_channel_csv(
    service: State<'_, Arc<ChannelService>>,
    channel_id: u32,
    csv_type: CsvType,
    content: String,
) -> std::result::Result<crate::models::point_table::ValidationResult, String> {
    service.upload_channel_csv(channel_id, csv_type, content).await
}

#[tauri::command]
pub async fn export_channel_csv(
    service: State<'_, Arc<ChannelService>>,
    channel_id: u32,
    csv_type: CsvType,
) -> std::result::Result<String, String> {
    service.export_channel_csv(channel_id, csv_type).await
}

#[tauri::command]
pub async fn get_channel_protocol_template(
    service: State<'_, Arc<ChannelService>>,
    protocol_type: String,
    csv_type: CsvType,
) -> std::result::Result<String, String> {
    service.get_channel_protocol_template(&protocol_type, csv_type).await
}

#[tauri::command]
pub async fn validate_channel_points(
    service: State<'_, Arc<ChannelService>>,
    channel_id: u32,
) -> std::result::Result<crate::models::point_table::ValidationResult, String> {
    service.validate_channel_points(channel_id).await
}

#[tauri::command]
pub async fn export_channel_config(
    service: State<'_, Arc<ChannelService>>,
    channel_id: u32,
) -> std::result::Result<String, String> {
    service.export_channel_config(channel_id).await
}

#[tauri::command]
pub async fn import_channel_config(
    service: State<'_, Arc<ChannelService>>,
    yaml_content: String,
) -> std::result::Result<Channel, String> {
    service.import_channel_config(yaml_content).await
}