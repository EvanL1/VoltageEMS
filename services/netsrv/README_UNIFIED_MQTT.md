# NetSrv 统一MQTT架构

## 概述

NetSrv已重构为基于统一MQTT客户端的架构，使用mosquitto库支持多种云平台。新架构提供了更简洁、更易维护的代码结构，同时保持了对现有配置的向后兼容性。

## 架构优势

### 🎯 **统一架构**
- 所有云平台使用相同的MQTT客户端核心
- 减少代码重复，提高维护效率
- 统一的错误处理和连接管理

### 🔧 **灵活配置**
- 支持多种认证方式（证书、设备密钥、SAS Token等）
- 动态Topic模板系统
- 丰富的TLS配置选项

### 🌐 **多云支持**
- AWS IoT Core
- 阿里云IoT平台
- Azure IoT Hub
- 腾讯云IoT Hub
- 华为云IoTDA
- 自定义MQTT Broker

## 配置结构

### 新的云配置格式

```json
{
  "cloud_networks": [
    {
      "name": "AWS IoT Core",
      "enabled": true,
      "cloud_provider": "aws",
      "endpoint": "your-endpoint.iot.region.amazonaws.com",
      "port": 8883,
      "client_id": "device-001",
      "auth_config": {
        "type": "certificate",
        "cert_path": "/path/to/cert.pem",
        "key_path": "/path/to/key.pem",
        "ca_path": "/path/to/ca.pem"
      },
      "topic_config": {
        "publish_topic": "ems/{device_id}/data",
        "subscribe_topics": ["ems/{device_id}/commands"],
        "qos": 1,
        "retain": false
      }
    }
  ]
}
```

### 支持的认证方式

#### 1. 证书认证 (AWS IoT, 腾讯云IoT)
```json
{
  "auth_config": {
    "type": "certificate",
    "cert_path": "/path/to/device-cert.pem",
    "key_path": "/path/to/device-key.pem", 
    "ca_path": "/path/to/root-ca.pem"
  }
}
```

#### 2. 设备密钥认证 (阿里云IoT, 华为云IoT)
```json
{
  "auth_config": {
    "type": "device_secret",
    "product_key": "your-product-key",
    "device_name": "your-device-name",
    "device_secret": "your-device-secret"
  }
}
```

#### 3. SAS Token认证 (Azure IoT Hub)
```json
{
  "auth_config": {
    "type": "sas_token",
    "token": "SharedAccessSignature sr=...",
    "expiry": null
  }
}
```

#### 4. 用户名密码认证
```json
{
  "auth_config": {
    "type": "username_password",
    "username": "your-username",
    "password": "your-password"
  }
}
```

## Topic模板系统

支持动态Topic变量替换：

### 内置变量
- `{device_id}`: 客户端ID
- `{timestamp}`: 当前时间戳

### 自定义变量
通过`topic_variables`配置：
```json
{
  "topic_config": {
    "publish_topic": "ems/{site_id}/{device_id}/data",
    "topic_variables": {
      "site_id": "factory-001",
      "location": "workshop-a"
    }
  }
}
```

## 云平台特定配置

### AWS IoT Core
```json
{
  "cloud_provider": "aws",
  "endpoint": "xxx.iot.us-east-1.amazonaws.com",
  "port": 8883,
  "auth_config": {
    "type": "certificate",
    "cert_path": "/etc/ssl/aws-device-cert.pem",
    "key_path": "/etc/ssl/aws-device-key.pem",
    "ca_path": "/etc/ssl/aws-root-ca.pem"
  },
  "topic_config": {
    "publish_topic": "ems/{device_id}/telemetry",
    "subscribe_topics": [
      "ems/{device_id}/commands",
      "$aws/things/{device_id}/shadow/update/delta"
    ]
  }
}
```

### 阿里云IoT平台
```json
{
  "cloud_provider": "aliyun",
  "endpoint": "xxx.iot-as-mqtt.cn-shanghai.aliyuncs.com",
  "port": 443,
  "auth_config": {
    "type": "device_secret",
    "product_key": "your-product-key",
    "device_name": "your-device-name",
    "device_secret": "your-device-secret"
  },
  "topic_config": {
    "publish_topic": "/sys/{product_key}/{device_name}/thing/event/property/post",
    "subscribe_topics": [
      "/sys/{product_key}/{device_name}/thing/service/property/set"
    ]
  }
}
```

### Azure IoT Hub
```json
{
  "cloud_provider": "azure",
  "endpoint": "your-hub.azure-devices.net",
  "port": 8883,
  "auth_config": {
    "type": "sas_token",
    "token": "SharedAccessSignature sr=...",
    "expiry": null
  },
  "topic_config": {
    "publish_topic": "devices/{device_id}/messages/events/",
    "subscribe_topics": [
      "devices/{device_id}/messages/devicebound/#"
    ]
  }
}
```

## 迁移指南

### 从旧配置迁移

1. **保持兼容性**: 旧的`networks`配置仍然有效
2. **添加云配置**: 在配置文件中添加`cloud_networks`数组
3. **逐步迁移**: 可以逐一将云平台配置从旧格式迁移到新格式
4. **测试验证**: 启用新配置前先进行测试

### 示例迁移
```json
// 旧配置 (仍然支持)
{
  "networks": [
    {
      "name": "AWS IoT",
      "network_type": "aws_iot",
      "aws_iot_config": { ... }
    }
  ]
}

// 新配置 (推荐)
{
  "cloud_networks": [
    {
      "name": "AWS IoT Core",
      "cloud_provider": "aws",
      "auth_config": { ... },
      "topic_config": { ... }
    }
  ]
}
```

## 运行和测试

### 编译项目
```bash
cd services/netsrv
cargo build
```

### 运行测试
```bash
cargo test
```

### 使用示例配置运行
```bash
cargo run -- --config examples/cloud_config.json
```

### 日志输出
```
[INFO] Starting Network Service
[INFO] Found 1 legacy network configurations
[INFO] Found 5 cloud network configurations
[INFO] Initializing cloud network: Custom MQTT Broker (custom)
[INFO] Connecting to Custom MQTT Broker (custom)
[INFO] MQTT connected successfully
[INFO] Successfully connected to Custom MQTT Broker
[INFO] Subscribed to topic: ems/commands/#
```

## 故障排除

### 常见问题

1. **证书文件不存在**
   ```
   Error: Certificate file not found: /path/to/cert.pem
   ```
   检查证书文件路径是否正确

2. **认证失败**
   ```
   Error: MQTT connection failed: BadUserNameOrPassword
   ```
   检查认证配置是否正确

3. **连接超时**
   ```
   Error: Connection timeout
   ```
   检查网络连接和端点配置

### 调试技巧

1. **启用调试日志**
   ```json
   {
     "logging": {
       "level": "debug"
     }
   }
   ```

2. **验证配置**
   - 检查JSON格式是否正确
   - 使用配置验证功能
   - 查看启动日志中的配置信息

3. **网络诊断**
   ```bash
   # 测试端点连接
   telnet your-endpoint.com 8883
   
   # 检查证书
   openssl x509 -in cert.pem -text -noout
   ```

## 最佳实践

1. **安全配置**
   - 使用强密码和证书
   - 定期更新认证凭据
   - 启用TLS验证

2. **性能优化**
   - 合理设置保活时间
   - 调整重连参数
   - 监控连接状态

3. **运维管理**
   - 使用配置文件管理
   - 设置适当的日志级别
   - 监控连接和消息状态

## 后续发展

计划支持的功能：
- [ ] 华为云IoTDA集成
- [ ] 更多认证方式
- [ ] 消息路由规则
- [ ] 监控和指标收集
- [ ] 配置热重载
- [ ] 集群部署支持 