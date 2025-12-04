//! Configuration loading helper functions
//! Provides utilities for loading configuration with fallback logic

use std::fmt::Display;
use std::str::FromStr;
use tracing::{debug, info, warn};

/// Get configuration value with priority: DB > ENV > Default
///
/// # Arguments
/// * `db_value` - Value from database configuration
/// * `is_default` - Whether the DB value is a default value
/// * `env_var` - Environment variable name to check
/// * `default` - Default value to use as fallback
pub fn get_config_value<T>(db_value: Option<T>, is_default: bool, env_var: &str, default: T) -> T
where
    T: FromStr + PartialEq + Clone,
    T::Err: Display,
{
    // Priority 1: DB value (if not default)
    if let Some(val) = db_value {
        if !is_default {
            debug!("{} from DB", env_var);
            return val;
        }
    }

    // Priority 2: Environment variable
    if let Ok(env_str) = std::env::var(env_var) {
        match env_str.parse::<T>() {
            Ok(val) => {
                debug!("{} from env: {}", env_var, env_str);
                return val;
            },
            Err(e) => {
                warn!("Parse {} env: {}", env_var, e);
            },
        }
    }

    // Priority 3: Default value
    debug!("{} default", env_var);
    default
}

/// Get string configuration value with priority: DB > ENV > Default
pub fn get_string_config(
    db_value: Option<String>,
    is_default: bool,
    env_var: &str,
    default: String,
) -> String {
    // Priority 1: DB value (if not empty and not default)
    if let Some(val) = db_value {
        if !val.is_empty() && !is_default {
            debug!("{} from DB", env_var);
            return val;
        }
    }

    // Priority 2: Environment variable
    if let Ok(env_val) = std::env::var(env_var) {
        if !env_val.is_empty() {
            debug!("{} from env", env_var);
            return env_val;
        }
    }

    // Priority 3: Default value
    debug!("{} default", env_var);
    default
}

#[cfg(feature = "redis")]
use crate::redis::RedisClient;

#[cfg(feature = "redis")]
use std::time::Duration;

#[cfg(feature = "redis")]
/// Try to connect to Redis with multiple candidates and retry logic
///
/// # Arguments
/// * `candidates` - List of (source_name, redis_url) pairs to try
/// * `retry_interval` - How long to wait between retry attempts
///
/// # Returns
/// * Tuple of (successful_url, redis_client)
pub async fn connect_redis_with_retry(
    candidates: Vec<(&str, String)>,
    retry_interval: Duration,
) -> (String, RedisClient) {
    let mut attempt = 0;

    loop {
        attempt += 1;
        debug!("Redis try #{}", attempt);

        for (source, url) in &candidates {
            debug!("Redis {}: {}", source, url);

            match RedisClient::new(url).await {
                Ok(client) => {
                    // Test the connection with a ping
                    match client.ping().await {
                        Ok(_) => {
                            info!("Redis connected ({})", source);
                            return (url.clone(), client);
                        },
                        Err(e) => {
                            warn!("Redis ping {}: {}", url, e);
                        },
                    }
                },
                Err(e) => {
                    warn!("Redis client {}: {}", url, e);
                },
            }
        }

        warn!("Redis retry in {:?}", retry_interval);
        tokio::time::sleep(retry_interval).await;
    }
}

#[cfg(feature = "redis")]
/// Build Redis connection candidates list with priority
///
/// # Arguments
/// * `db_url` - URL from database configuration
/// * `default_url` - Default Redis URL
///
/// # Returns
/// * Vector of (source_name, url) pairs in priority order
pub fn build_redis_candidates(
    db_url: Option<String>,
    default_url: &str,
) -> Vec<(&'static str, String)> {
    let mut candidates = Vec::new();

    // Priority 1: DB configuration (if not default)
    if let Some(url) = db_url {
        if !url.is_empty() && url != default_url {
            candidates.push(("DB", url));
        }
    }

    // Priority 2: Environment variable
    if let Ok(env_url) = std::env::var("REDIS_URL") {
        if !env_url.is_empty() {
            candidates.push(("ENV", env_url));
        }
    }

    // Priority 3: Default value
    candidates.push(("DEFAULT", default_url.to_string()));

    candidates
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_get_config_value_priority() {
        // Test DB priority
        let val = get_config_value(Some(8080u16), false, "TEST_PORT", 3000);
        assert_eq!(val, 8080);

        // Test default when DB is default value
        let val = get_config_value(Some(3000u16), true, "TEST_PORT", 3000);
        assert_eq!(val, 3000);
    }

    #[test]
    fn test_build_redis_candidates() {
        let candidates = build_redis_candidates(
            Some("redis://custom:6379".to_string()),
            "redis://localhost:6379",
        );

        assert!(!candidates.is_empty());
        assert_eq!(candidates[0].0, "DB");
    }
}
