# Rulesrv API 参考文档

## 概述

Rulesrv提供RESTful API用于管理规则和查看执行状态。所有API响应均为JSON格式。

**基础URL**: `http://localhost:8083`

## 认证

当前版本暂不需要认证。生产环境建议配合API Gateway使用JWT认证。

## 通用响应格式

### 成功响应

```json
{
  "data": {
    // 响应数据
  }
}
```

### 错误响应

```json
{
  "error": {
    "code": "RULE_NOT_FOUND",
    "message": "Rule with id 'temp_alarm' not found"
  }
}
```

## API端点

### 健康检查

#### GET /health

检查服务健康状态。

**请求示例**:
```bash
curl http://localhost:8083/health
```

**响应示例**:
```json
{
  "status": "ok",
  "version": "0.1.0",
  "redis_connected": true
}
```

**响应字段**:
- `status` (string): 服务状态，"ok" 或 "error"
- `version` (string): 服务版本号
- `redis_connected` (boolean): Redis连接状态

---

### 规则管理

#### GET /api/v1/rules

获取所有规则列表。

**请求参数**:
- `enabled` (boolean, 可选): 过滤启用/禁用的规则
- `group_id` (string, 可选): 按规则组过滤
- `page` (integer, 可选): 页码，默认1
- `limit` (integer, 可选): 每页数量，默认20

**请求示例**:
```bash
curl "http://localhost:8083/api/v1/rules?enabled=true&limit=10"
```

**响应示例**:
```json
{
  "data": [
    {
      "id": "temperature_alarm",
      "name": "Temperature Alarm Rule",
      "description": "Trigger alarm when temperature exceeds threshold",
      "group_id": null,
      "condition": "temperature > 30",
      "actions": [
        {
          "type": "publish",
          "channel": "alarm:temperature:high",
          "message": "Temperature exceeded 30°C"
        }
      ],
      "enabled": true,
      "priority": 10,
      "created_at": "2025-01-15T10:30:00Z",
      "updated_at": "2025-01-15T10:30:00Z"
    }
  ],
  "total": 15,
  "page": 1,
  "limit": 10
}
```

---

#### POST /api/v1/rules

创建新规则。

**请求体**:
```json
{
  "rule": {
    "id": "pressure_alarm",
    "name": "Pressure Alarm Rule",
    "description": "Monitor pressure levels",
    "group_id": null,
    "condition": "pressure > 10",
    "actions": [
      {
        "type": "publish",
        "channel": "alarm:pressure:high",
        "message": "Pressure exceeded 10 bar"
      }
    ],
    "enabled": true,
    "priority": 20
  }
}
```

**请求示例**:
```bash
curl -X POST http://localhost:8083/api/v1/rules \
  -H "Content-Type: application/json" \
  -d '{
    "rule": {
      "id": "pressure_alarm",
      "name": "Pressure Alarm Rule",
      "condition": "pressure > 10",
      "actions": [{
        "type": "publish",
        "channel": "alarm:pressure:high",
        "message": "Pressure exceeded 10 bar"
      }],
      "enabled": true,
      "priority": 20
    }
  }'
```

**响应示例**:
```json
{
  "data": {
    "id": "pressure_alarm",
    "name": "Pressure Alarm Rule",
    "description": "Monitor pressure levels",
    "group_id": null,
    "condition": "pressure > 10",
    "actions": [
      {
        "type": "publish",
        "channel": "alarm:pressure:high",
        "message": "Pressure exceeded 10 bar"
      }
    ],
    "enabled": true,
    "priority": 20,
    "created_at": "2025-01-15T11:00:00Z",
    "updated_at": "2025-01-15T11:00:00Z"
  }
}
```

**错误响应**:
- `400 Bad Request`: 规则格式错误
- `409 Conflict`: 规则ID已存在

---

#### GET /api/v1/rules/{rule_id}

获取指定规则详情。

**路径参数**:
- `rule_id` (string): 规则ID

**请求示例**:
```bash
curl http://localhost:8083/api/v1/rules/temperature_alarm
```

**响应示例**:
```json
{
  "data": {
    "id": "temperature_alarm",
    "name": "Temperature Alarm Rule",
    "description": "Trigger alarm when temperature exceeds threshold",
    "group_id": null,
    "condition": "temperature > 30",
    "actions": [
      {
        "type": "publish",
        "channel": "alarm:temperature:high",
        "message": "Temperature exceeded 30°C"
      }
    ],
    "enabled": true,
    "priority": 10,
    "created_at": "2025-01-15T10:30:00Z",
    "updated_at": "2025-01-15T10:30:00Z",
    "last_triggered": "2025-01-15T11:45:30Z",
    "trigger_count": 5
  }
}
```

**错误响应**:
- `404 Not Found`: 规则不存在

---

#### PUT /api/v1/rules/{rule_id}

更新指定规则。

**路径参数**:
- `rule_id` (string): 规则ID

**请求体**:
```json
{
  "rule": {
    "name": "Updated Temperature Alarm",
    "condition": "temperature > 35",
    "enabled": false
  }
}
```

**请求示例**:
```bash
curl -X PUT http://localhost:8083/api/v1/rules/temperature_alarm \
  -H "Content-Type: application/json" \
  -d '{
    "rule": {
      "condition": "temperature > 35",
      "enabled": false
    }
  }'
```

**响应示例**:
```json
{
  "data": {
    "id": "temperature_alarm",
    "name": "Temperature Alarm Rule",
    "condition": "temperature > 35",
    "enabled": false,
    "updated_at": "2025-01-15T12:00:00Z"
  }
}
```

**错误响应**:
- `400 Bad Request`: 更新数据格式错误
- `404 Not Found`: 规则不存在

---

#### DELETE /api/v1/rules/{rule_id}

删除指定规则。

**路径参数**:
- `rule_id` (string): 规则ID

**请求示例**:
```bash
curl -X DELETE http://localhost:8083/api/v1/rules/temperature_alarm
```

**响应示例**:
```json
{
  "data": {
    "message": "Rule deleted successfully"
  }
}
```

**错误响应**:
- `404 Not Found`: 规则不存在

---

### 规则执行

#### POST /api/v1/rules/{rule_id}/execute

手动执行指定规则。

**路径参数**:
- `rule_id` (string): 规则ID

**请求体** (可选):
```json
{
  "context": {
    "temperature": 35,
    "pressure": 8.5,
    "timestamp": 1736981591526
  }
}
```

**请求示例**:
```bash
curl -X POST http://localhost:8083/api/v1/rules/temperature_alarm/execute \
  -H "Content-Type: application/json" \
  -d '{
    "context": {
      "temperature": 35
    }
  }'
```

**响应示例**:
```json
{
  "data": {
    "execution_id": "exec_12345",
    "rule_id": "temperature_alarm",
    "triggered": true,
    "condition_result": true,
    "actions_executed": [
      {
        "type": "publish",
        "status": "success",
        "message": "Published to channel 'alarm:temperature:high'"
      }
    ],
    "duration_ms": 15,
    "timestamp": "2025-01-15T12:15:00Z"
  }
}
```

**错误响应**:
- `404 Not Found`: 规则不存在
- `400 Bad Request`: 上下文数据格式错误

---

#### POST /api/v1/rules/test

测试规则条件（不执行动作）。

**请求体**:
```json
{
  "condition": "temperature > 30 && pressure < 10",
  "context": {
    "temperature": 35,
    "pressure": 8
  }
}
```

**请求示例**:
```bash
curl -X POST http://localhost:8083/api/v1/rules/test \
  -H "Content-Type: application/json" \
  -d '{
    "condition": "temperature > 30",
    "context": {
      "temperature": 35
    }
  }'
```

**响应示例**:
```json
{
  "data": {
    "condition": "temperature > 30",
    "result": true,
    "evaluation_details": {
      "temperature": 35,
      "operator": ">",
      "threshold": 30,
      "result": true
    }
  }
}
```

---

### 执行历史

#### GET /api/v1/rules/{rule_id}/history

获取规则执行历史。

**路径参数**:
- `rule_id` (string): 规则ID

**请求参数**:
- `start_time` (string, 可选): 开始时间，ISO 8601格式
- `end_time` (string, 可选): 结束时间，ISO 8601格式
- `limit` (integer, 可选): 返回记录数，默认20

**请求示例**:
```bash
curl "http://localhost:8083/api/v1/rules/temperature_alarm/history?limit=5"
```

**响应示例**:
```json
{
  "data": [
    {
      "execution_id": "exec_12345",
      "rule_id": "temperature_alarm",
      "triggered": true,
      "condition_result": true,
      "context": {
        "temperature": 35,
        "timestamp": 1736981591526
      },
      "actions_executed": [
        {
          "type": "publish",
          "status": "success"
        }
      ],
      "duration_ms": 15,
      "timestamp": "2025-01-15T12:15:00Z"
    }
  ],
  "total": 100,
  "limit": 5
}
```

---

### 规则组管理

#### GET /api/v1/groups

获取所有规则组。

**请求示例**:
```bash
curl http://localhost:8083/api/v1/groups
```

**响应示例**:
```json
{
  "data": [
    {
      "id": "temperature_rules",
      "name": "温度监控规则组",
      "description": "所有温度相关的监控规则",
      "rules": ["temp_high_warning", "temp_critical", "temp_low_warning"],
      "enabled": true,
      "created_at": "2025-01-15T09:00:00Z"
    }
  ]
}
```

---

#### POST /api/v1/groups

创建规则组。

**请求体**:
```json
{
  "group": {
    "id": "power_rules",
    "name": "电力监控规则组",
    "description": "电力系统监控规则",
    "rules": [],
    "enabled": true
  }
}
```

---

#### GET /api/v1/groups/{group_id}

获取指定规则组详情。

---

#### DELETE /api/v1/groups/{group_id}

删除规则组。

---

#### GET /api/v1/groups/{group_id}/rules

获取规则组中的所有规则。

---

## 数据类型

### Rule对象

```typescript
interface Rule {
  id: string;                    // 规则唯一标识
  name: string;                  // 规则名称
  description?: string;          // 规则描述
  group_id?: string;            // 所属规则组ID
  condition: string;            // 条件表达式
  actions: Action[];            // 动作列表
  enabled: boolean;             // 是否启用
  priority: number;             // 优先级(1-100)
  created_at?: string;          // 创建时间
  updated_at?: string;          // 更新时间
  last_triggered?: string;      // 最后触发时间
  trigger_count?: number;       // 触发次数
}
```

### Action对象

```typescript
interface Action {
  type: "publish" | "control" | "notification";
  // publish类型
  channel?: string;
  message?: string;
  // control类型
  channel_id?: number;
  point_type?: string;
  point_id?: number;
  value?: any;
  // notification类型
  method?: string;
  url?: string;
  template?: string;
  data?: object;
}
```

### ExecutionResult对象

```typescript
interface ExecutionResult {
  execution_id: string;         // 执行ID
  rule_id: string;             // 规则ID
  triggered: boolean;          // 是否触发
  condition_result: boolean;   // 条件评估结果
  context?: object;           // 执行上下文
  actions_executed: ActionResult[];
  duration_ms: number;        // 执行耗时(毫秒)
  timestamp: string;          // 执行时间
  error?: string;             // 错误信息
}
```

## 错误码

| 错误码 | HTTP状态码 | 说明 |
|--------|-----------|------|
| RULE_NOT_FOUND | 404 | 规则不存在 |
| RULE_ALREADY_EXISTS | 409 | 规则ID已存在 |
| INVALID_RULE_FORMAT | 400 | 规则格式无效 |
| INVALID_CONDITION | 400 | 条件表达式无效 |
| INVALID_ACTION | 400 | 动作配置无效 |
| EXECUTION_FAILED | 500 | 规则执行失败 |
| REDIS_ERROR | 503 | Redis连接错误 |
| INTERNAL_ERROR | 500 | 内部服务器错误 |

## 使用示例

### Python示例

```python
import requests
import json

# 基础URL
base_url = "http://localhost:8083"

# 创建规则
def create_rule(rule_data):
    response = requests.post(
        f"{base_url}/api/v1/rules",
        headers={"Content-Type": "application/json"},
        data=json.dumps({"rule": rule_data})
    )
    return response.json()

# 获取所有规则
def list_rules(enabled=None):
    params = {}
    if enabled is not None:
        params["enabled"] = enabled
    response = requests.get(f"{base_url}/api/v1/rules", params=params)
    return response.json()

# 执行规则
def execute_rule(rule_id, context=None):
    data = {}
    if context:
        data["context"] = context
    response = requests.post(
        f"{base_url}/api/v1/rules/{rule_id}/execute",
        headers={"Content-Type": "application/json"},
        data=json.dumps(data)
    )
    return response.json()

# 示例使用
if __name__ == "__main__":
    # 创建温度告警规则
    rule = {
        "id": "temp_alarm_test",
        "name": "温度测试告警",
        "condition": "temperature > 30",
        "actions": [{
            "type": "publish",
            "channel": "alarm:test",
            "message": "测试告警"
        }],
        "enabled": True,
        "priority": 10
    }
    
    result = create_rule(rule)
    print("创建规则:", result)
    
    # 手动执行规则
    exec_result = execute_rule("temp_alarm_test", {"temperature": 35})
    print("执行结果:", exec_result)
```

### JavaScript示例

```javascript
// 使用fetch API
const baseUrl = 'http://localhost:8083';

// 创建规则
async function createRule(ruleData) {
  const response = await fetch(`${baseUrl}/api/v1/rules`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ rule: ruleData }),
  });
  return response.json();
}

// 获取规则列表
async function listRules(enabled = null) {
  const params = new URLSearchParams();
  if (enabled !== null) {
    params.append('enabled', enabled);
  }
  const response = await fetch(`${baseUrl}/api/v1/rules?${params}`);
  return response.json();
}

// 删除规则
async function deleteRule(ruleId) {
  const response = await fetch(`${baseUrl}/api/v1/rules/${ruleId}`, {
    method: 'DELETE',
  });
  return response.json();
}

// 示例使用
(async () => {
  // 创建规则
  const rule = {
    id: 'js_test_rule',
    name: 'JavaScript测试规则',
    condition: 'value > 100',
    actions: [{
      type: 'publish',
      channel: 'test:js',
      message: 'Value exceeded 100'
    }],
    enabled: true,
    priority: 20
  };
  
  const createResult = await createRule(rule);
  console.log('Rule created:', createResult);
  
  // 获取所有启用的规则
  const rules = await listRules(true);
  console.log('Enabled rules:', rules);
})();
```

## 最佳实践

1. **规则ID命名**：使用有意义的ID，如 `{系统}_{参数}_{级别}`
2. **批量操作**：需要创建多个规则时，建议分批操作避免超时
3. **错误处理**：始终检查响应状态码和错误信息
4. **测试规则**：使用 `/api/v1/rules/test` 端点先测试条件
5. **监控执行**：定期检查规则执行历史，识别异常模式
6. **性能考虑**：避免创建过于复杂的条件表达式