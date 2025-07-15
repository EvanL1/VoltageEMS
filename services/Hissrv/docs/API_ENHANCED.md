# HisSrv 增强 API 文档

## 概述

HisSrv 增强 API 提供了高级的历史数据查询、分析和导出功能。这些 API 端点支持复杂的查询场景，包括批量查询、流式输出、趋势分析等。

## API 端点列表

### 1. 高级查询 API

#### POST `/api/v1/history/query/advanced`

高级历史数据查询，支持复杂的过滤、聚合和分组。

**请求参数:**

```json
{
  "time_range": {
    "start_time": "2025-01-01T00:00:00Z",
    "end_time": "2025-01-02T00:00:00Z"
  },
  "filters": [
    {
      "field": "source_id",
      "operator": "eq",
      "value": "device_001"
    }
  ],
  "aggregations": [
    {
      "function": "avg",
      "field": "value",
      "window": "5m"
    }
  ],
  "group_by": ["source_id", "point_name"],
  "order_by": [
    {
      "field": "timestamp",
      "direction": "desc"
    }
  ],
  "pagination": {
    "page": 1,
    "page_size": 100
  }
}
```

**查询参数:**
- `mode`: 查询模式 (fast/balanced/accurate)
- `use_cache`: 是否使用缓存
- `cache_ttl`: 缓存TTL（秒）
- `include_query_plan`: 是否返回查询计划

### 2. 批量查询 API

#### POST `/api/v1/history/query/batch`

执行多个查询，支持并行处理。

**请求示例:**

```json
{
  "queries": [
    {
      "time_range": {...},
      "filters": [...],
      "aggregations": [...]
    },
    {
      "time_range": {...},
      "filters": [...],
      "aggregations": [...]
    }
  ],
  "parallel": true,
  "failure_strategy": "partial"
}
```

### 3. 流式查询 API

#### POST `/api/v1/history/query/stream`

使用 Server-Sent Events 流式返回数据。

**请求示例:**

```json
{
  "query": {
    "time_range": {...},
    "filters": [...]
  },
  "chunk_size": 1000,
  "chunk_delay_ms": 100,
  "include_partial_aggregations": true
}
```

### 4. 趋势分析 API

#### POST `/api/v1/history/analysis/trend`

分析数据趋势，支持异常检测和预测。

**请求示例:**

```json
{
  "source_id": "device_001",
  "point_name": "temperature",
  "time_range": {
    "start_time": "2025-01-01T00:00:00Z",
    "end_time": "2025-01-02T00:00:00Z"
  }
}
```

**查询参数:**
- `algorithm`: 趋势算法 (linear_regression/moving_average/exponential_smoothing)
- `smoothing_window`: 平滑窗口大小
- `anomaly_threshold`: 异常检测阈值
- `forecast_minutes`: 预测时间范围

### 5. 聚合分析 API

#### POST `/api/v1/history/analysis/aggregate`

执行复杂的聚合分析。

**请求示例:**

```json
{
  "filter": {
    "time_range": {...},
    "filters": [...]
  },
  "aggregations": ["sum", "avg", "min", "max", "stddev"],
  "group_by": ["source_id", "hour"],
  "include_sub_aggregations": true
}
```

### 6. 数据质量报告 API

#### GET `/api/v1/history/quality/report`

生成数据质量分析报告。

**查询参数:**
- `start_time`: 开始时间
- `end_time`: 结束时间
- `sources`: 数据源列表（可选）
- `include_details`: 是否包含详细信息

## 查询优化

### 查询模式说明

1. **Fast (快速模式)**
   - 优先使用 Redis 缓存
   - 适合实时数据查询
   - 响应时间 < 100ms

2. **Balanced (平衡模式)**
   - 自动选择最优数据源
   - 平衡性能和准确性
   - 响应时间 100ms - 1s

3. **Accurate (精确模式)**
   - 优先使用 InfluxDB
   - 确保数据完整性
   - 响应时间 > 1s

### 过滤操作符

- `eq`: 等于
- `ne`: 不等于
- `gt`: 大于
- `gte`: 大于等于
- `lt`: 小于
- `lte`: 小于等于
- `in`: 在列表中
- `not_in`: 不在列表中
- `like`: 模糊匹配（支持 % 通配符）
- `regex`: 正则表达式
- `between`: 区间查询
- `is_null`: 为空
- `is_not_null`: 不为空

### 聚合函数

- `count`: 计数
- `sum`: 求和
- `avg`: 平均值
- `min`: 最小值
- `max`: 最大值
- `median`: 中位数
- `stddev`: 标准差
- `variance`: 方差
- `first`: 第一个值
- `last`: 最后一个值
- `percentile(n)`: 百分位数

## 性能优化建议

1. **使用分页**
   - 大数据集查询必须使用分页
   - 推荐每页 100-1000 条记录

2. **合理选择时间范围**
   - 避免查询超过 30 天的数据
   - 使用聚合减少返回数据量

3. **使用查询缓存**
   - 重复查询启用缓存
   - 设置合理的 TTL

4. **批量查询优化**
   - 相似查询合并为批量查询
   - 使用并行执行提高效率

5. **索引优化**
   - 确保过滤字段已建立索引
   - 使用复合索引优化多字段查询

## 错误处理

所有 API 返回统一的错误格式：

```json
{
  "error": "错误描述",
  "code": "ERROR_CODE",
  "timestamp": "2025-01-14T12:00:00Z"
}
```

常见错误代码：
- `INVALID_TIME_RANGE`: 无效的时间范围
- `QUERY_TIMEOUT`: 查询超时
- `RATE_LIMIT_EXCEEDED`: 超过速率限制
- `INVALID_FILTER`: 无效的过滤条件
- `RESOURCE_NOT_FOUND`: 资源未找到

## 示例代码

### Python 示例

```python
import requests
import json

# 高级查询示例
url = "http://localhost:8082/api/v1/history/query/advanced"
query = {
    "time_range": {
        "start_time": "2025-01-14T00:00:00Z",
        "end_time": "2025-01-14T01:00:00Z"
    },
    "filters": [
        {
            "field": "source_id",
            "operator": "eq",
            "value": "device_001"
        }
    ],
    "aggregations": [
        {
            "function": "avg",
            "field": "value",
            "window": "1m"
        }
    ]
}

response = requests.post(
    url,
    json=query,
    params={"mode": "balanced", "use_cache": True}
)

if response.status_code == 200:
    result = response.json()
    print(f"查询成功，返回 {len(result['query_result']['data_points'])} 个数据点")
else:
    print(f"查询失败: {response.json()}")
```

### 流式查询示例

```python
import requests
import json

url = "http://localhost:8082/api/v1/history/query/stream"
query = {
    "query": {
        "time_range": {
            "start_time": "2025-01-14T00:00:00Z",
            "end_time": "2025-01-14T01:00:00Z"
        },
        "filters": [
            {
                "field": "source_id",
                "operator": "eq",
                "value": "device_001"
            }
        ]
    },
    "chunk_size": 100
}

# 使用 SSE 客户端接收流式数据
from sseclient import SSEClient

response = requests.post(url, json=query, stream=True)
client = SSEClient(response)

for event in client.events():
    chunk = json.loads(event.data)
    print(f"接收到块 {chunk['chunk_id']}, 包含 {len(chunk['data_points'])} 个数据点")
    
    if not chunk['has_more']:
        break
```