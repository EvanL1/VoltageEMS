use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::path::Path;

use crate::Result;

#[async_trait]
pub trait Configurable: Serialize + for<'de> Deserialize<'de> + Send + Sync {
    fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn merge_with(&mut self, other: &Self) -> Result<()>
    where
        Self: Sized,
    {
        let other_value = serde_json::to_value(other)?;
        let self_value = serde_json::to_value(&*self)?;

        let merged = merge_json_values(self_value, other_value);
        *self = serde_json::from_value(merged)?;

        Ok(())
    }

    fn as_any(&self) -> &dyn Any;
}

#[async_trait]
pub trait ConfigSource: Send + Sync {
    async fn load(&self, path: &Path) -> Result<Box<dyn Any + Send + Sync>>;

    fn supports_format(&self, format: &str) -> bool;

    fn priority(&self) -> i32 {
        0
    }
}

#[async_trait]
pub trait ConfigValidator: Send + Sync {
    async fn validate(&self, config: &(dyn Any + Send + Sync)) -> Result<()>;

    fn name(&self) -> &str;
}

fn merge_json_values(base: serde_json::Value, other: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;

    match (base, other) {
        (Value::Object(mut base_map), Value::Object(other_map)) => {
            for (key, value) in other_map {
                match base_map.get(&key) {
                    Some(base_value) if base_value.is_object() && value.is_object() => {
                        base_map.insert(key, merge_json_values(base_value.clone(), value));
                    }
                    _ => {
                        base_map.insert(key, value);
                    }
                }
            }
            Value::Object(base_map)
        }
        (_, other) => other,
    }
}
