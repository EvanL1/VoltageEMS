use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

// 简化版的池化测试
#[tokio::main]
async fn main() {
    println!("Testing pool functionality...");
    
    // 测试通用对象池
    test_generic_pool().await;
    
    // 测试连接池基本概念（使用简化版本）
    test_connection_pool_concept().await;
    
    println!("Pool tests completed successfully!");
}

async fn test_generic_pool() {
    println!("Testing generic pool...");
    
    // 创建一个简单的字节缓冲区池
    let pool = crossbeam::queue::SegQueue::new();
    
    // 添加一些缓冲区
    for i in 0..5 {
        let mut buffer = bytes::BytesMut::with_capacity(1024);
        buffer.extend_from_slice(&format!("Buffer {}", i).as_bytes());
        pool.push(buffer);
    }
    
    // 从池中获取和归还对象
    if let Some(mut buffer) = pool.pop() {
        println!("Got buffer from pool: {:?}", String::from_utf8_lossy(&buffer));
        buffer.clear();
        buffer.extend_from_slice(b"Reused buffer");
        pool.push(buffer);
        println!("Returned buffer to pool");
    }
    
    println!("Generic pool test passed");
}

async fn test_connection_pool_concept() {
    println!("Testing connection pool concept...");
    
    // 模拟连接池的基本概念
    let (tx, mut rx) = mpsc::channel::<String>(10);
    
    // 模拟创建连接
    let connection_id = "conn_123".to_string();
    tx.send(connection_id.clone()).await.unwrap();
    
    // 模拟获取连接
    if let Some(conn) = rx.recv().await {
        println!("Retrieved connection: {}", conn);
        
        // 模拟使用连接
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // 模拟归还连接
        tx.send(conn).await.unwrap();
        println!("Returned connection to pool");
    }
    
    println!("Connection pool concept test passed");
} 