# 数据格式化指南

## 概述

netsrv 提供灵活的数据格式化机制，支持将 Redis 中的原始数据转换为各种目标格式。本文档详细介绍各种格式化器的配置和使用方法。

## 数据模型

### 输入数据结构

从 Redis 读取的数据具有以下结构：

```rust
pub struct DataPoint {
    pub channel_id: u32,        // 通道 ID
    pub data_type: String,      // 数据类型: m, s, control, measurement
    pub point_id: u32,          // 点位 ID
    pub value: f64,             // 数值（6位小数精度）
    pub timestamp: DateTime<Utc>, // 时间戳
    pub metadata: Option<HashMap<String, String>>, // 元数据
}
```

### 标准化浮点数

所有浮点数值强制保持 6 位小数精度：

```rust
// 值总是格式化为 6 位小数
"220.123456"
"0.000000"
"-15.500000"
```

## JSON 格式化器

### 标准格式

```yaml
json_formatter:
  structure: "standard"
  pretty: false
  include_metadata: true
  
  # 字段映射
  field_mapping:
    channel_id: "channel"
    data_type: "type"
    point_id: "point"
    value: "value"
    timestamp: "ts"
```

输出示例：

```json
{
  "channel": 1001,
  "type": "m",
  "point": 10001,
  "value": "220.123456",
  "ts": "2025-07-23T10:00:00.000Z",
  "metadata": {
    "unit": "V",
    "name": "voltage_a"
  }
}
```

### 扁平格式

```yaml
json_formatter:
  structure: "flat"
  key_pattern: "{prefix}_{channel}_{type}_{point}"
  prefix: "ems"
```

输出示例：

```json
{
  "timestamp": "2025-07-23T10:00:00.000Z",
  "ems_1001_m_10001": "220.123456",
  "ems_1001_m_10002": "221.234567",
  "ems_1001_m_10003": "219.345678"
}
```

### 嵌套格式

```yaml
json_formatter:
  structure: "nested"
  hierarchy:
    - "channel"
    - "type"
    - "point"
```

输出示例：

```json
{
  "timestamp": "2025-07-23T10:00:00.000Z",
  "channels": {
    "1001": {
      "measurements": {
        "10001": "220.123456",
        "10002": "221.234567"
      },
      "signals": {
        "20001": "1.000000",
        "20002": "0.000000"
      }
    }
  }
}
```

### 数组格式

```yaml
json_formatter:
  structure: "array"
  batch_wrapper: true
```

输出示例：

```json
{
  "device_id": "voltage_ems_001",
  "timestamp": "2025-07-23T10:00:00.000Z",
  "data": [
    {
      "channel": 1001,
      "type": "m",
      "point": 10001,
      "value": "220.123456"
    },
    {
      "channel": 1001,
      "type": "m",
      "point": 10002,
      "value": "221.234567"
    }
  ]
}
```

## ASCII 格式化器

### CSV 格式

```yaml
ascii_formatter:
  type: "csv"
  delimiter: ","
  headers: true
  quote_strings: true
  
  # 列定义
  columns:
    - "timestamp"
    - "channel"
    - "type"
    - "point"
    - "value"
```

输出示例：

```csv
timestamp,channel,type,point,value
2025-07-23T10:00:00.000Z,1001,m,10001,220.123456
2025-07-23T10:00:00.000Z,1001,m,10002,221.234567
```

### 固定宽度格式

```yaml
ascii_formatter:
  type: "fixed_width"
  
  # 列宽定义
  columns:
    - name: "timestamp"
      width: 24
      align: "left"
    - name: "channel"
      width: 6
      align: "right"
    - name: "point"
      width: 8
      align: "right"
    - name: "value"
      width: 12
      align: "right"
      precision: 6
```

输出示例：

```
2025-07-23T10:00:00.000Z  1001   10001  220.123456
2025-07-23T10:00:00.000Z  1001   10002  221.234567
```

### 自定义分隔符

```yaml
ascii_formatter:
  type: "delimited"
  delimiter: "|"
  row_terminator: "\r\n"
  escape_char: "\\"
```

## XML 格式化器

### 基本配置

```yaml
xml_formatter:
  root_element: "data"
  namespace: "http://voltage-ems.com/data/v1"
  pretty: true
  
  # 属性映射
  attributes:
    - field: "channel_id"
      name: "channel"
    - field: "timestamp"
      name: "ts"
```

输出示例：

```xml
<?xml version="1.0" encoding="UTF-8"?>
<data xmlns="http://voltage-ems.com/data/v1" ts="2025-07-23T10:00:00.000Z">
  <measurement channel="1001">
    <point id="10001">
      <value>220.123456</value>
    </point>
    <point id="10002">
      <value>221.234567</value>
    </point>
  </measurement>
</data>
```

### IEC 61850 格式

```yaml
xml_formatter:
  template: "iec61850"
  logical_device: "MMXU1"
  dataset: "Measurements"
```

输出示例：

```xml
<IED name="VoltageEMS">
  <AccessPoint name="S1">
    <LDevice inst="MMXU1">
      <LN0 lnClass="LLN0" inst="">
        <DataSet name="Measurements">
          <FCDA ldInst="MMXU1" lnClass="MMXU" lnInst="1" 
                doName="PhV" daName="phsA" fc="MX"/>
        </DataSet>
      </LN0>
      <LN lnClass="MMXU" inst="1">
        <DO name="PhV">
          <DA name="phsA" bType="Struct">
            <Val>220.123456</Val>
          </DA>
        </DO>
      </LN>
    </LDevice>
  </AccessPoint>
</IED>
```

## 协议缓冲区（Protocol Buffers）

### Proto 定义

```protobuf
syntax = "proto3";

package voltage_ems;

message DataPoint {
  uint32 channel_id = 1;
  string data_type = 2;
  uint32 point_id = 3;
  string value = 4;  // 保持字符串格式以保证精度
  int64 timestamp = 5;
  map<string, string> metadata = 6;
}

message DataBatch {
  string device_id = 1;
  int64 timestamp = 2;
  repeated DataPoint data = 3;
}
```

### 配置

```yaml
protobuf_formatter:
  message_type: "DataBatch"
  include_field_names: false
  base64_encode: false
```

## 自定义模板

### Jinja2 模板

```yaml
template_formatter:
  engine: "jinja2"
  template_file: "/templates/custom.j2"
  
  # 模板变量
  variables:
    device_id: "${DEVICE_ID}"
    location: "Factory-01"
```

模板示例（custom.j2）：

```jinja2
{
  "device": "{{ device_id }}",
  "location": "{{ location }}",
  "timestamp": "{{ timestamp }}",
  "readings": [
    {% for point in data_points %}
    {
      "id": "{{ point.channel_id }}_{{ point.point_id }}",
      "value": {{ point.value }},
      "unit": "{{ point.metadata.unit | default('') }}"
    }{% if not loop.last %},{% endif %}
    {% endfor %}
  ]
}
```

### Handlebars 模板

```yaml
template_formatter:
  engine: "handlebars"
  template: |
    {{#each data_points}}
    {{channel_id}},{{point_id}},{{value}}
    {{/each}}
```

## 转换和处理

### 值转换

```yaml
transformations:
  # 单位转换
  - field: "value"
    condition: "metadata.unit == 'kV'"
    operation: "multiply"
    factor: 1000
    new_unit: "V"
    
  # 精度调整
  - field: "value"
    operation: "round"
    precision: 2
    
  # 范围限制
  - field: "value"
    operation: "clamp"
    min: 0
    max: 1000
```

### 字段重命名

```yaml
field_mapping:
  # 简单映射
  channel_id: "channelID"
  point_id: "tagID"
  
  # 条件映射
  conditional:
    - source: "value"
      target: "voltage"
      condition: "point_id >= 10001 && point_id <= 10003"
    - source: "value"
      target: "current"
      condition: "point_id >= 10004 && point_id <= 10006"
```

### 数据聚合

```yaml
aggregation:
  enabled: true
  window: 60  # 秒
  
  # 聚合规则
  rules:
    - points: [10001, 10002, 10003]
      operation: "average"
      output_point: 10000
      name: "avg_voltage"
      
    - points: [10004, 10005, 10006]
      operation: "sum"
      output_point: 10007
      name: "total_current"
```

## 压缩和编码

### 压缩选项

```yaml
compression:
  enabled: true
  algorithm: "gzip"  # gzip, deflate, brotli, lz4
  level: 6  # 1-9
  
  # 条件压缩
  conditional:
    min_size: 1024  # 只压缩大于 1KB 的数据
```

### 编码选项

```yaml
encoding:
  # 字符编码
  charset: "UTF-8"
  
  # 二进制编码
  binary_encoding: "base64"  # base64, hex
  
  # 特殊字符处理
  escape_special_chars: true
  escape_unicode: false
```

## 批量格式化

### 批量配置

```yaml
batch_formatting:
  # 分组策略
  grouping:
    by: ["channel_id", "data_type"]
    sort: true
    
  # 包装器
  wrapper:
    enabled: true
    template: |
      {
        "batch_id": "{{ batch_id }}",
        "count": {{ count }},
        "start_time": "{{ start_time }}",
        "end_time": "{{ end_time }}",
        "data": {{ data }}
      }
```

### 流式处理

```yaml
streaming:
  enabled: true
  format: "ndjson"  # newline-delimited JSON
  
  # 流控制
  buffer_size: 100
  flush_on_newline: true
```

## 性能优化

### 格式化缓存

```rust
pub struct FormatterCache {
    templates: LruCache<String, CompiledTemplate>,
    results: LruCache<u64, Vec<u8>>,
}

impl FormatterCache {
    pub fn format_with_cache(&mut self, data: &DataPoint) -> Vec<u8> {
        let hash = self.calculate_hash(data);
        
        if let Some(cached) = self.results.get(&hash) {
            return cached.clone();
        }
        
        let formatted = self.format(data);
        self.results.put(hash, formatted.clone());
        formatted
    }
}
```

### 零拷贝格式化

```rust
pub trait ZeroCopyFormatter {
    fn format_into(&self, data: &DataPoint, buffer: &mut BytesMut);
}

impl ZeroCopyFormatter for AsciiFormatter {
    fn format_into(&self, data: &DataPoint, buffer: &mut BytesMut) {
        use std::io::Write;
        
        write!(
            buffer,
            "{},{},{},{:.6}\n",
            data.timestamp.timestamp(),
            data.channel_id,
            data.point_id,
            data.value
        ).unwrap();
    }
}
```

## 错误处理

### 格式化错误

```yaml
error_handling:
  # 错误策略
  strategy: "skip"  # skip, default, fail
  
  # 默认值
  defaults:
    value: "0.000000"
    timestamp: "1970-01-01T00:00:00Z"
    
  # 错误日志
  log_errors: true
  error_file: "/logs/format_errors.log"
```

### 验证规则

```yaml
validation:
  # 数值验证
  - field: "value"
    type: "range"
    min: -1000000
    max: 1000000
    
  # 格式验证
  - field: "timestamp"
    type: "format"
    pattern: "ISO8601"
    
  # 必填字段
  required_fields:
    - "channel_id"
    - "point_id"
    - "value"
```

## 调试和测试

### 格式化测试

```bash
# 测试格式化器
curl -X POST http://localhost:8086/formatters/test \
  -H "Content-Type: application/json" \
  -d '{
    "formatter": "json",
    "config": {...},
    "data": {...}
  }'
```

### 性能测试

```bash
# 基准测试
cargo bench --package netsrv --bench formatters

# 性能分析
perf record -g cargo run --release
perf report
```

## 最佳实践

1. **选择合适的格式**
   - JSON: 通用性好，易于解析
   - CSV: 数据量大时效率高
   - Protocol Buffers: 高性能场景

2. **优化批量大小**
   - 根据网络带宽调整
   - 考虑目标系统的处理能力

3. **使用压缩**
   - 大数据量时启用压缩
   - 选择合适的压缩级别

4. **缓存模板**
   - 预编译复杂模板
   - 重用格式化结果