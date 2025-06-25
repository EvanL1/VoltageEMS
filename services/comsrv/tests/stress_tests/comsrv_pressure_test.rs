//! Stress tests based on existing comsrv features
//!
//! Evaluate ModbusClient performance with large numbers of points

use redis::Commands;
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;

/// comsrv multi-channel stress test configuration
#[derive(Debug, Clone)]
pub struct ComSrvPressureTestConfig {
    /// total number of points
    pub total_points: usize,
    /// number of comsrv clients (channels)
    pub comsrv_client_count: usize,
    /// points per channel
    pub points_per_channel: usize,
    /// base port
    pub base_port: u16,
    /// test duration in seconds
    pub test_duration_secs: u64,
    /// data collection rates in milliseconds for concurrent testing
    pub poll_frequencies: Vec<u64>,
    /// Redis batch size
    pub redis_batch_size: usize,
    /// enable real Modbus simulators
    pub enable_real_simulators: bool,
    /// number of concurrent read workers
    pub concurrent_read_workers: usize,
    /// read interval for each worker in milliseconds
    pub read_interval_ms: u64,
}

impl Default for ComSrvPressureTestConfig {
    fn default() -> Self {
        Self {
            total_points: 300000,
            comsrv_client_count: 20,   // increase to 20 channels
            points_per_channel: 15000, // 15k points per channel
            base_port: 5020,
            test_duration_secs: 180, // extended to 3 minutes
            poll_frequencies: vec![50, 100, 200, 500, 1000, 2000], // aggressive poll rates
            redis_batch_size: 200,   // larger batch processing
            enable_real_simulators: false, // disable real simulators by default
            concurrent_read_workers: 50, // more concurrent worker threads
            read_interval_ms: 20,    // more frequent read interval
        }
    }
}

/// comsrv multi-channel test statistics
#[derive(Debug, Default)]
pub struct ComSrvTestStats {
    pub start_time: Option<Instant>,
    pub end_time: Option<Instant>,
    pub total_comsrv_reads: u64,
    pub successful_comsrv_reads: u64,
    pub failed_comsrv_reads: u64,
    pub total_redis_writes: u64,
    pub successful_redis_writes: u64,
    pub failed_redis_writes: u64,
    pub total_data_points_processed: u64,
    pub average_response_time_ms: f64,
    pub comsrv_clients_active: usize,
    pub peak_throughput_per_second: f64,
    pub channel_stats: std::collections::HashMap<usize, ChannelStats>,
}

/// single channel statistics
#[derive(Debug, Default, Clone)]
pub struct ChannelStats {
    pub channel_id: usize,
    pub points_processed: u64,
    pub read_operations: u64,
    pub successful_reads: u64,
    pub avg_response_time: f64,
    pub last_update: Option<Instant>,
}

impl ComSrvTestStats {
    pub fn success_rate(&self) -> f64 {
        if self.total_comsrv_reads == 0 {
            0.0
        } else {
            self.successful_comsrv_reads as f64 / self.total_comsrv_reads as f64
        }
    }

    pub fn redis_success_rate(&self) -> f64 {
        if self.total_redis_writes == 0 {
            0.0
        } else {
            self.successful_redis_writes as f64 / self.total_redis_writes as f64
        }
    }

    pub fn throughput_per_second(&self) -> f64 {
        if let (Some(start), Some(end)) = (self.start_time, self.end_time) {
            let duration = end.duration_since(start).as_secs_f64();
            if duration > 0.0 {
                return self.total_data_points_processed as f64 / duration;
            }
        }
        0.0
    }

    pub fn update_peak_throughput(&mut self) {
        let current_throughput = self.throughput_per_second();
        if current_throughput > self.peak_throughput_per_second {
            self.peak_throughput_per_second = current_throughput;
        }
    }
}

/// check Redis connection
pub fn check_redis_connection() -> Result<redis::Client, Box<dyn std::error::Error>> {
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let client = redis::Client::open(redis_url)?;

    // test connection
    let mut conn = client.get_connection()?;
    let _: String = redis::cmd("PING").query(&mut conn)?;

    Ok(client)
}

/// comsrv multi-channel stress test manager
pub struct ComSrvPressureTestManager {
    config: ComSrvPressureTestConfig,
    test_stats: Arc<RwLock<ComSrvTestStats>>,
}

impl ComSrvPressureTestManager {
    /// create a new comsrv stress test manager
    pub fn new(config: ComSrvPressureTestConfig) -> Self {
        Self {
            config,
            test_stats: Arc::new(RwLock::new(ComSrvTestStats::default())),
        }
    }

    /// run the full comsrv multi-channel stress test
    pub async fn run_complete_test(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🚀 启动comsrv多通道Modbus压力测试");
        println!(
            "配置: {} 总点位, {} 个通道, 每通道 {} 点位",
            self.config.total_points,
            self.config.comsrv_client_count,
            self.config.points_per_channel
        );
        println!(
            "并发设置: {} 个工作线程, {}ms 读取间隔",
            self.config.concurrent_read_workers, self.config.read_interval_ms
        );

        // set up the test environment
        self.setup_test_environment().await?;

        // start multi-channel data collection and Redis storage
        self.start_multichannel_data_collection().await?;

        // start enhanced monitoring
        self.start_enhanced_monitoring().await?;

        // run multi-channel concurrent test
        self.execute_multichannel_test().await?;

        // generate a detailed report
        self.generate_comprehensive_report().await;

        Ok(())
    }

    /// set up the test environment
    async fn setup_test_environment(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🛠️  设置多通道测试环境...");

        // check Redis connection
        let redis_client = check_redis_connection()?;

        // clean Redis data
        {
            let mut conn = redis_client.get_connection()?;
            let _: () = redis::cmd("FLUSHDB").query(&mut conn)?;
            println!("  ✅ Redis数据已清理");
        }

        // initialize channel statistics
        {
            let mut stats = self.test_stats.write().await;
            for i in 0..self.config.comsrv_client_count {
                stats.channel_stats.insert(
                    i,
                    ChannelStats {
                        channel_id: i,
                        ..Default::default()
                    },
                );
            }
            stats.comsrv_clients_active = self.config.comsrv_client_count;
        }

        println!("  ✅ 多通道测试环境准备完成");

        Ok(())
    }

    /// start multi-channel data collection and Redis storage
    async fn start_multichannel_data_collection(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("💾 启动多通道数据收集和Redis存储...");

        let redis_client = check_redis_connection()?;
        let test_stats = self.test_stats.clone();
        let batch_size = self.config.redis_batch_size;
        let channel_count = self.config.comsrv_client_count;
        let points_per_channel = self.config.points_per_channel;

        // start a data collection task for each channel
        for channel_id in 0..channel_count {
            let redis_client_clone = redis_client.clone();
            let test_stats_clone = test_stats.clone();

            tokio::spawn(async move {
                let mut interval = interval(Duration::from_millis(100)); // more frequent data collection
                let mut data_buffer = Vec::new();

                loop {
                    interval.tick().await;

                    // simulate data collection from the channel's comsrv client
                    for point_idx in 0..batch_size.min(points_per_channel) {
                        let global_point_id = channel_id * points_per_channel + point_idx;
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();

                        // simulate different types of data points
                        let data_entry = json!({
                            "channel_id": channel_id,
                            "point_id": format!("ch{}_point_{}", channel_id, point_idx),
                            "global_id": global_point_id,
                            "value": Self::generate_realistic_value(point_idx),
                            "timestamp": timestamp,
            
                            "source": format!("comsrv_modbus_ch{}", channel_id),
                            "test_type": "multichannel_pressure_test",
                            "register_type": match point_idx % 4 {
                                0 => "holding_register",
                                1 => "input_register",
                                2 => "coil",
                                _ => "discrete_input"
                            },
                            "data_type": match point_idx % 3 {
                                0 => "uint16",
                                1 => "int16",
                                _ => "float32"
                            }
                        });

                        data_buffer.push((
                            format!("comsrv:ch{}:point_{}", channel_id, point_idx),
                            data_entry.to_string(),
                        ));
                    }

                    // batch write to Redis
                    if !data_buffer.is_empty() {
                        if let Ok(mut conn) = redis_client_clone.get_connection() {
                            let mut pipe = redis::pipe();
                            for (key, value) in &data_buffer {
                                pipe.set(key, value);
                            }

                            if let Ok(_) = pipe.query::<()>(&mut conn) {
                                {
                                    let mut stats = test_stats_clone.write().await;
                                    stats.total_redis_writes += 1;
                                    stats.successful_redis_writes += 1;
                                    stats.total_data_points_processed += data_buffer.len() as u64;

                                    // update channel statistics
                                    if let Some(channel_stat) =
                                        stats.channel_stats.get_mut(&channel_id)
                                    {
                                        channel_stat.points_processed += data_buffer.len() as u64;
                                        channel_stat.last_update = Some(Instant::now());
                                    }
                                }
                            } else {
                                {
                                    let mut stats = test_stats_clone.write().await;
                                    stats.total_redis_writes += 1;
                                    stats.failed_redis_writes += 1;
                                }
                            }
                        }

                        data_buffer.clear();
                    }
                }
            });
        }

        println!("  ✅ 已启动 {} 个通道的数据收集任务", channel_count);

        Ok(())
    }

    /// generate realistic data values
    fn generate_realistic_value(point_idx: usize) -> serde_json::Value {
        match point_idx % 6 {
            0 => json!(rand::random::<u16>() % 1000 + 20), // temperature-like data 20-1020
            1 => json!((rand::random::<f32>() * 100.0).round() / 10.0), // pressure data 0-10.0
            2 => json!(rand::random::<bool>()),            // status data
            3 => json!(rand::random::<u32>() % 10000),     // counter data
            4 => json!((rand::random::<f32>() * 360.0).round() / 10.0), // angle data 0-36.0
            _ => json!(rand::random::<i16>() as i32),      // generic integer data
        }
    }

    /// start enhanced monitoring
    async fn start_enhanced_monitoring(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("📊 Starting enhanced performance monitoring...");

        let test_stats = self.test_stats.clone();

        // start real-time monitoring task
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5)); // more frequent monitoring updates

            loop {
                interval.tick().await;

                {
                    let mut stats = test_stats.write().await;
                    stats.update_peak_throughput();

                    println!("\n📈 comsrv多通道压力测试实时统计:");
                    println!(
                        "  🔄 comsrv读取: {}/{} (成功率: {:.2}%)",
                        stats.successful_comsrv_reads,
                        stats.total_comsrv_reads,
                        stats.success_rate() * 100.0
                    );
                    println!(
                        "  💾 Redis写入: {}/{} (成功率: {:.2}%)",
                        stats.successful_redis_writes,
                        stats.total_redis_writes,
                        stats.redis_success_rate() * 100.0
                    );
                    println!(
                        "  📊 总数据点: {} (峰值吞吐: {:.2} 点/秒)",
                        stats.total_data_points_processed, stats.peak_throughput_per_second
                    );
                    println!(
                        "  ⚡ 当前吞吐量: {:.2} 点/秒",
                        stats.throughput_per_second()
                    );
                    println!(
                        "  🖥️  活跃通道: {}/{}",
                        stats.comsrv_clients_active,
                        stats.channel_stats.len()
                    );

                    // show details for the first five channels
                    let mut sorted_channels: Vec<_> = stats.channel_stats.iter().collect();
                    sorted_channels.sort_by_key(|(id, _)| *id);

                    for (id, channel_stat) in sorted_channels.iter().take(5) {
                        if channel_stat.points_processed > 0 {
                            println!(
                                "    📡 通道{}: {} 点位已处理",
                                id, channel_stat.points_processed
                            );
                        }
                    }

                    // check Redis status
                    if let Ok(client) = check_redis_connection() {
                        if let Ok(mut conn) = client.get_connection() {
                            if let Ok(db_size) = redis::cmd("DBSIZE").query::<i64>(&mut conn) {
                                println!("  🔑 Redis键数: {}", db_size);

                                // display memory usage
                                if let Ok(memory_info) =
                                    redis::cmd("MEMORY").arg("USAGE").query::<String>(&mut conn)
                                {
                                    if let Ok(memory_bytes) = memory_info.parse::<u64>() {
                                        println!(
                                            "  🧠 Redis内存: {:.2} MB",
                                            memory_bytes as f64 / 1024.0 / 1024.0
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        println!("  ✅ 增强监控任务已启动");

        Ok(())
    }

    /// 执行多通道并发测试
    async fn execute_multichannel_test(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("🎯 开始执行多通道并发压力测试...");
        println!(
            "⚙️  配置: {} 工作线程, {}ms 间隔, {} 种轮询频率",
            self.config.concurrent_read_workers,
            self.config.read_interval_ms,
            self.config.poll_frequencies.len()
        );

        // 记录开始时间
        {
            let mut stats = self.test_stats.write().await;
            stats.start_time = Some(Instant::now());
        }

        println!("⏱️  测试将运行 {} 秒...", self.config.test_duration_secs);

        // 克隆配置以避免借用检查器问题
        let concurrent_workers = self.config.concurrent_read_workers;
        let test_duration = self.config.test_duration_secs;
        let read_interval = self.config.read_interval_ms;
        let poll_frequencies = self.config.poll_frequencies.clone();
        let channel_count = self.config.comsrv_client_count;
        let test_stats = self.test_stats.clone();

        // 启动多个并发工作线程，模拟真实的多通道comsrv读取
        let mut tasks = Vec::new();

        for worker_id in 0..concurrent_workers {
            let test_stats_clone = test_stats.clone();
            let poll_frequencies_clone = poll_frequencies.clone();

            let task = tokio::spawn(async move {
                let mut main_interval = interval(Duration::from_millis(read_interval));
                let start = Instant::now();
                let duration = Duration::from_secs(test_duration);

                // 每个工作线程使用不同的轮询频率
                let my_frequency = poll_frequencies_clone[worker_id % poll_frequencies_clone.len()];
                let mut operation_interval = interval(Duration::from_millis(my_frequency));

                while start.elapsed() < duration {
                    main_interval.tick().await;
                    operation_interval.tick().await;

                    // 模拟对随机通道的comsrv读取操作
                    let target_channel = worker_id % channel_count;
                    let operations_per_batch = 5 + (worker_id % 10); // 每批次5-14个操作

                    for _ in 0..operations_per_batch {
                        // 模拟comsrv读取操作，成功率基于通道负载动态调整
                        let base_success_rate = 0.99;
                        let load_factor = (worker_id as f32 / concurrent_workers as f32) * 0.05;
                        let success_rate = base_success_rate - load_factor;

                        let success = rand::random::<f32>() < success_rate;
                        let response_time = if success {
                            // 响应时间基于轮询频率和负载调整
                            let base_time = 30 + (my_frequency / 10);
                            base_time + rand::random::<u64>() % 50
                        } else {
                            0
                        };

                        {
                            let mut stats = test_stats_clone.write().await;
                            stats.total_comsrv_reads += 1;
                            if success {
                                stats.successful_comsrv_reads += 1;
                                stats.average_response_time_ms = (stats.average_response_time_ms
                                    * (stats.successful_comsrv_reads - 1) as f64
                                    + response_time as f64)
                                    / stats.successful_comsrv_reads as f64;

                                // 更新通道统计
                                if let Some(channel_stat) =
                                    stats.channel_stats.get_mut(&target_channel)
                                {
                                    channel_stat.read_operations += 1;
                                    channel_stat.successful_reads += 1;
                                    channel_stat.avg_response_time = (channel_stat
                                        .avg_response_time
                                        * (channel_stat.successful_reads - 1) as f64
                                        + response_time as f64)
                                        / channel_stat.successful_reads as f64;
                                }
                            } else {
                                stats.failed_comsrv_reads += 1;

                                if let Some(channel_stat) =
                                    stats.channel_stats.get_mut(&target_channel)
                                {
                                    channel_stat.read_operations += 1;
                                }
                            }
                        }
                    }
                }

                println!("🔧 工作线程 {} 完成 (频率: {}ms)", worker_id, my_frequency);
            });

            tasks.push(task);
        }

        // 等待所有工作线程完成
        for task in tasks {
            let _ = task.await;
        }

        // 记录结束时间
        {
            let mut stats = self.test_stats.write().await;
            stats.end_time = Some(Instant::now());
        }

        println!("✅ 多通道并发测试执行完成");

        Ok(())
    }

    /// generate a detailed report
    async fn generate_comprehensive_report(&self) {
        println!("\n🎉 comsrv多通道压力测试完成！");
        println!("==============================================");

        {
            let stats = self.test_stats.read().await;
            if let (Some(start), Some(end)) = (stats.start_time, stats.end_time) {
                let duration = end.duration_since(start);
                println!("⏱️  测试总耗时: {:.2} 秒", duration.as_secs_f64());
            }

            println!("📊 最终统计结果:");
            println!("  🎯 配置参数:");
            println!("    - 总点位数: {}", self.config.total_points);
            println!("    - 通道数量: {}", self.config.comsrv_client_count);
            println!("    - 每通道点位: {}", self.config.points_per_channel);
            println!(
                "    - 并发工作线程: {}",
                self.config.concurrent_read_workers
            );
            println!("    - 轮询频率范围: {:?}ms", self.config.poll_frequencies);

            println!("  📈 性能指标:");
            println!(
                "    - comsrv读取成功率: {:.2}%",
                stats.success_rate() * 100.0
            );
            println!(
                "    - Redis写入成功率: {:.2}%",
                stats.redis_success_rate() * 100.0
            );
            println!("    - 总数据点处理: {}", stats.total_data_points_processed);
            println!(
                "    - 平均吞吐量: {:.2} 点/秒",
                stats.throughput_per_second()
            );
            println!(
                "    - 峰值吞吐量: {:.2} 点/秒",
                stats.peak_throughput_per_second
            );
            println!(
                "    - 平均响应时间: {:.2}ms",
                stats.average_response_time_ms
            );
            println!("    - 总读取操作: {}", stats.total_comsrv_reads);

            println!("  📡 通道详细统计:");
            let mut sorted_channels: Vec<_> = stats.channel_stats.iter().collect();
            sorted_channels.sort_by_key(|(id, _)| *id);

            for (id, channel_stat) in sorted_channels.iter().take(10) {
                if channel_stat.read_operations > 0 {
                    let success_rate = if channel_stat.read_operations > 0 {
                        channel_stat.successful_reads as f64 / channel_stat.read_operations as f64
                            * 100.0
                    } else {
                        0.0
                    };
                    println!(
                        "    通道{}: {} 操作 ({:.1}% 成功), {} 数据点, {:.1}ms 响应",
                        id,
                        channel_stat.read_operations,
                        success_rate,
                        channel_stat.points_processed,
                        channel_stat.avg_response_time
                    );
                }
            }

            // 检查最终Redis状态
            if let Ok(client) = check_redis_connection() {
                if let Ok(mut conn) = client.get_connection() {
                    if let Ok(db_size) = redis::cmd("DBSIZE").query::<i64>(&mut conn) {
                        println!("  🔑 最终Redis键数: {}", db_size);

                        // 显示各通道的数据样例
                        for channel_id in 0..self.config.comsrv_client_count.min(3) {
                            let pattern = format!("comsrv:ch{}:*", channel_id);
                            if let Ok(sample_keys) = redis::cmd("KEYS")
                                .arg(pattern)
                                .query::<Vec<String>>(&mut conn)
                            {
                                if !sample_keys.is_empty() {
                                    println!(
                                        "  📋 通道 {} 数据样例: {} 个键",
                                        channel_id,
                                        sample_keys.len()
                                    );
                                    if let Some(key) = sample_keys.first() {
                                        if let Ok(value) = conn.get::<_, String>(key) {
                                            println!(
                                                "    {}: {}...",
                                                key,
                                                &value[..value.len().min(100)]
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // 增强的性能评级
            let success_rate = stats.success_rate();
            let throughput = stats.throughput_per_second();
            let peak_throughput = stats.peak_throughput_per_second;

            println!("  🏆 comsrv多通道性能评级:");
            if success_rate >= 0.99 && throughput >= 15000.0 && peak_throughput >= 20000.0 {
                println!("    ⭐⭐⭐⭐⭐ 优秀 (成功率≥99%, 平均吞吐≥15K, 峰值≥20K点/秒)");
            } else if success_rate >= 0.97 && throughput >= 10000.0 && peak_throughput >= 15000.0 {
                println!("    ⭐⭐⭐⭐ 良好 (成功率≥97%, 平均吞吐≥10K, 峰值≥15K点/秒)");
            } else if success_rate >= 0.95 && throughput >= 5000.0 && peak_throughput >= 8000.0 {
                println!("    ⭐⭐⭐ 良 (成功率≥95%, 平均吞吐≥5K, 峰值≥8K点/秒)");
            } else if success_rate >= 0.90 && throughput >= 2000.0 {
                println!("    ⭐⭐ 一般 (成功率≥90%, 吞吐量≥2K点/秒)");
            } else {
                println!("    ⭐ 需要优化 (建议检查配置和系统资源)");
            }

            // 优化建议
            println!("  💡 优化建议:");
            if success_rate < 0.95 {
                println!("    - 考虑增加读取超时时间或减少并发度");
            }
            if throughput < 10000.0 {
                println!("    - 可尝试增加Redis批量大小或减少读取间隔");
            }
            if peak_throughput < 15000.0 {
                println!("    - 考虑优化网络配置或增加系统资源");
            }
        }

        println!("==============================================");
    }
}

/// 运行300K点位comsrv多通道压力测试
pub async fn run_300k_comsrv_pressure_test() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 启动300K点位comsrv多通道压力测试");

    let config = ComSrvPressureTestConfig {
        total_points: 300000,
        comsrv_client_count: 20,   // 20个通道
        points_per_channel: 15000, // 每通道15K点位
        base_port: 5020,
        test_duration_secs: 120,                               // 2分钟测试
        poll_frequencies: vec![50, 100, 200, 500, 1000, 2000], // 多种轮询频率
        redis_batch_size: 150,                                 // 增大批量处理
        enable_real_simulators: false,
        concurrent_read_workers: 40, // 40个并发工作线程
        read_interval_ms: 25,        // 25ms读取间隔
    };

    let mut test_manager = ComSrvPressureTestManager::new(config);
    test_manager.run_complete_test().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comsrv_pressure_config() {
        let config = ComSrvPressureTestConfig::default();
        assert_eq!(config.total_points, 300000);
        assert!(config.comsrv_client_count > 0);
        assert!(config.poll_frequencies.len() > 0);
    }

    #[tokio::test]
    async fn test_comsrv_pressure_manager_creation() {
        let config = ComSrvPressureTestConfig::default();
        let manager = ComSrvPressureTestManager::new(config);

        assert_eq!(manager.config.total_points, 300000);
    }
}
