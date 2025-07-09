use figment::{
    providers::{Env, Format, Json, Toml, Yaml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::{
    ConfigFormat, ConfigPath, ConfigValidator, Configurable, Environment, Result, SqliteProvider,
    ValidationRule,
};

pub struct ConfigLoader {
    figment: Figment,
    environment: Environment,
    validators: Vec<Box<dyn ConfigValidator>>,
    validation_rules: HashMap<String, Vec<Box<dyn ValidationRule>>>,
}

impl ConfigLoader {
    pub fn builder() -> ConfigLoaderBuilder {
        ConfigLoaderBuilder::new()
    }

    pub fn load<T>(&self) -> Result<T>
    where
        T: Configurable + for<'de> Deserialize<'de> + 'static,
    {
        info!(
            "Loading configuration for environment: {}",
            self.environment
        );

        let config: T = self.figment.extract()?;

        config.validate()?;

        for (path, rules) in &self.validation_rules {
            debug!("Validating field: {}", path);
            for rule in rules {
                rule.validate(&config as &dyn std::any::Any)?;
            }
        }

        for validator in &self.validators {
            debug!("Running validator: {}", validator.name());
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(validator.validate(&config as &(dyn std::any::Any + Send + Sync)))?;
        }

        info!("Configuration loaded and validated successfully");
        Ok(config)
    }

    pub async fn load_async<T>(&self) -> Result<T>
    where
        T: Configurable + for<'de> Deserialize<'de> + 'static,
    {
        info!(
            "Loading configuration asynchronously for environment: {}",
            self.environment
        );

        let config: T = self.figment.extract()?;

        config.validate()?;

        for (path, rules) in &self.validation_rules {
            debug!("Validating field: {}", path);
            for rule in rules {
                rule.validate(&config as &dyn std::any::Any)?;
            }
        }

        for validator in &self.validators {
            debug!("Running async validator: {}", validator.name());
            validator
                .validate(&config as &(dyn std::any::Any + Send + Sync))
                .await?;
        }

        info!("Configuration loaded and validated successfully");
        Ok(config)
    }

    pub fn reload<T>(&mut self) -> Result<T>
    where
        T: Configurable + for<'de> Deserialize<'de> + 'static,
    {
        info!("Reloading configuration");
        self.load()
    }
}

pub struct ConfigLoaderBuilder {
    base_path: Option<PathBuf>,
    config_files: Vec<ConfigPath>,
    environment: Environment,
    env_prefix: Option<String>,
    validators: Vec<Box<dyn ConfigValidator>>,
    validation_rules: HashMap<String, Vec<Box<dyn ValidationRule>>>,
    defaults: Option<serde_json::Value>,
    sqlite_config: Option<(String, String)>, // (database_url, service_name)
}

impl ConfigLoaderBuilder {
    pub fn new() -> Self {
        Self {
            base_path: None,
            config_files: Vec::new(),
            environment: Environment::from_env(),
            env_prefix: None,
            validators: Vec::new(),
            validation_rules: HashMap::new(),
            defaults: None,
            sqlite_config: None,
        }
    }

    pub fn base_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.base_path = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn add_file<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.config_files.push(ConfigPath::new(path));
        self
    }

    pub fn add_file_with_format<P: AsRef<Path>>(mut self, path: P, format: ConfigFormat) -> Self {
        self.config_files
            .push(ConfigPath::with_format(path, format));
        self
    }

    pub fn environment(mut self, env: Environment) -> Self {
        self.environment = env;
        self
    }

    pub fn env_prefix<S: Into<String>>(mut self, prefix: S) -> Self {
        self.env_prefix = Some(prefix.into());
        self
    }

    pub fn add_validator(mut self, validator: Box<dyn ConfigValidator>) -> Self {
        self.validators.push(validator);
        self
    }

    pub fn add_validation_rule<S: Into<String>>(
        mut self,
        field_path: S,
        rule: Box<dyn ValidationRule>,
    ) -> Self {
        self.validation_rules
            .entry(field_path.into())
            .or_insert_with(Vec::new)
            .push(rule);
        self
    }

    pub fn defaults<T: Serialize>(mut self, defaults: T) -> Result<Self> {
        self.defaults = Some(serde_json::to_value(defaults)?);
        Ok(self)
    }

    pub fn add_sqlite<S: Into<String>>(mut self, database_url: S, service_name: S) -> Self {
        self.sqlite_config = Some((database_url.into(), service_name.into()));
        self
    }

    pub fn build(self) -> Result<ConfigLoader> {
        let mut figment = Figment::new();

        if let Some(defaults) = self.defaults {
            figment = figment.merge(Json::string(&defaults.to_string()));
        }

        let base_path = self.base_path.unwrap_or_else(|| PathBuf::from("config"));

        figment = figment.merge(Yaml::file(base_path.join("default.yml")));

        figment = figment.merge(Yaml::file(
            base_path.join(format!("{}.yml", self.environment.as_str())),
        ));

        for config_path in self.config_files {
            if !config_path.exists() {
                warn!("Configuration file not found: {}", config_path);
                continue;
            }

            let path = if config_path.path().is_absolute() {
                config_path.path().to_path_buf()
            } else {
                base_path.join(config_path.path())
            };

            match config_path.format() {
                Some(ConfigFormat::Yaml) | None => {
                    figment = figment.merge(Yaml::file(&path));
                }
                Some(ConfigFormat::Toml) => {
                    figment = figment.merge(Toml::file(&path));
                }
                Some(ConfigFormat::Json) => {
                    figment = figment.merge(Json::file(&path));
                }
                Some(ConfigFormat::Env) => {
                    warn!("ENV format is handled separately");
                }
            }
        }

        // Add SQLite provider if configured
        if let Some((db_url, service_name)) = self.sqlite_config {
            // Create SQLite provider asynchronously
            let rt = tokio::runtime::Runtime::new().map_err(|e| {
                crate::ConfigError::Custom(format!("Failed to create runtime: {}", e))
            })?;

            let sqlite_provider = rt.block_on(async {
                SqliteProvider::new(&db_url, service_name)
                    .await
                    .map_err(|e| {
                        crate::ConfigError::Custom(format!(
                            "Failed to create SQLite provider: {}",
                            e
                        ))
                    })
            })?;

            figment = figment.merge(sqlite_provider);
        }

        if let Some(prefix) = self.env_prefix {
            figment = figment.merge(Env::prefixed(&prefix).split("_"));
        } else {
            figment = figment.merge(Env::prefixed("VOLTAGE_").split("_"));
        }

        Ok(ConfigLoader {
            figment,
            environment: self.environment,
            validators: self.validators,
            validation_rules: self.validation_rules,
        })
    }
}

impl Default for ConfigLoaderBuilder {
    fn default() -> Self {
        Self::new()
    }
}
