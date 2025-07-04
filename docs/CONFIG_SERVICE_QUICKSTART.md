# 配置中心服务快速开发指南

## 1. 创建配置中心服务

基于 config-framework 创建独立的配置中心 REST 服务。

### 项目结构

```
services/config-service/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── api/
│   │   ├── mod.rs
│   │   ├── routes.rs
│   │   └── handlers.rs
│   ├── models/
│   │   ├── mod.rs
│   │   └── config.rs
│   ├── storage/
│   │   ├── mod.rs
│   │   └── sqlite.rs
│   └── error.rs
└── config/
    └── config-service.yml
```

### Cargo.toml

```toml
[package]
name = "config-service"
version = "0.1.0"
edition = "2021"

[dependencies]
# Web framework
actix-web = "4"
actix-cors = "0.6"

# Config framework
config-framework = { path = "../config-framework" }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"

# Database
sqlx = { version = "0.7", features = ["runtime-tokio-native-tls", "sqlite"] }

# Async runtime
tokio = { version = "1", features = ["full"] }

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Utils
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
sha2 = "0.10"
```

### 主程序 (main.rs)

```rust
use actix_web::{web, App, HttpServer, middleware};
use actix_cors::Cors;
use std::sync::Arc;
use tracing::{info, error};

mod api;
mod models;
mod storage;
mod error;

use storage::ConfigStorage;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    // 初始化存储
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://config.db".to_string());
    
    let storage = Arc::new(
        ConfigStorage::new(&db_url)
            .await
            .expect("Failed to initialize storage")
    );

    // 运行数据库迁移
    storage.run_migrations().await
        .expect("Failed to run migrations");

    let bind_addr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8000".to_string());

    info!("Starting Config Service on {}", bind_addr);

    // 启动 HTTP 服务器
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .app_data(web::Data::new(storage.clone()))
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .configure(api::configure_routes)
    })
    .bind(&bind_addr)?
    .run()
    .await
}
```

### API 路由 (api/routes.rs)

```rust
use actix_web::web;
use super::handlers;

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            // 健康检查
            .route("/health", web::get().to(handlers::health_check))
            
            // 配置管理
            .service(
                web::scope("/config")
                    // 获取服务配置
                    .route("/{service}", web::get().to(handlers::get_config))
                    // 获取配置版本
                    .route("/{service}/version", web::get().to(handlers::get_version))
                    // 更新配置
                    .route("/{service}/update", web::put().to(handlers::update_config))
                    // 配置历史
                    .route("/{service}/history", web::get().to(handlers::get_history))
                    // 回滚配置
                    .route("/{service}/rollback", web::post().to(handlers::rollback_config))
                    // 导出配置
                    .route("/{service}/export", web::get().to(handlers::export_config))
                    // 导入配置
                    .route("/{service}/import", web::post().to(handlers::import_config))
                    // 验证配置
                    .route("/{service}/validate", web::post().to(handlers::validate_config))
                    // 订阅管理
                    .route("/subscribe", web::post().to(handlers::subscribe))
                    .route("/subscribe/{id}", web::delete().to(handlers::unsubscribe))
            )
    );
}
```

### 处理器示例 (api/handlers.rs)

```rust
use actix_web::{web, HttpResponse, HttpRequest};
use crate::{storage::ConfigStorage, models::*, error::ConfigError};
use std::sync::Arc;
use tracing::info;

pub async fn get_config(
    service: web::Path<String>,
    storage: web::Data<Arc<ConfigStorage>>,
    req: HttpRequest,
) -> Result<HttpResponse, ConfigError> {
    // 验证服务名
    let service_name = validate_service_name(&req, &service)?;
    
    // 获取配置
    let config = storage.get_service_config(&service_name).await?;
    let version = storage.get_config_version(&service_name).await?;
    
    // 计算校验和
    let checksum = calculate_checksum(&config);
    
    let response = ConfigResponse {
        version,
        data: config,
        checksum,
    };
    
    Ok(HttpResponse::Ok().json(response))
}

pub async fn update_config(
    service: web::Path<String>,
    update_req: web::Json<UpdateConfigRequest>,
    storage: web::Data<Arc<ConfigStorage>>,
    req: HttpRequest,
) -> Result<HttpResponse, ConfigError> {
    let service_name = validate_service_name(&req, &service)?;
    
    info!("Updating config for service: {} key: {}", service_name, update_req.key);
    
    // 更新配置
    let new_version = storage.update_config(
        &service_name,
        &update_req.key,
        &update_req.value,
        &update_req.reason,
    ).await?;
    
    // 触发通知
    notify_subscribers(&service_name, &update_req.key, new_version).await;
    
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "version": new_version,
        "message": "Configuration updated successfully"
    })))
}

fn validate_service_name(req: &HttpRequest, path_service: &str) -> Result<String, ConfigError> {
    let header_service = req.headers()
        .get("X-Service-Name")
        .and_then(|h| h.to_str().ok())
        .ok_or(ConfigError::Unauthorized)?;
    
    if header_service != path_service {
        return Err(ConfigError::Unauthorized);
    }
    
    Ok(path_service.to_string())
}

fn calculate_checksum(config: &serde_json::Value) -> String {
    use sha2::{Sha256, Digest};
    let json = serde_json::to_string(config).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(json.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

async fn notify_subscribers(service: &str, key: &str, version: u64) {
    // TODO: 实现 Webhook 通知逻辑
    info!("Notifying subscribers for {} config change", service);
}
```

### 存储层 (storage/sqlite.rs)

```rust
use sqlx::{SqlitePool, Row};
use crate::error::ConfigError;
use serde_json::Value;
use std::collections::HashMap;

pub struct ConfigStorage {
    pool: SqlitePool,
}

impl ConfigStorage {
    pub async fn new(db_url: &str) -> Result<Self, ConfigError> {
        let pool = SqlitePool::connect(db_url).await?;
        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<(), ConfigError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_service_config(&self, service: &str) -> Result<Value, ConfigError> {
        let rows = sqlx::query(
            "SELECT key, value, value_type FROM configs WHERE service = ? ORDER BY key"
        )
        .bind(service)
        .fetch_all(&self.pool)
        .await?;

        let mut config = serde_json::Map::new();
        
        for row in rows {
            let key: String = row.get("key");
            let value_str: String = row.get("value");
            let value_type: String = row.get("value_type");
            
            let value = parse_value(&value_str, &value_type)?;
            set_nested_value(&mut config, &key, value);
        }

        Ok(Value::Object(config))
    }

    pub async fn update_config(
        &self,
        service: &str,
        key: &str,
        value: &Value,
        reason: &str,
    ) -> Result<u64, ConfigError> {
        let mut tx = self.pool.begin().await?;
        
        // 获取当前版本
        let current_version = self.get_config_version(service).await?;
        let new_version = current_version + 1;
        
        // 保存旧值到历史
        let old_value = self.get_config_value(service, key).await.ok();
        
        // 更新配置
        let value_str = serde_json::to_string(value)?;
        let value_type = detect_value_type(value);
        
        sqlx::query(
            "INSERT OR REPLACE INTO configs (service, key, value, value_type, version) 
             VALUES (?, ?, ?, ?, ?)"
        )
        .bind(service)
        .bind(key)
        .bind(&value_str)
        .bind(&value_type)
        .bind(new_version as i64)
        .execute(&mut *tx)
        .await?;
        
        // 记录历史
        sqlx::query(
            "INSERT INTO config_history 
             (service, key, version, operation, old_value, new_value, reason) 
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(service)
        .bind(key)
        .bind(new_version as i64)
        .bind("update")
        .bind(old_value.as_ref().map(|v| serde_json::to_string(v).unwrap()))
        .bind(&value_str)
        .bind(reason)
        .execute(&mut *tx)
        .await?;
        
        tx.commit().await?;
        
        Ok(new_version)
    }
}

fn parse_value(value_str: &str, value_type: &str) -> Result<Value, ConfigError> {
    match value_type {
        "json" => Ok(serde_json::from_str(value_str)?),
        "string" => Ok(Value::String(value_str.to_string())),
        "number" => Ok(serde_json::from_str(value_str)?),
        "boolean" => Ok(Value::Bool(value_str.parse()?)),
        _ => Ok(Value::String(value_str.to_string())),
    }
}

fn detect_value_type(value: &Value) -> &'static str {
    match value {
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
        Value::Object(_) | Value::Array(_) => "json",
        Value::Null => "null",
    }
}

fn set_nested_value(map: &mut serde_json::Map<String, Value>, key: &str, value: Value) {
    let parts: Vec<&str> = key.split('.').collect();
    
    if parts.len() == 1 {
        map.insert(key.to_string(), value);
        return;
    }
    
    // 处理嵌套键
    let mut current = map;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            current.insert(part.to_string(), value);
        } else {
            let entry = current
                .entry(part.to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            
            if let Value::Object(obj) = entry {
                current = obj;
            }
        }
    }
}
```

### 数据库迁移 (migrations/001_init.sql)

```sql
-- 主配置表
CREATE TABLE IF NOT EXISTS configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service VARCHAR(50) NOT NULL,
    key VARCHAR(255) NOT NULL,
    value TEXT NOT NULL,
    value_type VARCHAR(20) NOT NULL DEFAULT 'string',
    version INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(service, key)
);

-- 配置历史表
CREATE TABLE IF NOT EXISTS config_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service VARCHAR(50) NOT NULL,
    key VARCHAR(255) NOT NULL,
    version INTEGER NOT NULL,
    operation VARCHAR(20) NOT NULL,
    old_value TEXT,
    new_value TEXT,
    reason TEXT,
    user VARCHAR(50),
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- 订阅表
CREATE TABLE IF NOT EXISTS config_subscriptions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subscription_id VARCHAR(50) UNIQUE NOT NULL,
    service VARCHAR(50) NOT NULL,
    callback_url TEXT NOT NULL,
    events TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_notified TIMESTAMP
);

-- 索引
CREATE INDEX idx_configs_service ON configs(service);
CREATE INDEX idx_configs_service_key ON configs(service, key);
CREATE INDEX idx_history_service_version ON config_history(service, version);
CREATE INDEX idx_subscriptions_service ON config_subscriptions(service);
```

## 2. Docker 部署

### Dockerfile

```dockerfile
FROM rust:1.75 as builder

WORKDIR /app
COPY . .
RUN cargo build --release --bin config-service

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/config-service /usr/local/bin/
COPY --from=builder /app/migrations /migrations

ENV DATABASE_URL=sqlite:///data/config.db
ENV BIND_ADDR=0.0.0.0:8000

VOLUME ["/data"]
EXPOSE 8000

CMD ["config-service"]
```

### docker-compose.yml

```yaml
version: '3.8'

services:
  config-service:
    build: ./services/config-service
    ports:
      - "8000:8000"
    environment:
      - DATABASE_URL=sqlite:///data/config.db
      - RUST_LOG=info
    volumes:
      - config-data:/data
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8000/api/v1/health"]
      interval: 30s
      timeout: 10s
      retries: 3

volumes:
  config-data:
```

## 3. 测试脚本

### 初始化配置 (scripts/init-config.sh)

```bash
#!/bin/bash

CONFIG_SERVICE_URL=${CONFIG_SERVICE_URL:-"http://localhost:8000"}

echo "Initializing configuration for all services..."

# API Gateway 配置
curl -X POST "$CONFIG_SERVICE_URL/api/v1/config/apigateway/import" \
  -H "Content-Type: application/json" \
  -H "X-Service-Name: apigateway" \
  -d @- <<EOF
{
  "format": "json",
  "content": {
    "server": {
      "host": "0.0.0.0",
      "port": 8080,
      "workers": 4
    },
    "redis": {
      "url": "redis://localhost:6379",
      "pool_size": 10,
      "timeout_seconds": 5
    },
    "services": {
      "comsrv": {
        "url": "http://localhost:8001",
        "timeout_seconds": 30
      },
      "modsrv": {
        "url": "http://localhost:8002",
        "timeout_seconds": 30
      },
      "hissrv": {
        "url": "http://localhost:8003",
        "timeout_seconds": 30
      },
      "netsrv": {
        "url": "http://localhost:8004",
        "timeout_seconds": 30
      },
      "alarmsrv": {
        "url": "http://localhost:8005",
        "timeout_seconds": 30
      }
    }
  },
  "merge": false,
  "reason": "Initial configuration"
}
EOF

echo "Configuration initialized successfully!"
```

### 测试配置更新 (scripts/test-config-update.sh)

```bash
#!/bin/bash

CONFIG_SERVICE_URL=${CONFIG_SERVICE_URL:-"http://localhost:8000"}

# 更新单个配置项
echo "Updating comsrv URL..."
curl -X PUT "$CONFIG_SERVICE_URL/api/v1/config/apigateway/update" \
  -H "Content-Type: application/json" \
  -H "X-Service-Name: apigateway" \
  -d '{
    "key": "services.comsrv.url",
    "value": "http://localhost:8091",
    "reason": "Testing configuration update"
  }'

# 检查版本
echo -e "\n\nChecking version..."
curl "$CONFIG_SERVICE_URL/api/v1/config/apigateway/version" \
  -H "X-Service-Name: apigateway"

# 获取更新后的配置
echo -e "\n\nFetching updated configuration..."
curl "$CONFIG_SERVICE_URL/api/v1/config/apigateway" \
  -H "X-Service-Name: apigateway" | jq .
```

## 4. 集成到现有系统

### 启动顺序

```bash
#!/bin/bash
# start-all-services.sh

# 1. 启动基础设施
docker-compose up -d redis influxdb

# 2. 启动配置中心
docker-compose up -d config-service

# 3. 等待配置中心就绪
until curl -f http://localhost:8000/api/v1/health; do
  echo "Waiting for config service..."
  sleep 2
done

# 4. 初始化配置
./scripts/init-config.sh

# 5. 启动其他服务
export CONFIG_SERVICE_URL=http://localhost:8000
docker-compose up -d apigateway comsrv modsrv hissrv netsrv alarmsrv

# 6. 启动前端
docker-compose up -d frontend
```

## 5. 监控和维护

### Prometheus 指标

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'config-service'
    static_configs:
      - targets: ['localhost:8000']
    metrics_path: '/metrics'
```

### 备份脚本

```bash
#!/bin/bash
# backup-config.sh

BACKUP_DIR="/backup/config"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# 备份数据库
sqlite3 /data/config.db ".backup $BACKUP_DIR/config_$TIMESTAMP.db"

# 导出所有服务配置
for service in apigateway comsrv modsrv hissrv netsrv alarmsrv; do
  curl "http://localhost:8000/api/v1/config/$service/export?format=yaml" \
    -H "X-Service-Name: $service" \
    -o "$BACKUP_DIR/${service}_$TIMESTAMP.yaml"
done

echo "Backup completed: $BACKUP_DIR/*_$TIMESTAMP.*"
```

这个快速开发指南提供了创建配置中心服务的完整步骤，从代码实现到部署运维的全流程。