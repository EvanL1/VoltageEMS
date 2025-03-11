use crate::config::network_config::AliyunIotConfig;
use crate::error::{NetSrvError, Result};
use crate::formatter::DataFormatter;
use crate::network::NetworkClient;
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use hmac::{Hmac, Mac};
use log::{debug, error, info, warn};
use paho_mqtt as paho;
use sha2::Sha256;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use url::form_urlencoded;

type HmacSha256 = Hmac<Sha256>;

pub struct AliyunIotClient {
    config: AliyunIotConfig,
    client: Option<paho::AsyncClient>,
    formatter: Box<dyn DataFormatter>,
    connected: Arc<Mutex<bool>>,
}

impl AliyunIotClient {
    pub fn new(config: AliyunIotConfig, formatter: Box<dyn DataFormatter>) -> Self {
        AliyunIotClient {
            config,
            client: None,
            formatter,
            connected: Arc::new(Mutex::new(false)),
        }
    }

    fn get_qos(&self) -> i32 {
        match self.config.qos {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => 0,
        }
    }

    fn generate_client_id(&self) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        format!("{}.{}_{}_{}", 
            self.config.device_name, 
            self.config.product_key,
            timestamp,
            "netsrv"
        )
    }

    fn generate_password(&self, client_id: &str) -> Result<String> {
        // 构建签名字符串
        let mut sign_content = form_urlencoded::Serializer::new(String::new());
        sign_content.append_pair("clientId", client_id);
        sign_content.append_pair("deviceName", &self.config.device_name);
        sign_content.append_pair("productKey", &self.config.product_key);
        let sign_content = sign_content.finish();

        // 使用设备密钥进行 HMAC-SHA256 签名
        let mut mac = HmacSha256::new_from_slice(self.config.device_secret.as_bytes())
            .map_err(|e| NetSrvError::AliyunIotError(format!("HMAC error: {}", e)))?;
        mac.update(sign_content.as_bytes());
        let result = mac.finalize();
        let sign = general_purpose::STANDARD.encode(result.into_bytes());

        // 构建密码
        let mut password = form_urlencoded::Serializer::new(String::new());
        password.append_pair("clientId", client_id);
        password.append_pair("deviceName", &self.config.device_name);
        password.append_pair("productKey", &self.config.product_key);
        password.append_pair("sign", &sign);
        password.append_pair("signmethod", "hmacsha256");
        
        Ok(password.finish())
    }

    fn get_server_url(&self) -> String {
        format!("tcp://{}.iot-as-mqtt.{}.aliyuncs.com:1883", 
            self.config.product_key, 
            self.config.region_id
        )
    }
}

#[async_trait]
impl NetworkClient for AliyunIotClient {
    async fn connect(&mut self) -> Result<()> {
        // 生成客户端 ID 和密码
        let client_id = self.generate_client_id();
        let password = self.generate_password(&client_id)?;
        let username = format!("{}&{}", self.config.device_name, self.config.product_key);
        
        // 创建 MQTT 客户端选项
        let server_url = self.get_server_url();
        let create_opts = paho::CreateOptionsBuilder::new()
            .server_uri(&server_url)
            .client_id(&client_id)
            .finalize();

        // 创建客户端
        let client = paho::AsyncClient::new(create_opts)
            .map_err(|e| NetSrvError::AliyunIotError(format!("Error creating Aliyun IoT client: {}", e)))?;

        // 设置连接选项
        let conn_opts = paho::ConnectOptionsBuilder::new()
            .user_name(&username)
            .password(&password)
            .keep_alive_interval(Duration::from_secs(60))
            .clean_session(true)
            .finalize();

        // 连接到阿里云 IoT
        client.connect(conn_opts).await
            .map_err(|e| NetSrvError::AliyunIotError(format!("Error connecting to Aliyun IoT: {}", e)))?;

        // 设置连接状态回调
        let connected = self.connected.clone();
        client.set_connected_callback(move |_| {
            *connected.lock().unwrap() = true;
            info!("Connected to Aliyun IoT");
        });

        client.set_connection_lost_callback(move |_| {
            *connected.lock().unwrap() = false;
            error!("Connection to Aliyun IoT lost");
        });

        self.client = Some(client);
        *self.connected.lock().unwrap() = true;

        info!("Connected to Aliyun IoT: {}", server_url);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(client) = &self.client {
            client.disconnect(None).await
                .map_err(|e| NetSrvError::AliyunIotError(format!("Error disconnecting from Aliyun IoT: {}", e)))?;
        }
        
        *self.connected.lock().unwrap() = false;
        self.client = None;
        
        Ok(())
    }

    async fn send(&self, data: &str) -> Result<()> {
        if !self.is_connected() {
            return Err(NetSrvError::ConnectionError(
                "Not connected to Aliyun IoT".to_string(),
            ));
        }

        if let Some(client) = &self.client {
            // 构建完整的主题
            let full_topic = format!("/sys/{}/{}/{}",
                self.config.product_key,
                self.config.device_name,
                self.config.topic
            );
            
            let message = paho::Message::new(&full_topic, data, self.get_qos());
            client.publish(message).await
                .map_err(|e| NetSrvError::AliyunIotError(format!("Error publishing to Aliyun IoT: {}", e)))?;
            
            debug!("Published message to Aliyun IoT topic: {}", full_topic);
            Ok(())
        } else {
            Err(NetSrvError::ConnectionError(
                "Aliyun IoT client not initialized".to_string(),
            ))
        }
    }

    fn is_connected(&self) -> bool {
        *self.connected.lock().unwrap()
    }
} 