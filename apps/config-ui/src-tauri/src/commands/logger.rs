use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use tauri::{AppHandle, Manager};

/// 写入日志到文件
#[tauri::command]
pub async fn write_log(app: AppHandle, entry: String) -> Result<(), String> {
    // 获取应用数据目录
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    
    // 创建日志目录
    let log_dir = app_dir.join("logs");
    create_dir_all(&log_dir)
        .map_err(|e| format!("Failed to create log directory: {}", e))?;
    
    // 生成日志文件名（按日期）
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let log_file = log_dir.join(format!("voltage-config-{}.log", today));
    
    // 写入日志
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .map_err(|e| format!("Failed to open log file: {}", e))?;
    
    writeln!(file, "{}", entry)
        .map_err(|e| format!("Failed to write log: {}", e))?;
    
    Ok(())
}

/// 获取日志文件路径
#[tauri::command]
pub async fn get_log_path(app: AppHandle) -> Result<String, String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    
    let log_dir = app_dir.join("logs");
    Ok(log_dir.to_string_lossy().to_string())
}

/// 读取最近的日志
#[tauri::command]
pub async fn read_recent_logs(app: AppHandle, lines: usize) -> Result<Vec<String>, String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let log_file = app_dir.join("logs").join(format!("voltage-config-{}.log", today));
    
    if !log_file.exists() {
        return Ok(vec![]);
    }
    
    // 读取文件
    let content = std::fs::read_to_string(&log_file)
        .map_err(|e| format!("Failed to read log file: {}", e))?;
    
    // 获取最后 N 行
    let lines: Vec<String> = content
        .lines()
        .rev()
        .take(lines)
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    
    Ok(lines)
}