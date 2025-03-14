# 网络服务 (Netsrv)

一个用于能源管理系统(EMS)的网络数据传输服务。该服务从Redis获取数据，并通过多种网络协议（MQTT、HTTP、AWS IoT Core、阿里云IoT）以JSON或ASCII格式将数据上送到外部系统。

## 功能特点

- 从Redis获取实时数据
- 支持多种网络协议：
  - 普通MQTT
  - HTTP/HTTPS
  - AWS IoT Core
  - 阿里云IoT
- 支持多种数据格式：
  - JSON
  - ASCII
- 可配置的数据过滤
- 异步处理，高效率传输
- 完全可配置，无需修改代码

## 架构

网络服务设计为在Docker容器中运行，并与以下组件交互：
- Redis容器：用于获取数据
- MQTT Broker（可选）：用于MQTT协议传输
- 外部API/云服务：用于HTTP、AWS IoT、阿里云IoT传输

## 配置

服务使用JSON文件(`netsrv.json`)进行配置。主要配置选项包括：

```json
{
  "redis": {
    "host": "localhost",
    "port": 6379,
    "password": "",
    "socket": "",
    "prefix": "ems:",
    "data_keys": [
      "ems:model:output:*",
      "ems:data:*"
    ],
    "poll_interval_ms": 1000
  },
  "logging": {
    "level": "info",
    "file": "/var/log/ems/netsrv.log",
    "console": true
  },
  "networks": [
    {
      "name": "Local MQTT",
      "enabled": true,
      "network_type": "mqtt",
      "format_type": "json",
      "mqtt_config": {
        "broker_url": "localhost",
        "port": 1883,
        "client_id": "ems-netsrv",
        "topic": "ems/data",
        "qos": 0
      }
    }
  ]
}
```

### 网络类型配置

#### MQTT配置

```json
{
  "mqtt_config": {
    "broker_url": "localhost",
    "port": 1883,
    "client_id": "ems-netsrv",
    "username": "user",
    "password": "pass",
    "topic": "ems/data",
    "qos": 0,
    "use_ssl": false,
    "ca_cert_path": null,
    "client_cert_path": null,
    "client_key_path": null
  }
}
```

#### HTTP配置

```json
{
  "http_config": {
    "url": "https://api.example.com/data",
    "method": "POST",
    "headers": {
      "Content-Type": "application/json",
      "X-API-Key": "your-api-key"
    },
    "auth_type": "basic",
    "username": "user",
    "password": "pass",
    "timeout_ms": 5000
  }
}
```

#### AWS IoT配置

```json
{
  "aws_iot_config": {
    "endpoint": "xxxxxxx-ats.iot.us-east-1.amazonaws.com",
    "region": "us-east-1",
    "topic": "ems/data",
    "thing_name": "ems-gateway",
    "client_id": "ems-netsrv",
    "cert_path": "/etc/ems/certs/certificate.pem.crt",
    "key_path": "/etc/ems/certs/private.pem.key",
    "ca_path": "/etc/ems/certs/AmazonRootCA1.pem",
    "qos": 1
  }
}
```

#### 阿里云IoT配置

```json
{
  "aliyun_iot_config": {
    "product_key": "your-product-key",
    "device_name": "your-device-name",
    "device_secret": "your-device-secret",
    "region_id": "cn-shanghai",
    "topic": "thing/event/property/post",
    "qos": 1
  }
}
```

## 构建和运行

### 使用Cargo构建

```bash
cargo build --release
```

### 运行服务

```bash
./target/release/netsrv --config netsrv.json
```

### 使用Docker

```bash
docker build -t netsrv .
docker run -d --name netsrv --network ems-network netsrv
```

### 使用Docker Compose

```bash
docker-compose up -d
```

## 开发

### 先决条件

- Rust 1.67或更高版本
- Redis服务器（用于开发）
- MQTT Broker（用于测试MQTT功能）

### 添加新的网络协议

1. 在`src/network/`目录下创建新的模块
2. 实现`NetworkClient` trait
3. 在`src/network/mod.rs`中添加新的网络类型和创建函数
4. 在`src/config/network_config.rs`中添加新的配置结构体

## 许可证

[您的许可证] 