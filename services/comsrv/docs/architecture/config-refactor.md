# 配置重构方案

## 1. 概述

本文档描述 ComSrv v2.0 的配置重构方案。主要目标是简化配置结构，实现四遥配置与协议配置的分离，提高配置的可维护性和可读性。

## 2. 设计目标

1. **分离关注点**：四遥配置与协议配置分离
2. **减少冗余**：避免在主配置中包含协议细节
3. **层次清晰**：配置文件按功能和层次组织
4. **易于维护**：相关配置集中管理
5. **向后兼容**：提供平滑迁移路径

## 3. 配置架构

### 3.1 配置文件组织

```
config/
├── comsrv.yaml                 # 主配置文件
├── protocols/                  # 协议特定配置
│   ├── modbus_tcp.yaml        # Modbus TCP 配置
│   ├── modbus_rtu.yaml        # Modbus RTU 配置
│   ├── iec104.yaml            # IEC104 配置
│   └── can.yaml               # CAN 配置
└── channels/                   # 通道点表配置
    ├── 1001/                   # 通道 1001
    │   ├── measurement.csv     # 遥测点表
    │   ├── signal.csv          # 遥信点表
    │   ├── control.csv         # 遥控点表
    │   └── adjustment.csv      # 遥调点表
    └── 1002/                   # 通道 1002
        └── ...
```

### 3.2 配置加载流程

```
1. 加载主配置 (comsrv.yaml)
2. 根据协议类型加载协议配置 (protocols/*.yaml)
3. 根据通道ID加载点表 (channels/{id}/*.csv)
4. 合并配置并验证
```

## 4. 主配置文件

### 4.1 简化后的结构

```yaml
# comsrv.yaml
service:
  name: "comsrv"
  version: "2.0.0"
  description: "Communication Service"
  
  # API 配置
  api:
    enabled: true
    host: "0.0.0.0"
    port: 8001
    workers: 4  # 默认: CPU核心数
    
  # Redis 配置
  redis:
    enabled: true
    url: "redis://localhost:6379"
    pool_size: 10
    timeout_ms: 5000
    
  # 日志配置
  logging:
    level: "info"  # trace, debug, info, warn, error
    format: "pretty"  # pretty, json
    console: true
    file: "logs/comsrv.log"
    rotation:
      strategy: "daily"  # daily, size
      max_files: 7
      
  # 全局重连策略
  reconnect:
    max_attempts: 3
    initial_delay: "1s"
    max_delay: "60s"
    backoff_multiplier: 2.0
    jitter: true

# 通道列表（仅包含基本信息）
channels:
  - id: 1001
    name: "南区电表"
    description: "南区配电室智能电表"
    protocol: "modbus_tcp"
    enabled: true
    # 可选：覆盖全局重连策略
    reconnect:
      max_attempts: 0  # 无限重试
      
  - id: 1002
    name: "北区电表"
    description: "北区配电室智能电表"
    protocol: "modbus_tcp"
    enabled: true
    
  - id: 2001
    name: "主站通信"
    description: "与调度主站的通信"
    protocol: "iec104"
    enabled: true
```

### 4.2 环境变量支持

```bash
# 覆盖配置值
COMSRV_SERVICE_NAME=comsrv-prod
COMSRV_SERVICE_API_PORT=8080
COMSRV_SERVICE_REDIS_URL=redis://redis-cluster:6379
COMSRV_SERVICE_LOGGING_LEVEL=debug
```

## 5. 协议配置文件

### 5.1 Modbus TCP 配置

```yaml
# protocols/modbus_tcp.yaml
modbus_tcp:
  # 默认配置（可被通道配置覆盖）
  defaults:
    port: 502
    timeout_ms: 3000
    retry_count: 3
    # 轮询配置
    polling:
      enabled: true
      interval_ms: 1000
    # 批量读取配置
    batch:
      enabled: true
      max_size: 100
      max_gap: 5
      
  # 通道特定配置
  channels:
    1001:
      host: "192.168.1.100"
      port: 502
      slave_id: 1
      # 覆盖默认轮询间隔
      polling:
        interval_ms: 500
        
    1002:
      host: "192.168.1.101"
      port: 502
      slave_id: 2
```

### 5.2 Modbus RTU 配置

```yaml
# protocols/modbus_rtu.yaml
modbus_rtu:
  defaults:
    baud_rate: 9600
    data_bits: 8
    stop_bits: 1
    parity: "None"  # None, Even, Odd
    timeout_ms: 1000
    
  channels:
    3001:
      port: "/dev/ttyUSB0"
      baud_rate: 19200
      slave_id: 1
      
    3002:
      port: "/dev/ttyUSB1"
      baud_rate: 9600
      slave_id: 2
```

### 5.3 IEC104 配置

```yaml
# protocols/iec104.yaml
iec104:
  defaults:
    port: 2404
    t1: 15  # 发送或测试 APDU 的超时
    t2: 10  # 接收确认的超时
    t3: 20  # 测试帧的超时
    k: 12   # 发送方最多未确认 I 格式 APDU 数目
    w: 8    # 接收方最多未确认 I 格式 APDU 数目
    
  channels:
    2001:
      host: "10.0.0.1"
      common_address: 1
      info_obj_address_size: 3
      cause_size: 2
      # 信息对象地址映射
      ioa_base: 1
```

## 6. 点表配置

### 6.1 目录结构

```
channels/
└── 1001/
    ├── measurement.csv    # 遥测
    ├── signal.csv        # 遥信
    ├── control.csv       # 遥控
    └── adjustment.csv    # 遥调
```

### 6.2 点表格式

#### measurement.csv（遥测）
```csv
point_id,name,description,unit,data_type,scale,offset
1,voltage_a,A相电压,V,float32,0.1,0
2,voltage_b,B相电压,V,float32,0.1,0
3,current_a,A相电流,A,float32,0.01,0
4,power_active,有功功率,kW,float32,1.0,0
5,energy_total,总电能,kWh,float64,0.001,0
```

#### signal.csv（遥信）
```csv
point_id,name,description,normal_state
1,breaker_status,断路器状态,0
2,fault_alarm,故障告警,0
3,door_open,柜门状态,0
```

#### control.csv（遥控）
```csv
point_id,name,description,control_type
1,breaker_control,断路器控制,toggle
2,reset_alarm,复位告警,pulse
```

#### adjustment.csv（遥调）
```csv
point_id,name,description,unit,min_value,max_value
1,voltage_setpoint,电压设定,V,380,420
2,power_limit,功率限制,kW,0,1000
```

## 7. 配置加载实现

### 7.1 ConfigManager 重构

```rust
pub struct ConfigManager {
    /// 主配置
    app_config: AppConfig,
    /// 协议配置缓存
    protocol_configs: HashMap<String, ProtocolConfig>,
    /// 配置根目录
    config_root: PathBuf,
}

impl ConfigManager {
    /// 从文件加载配置
    pub async fn from_file(path: &str) -> Result<Self> {
        // 1. 加载主配置
        let app_config = Self::load_app_config(path)?;
        
        // 2. 确定配置根目录
        let config_root = Path::new(path).parent()
            .ok_or_else(|| ComSrvError::ConfigError("Invalid config path".into()))?
            .to_path_buf();
            
        let mut manager = Self {
            app_config,
            protocol_configs: HashMap::new(),
            config_root,
        };
        
        // 3. 加载协议配置
        manager.load_protocol_configs().await?;
        
        Ok(manager)
    }
    
    /// 加载协议配置
    async fn load_protocol_configs(&mut self) -> Result<()> {
        let protocols_dir = self.config_root.join("protocols");
        
        // 收集所有使用的协议类型
        let protocol_types: HashSet<_> = self.app_config.channels
            .iter()
            .map(|ch| &ch.protocol)
            .collect();
            
        // 加载对应的协议配置
        for protocol in protocol_types {
            let config_file = protocols_dir.join(format!("{}.yaml", protocol));
            if config_file.exists() {
                let config = Self::load_protocol_config(&config_file)?;
                self.protocol_configs.insert(protocol.clone(), config);
            }
        }
        
        Ok(())
    }
    
    /// 获取通道的完整配置
    pub fn get_channel_config(&self, channel_id: u16) -> Result<ChannelConfig> {
        // 1. 找到通道基本配置
        let channel = self.app_config.channels
            .iter()
            .find(|ch| ch.id == channel_id)
            .ok_or_else(|| ComSrvError::ConfigError(
                format!("Channel {} not found", channel_id)
            ))?;
            
        // 2. 加载协议特定配置
        let protocol_params = self.get_protocol_params(&channel.protocol, channel_id)?;
        
        // 3. 加载点表
        let point_tables = self.load_point_tables(channel_id).await?;
        
        // 4. 组合成完整配置
        Ok(ChannelConfig {
            id: channel.id,
            name: channel.name.clone(),
            protocol: channel.protocol.clone(),
            parameters: protocol_params,
            table_config: Some(TableConfig {
                four_telemetry_route: format!("channels/{}", channel_id),
                // ... 其他配置
            }),
            // ... 点表数据
        })
    }
}
```

### 7.2 配置验证

```rust
impl ConfigManager {
    /// 验证配置完整性
    pub fn validate(&self) -> Result<()> {
        // 验证主配置
        self.validate_app_config()?;
        
        // 验证通道配置
        for channel in &self.app_config.channels {
            self.validate_channel_config(channel)?;
        }
        
        // 验证协议配置
        for (protocol, config) in &self.protocol_configs {
            self.validate_protocol_config(protocol, config)?;
        }
        
        Ok(())
    }
    
    fn validate_channel_config(&self, channel: &ChannelConfig) -> Result<()> {
        // 检查协议是否支持
        if !self.is_protocol_supported(&channel.protocol) {
            return Err(ComSrvError::ConfigError(
                format!("Unsupported protocol: {}", channel.protocol)
            ));
        }
        
        // 检查点表文件是否存在
        let channel_dir = self.config_root.join("channels").join(channel.id.to_string());
        if !channel_dir.exists() {
            return Err(ComSrvError::ConfigError(
                format!("Channel {} point tables not found", channel.id)
            ));
        }
        
        Ok(())
    }
}
```

## 8. 迁移策略

### 8.1 兼容性处理

```rust
impl ConfigManager {
    /// 加载配置（支持新旧格式）
    pub async fn load_config(path: &str) -> Result<Self> {
        // 尝试新格式
        match Self::from_file(path).await {
            Ok(config) => Ok(config),
            Err(_) => {
                // 尝试旧格式
                warn!("Failed to load new config format, trying legacy format");
                Self::from_legacy_file(path).await
            }
        }
    }
    
    /// 从旧格式迁移
    async fn from_legacy_file(path: &str) -> Result<Self> {
        let legacy_config = LegacyConfig::load(path)?;
        let migrated = Self::migrate_from_legacy(legacy_config)?;
        
        // 可选：保存为新格式
        if let Ok(backup_path) = Self::save_migrated_config(&migrated).await {
            info!("Legacy config migrated to: {}", backup_path.display());
        }
        
        Ok(migrated)
    }
}
```

### 8.2 迁移工具

```bash
# 迁移脚本
comsrv-migrate --input old-config.yaml --output-dir new-config/

# 验证迁移结果
comsrv-migrate --validate new-config/comsrv.yaml
```

## 9. 配置示例

### 9.1 最小配置

```yaml
# 最小可运行配置
service:
  name: "comsrv"

channels:
  - id: 1001
    name: "Test Channel"
    protocol: "modbus_tcp"
```

### 9.2 生产环境配置

```yaml
service:
  name: "comsrv-prod"
  api:
    host: "0.0.0.0"
    port: 8001
  redis:
    url: "${REDIS_URL}"  # 从环境变量读取
  logging:
    level: "info"
    file: "/var/log/comsrv/comsrv.log"
  reconnect:
    max_attempts: 0  # 生产环境无限重试

channels:
  # ... 通道配置
```

## 10. 优势总结

### 10.1 对比旧配置

| 方面 | 旧配置 | 新配置 |
|------|--------|--------|
| 文件数量 | 单一大文件 | 分层多文件 |
| 协议参数 | 混在主配置中 | 独立协议文件 |
| 可读性 | 较差 | 良好 |
| 维护性 | 困难 | 简单 |
| 扩展性 | 有限 | 灵活 |

### 10.2 具体改进

1. **更清晰的结构**：主配置只包含服务级和通道列表
2. **更好的组织**：相关配置放在一起
3. **更易维护**：修改协议参数不影响主配置
4. **更好的复用**：协议默认值避免重复
5. **更强的扩展性**：新协议只需添加配置文件

## 11. 注意事项

1. **配置路径**：所有相对路径都相对于主配置文件所在目录
2. **环境变量**：支持 `${VAR}` 语法引用环境变量
3. **配置覆盖**：通道配置 > 协议默认配置 > 全局配置
4. **向后兼容**：保持对旧配置格式的读取支持至少一个大版本