# modsrv Redis Hash结构设计

## 概述
modsrv（计算服务）负责实时计算、公式运算和数据聚合。从comsrv读取原始数据，计算后将结果存储在Hash结构中，供其他服务使用。

## Hash键结构

### 计算结果存储
```
Key Pattern: modsrv:realtime:module:{module_id}
```

### 字段结构
每个Hash包含该计算模块的所有计算点结果：
```
Field: {calc_point_id}
Value: JSON格式的计算结果
```

### 数据示例
```json
Key: modsrv:realtime:module:power_calc
Fields:
  total_active_power: {
    "value": 5678.9,
    "formula": "sum(line1_p, line2_p, line3_p)",
    "timestamp": "2025-01-10T10:30:01.456Z",
    "source_points": ["line1_p", "line2_p", "line3_p"],
    "unit": "kW",
    "quality": "calculated",
    "calc_time_ms": 2.3
  }
  
  power_factor: {
    "value": 0.95,
    "formula": "active_power / apparent_power",
    "timestamp": "2025-01-10T10:30:01.456Z", 
    "source_points": ["active_power", "apparent_power"],
    "unit": "",
    "quality": "calculated",
    "calc_time_ms": 1.1
  }
  
  daily_energy: {
    "value": 12345.67,
    "formula": "integrate(total_active_power, '1d')",
    "timestamp": "2025-01-10T10:30:01.456Z",
    "source_points": ["total_active_power"],
    "unit": "kWh",
    "quality": "calculated",
    "calc_time_ms": 5.6,
    "aggregation_type": "daily"
  }
```

## 计算类型

### 实时计算
- 基础数学运算：加减乘除
- 三角函数：sin/cos/tan
- 统计函数：avg/max/min/sum
- 逻辑运算：and/or/not

### 聚合计算
- 时间窗口：1分钟/5分钟/1小时/1天
- 聚合类型：平均值/最大值/最小值/累计值
- 滑动窗口：支持重叠窗口计算

### 高级计算
- 积分计算：电能累计
- 微分计算：变化率
- FFT分析：谐波计算
- 自定义脚本：Lua/Python

## 数据依赖管理

### DAG（有向无环图）
```
原始数据层：
  line1_v, line1_i (from comsrv)
  line2_v, line2_i (from comsrv)
  line3_v, line3_i (from comsrv)
    ↓
计算层1：
  line1_p = line1_v * line1_i
  line2_p = line2_v * line2_i
  line3_p = line3_v * line3_i
    ↓
计算层2：
  total_p = line1_p + line2_p + line3_p
    ↓
计算层3：
  daily_energy = integrate(total_p)
```

### 依赖追踪
```rust
pub struct CalcDependency {
    pub target_point: String,
    pub source_points: Vec<String>,
    pub formula: String,
    pub calc_order: u32,  // 计算顺序
}
```

## 写入策略

### 批量计算更新
```rust
pub fn batch_update_points(&mut self, module_id: &str, results: Vec<CalcResult>) -> Result<()> {
    let hash_key = format!("modsrv:realtime:module:{}", module_id);
    let mut updates = Vec::new();
    
    for result in results {
        let field = result.point_id;
        let value = serde_json::to_string(&result)?;
        updates.push((field, value));
    }
    
    self.redis.hset_multiple(&hash_key, updates)?;
    Ok(())
}
```

### 触发机制
1. **定时触发**：按配置的计算周期
2. **事件触发**：源数据更新时
3. **级联触发**：依赖点计算完成后

## 读取优化

### 模块级查询
```redis
HGETALL modsrv:realtime:module:power_calc
```

### 跨模块查询
```rust
pub async fn get_multi_module_points(&self, queries: Vec<(String, Vec<String>)>) -> Result<HashMap<String, Value>> {
    let mut pipeline = self.redis.pipeline();
    
    for (module_id, point_ids) in queries {
        let key = format!("modsrv:realtime:module:{}", module_id);
        if point_ids.is_empty() {
            pipeline.hgetall(&key);
        } else {
            pipeline.hmget(&key, point_ids);
        }
    }
    
    pipeline.execute().await
}
```

## 计算配置

### 模块配置示例
```yaml
modules:
  - id: "power_calc"
    name: "电力计算模块"
    calc_interval: 1000  # 毫秒
    calculations:
      - id: "total_active_power"
        formula: "sum(line1_p, line2_p, line3_p)"
        source_points:
          - channel: 1
            points: ["line1_p", "line2_p", "line3_p"]
        
      - id: "power_factor"
        formula: "active_power / apparent_power"
        source_points:
          - channel: 1
            points: ["active_power", "apparent_power"]
        validation:
          min: 0
          max: 1
```

### 公式语法
```
# 基础运算
result = a + b * c / d

# 函数调用
result = sqrt(pow(a, 2) + pow(b, 2))

# 条件判断
result = if(a > 100, a * 0.9, a)

# 聚合函数
result = avg(last_n_values(a, 10))
```

## 性能优化

### 计算优化
1. **并行计算**：同层级点位并行处理
2. **缓存机制**：缓存中间计算结果
3. **增量计算**：只计算变化的数据
4. **预计算**：提前计算常用指标

### 存储优化
1. **压缩存储**：对历史趋势数据压缩
2. **过期策略**：自动清理过期计算结果
3. **分片存储**：大模块拆分多个Hash

## 质量控制

### 数据质量标记
- **calculated**：正常计算结果
- **estimated**：估算值（部分数据缺失）
- **manual**：人工设定值
- **error**：计算错误

### 异常处理
```rust
pub enum CalcError {
    SourceDataMissing(String),
    DivisionByZero,
    FormulaError(String),
    Timeout,
    DependencyCycle,
}
```

## 监控指标

### 性能监控
- 计算延迟：从触发到完成的时间
- 计算吞吐量：每秒计算点数
- 缓存命中率：重复计算的优化效果
- CPU使用率：计算资源消耗

### 数据监控
- 计算成功率：成功/总计算次数
- 数据新鲜度：最后更新时间
- 异常点位数：计算失败的点位
- 依赖缺失数：源数据不可用

## 高可用设计

### 故障恢复
1. **计算状态持久化**：定期保存计算状态
2. **断点续算**：故障后从断点继续
3. **降级策略**：使用历史数据或默认值

### 负载均衡
1. **模块分片**：按module_id分配到不同实例
2. **动态调度**：根据计算复杂度分配
3. **弹性伸缩**：根据负载自动扩容

## 扩展接口

### 自定义函数
```rust
pub trait CustomFunction {
    fn name(&self) -> &str;
    fn calculate(&self, args: Vec<f64>) -> Result<f64>;
}

// 注册自定义函数
calculator.register_function(Box::new(MyCustomFunc));
```

### 外部数据源
- 支持从外部API获取数据
- 天气数据、电价数据等
- 缓存策略避免频繁调用

## 最佳实践

1. **合理设计计算图**：避免循环依赖
2. **分层计算**：复杂计算分解为多层
3. **异常值处理**：设置合理的数据范围
4. **版本管理**：公式变更要有版本控制
5. **性能测试**：定期评估计算性能
6. **文档完善**：每个公式都要有说明