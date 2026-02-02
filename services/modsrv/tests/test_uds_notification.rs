//! UDS 事件通知集成测试
//!
//! 验证 modsrv → comsrv 的 UDS 事件驱动通知机制
//!
//! ## 测试覆盖
//!
//! 1. UDS 通知发送和接收正确性
//! 2. 端到端延迟验证 < 5ms
//! 3. 降级场景：UDS 不可用时不阻塞
//! 4. 双通道并行：UDS 和 TODO 同时工作

#![allow(clippy::disallowed_methods)] // Integration test - unwrap is acceptable

use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use voltage_model::PointType;
use voltage_rtdb::{ShmNotification, ShmNotifier};

mod common;

/// 测试 1: UDS 通知发送和接收正确性
///
/// 验证 ShmNotifier 发送的通知能被正确接收和解析
#[tokio::test]
async fn test_uds_notification_roundtrip() {
    // 1. 创建测试 UDS 路径
    let test_uds_path = "/tmp/voltage-test-roundtrip.sock";
    let _ = std::fs::remove_file(test_uds_path);

    // 2. 启动本地 UDS 监听器
    let listener = UnixListener::bind(test_uds_path).unwrap();
    let (tx, mut rx) = mpsc::channel::<ShmNotification>(10);

    let listener_handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; ShmNotification::SIZE];
        while stream.read_exact(&mut buf).await.is_ok() {
            let notif = ShmNotification::from_bytes(&buf);
            if tx.send(notif).await.is_err() {
                break;
            }
        }
    });

    // 等待监听器启动
    tokio::time::sleep(Duration::from_millis(10)).await;

    // 3. 创建 ShmNotifier 并连接
    let mut notifier = ShmNotifier::connect(test_uds_path).await.unwrap();
    assert!(notifier.is_connected(), "Notifier should be connected");

    // 4. 发送 Control 通知
    let result = notifier.notify(1001, PointType::Control, 42).await;
    assert!(result.uds_sent, "UDS notification should be sent");

    // 5. 验证接收
    let received = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("Timeout waiting for notification")
        .expect("Channel closed unexpectedly");

    assert_eq!(received.channel_id, 1001);
    assert_eq!(received.point_id, 42);
    assert_eq!(
        received.get_point_type(),
        Some(PointType::Control),
        "Point type should be Control"
    );

    // 6. 发送 Adjustment 通知
    let result = notifier.notify(2002, PointType::Adjustment, 99).await;
    assert!(result.uds_sent, "UDS notification should be sent");

    let received = tokio::time::timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("Timeout waiting for notification")
        .expect("Channel closed unexpectedly");

    assert_eq!(received.channel_id, 2002);
    assert_eq!(received.point_id, 99);
    assert_eq!(
        received.get_point_type(),
        Some(PointType::Adjustment),
        "Point type should be Adjustment"
    );

    // 清理
    listener_handle.abort();
    let _ = std::fs::remove_file(test_uds_path);
}

/// 测试 2: 端到端延迟验证 < 5ms
///
/// 发送 100 个通知，验证平均延迟在 5ms 以内
#[tokio::test]
async fn test_uds_latency_under_5ms() {
    let test_uds_path = "/tmp/voltage-latency-test.sock";
    let _ = std::fs::remove_file(test_uds_path);

    let listener = UnixListener::bind(test_uds_path).unwrap();
    let (tx, mut rx) = mpsc::channel::<Instant>(100);

    let listener_handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; ShmNotification::SIZE];
        while stream.read_exact(&mut buf).await.is_ok() {
            if tx.send(Instant::now()).await.is_err() {
                break;
            }
        }
    });

    // 等待监听器启动
    tokio::time::sleep(Duration::from_millis(10)).await;

    let mut notifier = ShmNotifier::connect(test_uds_path).await.unwrap();
    assert!(notifier.is_connected());

    // 发送 100 个通知，测量延迟
    let mut latencies = vec![];
    for i in 0..100u32 {
        let start = Instant::now();
        let result = notifier.notify(1001, PointType::Adjustment, i).await;
        assert!(result.uds_sent, "UDS notification should be sent");
        let recv_time = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("Timeout")
            .expect("Channel closed");
        latencies.push(recv_time.duration_since(start));
    }

    let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
    let max_latency = latencies.iter().max().unwrap();
    let min_latency = latencies.iter().min().unwrap();

    println!("=== UDS Latency Results ===");
    println!("  Samples: {}", latencies.len());
    println!("  Min latency: {:?}", min_latency);
    println!("  Avg latency: {:?}", avg_latency);
    println!("  Max latency: {:?}", max_latency);

    assert!(
        avg_latency < Duration::from_millis(5),
        "Average latency {:?} exceeded 5ms threshold",
        avg_latency
    );

    listener_handle.abort();
    let _ = std::fs::remove_file(test_uds_path);
}

/// 测试 3: 降级场景 - UDS 不可用时不阻塞
///
/// 当 UDS 连接失败时，ShmNotifier 应该优雅降级，不 panic
#[tokio::test]
async fn test_uds_graceful_degradation() {
    // 连接不存在的 UDS 路径
    let notifier = ShmNotifier::connect("/tmp/nonexistent-socket-12345.sock")
        .await
        .unwrap();

    // 应该返回成功但未连接的 notifier
    assert!(
        !notifier.is_connected(),
        "Should not be connected to non-existent socket"
    );

    // 发送通知应该静默成功（因为 stream 为 None，会使用 fallback/disabled）
    let mut notifier = notifier;
    let result = notifier.notify(1001, PointType::Control, 1).await;
    // When not connected, UDS is disabled - notify still "succeeds" but doesn't send
    assert!(
        !result.uds_sent || result.disabled,
        "notify() should indicate not sent or disabled when not connected"
    );
}

/// 测试 4: 多通知批量发送
///
/// 验证批量发送多个通知的正确性
#[tokio::test]
async fn test_uds_batch_notifications() {
    let test_uds_path = "/tmp/voltage-batch-test.sock";
    let _ = std::fs::remove_file(test_uds_path);

    let listener = UnixListener::bind(test_uds_path).unwrap();
    let (tx, mut rx) = mpsc::channel::<ShmNotification>(100);

    let listener_handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; ShmNotification::SIZE];
        while stream.read_exact(&mut buf).await.is_ok() {
            let notif = ShmNotification::from_bytes(&buf);
            if tx.send(notif).await.is_err() {
                break;
            }
        }
    });

    tokio::time::sleep(Duration::from_millis(10)).await;

    let mut notifier = ShmNotifier::connect(test_uds_path).await.unwrap();

    // 批量发送 50 个通知
    let batch_size = 50u32;
    for i in 0..batch_size {
        let point_type = if i % 2 == 0 {
            PointType::Control
        } else {
            PointType::Adjustment
        };
        let result = notifier.notify(1000 + i, point_type, i * 10).await;
        assert!(result.uds_sent, "UDS notification {} should be sent", i);
    }

    // 验证接收
    let mut received_count = 0u32;
    while let Ok(Some(notif)) = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
        assert_eq!(notif.channel_id, 1000 + received_count);
        assert_eq!(notif.point_id, received_count * 10);
        received_count += 1;
        if received_count >= batch_size {
            break;
        }
    }

    assert_eq!(
        received_count, batch_size,
        "Should receive all {} notifications",
        batch_size
    );

    listener_handle.abort();
    let _ = std::fs::remove_file(test_uds_path);
}

/// 测试 5: 双通道并行 - UDS 不可用时 TODO 队列正常工作
///
/// 验证当 UDS 不可用时，TODO 队列（备用通道）仍然正常工作
#[tokio::test]
async fn test_dual_channel_fallback_to_todo() {
    use common::routing::{assert_todo_queue_triggered, setup_m2c_routing};
    use voltage_routing::set_action_point;

    // 设置测试环境
    let (rtdb, routing_cache) =
        setup_m2c_routing(vec![("23:A:1", "1001:A:1")], vec![("inverter_01", 23)]).await;

    // 不使用 UDS（模拟 UDS 不可用），验证 TODO 队列工作
    let outcome = set_action_point(
        rtdb.as_ref(),
        &routing_cache,
        23,   // instance_id
        "1",  // point_id
        99.0, // value
        None, // shm_writer
        None, // notifier - UDS 不可用
    )
    .await
    .unwrap();

    assert!(outcome.routed, "Action should be routed");
    assert_eq!(
        outcome.route_result,
        Some("1001".to_string()),
        "Should route to channel 1001"
    );

    // 验证 TODO 队列有消息（备用通道工作正常）
    assert_todo_queue_triggered(&rtdb, "comsrv:1001:A:TODO").await;
}

/// 测试 6: ShmNotification 序列化/反序列化
///
/// 验证 12 字节通知协议的正确性
#[test]
fn test_notification_serialization() {
    let notification = ShmNotification::new(12345, PointType::Control, 67890);

    // 验证字段
    assert_eq!(notification.channel_id, 12345);
    assert_eq!(notification.point_id, 67890);
    assert_eq!(notification.get_point_type(), Some(PointType::Control));

    // 序列化
    let bytes = notification.to_bytes();
    assert_eq!(bytes.len(), ShmNotification::SIZE);
    assert_eq!(bytes.len(), 12, "Notification should be exactly 12 bytes");

    // 反序列化
    let restored = ShmNotification::from_bytes(&bytes);
    assert_eq!(restored.channel_id, 12345);
    assert_eq!(restored.point_id, 67890);
    assert_eq!(restored.get_point_type(), Some(PointType::Control));
}

/// 测试 7: 不同 PointType 的序列化
#[test]
fn test_notification_point_types() {
    // Control
    let control = ShmNotification::new(1, PointType::Control, 1);
    assert_eq!(control.get_point_type(), Some(PointType::Control));

    // Adjustment
    let adjustment = ShmNotification::new(1, PointType::Adjustment, 1);
    assert_eq!(adjustment.get_point_type(), Some(PointType::Adjustment));

    // 往返测试
    let bytes = control.to_bytes();
    let restored = ShmNotification::from_bytes(&bytes);
    assert_eq!(restored.get_point_type(), Some(PointType::Control));

    let bytes = adjustment.to_bytes();
    let restored = ShmNotification::from_bytes(&bytes);
    assert_eq!(restored.get_point_type(), Some(PointType::Adjustment));
}
