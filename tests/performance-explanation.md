# 性能测试受限原因分析

## 问题描述
在Lua脚本中无法调用FCALL命令来测试Lua Functions的性能。

## 原因分析

### 1. Redis安全机制
- **禁止递归调用**: Redis不允许在Lua脚本内部调用其他Lua函数（通过FCALL）
- **防止死循环**: 这是为了防止函数之间的递归调用导致死循环
- **错误信息**: "ERR This Redis command is not allowed from script"

### 2. 受限的命令
在Lua脚本中不能使用的命令：
- FCALL / FCALL_RO - 调用其他Lua函数
- SCRIPT LOAD/KILL/FLUSH - 脚本管理命令
- EVAL/EVALSHA - 执行其他脚本
- 某些阻塞命令

### 3. 性能测试的替代方案

#### 方案1: 使用Redis Benchmark
```bash
# 测试基础Hash操作性能
redis-benchmark -h localhost -p 6379 -n 100000 -c 50 -t hset

# 自定义测试脚本
redis-benchmark -h localhost -p 6379 -n 10000 eval "redis.call('HSET', 'test', KEYS[1], ARGV[1])" 1 key value
```

#### 方案2: 编写外部测试程序
```rust
// Rust性能测试示例
use redis::Commands;
use std::time::Instant;

fn benchmark_lua_functions() {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_connection().unwrap();
    
    let start = Instant::now();
    for i in 0..1000 {
        let _: String = con.fcall(
            "model_upsert",
            &[format!("model_{}", i)],
            &[format!(r#"{{"name":"Model {}"}}"#, i)]
        ).unwrap();
    }
    let duration = start.elapsed();
    
    println!("1000 operations in {:?}", duration);
    println!("Operations per second: {}", 1000.0 / duration.as_secs_f64());
}
```

#### 方案3: 使用包装脚本
```bash
#!/bin/bash
# performance-test.sh
echo "Testing Lua Function Performance..."

# 测试model_upsert性能
start_time=$(date +%s%N)
for i in {1..100}; do
    docker exec redis-test redis-cli FCALL model_upsert 1 "perf_model_$i" "{\"name\":\"Test $i\"}" > /dev/null
done
end_time=$(date +%s%N)

elapsed_ms=$(( ($end_time - $start_time) / 1000000 ))
ops_per_sec=$(( 100 * 1000 / $elapsed_ms ))
echo "Model operations: $ops_per_sec ops/sec"

# 测试alarm存储性能
start_time=$(date +%s%N)
for i in {1..100}; do
    docker exec redis-test redis-cli FCALL store_alarm 1 "perf_alarm_$i" "{\"title\":\"Alarm $i\",\"level\":\"Info\"}" > /dev/null
done
end_time=$(date +%s%N)

elapsed_ms=$(( ($end_time - $start_time) / 1000000 ))
ops_per_sec=$(( 100 * 1000 / $elapsed_ms ))
echo "Alarm operations: $ops_per_sec ops/sec"
```

## 推荐的性能测试方法

### 1. 使用专业工具
- **wrk**: HTTP压测工具，适合测试API性能
- **JMeter**: 功能全面的性能测试工具
- **Gatling**: 高性能负载测试框架

### 2. 容器内测试
避免docker exec的开销：
```bash
# 在容器内运行测试脚本
docker run --rm --network voltageems_voltageems-test \
    -v $PWD/tests:/tests \
    redis:8-alpine \
    sh /tests/internal-performance-test.sh
```

### 3. 编写专门的性能测试服务
创建一个专门的测试服务，直接连接Redis进行高频测试，避免进程启动开销。

## 结论
性能测试受限是Redis的安全设计，需要使用外部工具或程序进行准确的性能测试。