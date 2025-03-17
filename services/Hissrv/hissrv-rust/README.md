# Hissrv - Historical Data Service

Hissrv是一个高性能的历史数据服务，用于从Redis数据库读取数据并存储到InfluxDB时序数据库。该服务设计用于能源管理系统(EMS)的历史数据存储和分析需求。

## 特性

- 高效从Redis读取数据
- 配置灵活的数据映射到InfluxDB
- 批处理写入以提高性能
- 完全基于YAML的配置文件
- 可配置的数据类型转换和缩放
- 健壮的错误处理和日志记录

## 安装

### 依赖

- Rust 1.56+ 
- Redis 6.0+
- InfluxDB 2.0+

### 编译

```bash
cd services/Hissrv/hissrv-rust
cargo build --release
```

编译后的二进制文件将位于`target/release/hissrv`。

### 配置

配置文件使用YAML格式。一个示例配置文件已经提供在项目根目录的`config.yaml`中。

## 使用方法

### 基本使用

```bash
# 使用默认配置文件 (config.yaml)
./target/release/hissrv

# 使用自定义配置文件
./target/release/hissrv --config /path/to/custom_config.yaml
```

### 配置详解

配置文件包含四个主要部分：

1. **Redis配置**：定义Redis连接参数和数据轮询设置
2. **InfluxDB配置**：定义InfluxDB连接参数和批处理设置
3. **日志配置**：定义日志级别和输出
4. **数据映射配置**：定义如何将Redis数据映射到InfluxDB

#### Redis配置

```yaml
redis:
  hostname: "localhost"   # Redis服务器主机名
  port: 6379              # Redis端口
  password: null          # Redis密码，null表示无密码
  database: 0             # Redis数据库索引
  connection_timeout: 5   # 连接超时(秒)
  key_pattern: "*"        # 要读取的Redis键模式
  polling_interval: 10    # 轮询间隔(秒)
```

#### InfluxDB配置

```yaml
influxdb:
  url: "http://localhost:8086"  # InfluxDB服务器URL
  org: "voltage"                # 组织名称
  token: "your-auth-token"      # 认证令牌
  bucket: "history"             # 存储桶名称
  batch_size: 1000              # 批处理大小
  flush_interval: 30            # 刷新间隔(秒)
```

#### 数据映射配置

```yaml
data_mapping:
  default_measurement: "ems_data"  # 默认测量名称
  
  # 标签映射定义Redis键/值如何映射到InfluxDB标签
  tag_mappings:
    - redis_source: "device"          # Redis中的源
      influx_tag: "device_id"         # InfluxDB中的标签名
      extract_from_key: true          # 是否从键名中提取
      extraction_pattern: "device:(.*):data"  # 提取模式(正则表达式)
  
  # 字段映射定义Redis值如何映射到InfluxDB字段
  field_mappings:
    - redis_source: "temperature"  # Redis中的源
      influx_field: "temperature"  # InfluxDB中的字段名
      data_type: "float"           # 数据类型(string,float,integer,boolean)
      scale_factor: 1.0            # 缩放因子
      measurement: "environmental"  # 测量名称(覆盖默认值)
```

## 数据类型

支持的数据类型:
- `string`: 字符串值
- `float`: 浮点数值
- `integer`: 整数值
- `boolean`: 布尔值

## 开发

### 测试

```bash
cargo test
```

### 代码风格检查

```bash
cargo clippy
```

## 许可证

[MIT](LICENSE) 