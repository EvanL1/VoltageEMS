# ModSrv 简化模型设计

## 设计理念

1. **模板（Template）**：定义设备类型的数据结构
2. **模型（Model）**：模板的实例，包含具体的映射信息

## 模板定义

模板文件存放在 `templates/` 目录，定义设备的数据点和操作：

```json
{
  "id": "power_meter",
  "data": {
    "voltage": "V",      // key: 数据点名称, value: 单位(null表示无单位)
    "current": "A",
    "power": "kW",
    "energy": "kWh",
    "power_factor": null
  },
  "action": {
    "reset": null,              // 无参数操作
    "clear": null,
    "set_limit": "kW",         // 有单位的操作（设置功率限制）
    "set_temperature": "°C"     // 有单位的操作（设置温度）
  }
}
```

## 模型实例

从模板创建的模型实例，添加了映射信息：

```json
{
  "id": "meter_001",
  "template": "power_meter",
  "name": "主楼电表",
  "mapping": {
    "channel": 1001,
    "data": {
      "voltage": 1,
      "current": 2,
      "power": 3,
      "energy": 4,
      "power_factor": 5
    },
    "action": {
      "reset": 101,
      "clear": 102
    }
  }
}
```

## 关键设计点

1. **简化结构**：
   - 去掉了复杂的属性定义
   - 单位直接作为值，null表示无单位
   - 不再区分telemetry/command等类型

2. **模板引用**：
   - 模型实例保留 `template` 字段记录来源
   - 可以追溯模型是从哪个模板创建的
   - 便于批量更新同类型设备

3. **映射关系**：
   - `channel`: 底层通道ID
   - `data`/`action`: 点位映射（点名 -> point_id）

## API 使用示例

### 创建模型从模板

```http
POST /api/models/from-template
{
  "template_id": "power_meter",
  "model_id": "meter_001",
  "name": "主楼电表",
  "mapping": {
    "channel": 1001,
    "data": {
      "voltage": 1,
      "current": 2,
      "power": 3,
      "energy": 4,
      "power_factor": 5
    },
    "action": {
      "reset": 101,
      "clear": 102
    }
  }
}
```

### 获取模型数据

```http
GET /api/models/meter_001/data

Response:
{
  "voltage": 220.5,
  "current": 15.3,
  "power": 3.366,
  "energy": 12345.6,
  "power_factor": 0.95
}
```

### 执行操作

```http
POST /api/models/meter_001/action/reset
{
  "confirm": true
}
```

## 优势

1. **简洁性**：结构扁平，易于理解
2. **灵活性**：模板定义结构，实例定义映射
3. **可维护性**：模板更新可追踪到所有实例
4. **高效性**：减少了嵌套和冗余字段