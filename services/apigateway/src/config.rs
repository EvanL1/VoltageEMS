use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub redis: RedisConfig,
    pub services: ServicesConfig,
    pub cors: CorsConfig,
    pub logging: LoggingConfig,
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
pub struct ServicesConfig {
    pub comsrv: ServiceConfig,
    pub modsrv: ServiceConfig,
    pub hissrv: ServiceConfig,
    pub netsrv: ServiceConfig,
    pub alarmsrv: ServiceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub url: String,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    pub allowed_origins: Vec<String>,
    pub allowed_methods: Vec<String>,
    pub allowed_headers: Vec<String>,
    pub max_age: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
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
            _ => None,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
                workers: 4,
            },
            redis: RedisConfig {
                url: "redis://127.0.0.1:6379".to_string(),
                pool_size: 10,
                timeout_seconds: 5,
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
            },
            cors: CorsConfig {
                allowed_origins: vec![
                    "http://localhost:8082".to_string(),
                    "http://localhost:3000".to_string(),
                    "http://localhost:5173".to_string(),
                ],
                allowed_methods: vec![
                    "GET".to_string(),
                    "POST".to_string(),
                    "PUT".to_string(),
                    "DELETE".to_string(),
                    "OPTIONS".to_string(),
                ],
                allowed_headers: vec![
                    "Content-Type".to_string(),
                    "Authorization".to_string(),
                ],
                max_age: 3600,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "json".to_string(),
            },
        }
    }
}