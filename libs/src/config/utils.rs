//! 配置工具函数
//!
//! 提供全局环境变量优先级处理

use std::env;

/// 获取环境变量，支持全局和服务特定的前缀
///
/// # Arguments
///
/// * `global_key` - 全局环境变量名 (如 "VOLTAGE_REDIS_URL")
/// * `service_key` - 服务特定环境变量名 (如 "APIGATEWAY_REDIS_URL")
/// * `default` - 默认值
///
/// # Returns
///
/// 按优先级返回：全局变量 > 服务变量 > 默认值
pub fn get_env_with_fallback(global_key: &str, service_key: &str, default: &str) -> String {
    // 首先检查全局环境变量
    if let Ok(value) = env::var(global_key) {
        return value;
    }

    // 其次检查服务特定环境变量
    if let Ok(value) = env::var(service_key) {
        return value;
    }

    // 最后返回默认值
    default.to_string()
}

/// 获取全局 Redis URL
pub fn get_global_redis_url(service_prefix: &str) -> String {
    get_env_with_fallback(
        "VOLTAGE_REDIS_URL",
        &format!("{}_REDIS_URL", service_prefix),
        "redis://redis:6379",
    )
}

/// 获取全局日志级别
pub fn get_global_log_level(service_prefix: &str) -> String {
    get_env_with_fallback(
        "VOLTAGE_LOG_LEVEL",
        &format!("{}_LOG_LEVEL", service_prefix),
        "info",
    )
}

/// 获取全局 InfluxDB URL
pub fn get_global_influxdb_url(service_prefix: &str) -> String {
    get_env_with_fallback(
        "VOLTAGE_INFLUXDB_URL",
        &format!("{}_INFLUXDB_URL", service_prefix),
        "http://influxdb:8086",
    )
}

/// 获取全局 InfluxDB Token
pub fn get_global_influxdb_token(service_prefix: &str) -> String {
    get_env_with_fallback(
        "VOLTAGE_INFLUXDB_TOKEN",
        &format!("{}_INFLUXDB_TOKEN", service_prefix),
        "",
    )
}

/// 获取全局 InfluxDB Org
pub fn get_global_influxdb_org(service_prefix: &str) -> String {
    get_env_with_fallback(
        "VOLTAGE_INFLUXDB_ORG",
        &format!("{}_INFLUXDB_ORG", service_prefix),
        "voltage",
    )
}

/// 获取全局 InfluxDB Bucket
pub fn get_global_influxdb_bucket(service_prefix: &str) -> String {
    get_env_with_fallback(
        "VOLTAGE_INFLUXDB_BUCKET",
        &format!("{}_INFLUXDB_BUCKET", service_prefix),
        "ems",
    )
}

/// 获取全局 JWT Secret
pub fn get_global_jwt_secret(service_prefix: &str) -> String {
    get_env_with_fallback(
        "VOLTAGE_JWT_SECRET",
        &format!("{}_JWT_SECRET", service_prefix),
        "dev-secret",
    )
}

/// 获取服务发现 URL
///
/// 在 Docker 环境中，服务名即主机名
pub fn get_service_url(service_name: &str) -> String {
    match service_name {
        "comsrv" => "http://comsrv:3000".to_string(),
        "modsrv" => "http://modsrv:8082".to_string(),
        "alarmsrv" => "http://alarmsrv:8083".to_string(),
        "rulesrv" => "http://rulesrv:8084".to_string(),
        "hissrv" => "http://hissrv:8085".to_string(),
        "netsrv" => "http://netsrv:8086".to_string(),
        _ => format!("http://{}:8080", service_name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_get_env_with_fallback() {
        // 测试默认值
        assert_eq!(
            get_env_with_fallback("TEST_GLOBAL", "TEST_SERVICE", "default"),
            "default"
        );

        // 测试服务特定变量
        env::set_var("TEST_SERVICE", "service_value");
        assert_eq!(
            get_env_with_fallback("TEST_GLOBAL", "TEST_SERVICE", "default"),
            "service_value"
        );

        // 测试全局变量优先级
        env::set_var("TEST_GLOBAL", "global_value");
        assert_eq!(
            get_env_with_fallback("TEST_GLOBAL", "TEST_SERVICE", "default"),
            "global_value"
        );

        // 清理
        env::remove_var("TEST_GLOBAL");
        env::remove_var("TEST_SERVICE");
    }

    #[test]
    fn test_service_discovery() {
        assert_eq!(get_service_url("comsrv"), "http://comsrv:3000");
        assert_eq!(get_service_url("modsrv"), "http://modsrv:8082");
        assert_eq!(get_service_url("unknown"), "http://unknown:8080");
    }
}
