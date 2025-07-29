use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub redis: RedisConfig,
    pub services: ServicesConfig,
    pub api: ApiConfig,
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub pool_size: u32,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expiry_hours: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicesConfig {
    pub comsrv: ServiceConfig,
    pub modsrv: ServiceConfig,
    pub hissrv: ServiceConfig,
    pub netsrv: ServiceConfig,
    pub alarmsrv: ServiceConfig,
    pub rulesrv: ServiceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub url: String,
    pub timeout_seconds: u64,
}

impl Config {
    pub fn load() -> Result<Self, config::ConfigError> {
        let settings = config::Config::builder()
            // Load from file
            .add_source(config::File::with_name("apigateway").required(false))
            // Override with environment variables
            .add_source(config::Environment::with_prefix("APIGATEWAY").separator("_"))
            .build()?;

        settings.try_deserialize()
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

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8089,
                workers: 4,
            },
            redis: RedisConfig {
                url: "redis://localhost:6379".to_string(),
                pool_size: 10,
                timeout_seconds: 5,
            },
            api: ApiConfig {
                prefix: "/api".to_string(),
            },
            auth: AuthConfig {
                jwt_secret: "your-secret-key-change-in-production".to_string(),
                jwt_expiry_hours: 24,
            },
            services: ServicesConfig {
                comsrv: ServiceConfig {
                    url: "http://localhost:8001".to_string(),
                    timeout_seconds: 30,
                },
                modsrv: ServiceConfig {
                    url: "http://localhost:8002".to_string(),
                    timeout_seconds: 30,
                },
                hissrv: ServiceConfig {
                    url: "http://localhost:8003".to_string(),
                    timeout_seconds: 30,
                },
                netsrv: ServiceConfig {
                    url: "http://localhost:8004".to_string(),
                    timeout_seconds: 30,
                },
                alarmsrv: ServiceConfig {
                    url: "http://localhost:8005".to_string(),
                    timeout_seconds: 30,
                },
                rulesrv: ServiceConfig {
                    url: "http://localhost:8084".to_string(),
                    timeout_seconds: 30,
                },
            },
        }
    }
}
