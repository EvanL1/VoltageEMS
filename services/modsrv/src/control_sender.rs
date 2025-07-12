//! 控制命令发送器
//!
//! 提供可靠的控制命令发送、状态跟踪和重试机制

use crate::comsrv_interface::{ComSrvInterface, ControlCommand};
use crate::error::{ModelSrvError, Result};
use crate::redis_handler::RedisConnection;
// use serde::{Deserialize, Serialize}; // 不再需要
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

/// 命令发送策略
#[derive(Debug, Clone)]
pub struct SendStrategy {
    /// 命令超时时间
    pub timeout: Duration,
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试间隔
    pub retry_interval: Duration,
    /// 是否等待确认
    pub wait_for_ack: bool,
    /// 批量发送大小
    pub batch_size: Option<usize>,
}

impl Default for SendStrategy {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            max_retries: 3,
            retry_interval: Duration::from_millis(500),
            wait_for_ack: true,
            batch_size: None,
        }
    }
}

/// 命令发送结果
#[derive(Debug, Clone)]
pub struct SendResult {
    pub command_id: String,
    pub success: bool,
    pub status: String,
    pub message: Option<String>,
    pub retry_count: u32,
    pub duration: Duration,
}

/// 批量发送结果
#[derive(Debug)]
pub struct BatchSendResult {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub results: Vec<SendResult>,
    pub duration: Duration,
}

/// 控制命令发送器
pub struct ControlSender {
    interface: ComSrvInterface,
    strategy: SendStrategy,
    pending_commands: Arc<Mutex<HashMap<String, PendingCommand>>>,
    command_history: VecDeque<SendResult>,
    history_limit: usize,
}

/// 待处理命令
#[derive(Debug, Clone)]
struct PendingCommand {
    command: ControlCommand,
    start_time: Instant,
    retry_count: u32,
    last_retry: Option<Instant>,
}

impl ControlSender {
    /// 创建新的控制发送器
    pub fn new(redis: RedisConnection, strategy: SendStrategy) -> Self {
        Self {
            interface: ComSrvInterface::new(redis),
            strategy,
            pending_commands: Arc::new(Mutex::new(HashMap::new())),
            command_history: VecDeque::new(),
            history_limit: 1000,
        }
    }

    /// 发送单个控制命令
    pub fn send_command(
        &mut self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: f64,
    ) -> Result<SendResult> {
        let start = Instant::now();

        // 发送命令
        let command_id = self
            .interface
            .send_control_command(channel_id, point_type, point_id, value)?;

        // 如果不需要等待确认，直接返回
        if !self.strategy.wait_for_ack {
            return Ok(SendResult {
                command_id,
                success: true,
                status: "sent".to_string(),
                message: Some("Command sent without waiting for acknowledgment".to_string()),
                retry_count: 0,
                duration: start.elapsed(),
            });
        }

        // 等待命令完成
        let result = self.wait_with_retry(&command_id)?;

        // 记录到历史
        self.add_to_history(result.clone());

        Ok(result)
    }

    /// 批量发送控制命令
    pub fn send_batch(&mut self, commands: Vec<(u16, &str, u32, f64)>) -> Result<BatchSendResult> {
        let start = Instant::now();
        let total = commands.len();
        let mut results = Vec::new();
        let mut successful = 0;
        let mut failed = 0;

        // 根据批量大小策略发送
        let chunks = match self.strategy.batch_size {
            Some(size) => commands.chunks(size).collect::<Vec<_>>(),
            None => vec![commands.as_slice()],
        };

        for chunk in chunks {
            for &(channel_id, point_type, point_id, value) in chunk {
                match self.send_command(channel_id, point_type, point_id, value) {
                    Ok(result) => {
                        if result.success {
                            successful += 1;
                        } else {
                            failed += 1;
                        }
                        results.push(result);
                    }
                    Err(e) => {
                        error!(
                            "Failed to send command to {}:{}:{}: {}",
                            channel_id, point_type, point_id, e
                        );
                        failed += 1;
                        results.push(SendResult {
                            command_id: String::new(),
                            success: false,
                            status: "error".to_string(),
                            message: Some(e.to_string()),
                            retry_count: 0,
                            duration: Duration::from_secs(0),
                        });
                    }
                }
            }
        }

        Ok(BatchSendResult {
            total,
            successful,
            failed,
            results,
            duration: start.elapsed(),
        })
    }

    /// 发送并确认命令（带自定义超时）
    pub fn send_and_confirm(
        &mut self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: f64,
        timeout: Duration,
    ) -> Result<SendResult> {
        // 临时修改策略
        let original_timeout = self.strategy.timeout;
        self.strategy.timeout = timeout;
        self.strategy.wait_for_ack = true;

        let result = self.send_command(channel_id, point_type, point_id, value);

        // 恢复原始策略
        self.strategy.timeout = original_timeout;

        result
    }

    /// 异步发送命令（不等待结果）
    pub fn send_async(
        &mut self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: f64,
    ) -> Result<String> {
        let command_id = self
            .interface
            .send_control_command(channel_id, point_type, point_id, value)?;

        // 记录为待处理命令
        let command = ControlCommand::new(channel_id, point_type, point_id, value);
        let pending = PendingCommand {
            command,
            start_time: Instant::now(),
            retry_count: 0,
            last_retry: None,
        };

        self.pending_commands
            .lock()
            .unwrap()
            .insert(command_id.clone(), pending);

        Ok(command_id)
    }

    /// 检查异步命令状态
    pub fn check_async_status(&mut self, command_id: &str) -> Result<Option<SendResult>> {
        // 检查命令是否还在待处理列表中
        let pending = self
            .pending_commands
            .lock()
            .unwrap()
            .get(command_id)
            .cloned();

        if let Some(pending_cmd) = pending {
            // 查询命令状态
            if let Some(status) = self.interface.get_command_status(command_id)? {
                match status.status.as_str() {
                    "success" | "failed" => {
                        // 命令完成，从待处理列表移除
                        self.pending_commands.lock().unwrap().remove(command_id);

                        let result = SendResult {
                            command_id: command_id.to_string(),
                            success: status.status == "success",
                            status: status.status,
                            message: status.message,
                            retry_count: pending_cmd.retry_count,
                            duration: pending_cmd.start_time.elapsed(),
                        };

                        self.add_to_history(result.clone());
                        return Ok(Some(result));
                    }
                    _ => {
                        // 命令仍在处理中
                        return Ok(None);
                    }
                }
            }
        }

        Ok(None)
    }

    /// 重试所有失败的命令
    pub fn retry_failed_commands(&mut self) -> Result<Vec<SendResult>> {
        let mut results = Vec::new();
        let pending = self.pending_commands.lock().unwrap().clone();

        for (command_id, mut pending_cmd) in pending {
            // 检查是否需要重试
            if pending_cmd.retry_count >= self.strategy.max_retries {
                continue;
            }

            // 检查重试间隔
            if let Some(last_retry) = pending_cmd.last_retry {
                if last_retry.elapsed() < self.strategy.retry_interval {
                    continue;
                }
            }

            // 执行重试
            info!("Retrying command {}", command_id);
            pending_cmd.retry_count += 1;
            pending_cmd.last_retry = Some(Instant::now());

            match self.interface.send_control_command(
                pending_cmd.command.channel_id,
                &pending_cmd.command.point_type,
                pending_cmd.command.point_id,
                pending_cmd.command.value,
            ) {
                Ok(new_command_id) => {
                    // 更新待处理命令
                    self.pending_commands.lock().unwrap().remove(&command_id);
                    self.pending_commands
                        .lock()
                        .unwrap()
                        .insert(new_command_id, pending_cmd);
                }
                Err(e) => {
                    error!("Failed to retry command {}: {}", command_id, e);

                    let result = SendResult {
                        command_id,
                        success: false,
                        status: "retry_failed".to_string(),
                        message: Some(e.to_string()),
                        retry_count: pending_cmd.retry_count,
                        duration: pending_cmd.start_time.elapsed(),
                    };

                    results.push(result);
                }
            }
        }

        Ok(results)
    }

    /// 获取命令历史
    pub fn get_history(&self) -> Vec<SendResult> {
        self.command_history.iter().cloned().collect()
    }

    /// 获取待处理命令数量
    pub fn pending_count(&self) -> usize {
        self.pending_commands.lock().unwrap().len()
    }

    /// 清理超时的待处理命令
    pub fn cleanup_timeout_commands(&mut self) -> Vec<String> {
        let mut timeout_commands = Vec::new();
        let mut pending = self.pending_commands.lock().unwrap();

        pending.retain(|command_id, pending_cmd| {
            if pending_cmd.start_time.elapsed() > self.strategy.timeout {
                timeout_commands.push(command_id.clone());
                false
            } else {
                true
            }
        });

        timeout_commands
    }

    // ===== 内部方法 =====

    /// 等待命令完成并支持重试
    fn wait_with_retry(&mut self, command_id: &str) -> Result<SendResult> {
        let start = Instant::now();
        let mut retry_count = 0;

        loop {
            match self
                .interface
                .wait_for_command(command_id, self.strategy.timeout)
            {
                Ok(status) => {
                    return Ok(SendResult {
                        command_id: command_id.to_string(),
                        success: status.status == "success",
                        status: status.status,
                        message: status.message,
                        retry_count,
                        duration: start.elapsed(),
                    });
                }
                Err(ModelSrvError::TimeoutError(_)) if retry_count < self.strategy.max_retries => {
                    retry_count += 1;
                    warn!(
                        "Command {} timeout, retrying ({}/{})",
                        command_id, retry_count, self.strategy.max_retries
                    );
                    std::thread::sleep(self.strategy.retry_interval);
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// 添加到历史记录
    fn add_to_history(&mut self, result: SendResult) {
        self.command_history.push_back(result);

        // 限制历史记录大小
        while self.command_history.len() > self.history_limit {
            self.command_history.pop_front();
        }
    }
}

/// 命令优先级队列（用于高级场景）
pub struct PriorityCommandQueue {
    high_priority: VecDeque<ControlCommand>,
    normal_priority: VecDeque<ControlCommand>,
    low_priority: VecDeque<ControlCommand>,
}

impl PriorityCommandQueue {
    pub fn new() -> Self {
        Self {
            high_priority: VecDeque::new(),
            normal_priority: VecDeque::new(),
            low_priority: VecDeque::new(),
        }
    }

    pub fn enqueue(&mut self, command: ControlCommand, priority: CommandPriority) {
        match priority {
            CommandPriority::High => self.high_priority.push_back(command),
            CommandPriority::Normal => self.normal_priority.push_back(command),
            CommandPriority::Low => self.low_priority.push_back(command),
        }
    }

    pub fn dequeue(&mut self) -> Option<ControlCommand> {
        self.high_priority
            .pop_front()
            .or_else(|| self.normal_priority.pop_front())
            .or_else(|| self.low_priority.pop_front())
    }

    pub fn len(&self) -> usize {
        self.high_priority.len() + self.normal_priority.len() + self.low_priority.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for PriorityCommandQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// 命令优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandPriority {
    High,
    Normal,
    Low,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_strategy_default() {
        let strategy = SendStrategy::default();
        assert_eq!(strategy.timeout, Duration::from_secs(5));
        assert_eq!(strategy.max_retries, 3);
        assert!(strategy.wait_for_ack);
    }

    #[test]
    fn test_priority_queue() {
        let mut queue = PriorityCommandQueue::new();

        let cmd1 = ControlCommand::new(1, 1, ControlType::RemoteControl, 1.0, "test".to_string());
        let cmd2 = ControlCommand::new(2, 2, ControlType::RemoteControl, 2.0, "test".to_string());
        let cmd3 = ControlCommand::new(3, 3, ControlType::RemoteControl, 3.0, "test".to_string());

        queue.enqueue(cmd1.clone(), CommandPriority::Low);
        queue.enqueue(cmd2.clone(), CommandPriority::High);
        queue.enqueue(cmd3.clone(), CommandPriority::Normal);

        assert_eq!(queue.len(), 3);

        // 高优先级应该先出队
        let first = queue.dequeue().unwrap();
        assert_eq!(first.channel_id, 2);

        // 然后是普通优先级
        let second = queue.dequeue().unwrap();
        assert_eq!(second.channel_id, 3);

        // 最后是低优先级
        let third = queue.dequeue().unwrap();
        assert_eq!(third.channel_id, 1);

        assert!(queue.is_empty());
    }

    #[test]
    fn test_send_result() {
        let result = SendResult {
            command_id: "test-123".to_string(),
            success: true,
            status: "success".to_string(),
            message: None,
            retry_count: 0,
            duration: Duration::from_millis(100),
        };

        assert!(result.success);
        assert_eq!(result.status, "success");
        assert_eq!(result.retry_count, 0);
    }
}
