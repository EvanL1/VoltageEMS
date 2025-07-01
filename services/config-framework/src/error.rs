use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Figment error: {0}")]
    Figment(#[from] figment::Error),

    #[error("Serde JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Serde YAML error: {0}")]
    SerdeYaml(#[from] serde_yaml::Error),

    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("Configuration not found: {0}")]
    NotFound(String),

    #[error("Invalid configuration: {0}")]
    Invalid(String),

    #[error("Environment variable error: {0}")]
    Environment(String),

    #[error("Watch error: {0}")]
    Watch(String),

    #[error("Other error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, ConfigError>;