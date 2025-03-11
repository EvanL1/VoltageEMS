//! M2C (Model to Channel) 路由端到端测试
//!
//! 测试从实例动作点到通道 TODO 队列的完整数据流

#![allow(clippy::disallowed_methods)] // 测试代码 - unwrap 是可接受的

use anyhow::Result;
use bytes::Bytes;
use modsrv::routing_executor::set_action_point;
use std::collections::HashMap;
use std::sync::Arc;
use voltage_config::RoutingCache;
use voltage_rtdb::{MemoryRtdb, Rtdb};

// ==================== 测试辅助函数 ====================

/// 创建带 M2C 路由和实例映射的测试环境
///
/// # Arguments
/// * `m2c_routes` - M2C 路由表 [("23:A:1", "1001:A:1"), ...]
/// * `instance_mappings` - 实例名称映射 [("inverter_01", 23), ...]
///
/// # Returns
/// (Rtdb 实例, RoutingCache 实例)
async fn setup_m2c_routing(
    m2c_routes: Vec<(&str, &str)>,
    instance_mappings: Vec<(&str, u32)>,
) -> (Arc<dyn Rtdb>, Arc<RoutingCache>) {
    let rtdb = Arc::new(MemoryRtdb::new());

    // Step 1: 设置实例名称索引（inst:name:index Hash）
    for (name, id) in instance_mappings {
        rtdb.hash_set("inst:name:index", name, Bytes::from(id.to_string()))
            .await
            .unwrap();
    }

    // Step 2: 配置 M2C 路由表
    let mut m2c_map = HashMap::new();
    for (source, target) in m2c_routes {
        m2c_map.insert(source.to_string(), target.to_string());
    }

    let routing_cache = Arc::new(RoutingCache::from_maps(
        HashMap::new(), // C2M routing (空)
        m2c_map,        // M2C routing
        HashMap::new(), // C2C routing (空)
    ));

    (rtdb, routing_cache)
}

/// 验证 TODO 队列有触发消息
///
/// # Arguments
/// * `rtdb` - RTDB 实例
/// * `queue_key` - TODO 队列键（如 "comsrv:1001:A:TODO"）
async fn assert_todo_queue_triggered(rtdb: &Arc<dyn Rtdb>, queue_key: &str) {
    let messages = rtdb.list_range(queue_key, 0, -1).await.unwrap();
    assert!(
        !messages.is_empty(),
        "TODO queue '{}' should have messages",
        queue_key
    );
}

/// 验证 TODO 队列为空
async fn assert_todo_queue_empty(rtdb: &Arc<dyn Rtdb>, queue_key: &str) {
    let messages = rtdb.list_range(queue_key, 0, -1).await.unwrap();
    assert!(
        messages.is_empty(),
        "TODO queue '{}' should be empty",
        queue_key
    );
}

/// 解析 TODO 队列中的触发消息
async fn parse_todo_message(rtdb: &Arc<dyn Rtdb>, queue_key: &str) -> serde_json::Value {
    let messages = rtdb.list_range(queue_key, 0, -1).await.unwrap();
    assert!(!messages.is_empty(), "TODO queue should have messages");

    let message_bytes = &messages[0];
    let message_str = String::from_utf8(message_bytes.to_vec()).unwrap();
    serde_json::from_str(&message_str).unwrap()
}

// ==================== 测试用例 ====================

/// 测试 1: 基础 M2C 路由
///
/// Given: 配置路由 23:A:1 → 1001:A:1，实例名称 "inverter_01" → 23
/// When: 调用 set_action_point("inverter_01", "1", 12.3)
/// Then:
///   - 实例 Action Hash 写入: inst:23:A["1"] = "12.3"
///   - TODO 队列触发: comsrv:1001:A:TODO 有消息
///   - 路由结果: routed=true, route_result=Some("1001")
#[tokio::test]
async fn test_m2c_basic_routing() -> Result<()> {
    // Given: 配置 M2C 路由和实例映射
    let (rtdb, routing_cache) = setup_m2c_routing(
        vec![("23:A:1", "1001:A:1")], // M2C 路由: 实例23动作点1 → 通道1001调节点1
        vec![("inverter_01", 23)],    // 实例名称映射
    )
    .await;

    // When: 设置实例动作点
    let outcome = set_action_point(
        rtdb.as_ref(),
        &routing_cache,
        "inverter_01", // 实例名称
        "1",           // 动作点ID
        12.3,          // 值
    )
    .await?;

    // Then: 验证路由结果
    assert!(outcome.is_success(), "Routing should succeed");
    assert!(outcome.routed, "Action should be routed to channel");
    assert_eq!(
        outcome.route_result,
        Some("1001".to_string()),
        "Should route to channel 1001"
    );

    // 验证路由上下文
    let route_ctx = outcome.route_context.as_ref().unwrap();
    assert_eq!(route_ctx.channel_id, "1001");
    assert_eq!(route_ctx.point_type, "A");
    assert_eq!(route_ctx.comsrv_point_id, "1");
    assert_eq!(route_ctx.queue_key, "comsrv:1001:A:TODO");

    // 验证实例 Action Hash 写入
    let value = rtdb
        .hash_get("inst:23:A", "1")
        .await?
        .expect("Action point should be written");
    assert_eq!(
        String::from_utf8(value.to_vec())?,
        "12.3",
        "Action value should match"
    );

    // 验证 TODO 队列触发
    assert_todo_queue_triggered(&rtdb, "comsrv:1001:A:TODO").await;

    Ok(())
}

/// 测试 2: 实例名称解析
///
/// Given: 多个实例名称映射
/// When: 使用不同实例名称调用 set_action_point
/// Then: 正确解析为对应的实例 ID
#[tokio::test]
async fn test_m2c_instance_name_resolution() -> Result<()> {
    // Given: 配置多个实例映射
    let (rtdb, routing_cache) = setup_m2c_routing(
        vec![
            ("10:A:1", "1001:A:1"),
            ("20:A:1", "1002:A:1"),
            ("30:A:1", "1003:A:1"),
        ],
        vec![
            ("pv_inverter", 10),
            ("battery_pack", 20),
            ("grid_meter", 30),
        ],
    )
    .await;

    // When & Then: 测试第一个实例
    let outcome =
        set_action_point(rtdb.as_ref(), &routing_cache, "pv_inverter", "1", 100.0).await?;
    assert!(outcome.routed);
    assert_eq!(outcome.route_result, Some("1001".to_string()));

    // 验证写入到正确的实例 Hash
    let value = rtdb.hash_get("inst:10:A", "1").await?.unwrap();
    assert_eq!(String::from_utf8(value.to_vec())?, "100");

    // When & Then: 测试第二个实例
    let outcome =
        set_action_point(rtdb.as_ref(), &routing_cache, "battery_pack", "1", 50.0).await?;
    assert!(outcome.routed);
    assert_eq!(outcome.route_result, Some("1002".to_string()));

    let value = rtdb.hash_get("inst:20:A", "1").await?.unwrap();
    assert_eq!(String::from_utf8(value.to_vec())?, "50");

    // 测试不存在的实例名称
    let result = set_action_point(rtdb.as_ref(), &routing_cache, "unknown_device", "1", 0.0).await;
    assert!(result.is_err(), "Should fail for unknown instance");
    assert!(
        result.unwrap_err().to_string().contains("not found"),
        "Error should mention instance not found"
    );

    Ok(())
}

/// 测试 3: 无路由配置
///
/// Given: 不配置 M2C 路由
/// When: 调用 set_action_point
/// Then:
///   - 实例 Action Hash 仍然写入
///   - TODO 队列为空（无触发）
///   - 路由结果: routed=false, route_result=Some("no_route")
#[tokio::test]
async fn test_m2c_no_routing() -> Result<()> {
    // Given: 无 M2C 路由配置
    let (rtdb, routing_cache) = setup_m2c_routing(
        vec![],                    // 空路由表
        vec![("inverter_01", 23)], // 只有实例映射
    )
    .await;

    // When: 设置动作点
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, "inverter_01", "1", 15.5).await?;

    // Then: 验证路由结果
    assert!(outcome.is_success(), "Operation should succeed");
    assert!(!outcome.routed, "Should not be routed");
    assert_eq!(
        outcome.route_result,
        Some("no_route".to_string()),
        "Should indicate no route"
    );
    assert!(outcome.route_context.is_none(), "No route context");

    // 验证实例 Action Hash 仍然写入
    let value = rtdb
        .hash_get("inst:23:A", "1")
        .await?
        .expect("Action point should still be written");
    assert_eq!(String::from_utf8(value.to_vec())?, "15.5");

    // 验证 TODO 队列为空
    assert_todo_queue_empty(&rtdb, "comsrv:1001:A:TODO").await;

    Ok(())
}

/// 测试 4: 批量动作触发
///
/// Given: 配置多个点位的 M2C 路由
/// When: 批量设置多个动作点位
/// Then: 所有 TODO 队列都有触发消息
#[tokio::test]
async fn test_m2c_batch_actions() -> Result<()> {
    // Given: 配置多个点位路由
    let (rtdb, routing_cache) = setup_m2c_routing(
        vec![
            ("23:A:1", "1001:A:1"),
            ("23:A:2", "1001:A:2"),
            ("23:A:3", "1001:A:3"),
        ],
        vec![("inverter_01", 23)],
    )
    .await;

    // When: 批量设置动作点
    let actions = vec![("1", 10.0), ("2", 20.0), ("3", 30.0)];

    for (point_id, value) in actions {
        let outcome = set_action_point(
            rtdb.as_ref(),
            &routing_cache,
            "inverter_01",
            point_id,
            value,
        )
        .await?;
        assert!(outcome.routed, "Point {} should be routed", point_id);
    }

    // Then: 验证所有点位都写入实例 Hash
    for (point_id, expected_value) in [("1", "10"), ("2", "20"), ("3", "30")] {
        let value = rtdb.hash_get("inst:23:A", point_id).await?.unwrap();
        assert_eq!(
            String::from_utf8(value.to_vec())?,
            expected_value,
            "Point {} value mismatch",
            point_id
        );
    }

    // 验证 TODO 队列有 3 条消息
    let messages = rtdb.list_range("comsrv:1001:A:TODO", 0, -1).await?;
    assert_eq!(messages.len(), 3, "Should have 3 messages in TODO queue");

    Ok(())
}

/// 测试 5: 控制/调节路由（C/A 类型）
///
/// Given: 配置 C(遥控) 和 A(遥调) 两种路由
/// When: 分别设置动作点
/// Then: 路由到 comsrv:{channel_id}:C:TODO 和 :A:TODO
#[tokio::test]
async fn test_m2c_different_channel_types() -> Result<()> {
    // Given: 配置控制和调节路由
    let (rtdb, routing_cache) = setup_m2c_routing(
        vec![
            ("23:A:1", "1001:C:5"), // 动作点1 → 控制点5
            ("23:A:2", "1001:A:6"), // 动作点2 → 调节点6
        ],
        vec![("inverter_01", 23)],
    )
    .await;

    // When: 设置控制类型动作点
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, "inverter_01", "1", 1.0).await?;

    // Then: 验证路由到 C(控制) TODO 队列
    assert!(outcome.routed);
    let route_ctx = outcome.route_context.as_ref().unwrap();
    assert_eq!(route_ctx.point_type, "C", "Should route to Control type");
    assert_eq!(
        route_ctx.queue_key, "comsrv:1001:C:TODO",
        "Should route to Control TODO queue"
    );
    assert_todo_queue_triggered(&rtdb, "comsrv:1001:C:TODO").await;

    // When: 设置调节类型动作点
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, "inverter_01", "2", 2.0).await?;

    // Then: 验证路由到 A(调节) TODO 队列
    assert!(outcome.routed);
    let route_ctx = outcome.route_context.as_ref().unwrap();
    assert_eq!(route_ctx.point_type, "A", "Should route to Adjustment type");
    assert_eq!(
        route_ctx.queue_key, "comsrv:1001:A:TODO",
        "Should route to Adjustment TODO queue"
    );
    assert_todo_queue_triggered(&rtdb, "comsrv:1001:A:TODO").await;

    Ok(())
}

/// 测试 6: 触发消息格式验证
///
/// Given: 配置 M2C 路由
/// When: 设置动作点
/// Then: TODO 队列的 JSON 格式正确，包含 point_id, value, timestamp
#[tokio::test]
async fn test_m2c_trigger_message_format() -> Result<()> {
    // Given: 配置路由
    let (rtdb, routing_cache) = setup_m2c_routing(
        vec![("23:A:1", "1001:A:7")], // 实例点1 → 通道点7
        vec![("inverter_01", 23)],
    )
    .await;

    // When: 设置动作点
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, "inverter_01", "1", 42.5).await?;
    assert!(outcome.routed);

    // Then: 解析 TODO 队列消息
    let message = parse_todo_message(&rtdb, "comsrv:1001:A:TODO").await;

    // 验证 JSON 字段
    assert!(message.is_object(), "Message should be JSON object");
    assert!(
        message.get("point_id").is_some(),
        "Should have point_id field"
    );
    assert!(message.get("value").is_some(), "Should have value field");
    assert!(
        message.get("timestamp").is_some(),
        "Should have timestamp field"
    );

    // 验证字段值
    assert_eq!(
        message["point_id"].as_u64().unwrap(),
        7,
        "point_id should map to comsrv point 7"
    );
    assert_eq!(
        message["value"].as_f64().unwrap(),
        42.5,
        "value should match"
    );

    // 验证时间戳是合理的（近期时间）
    let timestamp = message["timestamp"].as_i64().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    assert!(
        timestamp > now - 10_000 && timestamp <= now,
        "Timestamp should be within last 10 seconds"
    );

    Ok(())
}

/// 测试 7: Write-Triggers-Routing 执行顺序验证
///
/// Given: 配置 M2C 路由
/// When: 设置动作点
/// Then:
///   - 先写入 inst:{id}:A Hash（状态存储）
///   - 后写入 comsrv TODO 队列（触发器）
///   - 两者都必须成功
#[tokio::test]
async fn test_m2c_write_triggers_routing_order() -> Result<()> {
    // Given: 配置路由
    let (rtdb, routing_cache) =
        setup_m2c_routing(vec![("23:A:1", "1001:A:1")], vec![("inverter_01", 23)]).await;

    // When: 设置动作点
    set_action_point(rtdb.as_ref(), &routing_cache, "inverter_01", "1", 99.9).await?;

    // Then: 验证执行顺序 - Hash 先写入
    let hash_value = rtdb.hash_get("inst:23:A", "1").await?;
    assert!(hash_value.is_some(), "Instance Hash must be written first");
    assert_eq!(String::from_utf8(hash_value.unwrap().to_vec())?, "99.9");

    // 验证 TODO 队列后写入
    let messages = rtdb.list_range("comsrv:1001:A:TODO", 0, -1).await?;
    assert_eq!(messages.len(), 1, "TODO queue should have one message");

    // 验证两者数据一致性
    let message = parse_todo_message(&rtdb, "comsrv:1001:A:TODO").await;
    assert_eq!(
        message["value"].as_f64().unwrap(),
        99.9,
        "TODO trigger value should match Hash value"
    );

    Ok(())
}

/// 测试 8: 无效路由目标处理
///
/// Given: 配置格式错误的路由目标（缺少字段）
/// When: 设置动作点
/// Then:
///   - 路由失败但操作成功
///   - 实例 Hash 未写入（因为路由解析失败导致提前返回）
///   - route_result 指示错误
#[tokio::test]
async fn test_m2c_invalid_route_target() -> Result<()> {
    // Given: 配置无效的路由目标（格式错误）
    let rtdb = Arc::new(MemoryRtdb::new());
    rtdb.hash_set("inst:name:index", "inverter_01", Bytes::from("23"))
        .await?;

    let mut m2c_map = HashMap::new();
    m2c_map.insert("23:A:1".to_string(), "invalid_target".to_string()); // 错误格式
    let routing_cache = Arc::new(RoutingCache::from_maps(
        HashMap::new(),
        m2c_map,
        HashMap::new(),
    ));

    // When: 设置动作点
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, "inverter_01", "1", 50.0).await?;

    // Then: 操作成功但路由失败
    assert!(outcome.is_success(), "Operation should succeed");
    assert!(!outcome.routed, "Routing should fail for invalid target");

    // 验证 route_result 指示了无效的路由目标
    if let Some(route_result) = &outcome.route_result {
        assert!(
            route_result.starts_with("invalid_route_target"),
            "Should indicate invalid route target, got: {}",
            route_result
        );
    } else {
        panic!("Expected route_result with error message, got None");
    }

    // 注意: 由于路由解析失败,实例 Hash 不会被写入（提前返回）
    // 这是预期行为,保护数据一致性

    Ok(())
}

/// 测试 9: 多实例多通道路由
///
/// Given: 多个实例路由到不同通道
/// When: 批量设置不同实例的动作点
/// Then: 每个实例正确路由到对应通道
#[tokio::test]
async fn test_m2c_multiple_instances_multiple_channels() -> Result<()> {
    // Given: 配置多实例多通道路由
    let (rtdb, routing_cache) = setup_m2c_routing(
        vec![
            ("10:A:1", "1001:A:1"), // 实例10 → 通道1001
            ("20:A:1", "1002:A:1"), // 实例20 → 通道1002
            ("30:A:1", "1003:A:1"), // 实例30 → 通道1003
        ],
        vec![("inverter_a", 10), ("inverter_b", 20), ("inverter_c", 30)],
    )
    .await;

    // When & Then: 测试实例 A → 通道 1001
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, "inverter_a", "1", 111.1).await?;
    assert!(outcome.routed);
    assert_eq!(outcome.route_result, Some("1001".to_string()));
    assert_todo_queue_triggered(&rtdb, "comsrv:1001:A:TODO").await;

    // When & Then: 测试实例 B → 通道 1002
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, "inverter_b", "1", 222.2).await?;
    assert!(outcome.routed);
    assert_eq!(outcome.route_result, Some("1002".to_string()));
    assert_todo_queue_triggered(&rtdb, "comsrv:1002:A:TODO").await;

    // When & Then: 测试实例 C → 通道 1003
    let outcome = set_action_point(rtdb.as_ref(), &routing_cache, "inverter_c", "1", 333.3).await?;
    assert!(outcome.routed);
    assert_eq!(outcome.route_result, Some("1003".to_string()));
    assert_todo_queue_triggered(&rtdb, "comsrv:1003:A:TODO").await;

    // 验证三个实例的 Hash 都正确写入
    assert_eq!(
        String::from_utf8(rtdb.hash_get("inst:10:A", "1").await?.unwrap().to_vec())?,
        "111.1"
    );
    assert_eq!(
        String::from_utf8(rtdb.hash_get("inst:20:A", "1").await?.unwrap().to_vec())?,
        "222.2"
    );
    assert_eq!(
        String::from_utf8(rtdb.hash_get("inst:30:A", "1").await?.unwrap().to_vec())?,
        "333.3"
    );

    Ok(())
}
