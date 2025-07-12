mod commands;
mod error;
mod models;
mod services;

use services::{ConfigService, PointTableService, ChannelService};
use tauri::Manager;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter("voltage_config_ui=debug,info")
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 初始化配置服务
            let config_service = tauri::async_runtime::block_on(async {
                ConfigService::new("redis://localhost:6379")
                    .await
                    .expect("Failed to connect to Redis")
            });

            app.manage(std::sync::Arc::new(config_service));
            
            // 初始化点表服务
            let point_table_service = std::sync::Arc::new(PointTableService::new());
            app.manage(point_table_service.clone());
            
            // 初始化通道服务
            let channel_service = ChannelService::new(point_table_service);
            app.manage(std::sync::Arc::new(channel_service));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::get_all_services,
            commands::get_service_config,
            commands::update_service_config,
            commands::validate_config,
            commands::get_service_status,
            commands::get_config_diff,
            commands::import_config,
            commands::export_config,
            // Point table commands
            commands::get_point_tables,
            commands::get_point_table,
            commands::create_point_table,
            commands::delete_point_table,
            commands::upload_csv_file,
            commands::export_csv_file,
            commands::validate_point_table,
            commands::update_point,
            commands::delete_point,
            commands::export_to_comsrv_format,
            commands::get_protocol_csv_template,
            // Channel commands
            commands::get_all_channels,
            commands::get_channel,
            commands::create_channel,
            commands::update_channel,
            commands::delete_channel,
            commands::upload_channel_csv,
            commands::export_channel_csv,
            commands::get_channel_protocol_template,
            commands::validate_channel_points,
            commands::export_channel_config,
            commands::import_channel_config,
            // Logger commands
            commands::write_log,
            commands::get_log_path,
            commands::read_recent_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
