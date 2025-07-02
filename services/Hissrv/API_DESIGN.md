# HisSrv API 设计文档

## 🎯 设计原则

HisSrv 作为**历史数据服务**，其 API 设计专注于：
- **历史数据查询**：而非实时数据获取
- **数据分析**：统计、聚合、趋势分析
- **数据导出**：批量数据导出功能
- **数据源管理**：历史数据源的信息管理

与 ComsRv 的区别：
- **ComsRv**: 专注实时通信、设备控制、当前状态
- **HisSrv**: 专注历史存储、趋势分析、数据挖掘

## 📡 API 端点设计

### 1. 历史数据查询 `/history/*`

#### `GET /api/v1/history/query`
查询历史数据点

**参数:**
```yaml
source_id: string          # 数据源/设备ID
point_name: string         # 数据点名称
start_time: datetime       # 开始时间 (必需)
end_time: datetime         # 结束时间 (必需)
aggregation: string        # 聚合类型: raw, avg, min, max, count
interval: string           # 聚合间隔: 1m, 5m, 1h, 1d
limit: integer            # 限制数量
offset: integer           # 偏移量
order: string             # 排序: asc, desc
```

**响应:**
```json
{
  "success": true,
  "data": {
    "query_summary": {
      "time_range": {...},
      "source_count": 5,
      "point_count": 1000,
      "execution_time_ms": 150
    },
    "data_points": [...],
    "aggregated_data": [...],
    "pagination": {...}
  }
}
```

#### `GET /api/v1/history/sources`
获取历史数据源列表

#### `GET /api/v1/history/sources/{source_id}`
获取特定数据源详情

### 2. 数据分析 `/history/statistics`

#### `GET /api/v1/history/statistics`
获取时间序列统计信息

**参数:**
```yaml
start_time: datetime       # 统计开始时间
end_time: datetime         # 统计结束时间
granularity: string        # 统计粒度: hour, day, week, month
sources: array            # 数据源过滤
```

### 3. 数据导出 `/history/export/*`

#### `POST /api/v1/history/export`
创建数据导出任务

**请求体:**
```json
{
  "query": {...},           # 查询条件
  "format": "csv",          # 导出格式: csv, json, parquet
  "options": {
    "include_header": true,
    "time_format": "ISO8601",
    "compression": "gzip"
  }
}
```

#### `GET /api/v1/history/export/{job_id}`
获取导出任务状态

### 4. 管理接口 `/admin/*`

#### `GET /api/v1/admin/storage-stats`
获取存储后端统计信息

#### `GET /api/v1/admin/config`
获取服务配置信息

### 5. 健康检查 `/health`

#### `GET /api/v1/health`
服务健康检查

## 🔄 数据流设计

### 数据写入流程
```
其他服务 → Redis Pub/Sub → HisSrv → 存储后端
  ↓                                      ↓
ComsRv               →              InfluxDB/PostgreSQL
ModSrv                              
AlarmSrv                           
```

### 数据查询流程
```
客户端 → HisSrv API → 存储后端 → 数据处理 → 响应
  ↓                      ↓            ↓
Web UI              InfluxDB      聚合计算
Dashboard           PostgreSQL    格式转换
报表系统             Redis        分页处理
```

## 📊 数据模型

### 历史数据点结构
```json
{
  "timestamp": "2024-01-01T12:00:00Z",
  "source_id": "device_001",
  "point_name": "temperature",
  "value": 25.5,
  "quality": "good",
  "tags": {
    "location": "room_a",
    "unit": "celsius"
  }
}
```

### 聚合数据结构
```json
{
  "window_start": "2024-01-01T12:00:00Z",
  "window_end": "2024-01-01T13:00:00Z",
  "value": 25.3,
  "aggregation_type": "avg",
  "sample_count": 60
}
```

## 🔍 查询优化

### 时间范围限制
- 单次查询最大范围: 365天
- 推荐查询范围: ≤ 30天
- 实时数据查询: 使用 ComsRv API

### 聚合查询建议
- 大时间范围 (> 7天): 建议使用聚合查询
- 聚合间隔选择:
  - 1小时内: raw 数据
  - 1天内: 1分钟聚合
  - 1周内: 5分钟聚合
  - 1月内: 1小时聚合
  - 1年内: 1天聚合

### 分页策略
- 默认限制: 1000条/页
- 最大限制: 10000条/页
- 大数据集: 使用导出功能

## 🚀 性能优化

### 缓存策略
- 查询结果缓存: 热点数据 15分钟
- 聚合结果缓存: 1小时
- 统计信息缓存: 1天

### 索引优化
- 时间索引: 主要查询维度
- 数据源索引: 按设备/源查询
- 复合索引: (source_id, timestamp)

### 异步处理
- 大查询: 异步执行，返回任务ID
- 导出任务: 后台处理，支持进度查询
- 统计计算: 定时预计算常用统计

## 🔐 安全考虑

### 访问控制
- 读权限: 历史数据查询
- 导出权限: 批量数据导出
- 管理权限: 配置和统计查看

### 限流策略
- 查询频率: 100次/分钟/用户
- 导出频率: 5次/小时/用户
- 大查询限制: 并发数控制

## 📈 监控指标

### API 指标
- 查询响应时间分布
- 查询数据量分布
- 错误率统计
- 热点查询识别

### 业务指标
- 数据源活跃度
- 存储增长趋势
- 查询模式分析
- 导出任务成功率

## 🔧 集成示例

### 前端 Dashboard 集成
```javascript
// 查询最近24小时的温度数据
const response = await fetch('/api/v1/history/query', {
  method: 'GET',
  params: {
    source_id: 'sensor_01',
    point_name: 'temperature',
    start_time: '2024-01-01T00:00:00Z',
    end_time: '2024-01-02T00:00:00Z',
    aggregation: 'avg',
    interval: '1h'
  }
});
```

### 报表系统集成
```python
# 创建数据导出任务
export_request = {
    "query": {
        "start_time": "2024-01-01T00:00:00Z",
        "end_time": "2024-01-31T23:59:59Z",
        "source_id": "plant_01"
    },
    "format": "csv",
    "options": {
        "include_header": True,
        "compression": "gzip"
    }
}

response = requests.post('/api/v1/history/export', json=export_request)
job_id = response.json()['data']['job_id']
```

## 🎯 API 版本规划

### v1.0 (当前)
- 基础历史数据查询
- 简单聚合功能
- CSV 导出

### v1.1 (计划)
- 高级聚合函数
- 多格式导出 (JSON, Parquet)
- 查询模板功能

### v2.0 (远期)
- 实时订阅历史数据变更
- 自定义计算引擎
- ML 预测接口