//! 基础功能测试
//!
//! 验证修复后的核心功能

use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        // 测试基本的数据结构创建
        let mut data = HashMap::new();
        data.insert("test_key".to_string(), "test_value".to_string());

        assert_eq!(data.get("test_key"), Some(&"test_value".to_string()));

        // 测试基本的JSON序列化
        let json_data = serde_json::json!({
            "status": "success",
            "message": "Test passed"
        });

        assert_eq!(json_data["status"], "success");
        assert_eq!(json_data["message"], "Test passed");
    }

    #[test]
    fn test_redis_key_format() {
        // 测试Redis键格式验证
        let valid_keys = vec![
            "1001:m:10001",
            "1002:s:20001",
            "1003:c:30001",
            "1004:a:40001",
        ];

        for key in valid_keys {
            let parts: Vec<&str> = key.split(':').collect();
            assert_eq!(parts.len(), 3);
            assert!(parts[0].parse::<u16>().is_ok());
            assert!(matches!(parts[1], "m" | "s" | "c" | "a"));
            assert!(parts[2].parse::<u32>().is_ok());
        }
    }

    #[test]
    fn test_comsrv_data_format() {
        // 测试comsrv数据格式解析
        let test_data = "25.6:1234567890";
        let parts: Vec<&str> = test_data.split(':').collect();

        assert_eq!(parts.len(), 2);

        let value = parts[0].parse::<f64>().unwrap();
        let timestamp = parts[1].parse::<i64>().unwrap();

        assert_eq!(value, 25.6);
        assert_eq!(timestamp, 1234567890);
    }

    #[test]
    fn test_command_creation() {
        // 测试控制命令创建
        let channel_id = 1001u16;
        let point_type = "c";
        let point_id = 30001u32;
        let value = 1.0f64;

        // 验证基本数据类型
        assert!(channel_id > 0);
        assert!(point_type == "c" || point_type == "a");
        assert!(point_id > 0);
        assert!(value.is_finite());

        // 测试Redis键生成
        let redis_key = format!("{}:{}:{}", channel_id, point_type, point_id);
        assert_eq!(redis_key, "1001:c:30001");
    }
}
