# Grafana 数据显示问题修复方案

## 问题总结
1. InfluxDB 版本不匹配（配置是 1.x，实际运行 2.x）
2. Hissrv 服务的 InfluxDB 连接配置错误
3. Grafana 数据源配置需要更新
4. Dashboard 查询语句需要适配

## 修复步骤

### 1. 启动服务
```bash
# 启动 Grafana 和 InfluxDB
docker-compose -f docker-compose.grafana.yml up -d

# 检查服务状态
docker-compose -f docker-compose.grafana.yml ps
```

### 2. 更新 hissrv 配置
创建新的配置文件 `hissrv-influxdb2.yaml`：

```yaml
# HisSrv Configuration for InfluxDB 2.x
service:
  name: "hissrv"
  version: "0.2.0"
  port: 8080
  host: "0.0.0.0"

redis:
  connection:
    host: "127.0.0.1"
    port: 6379
    database: 0
    pool_size: 10
    timeout: 5
  
  subscription:
    channels:
      - "data:*"
      - "events:*"
    key_patterns:
      - "*"

storage:
  default: "influxdb2"
  
  backends:
    influxdb2:
      enabled: true
      url: "http://localhost:8086"
      token: "voltage-super-secret-auth-token"
      org: "voltageems"
      bucket: "history"
      batch_size: 1000
      flush_interval: 10

data:
  filters:
    default_policy: "store"
    rules:
      - pattern: "data:*"
        action: "store"
        storage: "influxdb2"

api:
  enabled: true
  prefix: "/api/v1"
  cors:
    enabled: true
    origins: ["*"]

logging:
  level: "debug"
  format: "json"
```

### 3. 创建模拟数据脚本
创建 `mock-data-generator.js` 来生成测试数据：

```javascript
const redis = require('redis');
const client = redis.createClient();

// 连接 Redis
client.on('connect', () => {
  console.log('Connected to Redis');
  generateMockData();
});

// 生成模拟数据
function generateMockData() {
  setInterval(() => {
    // 温度数据
    const temp1 = 20 + Math.random() * 10;
    const temp2 = 25 + Math.random() * 8;
    
    // 电压数据
    const voltage = 220 + Math.random() * 10;
    const current = 10 + Math.random() * 5;
    
    // 发布到 Redis
    client.publish('data:temperature', JSON.stringify({
      device_id: 'sensor_01',
      value: temp1,
      timestamp: new Date().toISOString()
    }));
    
    client.publish('data:voltage', JSON.stringify({
      device_id: 'meter_01',
      voltage: voltage,
      current: current,
      power: voltage * current,
      timestamp: new Date().toISOString()
    }));
    
    console.log(`Published: temp=${temp1.toFixed(2)}, voltage=${voltage.toFixed(2)}`);
  }, 1000);
}

client.connect();
```

### 4. 更新 Grafana Dashboard
创建适配 InfluxDB 2.x 的查询：

```flux
from(bucket: "history")
  |> range(start: -30m)
  |> filter(fn: (r) => r["_measurement"] == "temperature")
  |> filter(fn: (r) => r["device_id"] == "sensor_01")
  |> aggregateWindow(every: 10s, fn: mean)
```

### 5. 前端集成优化
更新 `GrafanaEmbedded.vue` 中的 dashboard UID：

```javascript
const dashboards = ref([
  { uid: 'voltage-ems-overview', title: 'EMS 总览' },
  { uid: 'temperature-monitor', title: '温度监控' },
  { uid: 'power-analysis', title: '电力分析' }
])
```

### 6. 验证步骤

1. 检查 InfluxDB 数据：
```bash
# 进入 InfluxDB 容器
docker exec -it voltage-influxdb influx

# 查询数据
from(bucket: "history") |> range(start: -1h) |> limit(n: 10)
```

2. 访问 Grafana：
- URL: http://localhost:3000
- 用户名: admin
- 密码: admin

3. 检查数据源连接状态

4. 在前端访问嵌入的 Grafana 视图

## 快速启动命令
```bash
# 1. 启动基础服务
docker-compose -f docker-compose.grafana.yml up -d

# 2. 启动 Redis
redis-server

# 3. 启动模拟数据生成器
node mock-data-generator.js

# 4. 启动 hissrv（使用新配置）
cd services/Hissrv
cargo run -- --config hissrv-influxdb2.yaml

# 5. 启动前端
cd frontend
npm run serve
```

## 故障排查

1. **无数据显示**
   - 检查 InfluxDB 是否收到数据
   - 验证 Grafana 数据源测试是否通过
   - 查看 hissrv 日志确认数据写入

2. **连接错误**
   - 确保所有服务都在运行
   - 检查防火墙和端口设置
   - 验证 Docker 网络配置

3. **认证问题**
   - 确认 InfluxDB token 正确
   - 检查 CORS 设置
   - 验证 Grafana 匿名访问配置