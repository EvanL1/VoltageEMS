//! Cross-service coordination with comsrv
//!
//! Handles HTTP communication with comsrv for routing reload
//! and future health check / version negotiation.

use anyhow::anyhow;
use std::time::Duration;
use tracing::info;

#[derive(serde::Deserialize)]
struct ReloadResult {
    errors: Vec<String>,
}

#[derive(serde::Deserialize)]
struct ReloadResponse {
    success: bool,
    data: ReloadResult,
}

/// Coordinates cross-service communication with comsrv
///
/// Encapsulates HTTP client and retry logic for calling comsrv endpoints.
/// Currently supports routing reload; extensible for health checks.
pub struct ComsrvCoordinator {
    client: reqwest::Client,
}

impl Default for ComsrvCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl ComsrvCoordinator {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to build reqwest client");
        Self { client }
    }

    /// Ask comsrv to reload its routing configuration and SHM view
    ///
    /// POSTs to `{COMSRV_URL}/api/routing/reload`.
    /// The base URL is read from `ENV_COMSRV_URL` env var,
    /// falling back to `DEFAULT_COMSRV_URL`.
    pub async fn reload_routing(&self) -> anyhow::Result<()> {
        let base_url = std::env::var(common::ENV_COMSRV_URL)
            .unwrap_or_else(|_| common::DEFAULT_COMSRV_URL.to_string());
        let url = format!("{}/api/routing/reload", base_url.trim_end_matches('/'));

        let response = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to call comsrv routing reload at {}: {}", url, e))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "comsrv routing reload returned HTTP {}: {}",
                status,
                body
            ));
        }

        let payload: ReloadResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Invalid comsrv routing reload response: {}", e))?;

        if !payload.success {
            return Err(anyhow!("comsrv routing reload reported success=false"));
        }

        if !payload.data.errors.is_empty() {
            return Err(anyhow!(
                "comsrv routing reload completed with errors: {}",
                payload.data.errors.join("; ")
            ));
        }

        info!("comsrv routing reload completed via {}", url);
        Ok(())
    }
}
