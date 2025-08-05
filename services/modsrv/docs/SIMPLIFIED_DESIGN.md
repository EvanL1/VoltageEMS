# ModSrv 极简设计方案

## 核心理念

1. **极简模板系统** - 只定义数据点和操作，无冗余字段
2. **清晰的实例映射** - 模型实例包含模板引用和通道映射
3. **API驱动** - 所有模型通过API创建和管理

## 模板定义 (Template)

模板定义设备的数据结构，存储在 `templates/` 目录下的JSON文件中：

```json
{
  "id": "power_meter",
  "data": {
    "voltage": "V",        // 数据点名: 单位
    "current": "A",
    "power": "kW",
    "energy": "kWh",
    "frequency": "Hz",
    "power_factor": null   // null表示无单位
  },
  "action": {
    "reset": null,         // 无参数操作
    "set_limit": "kW"      // 有参数操作（单位）
  }
}
```

## 模型实例 (Model Instance)

模型实例是模板的具体化，包含映射信息：

```json
{
  "id": "meter_001",
  "name": "主楼电表",
  "template": "power_meter",    // 引用的模板ID
  "mapping": {
    "channel": 1001,            // 底层通道ID
    "data": {                   // 数据点映射
      "voltage": 1,
      "current": 2,
      "power": 3,
      "energy": 4,
      "frequency": 5,
      "power_factor": 6
    },
    "action": {                 // 操作映射
      "reset": 101,
      "set_limit": 102
    }
  }
}
```

## API 使用

### 1. 列出可用模板

```http
GET /templates

Response:
{
  "templates": [
    {
      "id": "power_meter",
      "data_count": 6,
      "action_count": 2
    }
  ],
  "total": 1
}
```

### 2. 从模板创建模型

```http
POST /templates/create-model
{
  "template_id": "power_meter",
  "model_id": "meter_001",
  "model_name": "主楼电表",
  "mapping": {
    "channel": 1001,
    "data": {
      "voltage": 1,
      "current": 2,
      "power": 3,
      "energy": 4,
      "frequency": 5,
      "power_factor": 6
    },
    "action": {
      "reset": 101,
      "set_limit": 102
    }
  }
}
```

### 3. 获取模型数据

```http
GET /models/meter_001/values

Response:
{
  "model_id": "meter_001",
  "values": {
    "voltage": 220.5,
    "current": 15.3,
    "power": 3.366,
    "energy": 12345.6,
    "frequency": 50.01,
    "power_factor": 0.95
  },
  "timestamp": 1643723456
}
```

### 4. 执行操作

```http
POST /models/meter_001/control/set_limit
{
  "value": 100.0
}

Response:
{
  "success": true,
  "message": "Control command sent: meter_001.set_limit = 100.000000",
  "timestamp": 1643723456
}
```

## 设计优势

1. **极简性** - 结构扁平，易于理解和维护
2. **灵活性** - 模板定义结构，实例定义映射
3. **可追溯性** - 实例保留模板引用，便于批量管理
4. **高效性** - 减少了嵌套和冗余字段

## 内置模板

- `power_meter` - 电表
- `diesel_generator` - 柴油发电机
- `transformer` - 变压器
- `energy_storage` - 储能系统

## 未来扩展

1. 支持模板继承（基于已有模板扩展）
2. 支持模板版本管理
3. 支持批量创建相同模板的多个实例
4. 支持模板校验规则