# Alarmsrv 数据结构分析

## 概述

Alarmsrv（告警服务）是 VoltageEMS 系统中负责告警管理的核心服务。本文档详细分析了该服务中使用的所有数据结构，包括其设计目的、字段含义和相互关系。

## 核心数据模型

### 1. 告警基础结构

#### AlarmLevel（告警级别枚举）
```rust
pub enum AlarmLevel {
    Critical,  // 严重
    Major,     // 重要
    Minor,     // 次要
    Warning,   // 警告
    Info,      // 信息
}
```
- **用途**：定义告警的严重程度
- **设计理念**：从高到低的5级告警体系，符合工业标准

#### AlarmStatus（告警状态枚举）
```rust
pub enum AlarmStatus {
    New,           // 新告警
    Acknowledged,  // 已确认
    Resolved,      // 已解决
}
```
- **用途**：跟踪告警的处理状态
- **状态流转**：New → Acknowledged → Resolved

#### Alarm（告警主体）
```rust
pub struct Alarm {
    pub id: Uuid,                              // 唯一标识符
    pub title: String,                         // 告警标题
    pub description: String,                   // 详细描述
    pub level: AlarmLevel,                     // 告警级别
    pub status: AlarmStatus,                   // 当前状态
    pub classification: AlarmClassification,   // 分类信息
    pub created_at: DateTime<Utc>,            // 创建时间
    pub updated_at: DateTime<Utc>,            // 更新时间
    pub acknowledged_at: Option<DateTime<Utc>>, // 确认时间
    pub acknowledged_by: Option<String>,       // 确认人
    pub resolved_at: Option<DateTime<Utc>>,    // 解决时间
    pub resolved_by: Option<String>,           // 解决人
}
```

**关键方法**：
- `new()`: 创建新告警
- `acknowledge()`: 确认告警
- `resolve()`: 解决告警
- `escalate()`: 升级告警级别
- `is_active()`: 检查是否为活跃告警

### 2. 分类系统

#### AlarmClassification（告警分类）
```rust
pub struct AlarmClassification {
    pub category: String,      // 分类名称
    pub priority: u32,         // 优先级分数 (0-100)
    pub tags: Vec<String>,     // 标签列表
    pub confidence: f64,       // 分类置信度 (0.0-1.0)
    pub reason: String,        // 分类原因
}
```
- **用途**：为告警提供智能分类和优先级排序
- **默认值**：未分类告警使用 "unclassified" 类别

#### ClassificationRule（分类规则）
```rust
pub struct ClassificationRule {
    pub name: String,                          // 规则名称
    pub category: String,                      // 目标分类
    pub title_patterns: Vec<String>,           // 标题匹配模式
    pub description_patterns: Vec<String>,     // 描述匹配模式
    pub level_filter: Option<Vec<AlarmLevel>>, // 级别过滤器
    pub priority_boost: u32,                   // 优先级提升值
    pub tags: Vec<String>,                     // 添加的标签
    pub confidence: f64,                       // 规则置信度
    pub reason: String,                        // 规则说明
}
```

#### AlarmCategory（告警类别定义）
```rust
pub struct AlarmCategory {
    pub name: String,           // 类别名称
    pub description: String,    // 类别描述
    pub color: String,         // 显示颜色（十六进制）
    pub icon: String,          // 显示图标
    pub priority_weight: f32,  // 优先级权重乘数
}
```

### 3. 升级规则

#### EscalationRule（升级规则）
```rust
pub struct EscalationRule {
    pub name: String,               // 规则名称
    pub from_status: AlarmStatus,   // 源状态
    pub from_level: AlarmLevel,     // 源级别
    pub to_level: AlarmLevel,       // 目标级别
    pub duration_minutes: u32,      // 升级前等待时间（分钟）
    pub condition: String,          // 升级条件描述
}
```
- **用途**：定义告警自动升级的条件
- **示例**：30分钟未处理的Warning升级为Major

### 4. 统计数据结构

#### AlarmStatistics（告警统计）
```rust
pub struct AlarmStatistics {
    pub total: usize,                              // 总数
    pub by_status: AlarmStatusStats,               // 按状态统计
    pub by_level: AlarmLevelStats,                 // 按级别统计
    pub by_category: HashMap<String, usize>,       // 按类别统计
    pub today_handled: usize,                      // 今日处理数
    pub active: usize,                             // 活跃告警数
}
```

#### AlarmStatusStats（状态统计）
```rust
pub struct AlarmStatusStats {
    pub new: usize,          // 新告警数
    pub acknowledged: usize, // 已确认数
    pub resolved: usize,     // 已解决数
}
```

#### AlarmLevelStats（级别统计）
```rust
pub struct AlarmLevelStats {
    pub critical: usize,  // 严重级别数
    pub major: usize,     // 重要级别数
    pub minor: usize,     // 次要级别数
    pub warning: usize,   // 警告级别数
    pub info: usize,      // 信息级别数
}
```

### 5. 云平台集成

#### CloudAlarm（云平台告警格式）
```rust
pub struct CloudAlarm {
    pub id: String,
    pub title: String,
    pub description: String,
    pub level: String,
    pub status: String,
    pub category: String,
    pub priority: u32,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub source: String,                         // 来源服务
    pub facility: String,                       // 设施/位置
    pub cloud_metadata: HashMap<String, String>, // 云平台元数据
}
```
- **用途**：与 netsrv 集成，推送告警到云平台
- **转换**：通过 `CloudAlarm::from_alarm()` 方法从内部格式转换

## API 数据传输对象

### 1. 请求对象

#### AlarmQuery（查询参数）
```rust
struct AlarmQuery {
    category: Option<String>,      // 类别筛选
    level: Option<AlarmLevel>,     // 级别筛选
    status: Option<AlarmStatus>,   // 状态筛选
    limit: Option<usize>,          // 每页数量
    offset: Option<usize>,         // 偏移量
    start_time: Option<String>,    // 开始时间
    end_time: Option<String>,      // 结束时间
    keyword: Option<String>,       // 关键词搜索
}
```

#### CreateAlarmRequest（创建告警请求）
```rust
struct CreateAlarmRequest {
    title: String,         // 告警标题
    description: String,   // 告警描述
    level: AlarmLevel,     // 告警级别
}
```

### 2. 响应对象

#### AlarmListResponse（告警列表响应）
```rust
struct AlarmListResponse {
    alarms: Vec<Alarm>,  // 告警列表
    total: usize,        // 总数
    offset: usize,       // 当前偏移
    limit: usize,        // 每页限制
}
```

#### StatusResponse（服务状态响应）
```rust
struct StatusResponse {
    service: String,           // 服务名称
    status: String,            // 运行状态
    total_alarms: usize,       // 告警总数
    active_alarms: usize,      // 活跃告警数
    redis_connected: bool,     // Redis连接状态
    classifier_rules: usize,   // 分类规则数
}
```

#### ClassificationResult（分类结果）
```rust
struct ClassificationResult {
    classified_count: usize,  // 成功分类数
    failed_count: usize,      // 失败数
}
```

## 存储层设计

### 1. Redis 存储结构

#### 主数据存储
- **Key**: `ems:alarms:{alarm_id}`
- **Type**: Hash
- **Fields**:
  ```
  id: UUID字符串
  title: 标题
  description: 描述
  level: 级别序列化值
  status: 状态序列化值
  category: 分类名称
  priority: 优先级分数
  tags: 标签JSON数组
  created_at: RFC3339时间戳
  updated_at: RFC3339时间戳
  data: 完整告警JSON
  ```

#### 索引设计
1. **分类索引**: `ems:alarms:category:{category}` (Set)
2. **级别索引**: `ems:alarms:level:{level}` (Set)
3. **状态索引**: `ems:alarms:status:{status}` (Set)
4. **日期索引**: `ems:alarms:date:{YYYY-MM-DD}` (Set)

#### 统计数据
- **Key**: `ems:alarms:stats` (Hash)
- **Fields**: total, new, acknowledged, resolved, critical, major, minor, warning, info

#### 今日处理计数
- **Key**: `ems:alarms:handled:{YYYY-MM-DD}`
- **Type**: String (计数器)
- **TTL**: 7天

### 2. 云平台数据发布
- **Channel**: `ems:data:alarms`
- **Format**: CloudAlarm JSON

## 配置数据结构

### AlarmConfig（主配置）
```rust
pub struct AlarmConfig {
    pub redis: RedisConfig,      // Redis配置
    pub api: ApiConfig,          // API配置
    pub storage: StorageConfig,  // 存储配置
}
```

### RedisConfig（Redis配置）
```rust
pub struct RedisConfig {
    pub connection_type: RedisConnectionType,  // 连接类型
    pub host: String,                          // 主机地址
    pub port: u16,                            // 端口
    pub socket_path: Option<String>,          // Unix socket路径
    pub password: Option<String>,             // 密码
    pub database: u8,                         // 数据库号
}
```

### ApiConfig（API配置）
```rust
pub struct ApiConfig {
    pub host: String,  // 监听地址
    pub port: u16,     // 监听端口
}
```

### StorageConfig（存储配置）
```rust
pub struct StorageConfig {
    pub retention_days: u32,           // 保留天数
    pub auto_cleanup: bool,            // 自动清理
    pub cleanup_interval_hours: u32,   // 清理间隔（小时）
}
```

## 运行时状态

### AppState（应用状态）
```rust
struct AppState {
    alarms: Arc<RwLock<Vec<Alarm>>>,       // 内存告警缓存
    config: Arc<AlarmConfig>,              // 配置信息
    redis_storage: Arc<RedisStorage>,      // Redis存储实例
    classifier: Arc<AlarmClassifier>,      // 分类器实例
}
```

## 数据流转关系

### 1. 告警创建流程
```
用户请求 → CreateAlarmRequest → Alarm对象创建 → 分类处理 → Redis存储 → 云平台发布
```

### 2. 告警查询流程
```
查询参数 → AlarmQuery → Redis查询 → 过滤/排序 → AlarmListResponse → 返回前端
```

### 3. 告警状态更新
```
确认/解决请求 → Redis更新 → 统计更新 → 云平台通知 → 响应返回
```

### 4. 自动升级流程
```
定时任务 → 查询符合条件的告警 → 升级处理 → Redis更新 → 云平台通知
```

## 设计特点

1. **分层设计**：清晰的数据模型层、API层、存储层分离
2. **类型安全**：充分利用 Rust 的类型系统，减少运行时错误
3. **扩展性**：通过分类规则和升级规则支持灵活的业务逻辑
4. **性能优化**：使用多级索引加速查询，内存缓存减少Redis访问
5. **云原生**：支持与云平台的无缝集成，便于监控和分析

## 与其他服务的交互

1. **与 Redis 的交互**：作为主要的数据存储和消息传递中介
2. **与 netsrv 的集成**：通过 CloudAlarm 格式推送告警到云平台
3. **与前端的交互**：提供 RESTful API 支持告警管理界面
4. **与其他微服务**：通过 Redis 订阅机制接收系统告警事件