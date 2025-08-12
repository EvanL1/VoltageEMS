# VoltageEMS 数据模型定义

## 概述

本文档定义了VoltageEMS系统中所有的数据结构，包括Redis存储格式、API传输格式和数据类型定义。

## 核心概念

### 数据类型分类 (四遥)

| 类型 | 标识 | 中文名 | 说明 | 数据特点 |
|------|------|--------|------|----------|
| Telemetry | T | 遥测 | 模拟量数据 | 连续变化的数值，如温度、压力、流量 |
| Signal | S | 遥信 | 开关量数据 | 离散状态，如开/关、运行/停止 |
| Control | C | 遥控 | 控制命令 | 下发的控制指令，如启动、停止 |
| Adjustment | A | 遥调 | 调节设定 | 参数设定值，如温度设定、压力设定 |

## Redis 数据结构

### 1. 实时数据存储

#### Key模式
```
comsrv:{channel_id}:{data_type}
```

#### 存储结构 (Hash)

```redis
# 遥测数据 (T)
comsrv:1001:T = {
  "1": '{"value":25.6,"quality":"good","timestamp":"2025-08-12T10:30:00Z"}',
  "2": '{"value":30.2,"quality":"good","timestamp":"2025-08-12T10:30:00Z"}',
  "3": '{"value":101.3,"quality":"good","timestamp":"2025-08-12T10:30:00Z"}'
}

# 遥信数据 (S)
comsrv:1001:S = {
  "10": '{"value":1,"quality":"good","timestamp":"2025-08-12T10:30:00Z","desc":"运行"}',
  "11": '{"value":0,"quality":"good","timestamp":"2025-08-12T10:30:00Z","desc":"停止"}',
  "12": '{"value":1,"quality":"good","timestamp":"2025-08-12T10:30:00Z","desc":"正常"}'
}

# 遥控数据 (C)
comsrv:2001:C = {
  "20": '{"value":1,"timestamp":"2025-08-12T10:30:00Z","operator":"user_001","status":"executed"}',
  "21": '{"value":0,"timestamp":"2025-08-12T10:30:00Z","operator":"system","status":"pending"}'
}

# 遥调数据 (A)
comsrv:2001:A = {
  "30": '{"value":50.0,"timestamp":"2025-08-12T10:30:00Z","operator":"user_001"}',
  "31": '{"value":75.5,"timestamp":"2025-08-12T10:30:00Z","operator":"user_002"}'
}
```

### 2. 设备状态存储

```redis
# 设备状态
device:{device_id}:status = {
  "status": "online",
  "last_update": "2025-08-12T10:30:00Z",
  "ip_address": "192.168.1.100",
  "connected_at": "2025-08-12T08:00:00Z",
  "error_count": 0,
  "channels": "[1001,1002,1003]"
}

# 设备配置
device:{device_id}:config = {
  "name": "1号PLC",
  "type": "plc",
  "protocol": "modbus_tcp",
  "area_id": "north",
  "timeout": 3000,
  "retry_count": 3
}
```

### 3. 通道元数据

```redis
# 通道信息
channel:{channel_id}:meta = {
  "name": "温度传感器组",
  "device_id": "PLC_001",
  "data_type": "T",
  "point_count": 10,
  "enabled": true,
  "sample_rate": 1000
}

# 点位定义
channel:{channel_id}:points = {
  "1": '{"name":"温度1","unit":"°C","scale":0.1,"offset":0,"min":0,"max":100}',
  "2": '{"name":"温度2","unit":"°C","scale":0.1,"offset":0,"min":0,"max":100}',
  "3": '{"name":"压力1","unit":"kPa","scale":1,"offset":0,"min":0,"max":200}'
}
```

### 4. 告警数据

```redis
# 活动告警集合
alarms:active = Set["ALM_12345", "ALM_12346", "ALM_12347"]

# 告警详情
alarm:{alarm_id} = {
  "channel_id": "1001",
  "point_id": 1,
  "device_id": "PLC_001",
  "severity": "high",
  "status": "active",
  "message": "温度超过上限",
  "value": 95.5,
  "threshold": 90.0,
  "triggered_at": "2025-08-12T10:25:00Z",
  "rule_id": "RULE_001"
}
```

## API 数据模型

### 1. 点位数据 (Point)

```typescript
interface PointData {
  point_id: number;           // 点位ID
  name?: string;              // 点位名称
  value: number | boolean;    // 数据值
  unit?: string;              // 单位
  quality: DataQuality;       // 数据质量
  timestamp: string;          // ISO 8601时间戳
}

enum DataQuality {
  GOOD = "good",           // 数据正常
  BAD = "bad",            // 数据异常
  UNCERTAIN = "uncertain", // 数据不确定
  OFFLINE = "offline"     // 设备离线
}
```

### 2. 通道数据 (Channel)

```typescript
interface ChannelData {
  channel_id: string;         // 通道ID
  name: string;              // 通道名称
  device_id: string;         // 所属设备ID
  data_type: DataType;       // 数据类型
  values: PointData[];       // 点位数据数组
  last_update: string;       // 最后更新时间
  status: ChannelStatus;     // 通道状态
}

enum DataType {
  TELEMETRY = "T",    // 遥测
  SIGNAL = "S",       // 遥信
  CONTROL = "C",      // 遥控
  ADJUSTMENT = "A"    // 遥调
}

enum ChannelStatus {
  NORMAL = "normal",
  WARNING = "warning",
  ERROR = "error",
  OFFLINE = "offline"
}
```

### 3. 设备数据 (Device)

```typescript
interface Device {
  device_id: string;          // 设备ID
  name: string;              // 设备名称
  type: DeviceType;          // 设备类型
  status: DeviceStatus;      // 设备状态
  area_id: string;           // 区域ID
  protocol: ProtocolType;    // 通信协议
  address: string;           // 设备地址
  channels: string[];        // 通道ID列表
  configuration: DeviceConfig; // 设备配置
  statistics: DeviceStats;   // 统计信息
  created_at: string;        // 创建时间
  updated_at: string;        // 更新时间
}

enum DeviceType {
  PLC = "plc",
  RTU = "rtu",
  GATEWAY = "gateway",
  SENSOR = "sensor",
  METER = "meter"
}

enum DeviceStatus {
  ONLINE = "online",
  OFFLINE = "offline",
  MAINTENANCE = "maintenance",
  ERROR = "error"
}

enum ProtocolType {
  MODBUS_TCP = "modbus_tcp",
  MODBUS_RTU = "modbus_rtu",
  OPCUA = "opcua",
  MQTT = "mqtt",
  VIRTUAL = "virtual"
}

interface DeviceConfig {
  timeout: number;           // 超时时间(ms)
  retry_count: number;       // 重试次数
  poll_interval: number;     // 轮询间隔(ms)
  slave_id?: number;         // Modbus从站ID
  port?: number;            // 端口号
  baudrate?: number;        // 波特率
}

interface DeviceStats {
  uptime: string;           // 运行时间
  last_error?: string;      // 最后错误
  total_points: number;     // 总点位数
  active_alarms: number;    // 活动告警数
  data_quality: number;     // 数据质量(%)
  message_count: number;    // 消息计数
}
```

### 4. 告警数据 (Alarm)

```typescript
interface Alarm {
  alarm_id: string;          // 告警ID
  channel_id: string;        // 通道ID
  point_id: number;          // 点位ID
  device_id: string;         // 设备ID
  device_name: string;       // 设备名称
  point_name: string;        // 点位名称
  severity: AlarmSeverity;   // 严重级别
  status: AlarmStatus;       // 告警状态
  message: string;           // 告警消息
  description?: string;      // 详细描述
  value: number;            // 当前值
  threshold: number;         // 阈值
  rule_id: string;          // 规则ID
  triggered_at: string;      // 触发时间
  duration?: string;         // 持续时间
  acknowledged_at?: string;  // 确认时间
  acknowledged_by?: string;  // 确认人
  cleared_at?: string;       // 清除时间
  cleared_by?: string;       // 清除人
  history: AlarmEvent[];     // 历史事件
  recommended_actions?: string[]; // 建议操作
}

enum AlarmSeverity {
  CRITICAL = "critical",  // 紧急
  HIGH = "high",         // 高
  MEDIUM = "medium",     // 中
  LOW = "low"           // 低
}

enum AlarmStatus {
  ACTIVE = "active",           // 活动
  ACKNOWLEDGED = "acknowledged", // 已确认
  CLEARED = "cleared",         // 已清除
  SUPPRESSED = "suppressed"    // 已抑制
}

interface AlarmEvent {
  timestamp: string;
  event: AlarmEventType;
  value?: number;
  from?: string;
  to?: string;
  operator?: string;
  comment?: string;
}

enum AlarmEventType {
  TRIGGERED = "triggered",
  ACKNOWLEDGED = "acknowledged",
  ESCALATED = "escalated",
  CLEARED = "cleared",
  SUPPRESSED = "suppressed"
}
```

### 5. 控制命令 (Control Command)

```typescript
interface ControlCommand {
  command_id: string;        // 命令ID
  channel_id: string;        // 通道ID
  point_id: number;         // 点位ID
  command_type: CommandType; // 命令类型
  value: number | boolean;   // 设定值
  safety_check: boolean;     // 安全检查
  operator: string;         // 操作员
  reason?: string;          // 操作原因
  expire_time?: string;      // 过期时间
  created_at: string;        // 创建时间
  executed_at?: string;      // 执行时间
  status: CommandStatus;     // 命令状态
  result?: CommandResult;    // 执行结果
}

enum CommandType {
  SET_VALUE = "set_value",     // 设定值
  START = "start",            // 启动
  STOP = "stop",             // 停止
  RESET = "reset",           // 复位
  EMERGENCY_STOP = "e_stop"   // 紧急停止
}

enum CommandStatus {
  PENDING = "pending",       // 待执行
  EXECUTING = "executing",   // 执行中
  EXECUTED = "executed",     // 已执行
  FAILED = "failed",        // 失败
  EXPIRED = "expired",      // 已过期
  CANCELLED = "cancelled"   // 已取消
}

interface CommandResult {
  success: boolean;
  actual_value?: number | boolean;
  execution_time: number;    // 执行耗时(ms)
  error_message?: string;
  error_code?: string;
}
```

### 6. 历史数据 (Historical Data)

```typescript
interface HistoricalData {
  channel_id: string;
  point_id: number;
  start_time: string;
  end_time: string;
  interval?: string;         // 数据间隔: 1m, 5m, 1h, 1d
  aggregation?: AggregationType;
  data: TimeSeriesData[];
  statistics?: DataStatistics;
}

enum AggregationType {
  RAW = "raw",         // 原始数据
  AVG = "avg",         // 平均值
  MAX = "max",         // 最大值
  MIN = "min",         // 最小值
  SUM = "sum",         // 总和
  COUNT = "count",     // 计数
  FIRST = "first",     // 第一个值
  LAST = "last"        // 最后一个值
}

interface TimeSeriesData {
  timestamp: string;
  value: number;
  quality?: DataQuality;
  annotation?: string;      // 数据标注
}

interface DataStatistics {
  count: number;           // 数据点数
  avg: number;            // 平均值
  max: number;            // 最大值
  min: number;            // 最小值
  std: number;            // 标准差
  sum: number;            // 总和
  missing: number;        // 缺失数
  quality_good: number;   // 良好数据百分比
}
```

## WebSocket 消息格式

### 1. 基础消息结构

```typescript
interface WebSocketMessage {
  type: string;              // 消息类型
  id: string;               // 消息ID
  timestamp: string;         // 时间戳
  data: any;                // 消息数据
  meta?: MessageMeta;        // 元数据
}

interface MessageMeta {
  version: string;          // 协议版本
  source: string;          // 消息来源
  compression?: string;     // 压缩方式
  encoding?: string;       // 编码方式
}
```

### 2. 数据订阅消息

```typescript
interface SubscribeMessage {
  type: "subscribe";
  data: {
    channels: ChannelSubscription[];
    mode?: SubscriptionMode;
  };
}

interface ChannelSubscription {
  channel_id: string;
  data_types: DataType[];    // ["T", "S", "C", "A"]
  interval?: number;         // 推送间隔(ms)
  mode?: UpdateMode;         // 更新模式
  filters?: DataFilter;      // 数据过滤器
}

enum SubscriptionMode {
  CHANNEL = "channel",      // 按通道订阅
  DEVICE = "device",       // 按设备订阅
  AREA = "area",          // 按区域订阅
  ALARM = "alarm"         // 订阅告警
}

enum UpdateMode {
  VALUE = "value",        // 全量值更新
  DELTA = "delta",       // 增量更新
  CHANGE = "change"      // 仅变化时更新
}

interface DataFilter {
  quality?: DataQuality[];   // 质量过滤
  value_range?: {            // 值范围过滤
    min: number;
    max: number;
  };
  point_ids?: number[];      // 点位过滤
}
```

### 3. 数据推送消息

```typescript
interface DataUpdateMessage {
  type: "data_update";
  data: {
    channel_id: string;
    data_type: DataType;
    values: PointData[];
    update_type: "full" | "partial";
    sequence?: number;       // 序列号，用于检测丢包
  };
}

interface DeltaUpdateMessage {
  type: "delta_update";
  data: {
    channel_id: string;
    changes: DataChange[];
  };
}

interface DataChange {
  point_id: number;
  field: string;          // 变化的字段
  old_value: any;         // 旧值
  new_value: any;         // 新值
  timestamp: string;      // 变化时间
}
```

### 4. 批量数据消息

```typescript
interface BatchDataMessage {
  type: "data_batch";
  data: {
    updates: ChannelUpdate[];
    total_points: number;
    compression?: "none" | "gzip" | "lz4";
    sequence?: number;
  };
}

interface ChannelUpdate {
  channel_id: string;
  data_type: DataType;
  values: PointData[];
}
```

## InfluxDB 数据结构

### 1. 测量(Measurement)设计

```sql
-- 遥测数据
measurement: telemetry
tags:
  - channel_id: string
  - device_id: string
  - area_id: string
  - point_id: string
  - point_name: string
fields:
  - value: float
  - quality: string
time: timestamp

-- 遥信数据
measurement: signal
tags:
  - channel_id: string
  - device_id: string
  - area_id: string
  - point_id: string
  - point_name: string
fields:
  - value: integer (0/1)
  - state: string
  - quality: string
time: timestamp

-- 控制记录
measurement: control
tags:
  - channel_id: string
  - device_id: string
  - point_id: string
  - operator: string
  - command_type: string
fields:
  - value: float
  - success: boolean
  - execution_time: integer
time: timestamp

-- 告警记录
measurement: alarm
tags:
  - alarm_id: string
  - device_id: string
  - channel_id: string
  - severity: string
  - status: string
fields:
  - value: float
  - threshold: float
  - message: string
time: timestamp
```

### 2. 查询示例

```sql
-- 查询最近1小时的温度数据
SELECT mean("value") AS avg_temp
FROM "telemetry"
WHERE "channel_id" = '1001'
  AND "point_name" = '温度1'
  AND time >= now() - 1h
GROUP BY time(5m)

-- 查询设备在线率
SELECT count("value") AS online_count
FROM "signal"
WHERE "device_id" = 'PLC_001'
  AND "point_name" = 'online_status'
  AND "value" = 1
  AND time >= now() - 24h
GROUP BY time(1h)

-- 统计告警频率
SELECT count("alarm_id") AS alarm_count
FROM "alarm"
WHERE "severity" IN ('high', 'critical')
  AND time >= now() - 7d
GROUP BY time(1d), "device_id"
```

## 配置文件数据结构

### 1. CSV点表结构

#### telemetry.csv (遥测点表)
```csv
point_id,signal_name,scale,offset,unit,reverse,data_type,min_value,max_value,description
1,温度1,0.1,0,°C,false,float32,0,100,1号温度传感器
2,温度2,0.1,0,°C,false,float32,0,100,2号温度传感器
3,压力1,1,0,kPa,false,float32,0,200,1号压力传感器
```

#### signal.csv (遥信点表)
```csv
point_id,signal_name,data_type,state_0,state_1,description
10,运行状态,bool,停止,运行,设备运行状态
11,故障状态,bool,正常,故障,设备故障状态
12,门状态,bool,关闭,开启,柜门开关状态
```

#### control.csv (遥控点表)
```csv
point_id,signal_name,data_type,control_0,control_1,pulse_width,description
20,启动/停止,bool,停止,启动,500,设备启停控制
21,复位,bool,正常,复位,1000,故障复位
```

#### adjustment.csv (遥调点表)
```csv
point_id,signal_name,scale,offset,unit,data_type,min_value,max_value,description
30,温度设定,0.1,0,°C,float32,0,100,温度设定值
31,压力设定,1,0,kPa,float32,0,200,压力设定值
```

### 2. Modbus映射表结构

```csv
point_id,slave_id,function_code,register_address,data_type,byte_order,bit_position
1,1,3,100,float32,ABCD,-1
2,1,3,102,float32,ABCD,-1
10,1,1,0,bool,none,0
20,1,5,0,bool,none,0
30,1,6,200,float32,ABCD,-1
```

## 数据流转示例

### 1. 实时数据流

```
设备 -> Modbus -> comsrv -> Redis -> WebSocket -> 前端
                     ↓
                  InfluxDB (历史存储)
```

### 2. 数据处理流程

```javascript
// 1. 原始数据采集
raw_value = modbus_read(register_address)

// 2. 数据转换
actual_value = raw_value * scale + offset

// 3. 质量判断
if (actual_value < min_value || actual_value > max_value) {
  quality = "bad"
} else if (device.status === "offline") {
  quality = "offline"
} else {
  quality = "good"
}

// 4. 存储到Redis
redis.hset(
  `comsrv:${channel_id}:T`,
  point_id,
  JSON.stringify({
    value: actual_value,
    quality: quality,
    timestamp: new Date().toISOString()
  })
)

// 5. 推送到WebSocket
websocket.send({
  type: "data_update",
  data: {
    channel_id: channel_id,
    data_type: "T",
    values: [{
      point_id: point_id,
      value: actual_value,
      quality: quality,
      timestamp: timestamp
    }]
  }
})

// 6. 存储到InfluxDB
influxdb.write({
  measurement: "telemetry",
  tags: {
    channel_id: channel_id,
    device_id: device_id,
    point_id: point_id
  },
  fields: {
    value: actual_value,
    quality: quality
  },
  timestamp: timestamp
})
```

## 性能优化建议

### 1. Redis优化

- 使用Pipeline批量操作
- 设置合理的TTL避免内存溢出
- 使用Hash结构减少内存占用
- 启用持久化保证数据安全

### 2. 数据压缩

- WebSocket消息使用MessagePack或Protocol Buffers
- 批量数据使用LZ4压缩
- 历史数据查询结果使用Gzip压缩

### 3. 缓存策略

- 热点数据缓存在Redis
- 静态配置缓存在内存
- 查询结果缓存，设置合理TTL

### 4. 批量处理

- 批量读取Modbus寄存器
- 批量写入Redis
- 批量推送WebSocket消息
- 批量写入InfluxDB

## 数据一致性保证

### 1. 事务处理

```rust
// Redis事务
redis.multi()
  .hset(channel_key, point_id, value)
  .zadd(timeseries_key, timestamp, value)
  .exec()
```

### 2. 消息确认

```javascript
// WebSocket消息确认机制
client.send(message)
await client.waitForAck(message.id, timeout)
```

### 3. 数据同步

```rust
// 定期同步Redis和InfluxDB
async fn sync_data() {
    let redis_data = redis.get_all().await?;
    let influx_data = influxdb.query_latest().await?;

    // 比较并同步差异
    for diff in compare_data(redis_data, influx_data) {
        influxdb.write(diff).await?;
    }
}
```
