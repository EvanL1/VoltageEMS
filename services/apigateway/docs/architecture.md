# apigateway 架构设计

## 概述

apigateway 采用混合数据访问架构，通过智能路由策略为不同类型的数据选择最优访问路径。服务基于 axum 框架构建，提供高性能的异步处理能力，支持 RESTful API 和 WebSocket 双协议。

## 核心架构

```
┌─────────────────────────────────────────────────────────┐
│                    API Gateway                          │
├─────────────────────────────────────────────────────────┤
│                   HTTP Server (axum)                    │
│     ┌──────────────┬──────────────┬──────────────┐     │
│     │   Router     │  Middleware  │  WebSocket   │     │
│     │   Layer      │    Stack     │   Handler    │     │
│     └──────────────┴──────────────┴──────────────┘     │
├─────────────────────────────────────────────────────────┤
│                 Authentication Layer                    │
│     ┌──────────────┬──────────────┬──────────────┐     │
│     │ JWT Handler  │   Session    │  Permission  │     │
│     │              │   Manager    │   Control    │     │
│     └──────────────┴──────────────┴──────────────┘     │
├─────────────────────────────────────────────────────────┤
│               Data Access Layer (DAL)                   │
│     ┌──────────┬──────────┬──────────┬──────────┐     │
│     │  Redis   │ InfluxDB │   HTTP   │  Cache   │     │
│     │  Client  │  Client  │  Client  │  Manager │     │
│     └──────────┴──────────┴──────────┴──────────┘     │
├─────────────────────────────────────────────────────────┤
│                 Smart Router                            │
│     ┌──────────────┬──────────────┬──────────────┐     │
│     │ Real-time    │ Historical   │ Config/Mgmt  │     │
│     │ (Redis)      │ (InfluxDB)   │ (HTTP+Cache) │     │
│     └──────────────┴──────────────┴──────────────┘     │
├─────────────────────────────────────────────────────────┤
│                Service Handlers                         │
│     ┌─────┬─────┬─────┬─────┬─────┬─────┬─────┐      │
│     │Auth │Chan │Data │Alarm│Rule │Net  │Sys  │      │
│     └─────┴─────┴─────┴─────┴─────┴─────┴─────┘      │
└─────────────────────────────────────────────────────────┘
```

## 组件说明

### 1. HTTP Server Layer

基于 axum 框架的高性能 HTTP 服务器：

```rust
pub struct ApiServer {
    config: Arc<ApiConfig>,
    dal: Arc<dyn DataAccessLayer>,
    ws_hub: Arc<WebSocketHub>,
}

impl ApiServer {
    pub async fn start(&self) -> Result<()> {
        let app = Router::new()
            // API 路由
            .nest("/api", self.api_routes())
            // WebSocket 路由
            .route("/ws", get(websocket_handler))
            // 健康检查
            .route("/health", get(health_check))
            // 中间件
            .layer(middleware::from_fn(auth_middleware))
            .layer(CorsLayer::new())
            .layer(TraceLayer::new_for_http())
            .layer(CompressionLayer::new())
            // 共享状态
            .with_state(AppState {
                dal: self.dal.clone(),
                ws_hub: self.ws_hub.clone(),
            });
            
        let addr = SocketAddr::from(([0, 0, 0, 0], self.config.port));
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await?;
            
        Ok(())
    }
}
```

### 2. Authentication Layer

JWT 认证和授权管理：

```rust
pub struct AuthManager {
    jwt_secret: String,
    token_expiry: Duration,
    refresh_expiry: Duration,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,        // 用户 ID
    pub exp: i64,          // 过期时间
    pub iat: i64,          // 签发时间
    pub roles: Vec<String>, // 用户角色
}

impl AuthManager {
    pub fn generate_token(&self, user_id: &str, roles: Vec<String>) -> Result<TokenPair> {
        let now = Utc::now();
        
        // Access Token
        let access_claims = Claims {
            sub: user_id.to_string(),
            exp: (now + self.token_expiry).timestamp(),
            iat: now.timestamp(),
            roles: roles.clone(),
        };
        
        let access_token = encode(
            &Header::default(),
            &access_claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?;
        
        // Refresh Token
        let refresh_claims = Claims {
            sub: user_id.to_string(),
            exp: (now + self.refresh_expiry).timestamp(),
            iat: now.timestamp(),
            roles,
        };
        
        let refresh_token = encode(
            &Header::default(),
            &refresh_claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?;
        
        Ok(TokenPair {
            access_token,
            refresh_token,
            expires_in: self.token_expiry.num_seconds(),
        })
    }
    
    pub fn verify_token(&self, token: &str) -> Result<Claims> {
        let validation = Validation::default();
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &validation,
        )?;
        
        Ok(token_data.claims)
    }
}

// 认证中间件
pub async fn auth_middleware<B>(
    State(state): State<AppState>,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    let auth_header = req.headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok());
        
    if let Some(auth) = auth_header {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            match state.auth_manager.verify_token(token) {
                Ok(claims) => {
                    req.extensions_mut().insert(claims);
                }
                Err(_) => return Err(StatusCode::UNAUTHORIZED),
            }
        }
    }
    
    Ok(next.run(req).await)
}
```

### 3. Data Access Layer

统一的数据访问接口：

```rust
#[async_trait]
pub trait DataAccessLayer: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<String>>;
    async fn hget(&self, key: &str, field: &str) -> Result<Option<String>>;
    async fn hgetall(&self, key: &str) -> Result<HashMap<String, String>>;
    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<()>;
    async fn query_history(&self, query: &HistoryQuery) -> Result<Vec<DataPoint>>;
    async fn http_request(&self, service: &str, path: &str, method: Method) -> Result<Response>;
}

pub struct HybridDataAccess {
    redis_client: Arc<RedisClient>,
    influx_client: Arc<InfluxClient>,
    http_client: Arc<HttpClient>,
    cache_manager: Arc<CacheManager>,
}

#[async_trait]
impl DataAccessLayer for HybridDataAccess {
    async fn get(&self, key: &str) -> Result<Option<String>> {
        // 智能路由
        let strategy = self.determine_strategy(key);
        
        match strategy {
            AccessStrategy::RedisOnly => {
                self.redis_client.get(key).await
            }
            AccessStrategy::RedisWithHttpFallback => {
                // 先尝试 Redis
                if let Some(value) = self.redis_client.get(key).await? {
                    return Ok(Some(value));
                }
                
                // 回源到 HTTP
                if let Some(value) = self.fetch_from_http(key).await? {
                    // 写入缓存
                    self.redis_client.set(key, &value, Some(3600)).await?;
                    return Ok(Some(value));
                }
                
                Ok(None)
            }
            AccessStrategy::InfluxDB => {
                // 历史数据查询
                Err(Error::InvalidKey("Use query_history for historical data"))
            }
            AccessStrategy::HttpOnly => {
                // 直接 HTTP
                self.fetch_from_http(key).await
            }
        }
    }
    
    async fn hgetall(&self, key: &str) -> Result<HashMap<String, String>> {
        // 批量获取 Hash 数据
        if key.starts_with("comsrv:") || key.starts_with("modsrv:") {
            self.redis_client.hgetall(key).await
        } else {
            Err(Error::InvalidKey("Hash operations only for real-time data"))
        }
    }
}
```

### 4. Smart Router

智能路由决策引擎：

```rust
pub struct SmartRouter {
    route_rules: HashMap<String, AccessStrategy>,
}

#[derive(Debug, Clone)]
pub enum AccessStrategy {
    RedisOnly,              // 实时数据
    RedisWithHttpFallback,  // 配置数据
    InfluxDB,              // 历史数据
    HttpOnly,              // 复杂查询
}

impl SmartRouter {
    pub fn determine_strategy(&self, key: &str) -> AccessStrategy {
        // 实时数据 - Redis 直接访问
        if key.starts_with("comsrv:") || 
           key.starts_with("modsrv:") ||
           key.starts_with("alarm:") {
            return AccessStrategy::RedisOnly;
        }
        
        // 配置数据 - Redis + HTTP 回源
        if key.starts_with("cfg:") || 
           key.starts_with("model:def:") ||
           key.starts_with("rule:") {
            return AccessStrategy::RedisWithHttpFallback;
        }
        
        // 历史数据 - InfluxDB
        if key.starts_with("his:") {
            return AccessStrategy::InfluxDB;
        }
        
        // 其他 - HTTP
        AccessStrategy::HttpOnly
    }
    
    pub fn parse_key(&self, key: &str) -> KeyInfo {
        let parts: Vec<&str> = key.split(':').collect();
        
        KeyInfo {
            prefix: parts.get(0).unwrap_or("").to_string(),
            service: self.extract_service(&parts),
            resource_type: parts.get(1).map(|s| s.to_string()),
            resource_id: parts.get(2).map(|s| s.to_string()),
        }
    }
}
```

### 5. WebSocket Hub

WebSocket 连接管理和消息分发：

```rust
pub struct WebSocketHub {
    clients: Arc<RwLock<HashMap<Uuid, Client>>>,
    subscriptions: Arc<RwLock<HashMap<String, HashSet<Uuid>>>>,
    tx: broadcast::Sender<Message>,
}

pub struct Client {
    id: Uuid,
    user_id: String,
    sender: mpsc::UnboundedSender<Message>,
    subscriptions: HashSet<String>,
}

impl WebSocketHub {
    pub async fn add_client(&self, client: Client) {
        let id = client.id;
        self.clients.write().await.insert(id, client);
    }
    
    pub async fn remove_client(&self, id: Uuid) {
        if let Some(client) = self.clients.write().await.remove(&id) {
            // 清理订阅
            for topic in &client.subscriptions {
                if let Some(subs) = self.subscriptions.write().await.get_mut(topic) {
                    subs.remove(&id);
                }
            }
        }
    }
    
    pub async fn subscribe(&self, client_id: Uuid, topics: Vec<String>) {
        let mut clients = self.clients.write().await;
        let mut subscriptions = self.subscriptions.write().await;
        
        if let Some(client) = clients.get_mut(&client_id) {
            for topic in topics {
                client.subscriptions.insert(topic.clone());
                subscriptions
                    .entry(topic)
                    .or_insert_with(HashSet::new)
                    .insert(client_id);
            }
        }
    }
    
    pub async fn broadcast_to_topic(&self, topic: &str, message: Message) {
        let subscriptions = self.subscriptions.read().await;
        let clients = self.clients.read().await;
        
        if let Some(client_ids) = subscriptions.get(topic) {
            for client_id in client_ids {
                if let Some(client) = clients.get(client_id) {
                    let _ = client.sender.send(message.clone());
                }
            }
        }
    }
}
```

### 6. Cache Manager

多层缓存管理器：

```rust
pub struct CacheManager {
    l1_cache: Arc<Mutex<LruCache<String, CachedValue>>>,
    redis_client: Arc<RedisClient>,
    config: CacheConfig,
}

#[derive(Clone)]
pub struct CachedValue {
    data: String,
    expires_at: Instant,
}

impl CacheManager {
    pub async fn get(&self, key: &str) -> Option<String> {
        // L1 缓存查询
        {
            let mut cache = self.l1_cache.lock().unwrap();
            if let Some(cached) = cache.get(key) {
                if cached.expires_at > Instant::now() {
                    return Some(cached.data.clone());
                } else {
                    cache.pop(key);
                }
            }
        }
        
        // L2 Redis 缓存查询
        if let Ok(Some(value)) = self.redis_client.get(&self.cache_key(key)).await {
            // 写入 L1
            self.set_l1(key, &value, self.config.l1_ttl);
            return Some(value);
        }
        
        None
    }
    
    pub async fn set(&self, key: &str, value: &str, ttl: Duration) -> Result<()> {
        // 写入 L1
        self.set_l1(key, value, ttl);
        
        // 写入 L2
        let redis_key = self.cache_key(key);
        self.redis_client.set(&redis_key, value, Some(ttl.as_secs() as usize)).await?;
        
        Ok(())
    }
    
    fn set_l1(&self, key: &str, value: &str, ttl: Duration) {
        let mut cache = self.l1_cache.lock().unwrap();
        cache.put(
            key.to_string(),
            CachedValue {
                data: value.to_string(),
                expires_at: Instant::now() + ttl,
            },
        );
    }
}
```

## API 路由设计

### 路由结构

```rust
pub fn api_routes() -> Router<AppState> {
    Router::new()
        // 认证路由
        .nest("/auth", auth_routes())
        // 通道管理
        .nest("/channels", channel_routes())
        // 设备模型
        .nest("/device-models", model_routes())
        // 历史数据
        .nest("/historical", history_routes())
        // 告警管理
        .nest("/alarms", alarm_routes())
        // 规则管理
        .nest("/rules", rule_routes())
        // 配置管理
        .nest("/configs", config_routes())
        // 系统信息
        .nest("/system", system_routes())
        // 服务代理
        .nest("/comsrv", service_proxy("comsrv"))
        .nest("/modsrv", service_proxy("modsrv"))
        .nest("/hissrv", service_proxy("hissrv"))
        .nest("/alarmsrv", service_proxy("alarmsrv"))
        .nest("/rulesrv", service_proxy("rulesrv"))
        .nest("/netsrv", service_proxy("netsrv"))
}
```

### 请求处理流程

```
HTTP Request
     ↓
CORS Check
     ↓
Rate Limiting
     ↓
Authentication
     ↓
Authorization
     ↓
Route Handler
     ↓
Smart Router
     ↓
Data Access
     ↓
Response Format
     ↓
HTTP Response
```

## 性能优化

### 1. 连接池管理

```rust
pub struct ConnectionPools {
    redis_pool: bb8::Pool<RedisConnectionManager>,
    http_pools: HashMap<String, Client>,
}

impl ConnectionPools {
    pub async fn new(config: &PoolConfig) -> Result<Self> {
        // Redis 连接池
        let manager = RedisConnectionManager::new(config.redis_url)?;
        let redis_pool = bb8::Pool::builder()
            .max_size(config.redis_pool_size)
            .min_idle(Some(config.redis_min_idle))
            .build(manager)
            .await?;
            
        // HTTP 客户端池
        let mut http_pools = HashMap::new();
        for (service, url) in &config.services {
            let client = Client::builder()
                .pool_idle_timeout(Duration::from_secs(90))
                .pool_max_idle_per_host(10)
                .timeout(Duration::from_secs(30))
                .build()?;
            http_pools.insert(service.clone(), client);
        }
        
        Ok(Self {
            redis_pool,
            http_pools,
        })
    }
}
```

### 2. 批量操作

```rust
pub async fn batch_get_channels(
    State(state): State<AppState>,
    Query(params): Query<BatchQuery>,
) -> Result<Json<BatchResponse>> {
    let channel_ids = params.ids.split(',').collect::<Vec<_>>();
    
    // 并发获取
    let futures = channel_ids.iter().map(|id| {
        let dal = state.dal.clone();
        async move {
            let key = format!("comsrv:{}:m", id);
            dal.hgetall(&key).await
        }
    });
    
    let results = futures::future::join_all(futures).await;
    
    // 组装响应
    let mut data = HashMap::new();
    for (i, result) in results.into_iter().enumerate() {
        if let Ok(values) = result {
            data.insert(channel_ids[i].to_string(), values);
        }
    }
    
    Ok(Json(BatchResponse { data }))
}
```

### 3. 响应压缩

```rust
// 自动响应压缩
.layer(CompressionLayer::new()
    .gzip(true)
    .br(true)
    .deflate(true)
    .quality(CompressionLevel::Fastest))
```

## 监控和诊断

### 健康检查

```rust
pub async fn health_detailed(
    State(state): State<AppState>,
) -> Result<Json<HealthStatus>> {
    let mut services = HashMap::new();
    
    // Redis 健康检查
    let redis_start = Instant::now();
    let redis_health = match state.dal.get("health:check").await {
        Ok(_) => ServiceHealth {
            status: "healthy".to_string(),
            latency: redis_start.elapsed().as_millis() as u64,
        },
        Err(e) => ServiceHealth {
            status: "unhealthy".to_string(),
            latency: 0,
            error: Some(e.to_string()),
        },
    };
    services.insert("redis".to_string(), redis_health);
    
    // 后端服务健康检查
    for (service, client) in &state.http_pools {
        let start = Instant::now();
        let health = match client.get(&format!("{}/health", service)).send().await {
            Ok(resp) if resp.status().is_success() => ServiceHealth {
                status: "healthy".to_string(),
                latency: start.elapsed().as_millis() as u64,
            },
            _ => ServiceHealth {
                status: "unhealthy".to_string(),
                latency: 0,
            },
        };
        services.insert(service.clone(), health);
    }
    
    Ok(Json(HealthStatus {
        status: if services.values().all(|h| h.status == "healthy") {
            "healthy"
        } else {
            "degraded"
        },
        services,
        timestamp: Utc::now(),
    }))
}
```

### 请求追踪

```rust
// 请求追踪中间件
.layer(
    TraceLayer::new_for_http()
        .make_span_with(|request: &Request<_>| {
            let request_id = Uuid::new_v4();
            tracing::info_span!(
                "http_request",
                method = %request.method(),
                uri = %request.uri(),
                request_id = %request_id,
            )
        })
        .on_response(|response: &Response, latency: Duration, _span: &Span| {
            tracing::info!(
                status = response.status().as_u16(),
                latency = ?latency,
                "response"
            );
        })
)
```

## 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Authentication failed")]
    Unauthorized,
    
    #[error("Permission denied")]
    Forbidden,
    
    #[error("Resource not found")]
    NotFound,
    
    #[error("Invalid request: {0}")]
    BadRequest(String),
    
    #[error("Internal server error")]
    InternalError,
    
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, self.to_string()),
            ApiError::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::InternalError => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ApiError::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg),
        };
        
        let body = Json(json!({
            "error": message,
            "timestamp": Utc::now(),
        }));
        
        (status, body).into_response()
    }
}
```

## 安全性设计

### 1. 输入验证

```rust
#[derive(Debug, Deserialize, Validator)]
pub struct ChannelQuery {
    #[validate(range(min = 1, max = 9999))]
    pub channel_id: u32,
    
    #[validate(length(min = 1, max = 10))]
    pub data_type: String,
}
```

### 2. 速率限制

```rust
pub struct RateLimiter {
    limiters: Arc<RwLock<HashMap<String, Governor>>>,
}

impl RateLimiter {
    pub async fn check_limit(&self, key: &str) -> Result<(), ApiError> {
        let limiters = self.limiters.read().await;
        
        if let Some(limiter) = limiters.get(key) {
            match limiter.check() {
                Ok(_) => Ok(()),
                Err(_) => Err(ApiError::TooManyRequests),
            }
        } else {
            Ok(())
        }
    }
}
```

### 3. CORS 配置

```rust
let cors = CorsLayer::new()
    .allow_origin(config.cors.allowed_origins.clone())
    .allow_methods(config.cors.allowed_methods.clone())
    .allow_headers([AUTHORIZATION, CONTENT_TYPE])
    .allow_credentials(true)
    .max_age(Duration::from_secs(config.cors.max_age));
```