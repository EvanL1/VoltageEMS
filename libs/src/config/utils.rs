//! configuring工具function
//!
//! 提供globalcycle境variablepriorityprocessing

use std::env;

/// Getcycle境variable，supportingglobal和serving特定的前缀
///
/// # Arguments
///
/// * `global_key` - globalcycle境variable名 (如 "VOLTAGE_REDIS_URL")
/// * `service_key` - serving特定cycle境variable名 (如 "APIGATEWAY_REDIS_URL")
/// * `default` - defaultvalue
///
/// # Returns
///
/// 按priorityreturn：globalvariable > servingvariable > defaultvalue
pub fn get_env_with_fallback(global_key: &str, service_key: &str, default: &str) -> String {
    // 首先checkingglobalcycle境variable
    if let Ok(value) = env::var(global_key) {
        return value;
    }

    // 其次checkingserving特定cycle境variable
    if let Ok(value) = env::var(service_key) {
        return value;
    }

    // 最后returndefaultvalue
    default.to_string()
}

/// Getglobal Redis URL
pub fn get_global_redis_url(service_prefix: &str) -> String {
    // Check if running in Docker/container environment
    let default_url = if std::env::var("DOCKER_ENV").unwrap_or_default() == "true" {
        "redis://redis:6379"
    } else {
        "redis://localhost:6379"
    };

    get_env_with_fallback(
        "VOLTAGE_REDIS_URL",
        &format!("{service_prefix}_REDIS_URL"),
        default_url,
    )
}

/// Getgloballogginglevel
pub fn get_global_log_level(service_prefix: &str) -> String {
    get_env_with_fallback(
        "VOLTAGE_LOG_LEVEL",
        &format!("{service_prefix}_LOG_LEVEL"),
        "info",
    )
}

/// Getglobal InfluxDB URL
pub fn get_global_influxdb_url(service_prefix: &str) -> String {
    get_env_with_fallback(
        "VOLTAGE_INFLUXDB_URL",
        &format!("{service_prefix}_INFLUXDB_URL"),
        "http://influxdb:8086",
    )
}

/// Getglobal InfluxDB Token
pub fn get_global_influxdb_token(service_prefix: &str) -> String {
    get_env_with_fallback(
        "VOLTAGE_INFLUXDB_TOKEN",
        &format!("{service_prefix}_INFLUXDB_TOKEN"),
        "",
    )
}

/// Getglobal InfluxDB Org
pub fn get_global_influxdb_org(service_prefix: &str) -> String {
    get_env_with_fallback(
        "VOLTAGE_INFLUXDB_ORG",
        &format!("{service_prefix}_INFLUXDB_ORG"),
        "voltage",
    )
}

/// Getglobal InfluxDB Bucket
pub fn get_global_influxdb_bucket(service_prefix: &str) -> String {
    get_env_with_fallback(
        "VOLTAGE_INFLUXDB_BUCKET",
        &format!("{service_prefix}_INFLUXDB_BUCKET"),
        "ems",
    )
}

/// Getglobal JWT Secret
pub fn get_global_jwt_secret(service_prefix: &str) -> String {
    get_env_with_fallback(
        "VOLTAGE_JWT_SECRET",
        &format!("{service_prefix}_JWT_SECRET"),
        "dev-secret",
    )
}

/// Getserving发现 URL
///
/// 在 Docker cycle境medium，serving名即host名
pub fn get_service_url(service_name: &str) -> String {
    // Check if running in Docker/container environment
    let use_docker_urls = std::env::var("DOCKER_ENV").unwrap_or_default() == "true";

    if use_docker_urls {
        // Docker environment - use service names as hostnames
        match service_name {
            "comsrv" => "http://comsrv:8081".to_string(),
            "modsrv" => "http://modsrv:8092".to_string(),
            "alarmsrv" => "http://alarmsrv:8080".to_string(),
            "rulesrv" => "http://rulesrv:8080".to_string(),
            "hissrv" => "http://hissrv:8082".to_string(),
            "netsrv" => "http://netsrv:8087".to_string(),
            _ => format!("http://{service_name}:8080"),
        }
    } else {
        // Development environment - use localhost
        match service_name {
            "comsrv" => "http://localhost:6000".to_string(),
            "modsrv" => "http://localhost:6001".to_string(),
            "alarmsrv" => "http://localhost:6002".to_string(),
            "rulesrv" => "http://localhost:6003".to_string(),
            "hissrv" => "http://localhost:6004".to_string(),
            "netsrv" => "http://localhost:6006".to_string(),
            _ => "http://localhost:6005".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_get_env_with_fallback() {
        // testingdefaultvalue
        assert_eq!(
            get_env_with_fallback("TEST_GLOBAL", "TEST_SERVICE", "default"),
            "default"
        );

        // testingserving特定variable
        env::set_var("TEST_SERVICE", "service_value");
        assert_eq!(
            get_env_with_fallback("TEST_GLOBAL", "TEST_SERVICE", "default"),
            "service_value"
        );

        // testingglobalvariablepriority
        env::set_var("TEST_GLOBAL", "global_value");
        assert_eq!(
            get_env_with_fallback("TEST_GLOBAL", "TEST_SERVICE", "default"),
            "global_value"
        );

        // cleaning
        env::remove_var("TEST_GLOBAL");
        env::remove_var("TEST_SERVICE");
    }

    #[test]
    fn test_service_discovery() {
        // Test development environment (default)
        assert_eq!(get_service_url("comsrv"), "http://localhost:8081");
        assert_eq!(get_service_url("modsrv"), "http://localhost:8092");
        assert_eq!(get_service_url("unknown"), "http://localhost:8080");

        // Test Docker environment
        env::set_var("DOCKER_ENV", "true");
        assert_eq!(get_service_url("comsrv"), "http://comsrv:8081");
        assert_eq!(get_service_url("modsrv"), "http://modsrv:8092");
        assert_eq!(get_service_url("unknown"), "http://unknown:8080");

        // Cleanup
        env::remove_var("DOCKER_ENV");
    }
}
