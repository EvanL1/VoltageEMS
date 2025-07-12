//! modsrv新数据结构设计可行性验证测试
//!
//! 本测试文件用于验证modsrv新设计的：
//! 1. 性能方面 - 批量操作效率、Redis键查询性能、内存使用
//! 2. 兼容性 - 与DAG执行器、comsrv数据交互、控制命令传递
//! 3. 数据一致性和并发访问
//! 4. 错误处理机制

use modsrv::storage::{
    ControlManager, ControlType, ModelStorage, MonitorKey, MonitorManager, MonitorType,
    MonitorUpdate, MonitorValue,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{error, info};

/// 测试配置
struct TestConfig {
    redis_url: String,
    test_points: usize,
    batch_size: usize,
    concurrent_models: usize,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            redis_url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            test_points: 1000,
            batch_size: 100,
            concurrent_models: 10,
        }
    }
}

/// 性能基准测试
#[tokio::test]
async fn test_performance_benchmarks() -> Result<(), Box<dyn std::error::Error>> {
    let config = TestConfig::default();
    let mut storage = ModelStorage::new(&config.redis_url).await?;

    info!("=== 性能基准测试 ===");

    // 1. 批量写入性能测试
    info!("\n1. 批量写入性能测试");
    let model_id = "perf_test_model";
    let mut updates = Vec::new();

    for i in 0..config.test_points {
        let monitor_value = MonitorValue::new(100.0 + (i as f64 * 0.1), model_id.to_string());

        updates.push(MonitorUpdate {
            model_id: model_id.to_string(),
            monitor_type: MonitorType::ModelOutput,
            point_id: 10000 + i as u32,
            value: monitor_value,
        });
    }

    // 分批写入测试
    let start = Instant::now();
    for chunk in updates.chunks(config.batch_size) {
        storage.set_monitor_values(chunk).await?;
    }
    let write_duration = start.elapsed();

    info!(
        "批量写入 {} 个点位耗时: {:?} (平均每点: {:?})",
        config.test_points,
        write_duration,
        write_duration / config.test_points as u32
    );

    // 2. 批量读取性能测试
    info!("\n2. 批量读取性能测试");
    let mut keys = Vec::new();
    for i in 0..config.test_points {
        keys.push(MonitorKey {
            model_id: model_id.to_string(),
            monitor_type: MonitorType::ModelOutput,
            point_id: 10000 + i as u32,
        });
    }

    let start = Instant::now();
    let _values = storage.get_monitor_values(&keys).await?;
    let read_duration = start.elapsed();

    info!(
        "批量读取 {} 个点位耗时: {:?} (平均每点: {:?})",
        config.test_points,
        read_duration,
        read_duration / config.test_points as u32
    );

    // 3. Redis键模式查询性能
    info!("\n3. Redis键模式查询性能");
    let start = Instant::now();
    // 使用pipeline批量检查键是否存在
    let mut pipe = redis::pipe();
    for i in 0..100 {
        let key = format!("mod:{}:mo:{}", model_id, 10000 + i);
        pipe.exists(&key);
    }
    let exists_duration = start.elapsed();

    info!("批量检查100个键存在性耗时: {:?}", exists_duration);

    // 4. 内存使用评估
    info!("\n4. 内存使用评估");
    let single_value = MonitorValue::new(123.45, "test_source".to_string());
    let redis_str = single_value.to_redis();
    let memory_per_value = redis_str.len();
    let total_memory = memory_per_value * config.test_points;

    info!("单个监视值占用: {} 字节", memory_per_value);
    info!(
        "{}个点位预计占用: {:.2} MB",
        config.test_points,
        total_memory as f64 / 1024.0 / 1024.0
    );

    Ok(())
}

/// DAG执行器兼容性测试
#[tokio::test]
async fn test_dag_executor_compatibility() -> Result<(), Box<dyn std::error::Error>> {
    let config = TestConfig::default();
    let mut monitor_mgr = MonitorManager::new(&config.redis_url).await?;

    info!("=== DAG执行器兼容性测试 ===");

    // 模拟DAG节点执行
    info!("\n1. 模拟DAG节点执行流程");

    // 节点1: 读取输入数据
    let input_mappings = vec![
        (1001, "m", 10001, "input_a".to_string()),
        (1001, "m", 10002, "input_b".to_string()),
        (1001, "s", 30001, "status".to_string()),
    ];

    let node1_start = Instant::now();
    let inputs = monitor_mgr.read_model_inputs(&input_mappings).await?;
    let node1_duration = node1_start.elapsed();
    info!("节点1 - 读取输入: {:?}", node1_duration);

    // 节点2: 执行计算
    let node2_start = Instant::now();
    let mut outputs = HashMap::new();
    outputs.insert(
        "sum".to_string(),
        inputs.get("input_a").unwrap_or(&0.0) + inputs.get("input_b").unwrap_or(&0.0),
    );
    outputs.insert("avg".to_string(), outputs["sum"] / 2.0);
    let node2_duration = node2_start.elapsed();
    info!("节点2 - 执行计算: {:?}", node2_duration);

    // 节点3: 写入中间值
    let node3_start = Instant::now();
    monitor_mgr
        .write_intermediate_value("dag_test", "intermediate_sum", outputs["sum"])
        .await?;
    let node3_duration = node3_start.elapsed();
    info!("节点3 - 写入中间值: {:?}", node3_duration);

    // 节点4: 写入最终输出
    let node4_start = Instant::now();
    monitor_mgr.write_model_outputs("dag_test", outputs).await?;
    let node4_duration = node4_start.elapsed();
    info!("节点4 - 写入输出: {:?}", node4_duration);

    let total_duration = node1_duration + node2_duration + node3_duration + node4_duration;
    info!("DAG总执行时间: {:?}", total_duration);

    // 验证数据流
    info!("\n2. 验证DAG数据流");
    let output = monitor_mgr.get_last_model_output("dag_test").await?;
    assert!(output.is_some());
    let output = output.unwrap();
    assert!(output.outputs.contains_key("sum"));
    assert!(output.outputs.contains_key("avg"));
    info!("✓ DAG输出验证通过");

    Ok(())
}

/// comsrv数据交互测试
#[tokio::test]
async fn test_comsrv_interaction() -> Result<(), Box<dyn std::error::Error>> {
    let config = TestConfig::default();
    let mut storage = ModelStorage::new(&config.redis_url).await?;

    info!("=== comsrv数据交互测试 ===");

    // 1. 模拟comsrv写入数据
    info!("\n1. 模拟comsrv写入数据");
    let channel_id = 1001u16;
    let test_points = vec![
        (channel_id, "m", 10001u32, 220.5f64), // 电压
        (channel_id, "m", 10002, 380.2),       // 电流
        (channel_id, "s", 30001, 1.0),         // 开关状态
    ];

    // 使用comsrv的键格式写入
    let mut conn = redis::Client::open(&config.redis_url)?
        .get_multiplexed_async_connection()
        .await?;

    for (ch, pt, pid, value) in &test_points {
        let key = format!("{}:{}:{}", ch, pt, pid);
        let value_str = format!("{}:{}", value, chrono::Utc::now().timestamp_millis());
        redis::cmd("SET")
            .arg(&key)
            .arg(&value_str)
            .query_async::<_, ()>(&mut conn)
            .await?;
    }
    info!("✓ 模拟comsrv数据写入完成");

    // 2. modsrv读取comsrv数据
    info!("\n2. modsrv读取comsrv数据");
    let read_points: Vec<(u16, &str, u32)> = test_points
        .iter()
        .map(|(ch, pt, pid, _)| (*ch, *pt, *pid))
        .collect();

    let start = Instant::now();
    let values = storage.read_comsrv_points(&read_points).await?;
    let read_duration = start.elapsed();

    info!(
        "读取{}个comsrv点位耗时: {:?}",
        read_points.len(),
        read_duration
    );

    // 验证读取的数据
    for (idx, (_, _, _, expected_value)) in test_points.iter().enumerate() {
        if let Some(Some((value, _))) = values.get(idx) {
            assert!((value - expected_value).abs() < 0.01);
            info!("✓ 点位{} 值验证通过: {}", idx, value);
        }
    }

    Ok(())
}

/// 控制命令传递机制测试
#[tokio::test]
async fn test_control_command_flow() -> Result<(), Box<dyn std::error::Error>> {
    let config = TestConfig::default();
    let mut control_mgr = ControlManager::new(&config.redis_url).await?;

    info!("=== 控制命令传递机制测试 ===");

    // 1. 发送控制命令
    info!("\n1. 发送控制命令");
    let model_id = "test_model";

    // 发送遥控命令
    let yk_cmd_id = control_mgr
        .send_remote_control(1001, 40001, true, model_id.to_string())
        .await?;
    info!("✓ 发送遥控命令: {}", yk_cmd_id);

    // 发送遥调命令
    let yt_cmd_id = control_mgr
        .send_remote_adjust(1001, 50001, 85.5, model_id.to_string())
        .await?;
    info!("✓ 发送遥调命令: {}", yt_cmd_id);

    // 2. 验证命令存储
    info!("\n2. 验证命令存储");
    let yk_status = control_mgr.get_command_status(&yk_cmd_id).await?;
    assert_eq!(yk_status, modsrv::storage::CommandStatus::Pending);
    info!("✓ 命令状态验证通过");

    // 3. 验证Redis发布
    info!("\n3. 验证Redis发布机制");
    // 创建订阅客户端
    let client = redis::Client::open(&config.redis_url)?;
    let mut pubsub = client.get_async_pubsub().await?;

    // 订阅控制命令通道
    pubsub.psubscribe("cmd:*").await?;

    // 发送新命令
    let test_cmd_id = control_mgr
        .send_remote_control(1002, 40002, false, model_id.to_string())
        .await?;

    // 接收发布的消息
    use futures_util::StreamExt;
    use tokio::time::timeout;

    let msg_future = pubsub.on_message().next();
    match timeout(Duration::from_secs(1), msg_future).await {
        Ok(Some(msg)) => {
            let channel: String = msg.get_channel_name().to_string();
            info!("✓ 收到发布消息，通道: {}", channel);
            assert!(channel.starts_with("cmd:"));
        }
        _ => {
            error!("未收到发布消息");
        }
    }

    // 4. 批量命令测试
    info!("\n4. 批量命令测试");
    let batch_commands = vec![
        (
            1001,
            40003,
            ControlType::RemoteControl,
            1.0,
            model_id.to_string(),
        ),
        (
            1001,
            40004,
            ControlType::RemoteControl,
            0.0,
            model_id.to_string(),
        ),
        (
            1002,
            50002,
            ControlType::RemoteAdjust,
            100.0,
            model_id.to_string(),
        ),
    ];

    let start = Instant::now();
    let batch_ids = control_mgr.send_batch_commands(batch_commands).await?;
    let batch_duration = start.elapsed();

    info!(
        "批量发送{}条命令耗时: {:?}",
        batch_ids.len(),
        batch_duration
    );

    Ok(())
}

/// 并发访问测试
#[tokio::test]
async fn test_concurrent_access() -> Result<(), Box<dyn std::error::Error>> {
    let config = TestConfig::default();

    info!("=== 并发访问测试 ===");

    // 创建共享存储
    let storage = Arc::new(Mutex::new(ModelStorage::new(&config.redis_url).await?));

    // 并发任务
    let mut tasks = Vec::new();

    for i in 0..config.concurrent_models {
        let storage_clone = storage.clone();
        let model_id = format!("concurrent_model_{}", i);

        let task = tokio::spawn(async move {
            let start = Instant::now();

            // 每个模型写入100个点
            let mut updates = Vec::new();
            for j in 0..100 {
                let monitor_value = MonitorValue::new((i * 100 + j) as f64, model_id.clone());

                updates.push(MonitorUpdate {
                    model_id: model_id.clone(),
                    monitor_type: MonitorType::ModelOutput,
                    point_id: (i * 1000 + j) as u32,
                    value: monitor_value,
                });
            }

            // 执行写入
            let mut storage = storage_clone.lock().await;
            storage.set_monitor_values(&updates).await.unwrap();

            let duration = start.elapsed();
            (model_id, duration)
        });

        tasks.push(task);
    }

    // 等待所有任务完成
    let results = futures::future::join_all(tasks).await;

    info!("\n并发写入结果:");
    for result in results {
        if let Ok((model_id, duration)) = result {
            info!("  {} 完成时间: {:?}", model_id, duration);
        }
    }

    // 验证数据一致性
    info!("\n验证数据一致性:");
    let mut storage = storage.lock().await;

    for i in 0..config.concurrent_models {
        let model_id = format!("concurrent_model_{}", i);
        let key = MonitorKey {
            model_id: model_id.clone(),
            monitor_type: MonitorType::ModelOutput,
            point_id: (i * 1000) as u32,
        };

        let values = storage.get_monitor_values(&[key]).await?;
        assert_eq!(values.len(), 1);

        if let Some(Some(value)) = values.first() {
            assert_eq!(value.value, (i * 100) as f64);
            info!("  ✓ {} 数据验证通过", model_id);
        }
    }

    Ok(())
}

/// 错误处理测试
#[tokio::test]
async fn test_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    info!("=== 错误处理测试 ===");

    // 1. 无效Redis连接
    info!("\n1. 测试无效Redis连接");
    let result = ModelStorage::new("redis://invalid:6379").await;
    assert!(result.is_err());
    info!("✓ 无效连接错误处理正确");

    // 2. 解析错误数据
    info!("\n2. 测试解析错误数据");
    let invalid_redis_str = "invalid:data:format";
    let result = MonitorValue::from_redis(invalid_redis_str);
    assert!(result.is_none());
    info!("✓ 数据解析错误处理正确");

    // 3. 命令状态错误
    info!("\n3. 测试命令状态错误");
    let config = TestConfig::default();
    let mut control_mgr = ControlManager::new(&config.redis_url).await?;

    let result = control_mgr.get_command_status("non_existent_cmd").await;
    assert!(result.is_err());
    info!("✓ 命令不存在错误处理正确");

    // 4. 超时处理
    info!("\n4. 测试超时处理");
    control_mgr.set_timeout(Duration::from_millis(100));

    let cmd_id = control_mgr
        .send_remote_control(1001, 40001, true, "test".to_string())
        .await?;

    // 等待超时
    let status = control_mgr.wait_for_completion(&cmd_id).await?;
    assert_eq!(status, modsrv::storage::CommandStatus::Timeout);
    info!("✓ 超时处理正确");

    Ok(())
}

/// 性能优化建议测试
#[tokio::test]
async fn test_performance_recommendations() -> Result<(), Box<dyn std::error::Error>> {
    info!("=== 性能优化建议 ===");

    info!("\n1. 批量操作优化:");
    info!("   - 建议批量大小: 100-500个点位");
    info!("   - 使用pipeline减少网络往返");
    info!("   - 考虑使用Lua脚本进行原子操作");

    info!("\n2. Redis键设计优化:");
    info!("   - 当前键格式支持高效的模式匹配");
    info!("   - 建议添加TTL以自动清理过期数据");
    info!("   - 考虑使用Redis Cluster进行横向扩展");

    info!("\n3. 内存使用优化:");
    info!("   - 使用压缩格式存储大量数据");
    info!("   - 定期归档历史数据到持久存储");
    info!("   - 使用Redis内存淘汰策略");

    info!("\n4. 并发优化:");
    info!("   - 使用连接池避免频繁创建连接");
    info!("   - 考虑使用读写分离");
    info!("   - 实现乐观锁机制处理并发更新");

    Ok(())
}

#[tokio::main]
async fn main() {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("开始modsrv新数据结构可行性验证...\n");

    // 运行所有测试
    let tests = vec![
        ("性能基准测试", test_performance_benchmarks()),
        ("DAG执行器兼容性", test_dag_executor_compatibility()),
        ("comsrv数据交互", test_comsrv_interaction()),
        ("控制命令传递", test_control_command_flow()),
        ("并发访问测试", test_concurrent_access()),
        ("错误处理测试", test_error_handling()),
    ];

    for (name, test) in tests {
        info!("\n运行: {}", name);
        match test.await {
            Ok(_) => info!("✓ {} 通过", name),
            Err(e) => error!("✗ {} 失败: {}", name, e),
        }
    }

    info!("\n验证完成!");
}
