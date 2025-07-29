//! gRPC 插件客户端封装

use crate::utils::error::{ComSrvError, Result};
use std::time::Duration;
use tonic::transport::{Channel, Endpoint};
use tracing::{debug, error, info};

use super::proto::{
    protocol_plugin_client::ProtocolPluginClient, BatchReadRequest, BatchReadResponse, Empty,
    EncodeRequest, EncodeResponse, HealthStatus, ParseRequest, ParseResponse, PluginInfo,
};

/// gRPC 插件客户端
#[derive(Clone, Debug)]
pub struct GrpcPluginClient {
    client: ProtocolPluginClient<Channel>,
    endpoint: String,
}

impl GrpcPluginClient {
    /// 创建新的 gRPC 插件客户端
    pub async fn new(endpoint: &str) -> Result<Self> {
        info!("Creating gRPC plugin client for endpoint: {}", endpoint);

        let channel = Endpoint::from_shared(endpoint.to_string())
            .map_err(|e| ComSrvError::config(format!("Invalid endpoint: {e}")))?
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .connect()
            .await
            .map_err(|e| ComSrvError::connection(format!("Failed to connect to plugin: {e}")))?;

        let client = ProtocolPluginClient::new(channel);

        Ok(Self {
            client,
            endpoint: endpoint.to_string(),
        })
    }

    /// 获取插件信息
    pub async fn get_info(&mut self) -> Result<PluginInfo> {
        debug!("Getting plugin info from {}", self.endpoint);

        let response = self
            .client
            .get_info(tonic::Request::new(Empty {}))
            .await
            .map_err(|e| ComSrvError::protocol(format!("GetInfo failed: {e}")))?;

        Ok(response.into_inner())
    }

    /// 健康检查
    pub async fn health_check(&mut self) -> Result<HealthStatus> {
        let response = self
            .client
            .health_check(tonic::Request::new(Empty {}))
            .await
            .map_err(|e| ComSrvError::protocol(format!("HealthCheck failed: {e}")))?;

        Ok(response.into_inner())
    }

    /// 批量读取数据
    pub async fn batch_read(&mut self, request: BatchReadRequest) -> Result<BatchReadResponse> {
        info!(
            "Sending BatchRead request to plugin: channel_id={}, points={:?}, params={:?}",
            request.channel_id, request.point_ids, request.connection_params
        );

        let response = self
            .client
            .batch_read(tonic::Request::new(request))
            .await
            .map_err(|e| ComSrvError::protocol(format!("BatchRead failed: {e}")))?;

        let result = response.into_inner();
        if !result.error.is_empty() {
            error!("Plugin returned error: {}", result.error);
            return Err(ComSrvError::protocol(result.error));
        }

        Ok(result)
    }

    /// 解析原始数据
    pub async fn parse_data(&mut self, request: ParseRequest) -> Result<ParseResponse> {
        debug!("Parsing {} bytes of raw data", request.raw_data.len());

        let response = self
            .client
            .parse_data(tonic::Request::new(request))
            .await
            .map_err(|e| ComSrvError::protocol(format!("ParseData failed: {e}")))?;

        let result = response.into_inner();
        if !result.error.is_empty() {
            error!("Plugin parse error: {}", result.error);
            return Err(ComSrvError::protocol(result.error));
        }

        Ok(result)
    }

    /// 编码控制命令
    pub async fn encode_command(&mut self, request: EncodeRequest) -> Result<EncodeResponse> {
        debug!("Encoding command for point {}", request.point_id);

        let response = self
            .client
            .encode_command(tonic::Request::new(request))
            .await
            .map_err(|e| ComSrvError::protocol(format!("EncodeCommand failed: {e}")))?;

        let result = response.into_inner();
        if !result.error.is_empty() {
            error!("Plugin encode error: {}", result.error);
            return Err(ComSrvError::protocol(result.error));
        }

        Ok(result)
    }
}
