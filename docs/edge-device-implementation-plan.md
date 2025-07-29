# VoltageEMS 边端设备 Lua + 连接池实施方案

## 一、项目背景

### 1.1 边端设备特点
- **单机部署**：所有服务运行在一台边端设备上
- **资源受限**：CPU、内存、存储都有限制
- **实时要求**：毫秒级数据同步
- **维护灵活**：可随时修改，无需复杂流程

### 1.2 目标
- 简化服务间数据同步
- 降低资源消耗
- 提高系统稳定性
- 便于现场维护

## 二、技术方案

### 2.1 架构简图
```
边端设备 (单机)
├── Redis (单实例)
│   ├── Lua Scripts (数据同步)
│   └── Data Storage (Hash/Pub/Sub)
├── ComsRv (数据采集)
├── ModSrv (API服务)
├── AlarmSrv (告警服务)
└── NetSrv (上传服务)
```

### 2.2 资源规划
```yaml
# 边端设备典型配置
CPU: 4核 ARM/x86
内存: 4-8GB
存储: 32-128GB

# 资源分配
Redis: 512MB内存
ComsRv: 256MB内存, 15个Redis连接
ModSrv: 128MB内存, 5个Redis连接
AlarmSrv: 128MB内存, 3个Redis连接
NetSrv: 256MB内存, 5个Redis连接
```

## 三、实施步骤

### 步骤1: 准备Lua脚本 (1天)

创建文件 `scripts/edge_sync.lua`:

```lua
-- 边端设备数据同步脚本
-- 精简版，专注核心功能

local action = ARGV[1]

if action == "sync_measurement" then
    -- ComsRv测量数据同步到ModSrv
    local channel = ARGV[2]
    local point = ARGV[3]
    local value = ARGV[4]
    
    -- 查找映射（使用简单格式）
    local mapping = redis.call('HGET', 'mapping', channel .. ':' .. point)
    if mapping then
        -- 映射格式: "model_id:point_name"
        local model_id, point_name = string.match(mapping, "([^:]+):([^:]+)")
        
        -- 更新ModSrv数据
        redis.call('HSET', 'modsrv:' .. model_id, point_name, value)
        
        -- 发布更新（可选，用于WebSocket）
        redis.call('PUBLISH', 'update:' .. model_id, point_name .. ':' .. value)
        
        -- 简单告警检查
        local threshold = redis.call('HGET', 'alarm:threshold', model_id .. ':' .. point_name)
        if threshold and tonumber(value) > tonumber(threshold) then
            redis.call('LPUSH', 'alarm:queue', model_id .. ':' .. point_name .. ':' .. value)
        end
    end
    
elseif action == "send_control" then
    -- ModSrv控制命令同步到ComsRv
    local model_id = ARGV[2]
    local control_name = ARGV[3]
    local value = ARGV[4]
    
    local mapping = redis.call('HGET', 'mapping:reverse', model_id .. ':' .. control_name)
    if mapping then
        local channel, point = string.match(mapping, "([^:]+):([^:]+)")
        redis.call('HSET', 'cmd:' .. channel, point, value)
        redis.call('PUBLISH', 'cmd:' .. channel, point .. ':' .. value)
    end
end

return 'OK'
```

加载脚本：
```bash
# 在设备上执行
redis-cli SCRIPT LOAD "$(cat scripts/edge_sync.lua)"
# 记录返回的SHA值，例如: 2e5d4f3b7c6a5d4e3f2a1b0c9d8e7f6a5b4c3d2e
```

### 步骤2: 添加连接池支持 (1天)

修改 `voltage-libs/Cargo.toml`:
```toml
[dependencies]
redis = { version = "0.24", features = ["tokio-comp", "connection-manager"] }
```

创建 `voltage-libs/src/edge_redis.rs`:
```rust
use redis::aio::ConnectionManager;
use redis::Client;

/// 边端设备Redis连接管理
/// 使用ConnectionManager而非复杂的连接池
pub struct EdgeRedis {
    conn: ConnectionManager,
    sync_script_sha: String,
}

impl EdgeRedis {
    pub async fn new(redis_url: &str, sync_script_sha: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::open(redis_url)?;
        let conn = ConnectionManager::new(client).await?;
        
        Ok(Self {
            conn,
            sync_script_sha: sync_script_sha.to_string(),
        })
    }
    
    /// 同步测量数据
    pub async fn sync_measurement(&mut self, channel: u32, point: u32, value: f64) -> Result<(), Box<dyn std::error::Error>> {
        redis::cmd("EVALSHA")
            .arg(&self.sync_script_sha)
            .arg(0)
            .arg("sync_measurement")
            .arg(channel.to_string())
            .arg(point.to_string())
            .arg(format!("{:.6}", value))
            .query_async(&mut self.conn)
            .await?;
        Ok(())
    }
    
    /// 发送控制命令
    pub async fn send_control(&mut self, model_id: &str, control: &str, value: f64) -> Result<(), Box<dyn std::error::Error>> {
        redis::cmd("EVALSHA")
            .arg(&self.sync_script_sha)
            .arg(0)
            .arg("send_control")
            .arg(model_id)
            .arg(control)
            .arg(format!("{:.6}", value))
            .query_async(&mut self.conn)
            .await?;
        Ok(())
    }
}
```

### 步骤3: 修改ComsRv (半天)

修改 `services/comsrv/src/protocol_handler.rs`:

```rust
use voltage_libs::edge_redis::EdgeRedis;

pub struct ProtocolHandler {
    redis: EdgeRedis,
    enable_sync: bool,  // 配置开关
}

impl ProtocolHandler {
    pub async fn handle_measurement(&mut self, channel: u32, point: u32, value: f64) -> Result<()> {
        // 1. 写入原始数据
        let key = format!("comsrv:{}:m", channel);
        self.redis.conn.hset(&key, point.to_string(), format!("{:.6}", value)).await?;
        
        // 2. 可选的同步
        if self.enable_sync {
            // 异步同步，不阻塞主流程
            match self.redis.sync_measurement(channel, point, value).await {
                Ok(_) => debug!("同步成功: {}:{}={:.6}", channel, point, value),
                Err(e) => warn!("同步失败，但不影响主流程: {}", e),
            }
        }
        
        Ok(())
    }
}
```

### 步骤4: 简化ModSrv (半天)

修改 `services/modsrv/src/api.rs`:

```rust
use voltage_libs::edge_redis::EdgeRedis;

pub struct SimplifiedModSrv {
    redis: EdgeRedis,
    models: HashMap<String, ModelConfig>,
}

impl SimplifiedModSrv {
    // GET /models/{id}/values - 直接从Redis读取
    pub async fn get_model_values(&mut self, model_id: &str) -> Result<HashMap<String, f64>> {
        let key = format!("modsrv:{}", model_id);
        let values: HashMap<String, String> = self.redis.conn.hgetall(&key).await?;
        
        Ok(values.into_iter()
            .filter_map(|(k, v)| v.parse::<f64>().ok().map(|val| (k, val)))
            .collect())
    }
    
    // POST /models/{id}/control/{name} - 通过Lua脚本发送
    pub async fn send_control(&mut self, model_id: &str, control: &str, value: f64) -> Result<()> {
        self.redis.send_control(model_id, control, value).await?;
        Ok(())
    }
    
    // WebSocket - 订阅更新通知
    pub async fn handle_websocket(&mut self, model_id: &str, ws: WebSocket) {
        let mut pubsub = self.redis.conn.clone().into_pubsub();
        pubsub.subscribe(format!("update:{}", model_id)).await.unwrap();
        
        // 简单的消息转发
        while let Some(msg) = pubsub.on_message().next().await {
            let payload: String = msg.get_payload().unwrap();
            if ws.send(Message::text(payload)).await.is_err() {
                break;
            }
        }
    }
}
```

### 步骤5: 初始化映射数据 (半天)

创建 `tools/init_edge_mappings.sh`:

```bash
#!/bin/bash
# 边端设备映射初始化脚本

REDIS_CLI="redis-cli"

echo "初始化边端设备映射数据..."

# 清空旧数据
$REDIS_CLI DEL mapping mapping:reverse

# 加载正向映射 (ComsRv -> ModSrv)
$REDIS_CLI HSET mapping "1001:10001" "power_meter:voltage"
$REDIS_CLI HSET mapping "1001:10002" "power_meter:current"
$REDIS_CLI HSET mapping "1001:10003" "power_meter:power"

# 加载反向映射 (ModSrv -> ComsRv)
$REDIS_CLI HSET mapping:reverse "power_meter:switch" "1001:30001"
$REDIS_CLI HSET mapping:reverse "power_meter:setpoint" "1001:40001"

# 设置告警阈值
$REDIS_CLI HSET alarm:threshold "power_meter:voltage" "250"
$REDIS_CLI HSET alarm:threshold "power_meter:current" "100"

echo "映射数据初始化完成!"
```

### 步骤6: 本地测试 (1天)

创建 `docker-compose.edge.yml`:

```yaml
version: '3.8'

services:
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - ./redis-data:/data
    command: redis-server --maxmemory 512mb --maxmemory-policy allkeys-lru
    
  comsrv:
    build: ./services/comsrv
    environment:
      - REDIS_URL=redis://redis:6379
      - SYNC_SCRIPT_SHA=2e5d4f3b7c6a5d4e3f2a1b0c9d8e7f6a5b4c3d2e
      - ENABLE_SYNC=true
    depends_on:
      - redis
      
  modsrv:
    build: ./services/modsrv
    ports:
      - "8002:8002"
    environment:
      - REDIS_URL=redis://redis:6379
      - SYNC_SCRIPT_SHA=2e5d4f3b7c6a5d4e3f2a1b0c9d8e7f6a5b4c3d2e
    depends_on:
      - redis
```

测试脚本 `test_edge_sync.sh`:

```bash
#!/bin/bash

echo "测试边端数据同步..."

# 1. 模拟ComsRv写入数据
redis-cli HSET comsrv:1001:m 10001 "220.5"

# 2. 手动触发同步（测试脚本）
redis-cli EVALSHA 2e5d4f3b7c6a5d4e3f2a1b0c9d8e7f6a5b4c3d2e 0 sync_measurement 1001 10001 220.5

# 3. 检查ModSrv数据
echo "检查同步结果:"
redis-cli HGET modsrv:power_meter voltage

# 4. 测试API
echo "测试ModSrv API:"
curl http://localhost:8002/models/power_meter/values
```

## 四、运维指南

### 4.1 日常维护

```bash
# 检查Redis内存使用
redis-cli INFO memory | grep used_memory_human

# 查看连接数
redis-cli CLIENT LIST | wc -l

# 监控同步脚本执行
redis-cli MONITOR | grep EVALSHA

# 查看慢日志
redis-cli SLOWLOG GET 10
```

### 4.2 修改和更新

1. **更新Lua脚本**：
```bash
# 加载新脚本
NEW_SHA=$(redis-cli SCRIPT LOAD "$(cat scripts/edge_sync_v2.lua)")

# 更新环境变量
sed -i "s/SYNC_SCRIPT_SHA=.*/SYNC_SCRIPT_SHA=$NEW_SHA/g" .env

# 重启服务
docker-compose restart comsrv modsrv
```

2. **添加新映射**：
```bash
# 直接添加
redis-cli HSET mapping "1002:10001" "transformer:voltage"
redis-cli HSET mapping:reverse "transformer:tap_position" "1002:30001"
```

### 4.3 故障处理

```bash
# 脚本执行失败时的调试
redis-cli --eval scripts/edge_sync.lua , sync_measurement 1001 10001 220.5

# 清理错误数据
redis-cli DEL alarm:queue
redis-cli FLUSHDB  # 慎用！会清空所有数据

# 重新初始化
./tools/init_edge_mappings.sh
```

## 五、性能优化建议

### 5.1 Redis配置优化
```bash
# redis.conf
maxmemory 512mb
maxmemory-policy allkeys-lru
save ""  # 边端设备通常不需要持久化
```

### 5.2 批量操作
```rust
// 批量更新示例
pub async fn batch_update(&mut self, updates: Vec<(u32, u32, f64)>) -> Result<()> {
    let mut pipe = redis::pipe();
    
    for (channel, point, value) in updates {
        pipe.hset(format!("comsrv:{}:m", channel), point.to_string(), format!("{:.6}", value));
    }
    
    pipe.query_async(&mut self.redis.conn).await?;
    Ok(())
}
```

### 5.3 监控脚本
```bash
#!/bin/bash
# monitor_edge.sh - 边端设备监控脚本

while true; do
    echo "=== $(date) ==="
    echo "内存使用: $(redis-cli INFO memory | grep used_memory_human)"
    echo "连接数: $(redis-cli CLIENT LIST | wc -l)"
    echo "每秒命令数: $(redis-cli INFO stats | grep instantaneous_ops_per_sec)"
    sleep 10
done
```

## 六、注意事项

1. **资源限制**：
   - 始终监控内存使用
   - 避免存储过多历史数据
   - 定期清理过期数据

2. **稳定性优先**：
   - 同步失败不应影响数据采集
   - 使用简单的错误处理
   - 避免复杂的级联操作

3. **现场维护**：
   - 所有配置通过环境变量
   - 提供简单的测试脚本
   - 保留手动操作入口

## 七、快速开始

```bash
# 1. 加载Lua脚本
SYNC_SHA=$(redis-cli SCRIPT LOAD "$(cat scripts/edge_sync.lua)")
echo "SYNC_SCRIPT_SHA=$SYNC_SHA" > .env

# 2. 初始化映射
./tools/init_edge_mappings.sh

# 3. 启动服务
docker-compose -f docker-compose.edge.yml up -d

# 4. 验证同步
./test_edge_sync.sh

# 5. 查看日志
docker-compose logs -f comsrv modsrv
```

## 附录：常用命令速查

```bash
# Redis操作
redis-cli HGETALL modsrv:power_meter     # 查看模型数据
redis-cli HGETALL mapping                 # 查看映射配置
redis-cli MONITOR                         # 实时监控命令

# Docker操作
docker-compose ps                         # 查看服务状态
docker-compose restart comsrv             # 重启单个服务
docker-compose logs --tail=100 comsrv     # 查看最近日志

# 测试命令
curl http://localhost:8002/models         # 列出所有模型
curl http://localhost:8002/health         # 健康检查
```