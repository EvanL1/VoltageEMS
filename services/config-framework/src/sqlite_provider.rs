//! SQLite configuration provider for Figment
//!
//! This provider loads configuration from a SQLite database,
//! allowing for dynamic configuration management.

use async_trait::async_trait;
use figment::{
    value::{Dict, Map, Value},
    Metadata, Profile, Provider,
};
use serde_json;
use sqlx::{sqlite::SqlitePool, Row};
use tracing::{debug, error, info, warn};

/// SQLite configuration provider
pub struct SqliteProvider {
    /// Database connection pool
    pool: SqlitePool,
    /// Service name to filter configurations
    service_name: String,
    /// Profile to use (e.g., "default", "production", "development")
    profile: Profile,
}

impl SqliteProvider {
    /// Create a new SQLite provider
    pub async fn new(
        database_url: &str,
        service_name: impl Into<String>,
    ) -> Result<Self, sqlx::Error> {
        let pool = SqlitePool::connect(database_url).await?;

        Ok(Self {
            pool,
            service_name: service_name.into(),
            profile: Profile::Default,
        })
    }

    /// Create a new SQLite provider with a specific profile
    pub async fn with_profile(
        database_url: &str,
        service_name: impl Into<String>,
        profile: Profile,
    ) -> Result<Self, sqlx::Error> {
        let pool = SqlitePool::connect(database_url).await?;

        Ok(Self {
            pool,
            service_name: service_name.into(),
            profile,
        })
    }

    /// Initialize the database schema
    pub async fn init_schema(&self) -> Result<(), sqlx::Error> {
        let schema = include_str!("../schema/sqlite_schema.sql");
        let statements: Vec<&str> = schema.split(';').collect();

        for statement in statements {
            let trimmed = statement.trim();
            if !trimmed.is_empty() {
                sqlx::query(trimmed).execute(&self.pool).await?;
            }
        }

        info!("SQLite schema initialized successfully");
        Ok(())
    }

    /// Load configuration from database
    async fn load_config(&self) -> Result<Map<String, Value>, sqlx::Error> {
        let configs = sqlx::query(
            "SELECT key, value, type FROM configs 
             WHERE service = ? AND is_active = 1
             ORDER BY key",
        )
        .bind(&self.service_name)
        .fetch_all(&self.pool)
        .await?;

        let mut result = Map::new();

        for row in configs {
            let key: String = row.get("key");
            let value_str: String = row.get("value");
            let value_type: String = row.get("type");

            let value = match value_type.as_str() {
                "json" => match serde_json::from_str::<serde_json::Value>(&value_str) {
                    Ok(json_val) => self.json_to_figment_value(json_val),
                    Err(e) => {
                        error!("Failed to parse JSON value for key {}: {}", key, e);
                        continue;
                    }
                },
                "string" => Value::from(value_str),
                "number" => {
                    if let Ok(n) = value_str.parse::<i64>() {
                        Value::from(n)
                    } else if let Ok(f) = value_str.parse::<f64>() {
                        Value::from(f)
                    } else {
                        error!(
                            "Failed to parse number value for key {}: {}",
                            key, value_str
                        );
                        continue;
                    }
                }
                "boolean" => match value_str.to_lowercase().as_str() {
                    "true" | "1" | "yes" | "on" => Value::from(true),
                    "false" | "0" | "no" | "off" => Value::from(false),
                    _ => {
                        error!(
                            "Failed to parse boolean value for key {}: {}",
                            key, value_str
                        );
                        continue;
                    }
                },
                _ => {
                    warn!(
                        "Unknown type {} for key {}, treating as string",
                        value_type, key
                    );
                    Value::from(value_str)
                }
            };

            // Handle nested keys (e.g., "redis.host" -> { redis: { host: value } })
            self.insert_nested(&mut result, &key, value);
        }

        debug!(
            "Loaded {} configuration items for service {}",
            result.len(),
            self.service_name
        );
        Ok(result)
    }

    /// Convert serde_json::Value to figment::Value
    fn json_to_figment_value(&self, json: serde_json::Value) -> Value {
        Self::convert_json_to_figment_value(json)
    }

    fn convert_json_to_figment_value(json: serde_json::Value) -> Value {
        match json {
            serde_json::Value::Null => Value::from(""),
            serde_json::Value::Bool(b) => Value::from(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::from(i)
                } else if let Some(f) = n.as_f64() {
                    Value::from(f)
                } else {
                    Value::from(n.to_string())
                }
            }
            serde_json::Value::String(s) => Value::from(s),
            serde_json::Value::Array(arr) => {
                let values: Vec<Value> = arr
                    .into_iter()
                    .map(Self::convert_json_to_figment_value)
                    .collect();
                Value::from(values)
            }
            serde_json::Value::Object(obj) => {
                let mut map = Map::new();
                for (k, v) in obj {
                    map.insert(k, Self::convert_json_to_figment_value(v));
                }
                Value::from(map)
            }
        }
    }

    /// Insert a value into a nested map structure
    fn insert_nested(&self, map: &mut Map<String, Value>, key: &str, value: Value) {
        let parts: Vec<&str> = key.split('.').collect();

        if parts.len() == 1 {
            map.insert(key.to_string(), value);
            return;
        }

        let mut current = map;

        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                current.insert(part.to_string(), value.clone());
            } else {
                let entry = current
                    .entry(part.to_string())
                    .or_insert_with(|| Value::from(Map::<String, Value>::new()));

                if let Value::Dict(_, dict) = entry {
                    current = dict;
                } else {
                    error!(
                        "Cannot create nested structure for key {}: conflict at {}",
                        key, part
                    );
                    return;
                }
            }
        }
    }
}

impl Provider for SqliteProvider {
    fn metadata(&self) -> Metadata {
        Metadata::named("SQLite Provider")
            .source(format!("sqlite:{}", self.service_name))
            .interpolater(|_profile, _map| String::from("sqlite"))
    }

    fn data(&self) -> Result<Map<Profile, Dict>, figment::Error> {
        // This is a sync method, but we need to load async data
        // We'll use a runtime handle to execute the async operation
        let rt = tokio::runtime::Handle::current();

        let config = rt.block_on(async {
            self.load_config()
                .await
                .map_err(|e| figment::Error::from(e.to_string()))
        })?;

        let mut result = Map::new();
        result.insert(self.profile.clone(), config);

        Ok(result)
    }
}

/// Async provider trait for SQLite configuration
#[async_trait]
pub trait AsyncSqliteProvider {
    /// Load point table data
    async fn load_point_tables(&self, channel_id: i32)
        -> Result<Vec<PointTableEntry>, sqlx::Error>;

    /// Load protocol mappings
    async fn load_protocol_mappings(
        &self,
        channel_id: i32,
    ) -> Result<Vec<ProtocolMapping>, sqlx::Error>;

    /// Save configuration
    async fn save_config(
        &self,
        key: &str,
        value: &str,
        value_type: &str,
    ) -> Result<(), sqlx::Error>;

    /// Delete configuration
    async fn delete_config(&self, key: &str) -> Result<(), sqlx::Error>;
}

/// Point table entry
#[derive(Debug, Clone)]
pub struct PointTableEntry {
    pub point_id: String,
    pub point_name: String,
    pub point_type: String,
    pub data_type: Option<String>,
    pub unit: Option<String>,
    pub scale: f64,
    pub offset: f64,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub description: Option<String>,
    pub metadata: Option<String>,
}

/// Protocol mapping entry
#[derive(Debug, Clone)]
pub struct ProtocolMapping {
    pub point_id: String,
    pub protocol: String,
    pub address: String,
    pub params: Option<String>,
    pub slave_id: Option<i32>,
    pub function_code: Option<i32>,
    pub register_address: Option<i32>,
}

#[async_trait]
impl AsyncSqliteProvider for SqliteProvider {
    async fn load_point_tables(
        &self,
        channel_id: i32,
    ) -> Result<Vec<PointTableEntry>, sqlx::Error> {
        let entries = sqlx::query(
            r#"
            SELECT 
                point_id,
                point_name,
                point_type,
                data_type,
                unit,
                scale,
                offset,
                min_value,
                max_value,
                description,
                metadata
            FROM point_tables
            WHERE channel_id = ? AND is_active = 1
            ORDER BY point_id
            "#,
        )
        .bind(channel_id)
        .map(|row: sqlx::sqlite::SqliteRow| {
            use sqlx::Row;
            PointTableEntry {
                point_id: row.get("point_id"),
                point_name: row.get("point_name"),
                point_type: row.get("point_type"),
                data_type: row.get("data_type"),
                unit: row.get("unit"),
                scale: row.get("scale"),
                offset: row.get("offset"),
                min_value: row.get("min_value"),
                max_value: row.get("max_value"),
                description: row.get("description"),
                metadata: row.get("metadata"),
            }
        })
        .fetch_all(&self.pool)
        .await?;

        info!(
            "Loaded {} point table entries for channel {}",
            entries.len(),
            channel_id
        );
        Ok(entries)
    }

    async fn load_protocol_mappings(
        &self,
        channel_id: i32,
    ) -> Result<Vec<ProtocolMapping>, sqlx::Error> {
        let mappings = sqlx::query(
            r#"
            SELECT 
                point_id,
                protocol,
                address,
                params,
                slave_id,
                function_code,
                register_address
            FROM protocol_mappings
            WHERE channel_id = ? AND is_active = 1
            ORDER BY point_id
            "#,
        )
        .bind(channel_id)
        .map(|row: sqlx::sqlite::SqliteRow| {
            use sqlx::Row;
            ProtocolMapping {
                point_id: row.get("point_id"),
                protocol: row.get("protocol"),
                address: row.get("address"),
                params: row.get("params"),
                slave_id: row.get("slave_id"),
                function_code: row.get("function_code"),
                register_address: row.get("register_address"),
            }
        })
        .fetch_all(&self.pool)
        .await?;

        info!(
            "Loaded {} protocol mappings for channel {}",
            mappings.len(),
            channel_id
        );
        Ok(mappings)
    }

    async fn save_config(
        &self,
        key: &str,
        value: &str,
        value_type: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO configs (service, key, value, type)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(service, key) DO UPDATE SET
                value = excluded.value,
                type = excluded.type,
                version = version + 1,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&self.service_name)
        .bind(key)
        .bind(value)
        .bind(value_type)
        .execute(&self.pool)
        .await?;

        info!(
            "Saved configuration: service={}, key={}",
            self.service_name, key
        );
        Ok(())
    }

    async fn delete_config(&self, key: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE configs SET is_active = 0 WHERE service = ? AND key = ?")
            .bind(&self.service_name)
            .bind(key)
            .execute(&self.pool)
            .await?;

        info!(
            "Deleted configuration: service={}, key={}",
            self.service_name, key
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_sqlite_provider() {
        let temp_file = NamedTempFile::new().unwrap();
        let db_url = format!("sqlite:{}", temp_file.path().display());

        let provider = SqliteProvider::new(&db_url, "test_service").await.unwrap();

        provider.init_schema().await.unwrap();

        // Test saving configuration
        provider
            .save_config("test.key", "test_value", "string")
            .await
            .unwrap();
        provider
            .save_config("test.number", "42", "number")
            .await
            .unwrap();
        provider
            .save_config("test.bool", "true", "boolean")
            .await
            .unwrap();

        // Test loading configuration
        let config = provider.load_config().await.unwrap();
        assert_eq!(config.len(), 1); // Should have one top-level key "test"

        if let Some(Value::Dict(_, test_dict)) = config.get("test") {
            assert_eq!(test_dict.get("key"), Some(&Value::from("test_value")));
            assert_eq!(test_dict.get("number"), Some(&Value::from(42)));
            assert_eq!(test_dict.get("bool"), Some(&Value::from(true)));
        } else {
            panic!("Expected nested structure");
        }
    }
}
