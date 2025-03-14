use crate::config::network_config::MqttConfig;
use crate::error::{NetSrvError, Result};
use crate::formatter::DataFormatter;
use crate::network::NetworkClient;
use async_trait::async_trait;
use log::{debug, error, info, warn};
use rumqttc::{AsyncClient, ClientError, MqttOptions, QoS, Transport};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct MqttClient {
    config: MqttConfig,
    client: Option<AsyncClient>,
    formatter: Box<dyn DataFormatter>,
    connected: Arc<Mutex<bool>>,
}

impl MqttClient {
    pub fn new(config: MqttConfig, formatter: Box<dyn DataFormatter>) -> Self {
        MqttClient {
            config,
            client: None,
            formatter,
            connected: Arc::new(Mutex::new(false)),
        }
    }

    fn get_qos(&self) -> QoS {
        match self.config.qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => QoS::AtMostOnce,
        }
    }
}

#[async_trait]
impl NetworkClient for MqttClient {
    async fn connect(&mut self) -> Result<()> {
        let mut mqtt_options = MqttOptions::new(
            &self.config.client_id,
            &self.config.broker_url,
            self.config.port,
        );

        // 设置认证信息
        if let (Some(username), Some(password)) = (&self.config.username, &self.config.password) {
            mqtt_options.set_credentials(username, password);
        }

        // 设置连接参数
        mqtt_options.set_keep_alive(Duration::from_secs(30));
        mqtt_options.set_clean_session(true);

        // 设置 SSL/TLS
        if self.config.use_ssl {
            if let (Some(ca_path), Some(cert_path), Some(key_path)) = (
                &self.config.ca_cert_path,
                &self.config.client_cert_path,
                &self.config.client_key_path,
            ) {
                // 使用客户端证书认证
                let transport = Transport::Tls(
                    ca_path.into(),
                    Some((cert_path.into(), key_path.into())),
                );
                mqtt_options.set_transport(transport);
            } else if let Some(ca_path) = &self.config.ca_cert_path {
                // 仅使用 CA 证书
                let transport = Transport::Tls(ca_path.into(), None);
                mqtt_options.set_transport(transport);
            } else {
                return Err(NetSrvError::MqttError(
                    "SSL is enabled but certificates are not provided".to_string(),
                ));
            }
        }

        // 创建客户端
        let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);
        self.client = Some(client);

        // 启动事件循环处理
        let connected = self.connected.clone();
        tokio::spawn(async move {
            *connected.lock().unwrap() = true;
            
            loop {
                match eventloop.poll().await {
                    Ok(notification) => {
                        debug!("MQTT Event: {:?}", notification);
                    }
                    Err(e) => {
                        error!("MQTT Connection error: {:?}", e);
                        *connected.lock().unwrap() = false;
                        break;
                    }
                }
            }
        });

        info!("Connected to MQTT broker: {}", self.config.broker_url);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(client) = &self.client {
            if let Err(e) = client.disconnect().await {
                error!("Error disconnecting from MQTT broker: {:?}", e);
            }
        }
        
        *self.connected.lock().unwrap() = false;
        self.client = None;
        
        Ok(())
    }

    async fn send(&self, data: &str) -> Result<()> {
        if !self.is_connected() {
            return Err(NetSrvError::ConnectionError(
                "Not connected to MQTT broker".to_string(),
            ));
        }

        if let Some(client) = &self.client {
            client
                .publish(&self.config.topic, self.get_qos(), false, data)
                .await?;
            
            debug!("Published message to topic: {}", self.config.topic);
            Ok(())
        } else {
            Err(NetSrvError::ConnectionError(
                "MQTT client not initialized".to_string(),
            ))
        }
    }

    fn is_connected(&self) -> bool {
        *self.connected.lock().unwrap()
    }
} 