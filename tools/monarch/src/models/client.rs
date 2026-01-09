//! HTTP client for model management

use anyhow::Result;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;

pub struct ModelClient {
    client: Client,
    base_url: String,
}

impl ModelClient {
    pub fn new(base_url: &str) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            base_url: base_url.to_string(),
        })
    }

    // Product operations
    pub async fn list_products(&self) -> Result<Value> {
        let response = self
            .client
            .get(format!("{}/api/products", self.base_url))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to get products: {}",
                response.status()
            ))
        }
    }

    pub async fn get_product(&self, name: &str) -> Result<Value> {
        let response = self
            .client
            .get(format!("{}/api/products/{}", self.base_url, name))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to get product: {}",
                response.status()
            ))
        }
    }

    // Note: import_product and delete_product methods have been removed.
    // Products are now compile-time built-in constants from voltage-model crate.

    // Instance operations
    pub async fn list_instances(&self, product: Option<&str>) -> Result<Value> {
        let url = if let Some(p) = product {
            format!("{}/api/instances?product={}", self.base_url, p)
        } else {
            format!("{}/api/instances", self.base_url)
        };

        let response = self.client.get(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to get instances: {}",
                response.status()
            ))
        }
    }

    pub async fn get_instance(&self, name: &str) -> Result<Value> {
        let response = self
            .client
            .get(format!("{}/api/instances/{}", self.base_url, name))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to get instance: {}",
                response.status()
            ))
        }
    }

    #[allow(clippy::disallowed_methods)] // json! macro internally uses unwrap (safe for known valid JSON)
    pub async fn create_instance(
        &self,
        product: &str,
        name: &str,
        props: HashMap<String, String>,
    ) -> Result<()> {
        let response = self
            .client
            .post(format!("{}/api/instances", self.base_url))
            .json(&serde_json::json!({
                "product": product,
                "name": name,
                "properties": props
            }))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed to create instance: {}",
                response.status()
            ))
        }
    }

    #[allow(clippy::disallowed_methods)] // json! macro internally uses unwrap (safe for known valid JSON)
    pub async fn update_instance(&self, name: &str, props: HashMap<String, String>) -> Result<()> {
        let response = self
            .client
            .put(format!("{}/api/instances/{}", self.base_url, name))
            .json(&serde_json::json!({
                "properties": props
            }))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed to update instance: {}",
                response.status()
            ))
        }
    }

    pub async fn delete_instance(&self, name: &str) -> Result<()> {
        let response = self
            .client
            .delete(format!("{}/api/instances/{}", self.base_url, name))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed to delete instance: {}",
                response.status()
            ))
        }
    }
}
