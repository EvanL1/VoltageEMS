use serde::{Deserialize, Serialize};
use std::path::Path;
use voltage_libs::config::utils::{get_global_jwt_secret, get_global_redis_url, get_service_url};
use voltage_libs::config::ConfigLoader;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub redis: RedisConfig,
    #[serde(default)]
    pub services: ServicesConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_apigateway_port")]
    pub port: u16,
    #[serde(default = "default_workers")]
    pub workers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    #[serde(default = "default_redis_url")]
    pub url: String,
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    #[serde(default = "default_api_prefix")]
    pub prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    #[serde(default = "default_jwt_expiry")]
    pub jwt_expiry_hours: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicesConfig {
    #[serde(default = "default_comsrv_config")]
    pub comsrv: ServiceConfig,
    #[serde(default = "default_modsrv_config")]
    pub modsrv: ServiceConfig,
    #[serde(default = "default_hissrv_config")]
    pub hissrv: ServiceConfig,
    #[serde(default = "default_netsrv_config")]
    pub netsrv: ServiceConfig,
    #[serde(default = "default_alarmsrv_config")]
    pub alarmsrv: ServiceConfig,
    #[serde(default = "default_rulesrv_config")]
    pub rulesrv: ServiceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub url: String,
    pub timeout_seconds: u64,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // Try multiple configuration file paths
        let config_paths = [
            "config/apigateway/apigateway.yaml",
            "config/apigateway.yaml",
            "apigateway.yaml",
        ];

        let mut yaml_path = None;
        for path in &config_paths {
            if Path::new(path).exists() {
                yaml_path = Some(path.to_string());
                break;
            }
        }

        // Use the new ConfigLoader
        let loader = ConfigLoader::new()
            .with_defaults(Config::default())
            .with_env_prefix("APIGATEWAY");

        let config = if let Some(path) = yaml_path {
            loader.with_yaml_file(&path).build()
        } else {
            loader.build()
        }?;

        Ok(config)
    }

    pub fn get_service_url(&self, service: &str) -> Option<&str> {
        match service {
            "comsrv" => Some(&self.services.comsrv.url),
            "modsrv" => Some(&self.services.modsrv.url),
            "hissrv" => Some(&self.services.hissrv.url),
            "netsrv" => Some(&self.services.netsrv.url),
            "alarmsrv" => Some(&self.services.alarmsrv.url),
            "rulesrv" => Some(&self.services.rulesrv.url),
            _ => None,
        }
    }

    pub fn get_service_timeout(&self, service: &str) -> Option<u64> {
        match service {
            "comsrv" => Some(self.services.comsrv.timeout_seconds),
            "modsrv" => Some(self.services.modsrv.timeout_seconds),
            "hissrv" => Some(self.services.hissrv.timeout_seconds),
            "netsrv" => Some(self.services.netsrv.timeout_seconds),
            "alarmsrv" => Some(self.services.alarmsrv.timeout_seconds),
            "rulesrv" => Some(self.services.rulesrv.timeout_seconds),
            _ => None,
        }
    }
}

// Default value functions
fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_apigateway_port() -> u16 {
    8080
}

fn default_workers() -> usize {
    4
}

fn default_redis_url() -> String {
    get_global_redis_url("APIGATEWAY")
}

fn default_pool_size() -> u32 {
    10
}

fn default_timeout_seconds() -> u64 {
    5
}

fn default_api_prefix() -> String {
    "/api/v1".to_string()
}

fn default_jwt_secret() -> String {
    get_global_jwt_secret("APIGATEWAY")
}

fn default_jwt_expiry() -> u64 {
    24
}

fn default_comsrv_config() -> ServiceConfig {
    ServiceConfig {
        url: get_service_url("comsrv"),
        timeout_seconds: 30,
    }
}

fn default_modsrv_config() -> ServiceConfig {
    ServiceConfig {
        url: get_service_url("modsrv"),
        timeout_seconds: 30,
    }
}

fn default_hissrv_config() -> ServiceConfig {
    ServiceConfig {
        url: get_service_url("hissrv"),
        timeout_seconds: 30,
    }
}

fn default_netsrv_config() -> ServiceConfig {
    ServiceConfig {
        url: get_service_url("netsrv"),
        timeout_seconds: 30,
    }
}

fn default_alarmsrv_config() -> ServiceConfig {
    ServiceConfig {
        url: get_service_url("alarmsrv"),
        timeout_seconds: 30,
    }
}

fn default_rulesrv_config() -> ServiceConfig {
    ServiceConfig {
        url: get_service_url("rulesrv"),
        timeout_seconds: 30,
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_apigateway_port(),
            workers: default_workers(),
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: default_redis_url(),
            pool_size: default_pool_size(),
            timeout_seconds: default_timeout_seconds(),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            prefix: default_api_prefix(),
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: default_jwt_secret(),
            jwt_expiry_hours: default_jwt_expiry(),
        }
    }
}

impl Default for ServicesConfig {
    fn default() -> Self {
        Self {
            comsrv: default_comsrv_config(),
            modsrv: default_modsrv_config(),
            hissrv: default_hissrv_config(),
            netsrv: default_netsrv_config(),
            alarmsrv: default_alarmsrv_config(),
            rulesrv: default_rulesrv_config(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: default_host(),
                port: default_apigateway_port(),
                workers: default_workers(),
            },
            redis: RedisConfig {
                url: default_redis_url(),
                pool_size: default_pool_size(),
                timeout_seconds: default_timeout_seconds(),
            },
            api: ApiConfig {
                prefix: default_api_prefix(),
            },
            auth: AuthConfig {
                jwt_secret: default_jwt_secret(),
                jwt_expiry_hours: default_jwt_expiry(),
            },
            services: ServicesConfig {
                comsrv: default_comsrv_config(),
                modsrv: default_modsrv_config(),
                hissrv: default_hissrv_config(),
                netsrv: default_netsrv_config(),
                alarmsrv: default_alarmsrv_config(),
                rulesrv: default_rulesrv_config(),
            },
        }
    }
}
