#!/usr/bin/env python3
import re
import sys

# Translation mapping for common Chinese comments
translations = {
    # Headers
    "-- VoltageEMS 核心通用函数 (80%逻辑)": "-- VoltageEMS Core Common Functions (80% logic)",
    # Function names
    "-- 通用实体存储": "-- Generic Entity Storage",
    "-- 通用批量同步": "-- Generic Batch Sync",
    "-- 通用查询": "-- Generic Query",
    "-- 实体管理器": "-- Entity Manager",
    "-- 通用状态机": "-- Generic State Machine",
    "-- 通用多维索引管理": "-- Generic Multi-dimensional Index Management",
    "-- 通用条件评估器": "-- Generic Condition Evaluator",
    "-- 通用批量数据收集器": "-- Generic Batch Data Collector",
    "-- 通用事件发布器": "-- Generic Event Publisher",
    "-- 通用统计引擎": "-- Generic Statistics Engine",
    "-- 通用批量点位初始化函数": "-- Generic Batch Point Initialization Function",
    "-- 注册函数": "-- Register functions",
    # Comments
    "-- 存储实体数据": "-- Store entity data",
    "-- 处理索引": "-- Process indexes",
    "-- 更新统计": "-- Update statistics",
    "-- 设置过期时间": "-- Set expiration time",
    "-- 基于索引查询": "-- Index-based query",
    "-- 范围查询（用于排序索引）": "-- Range query (for sorted indexes)",
    "-- 模式匹配查询": "-- Pattern matching query",
    "-- 应用过滤器": "-- Apply filters",
    "-- 排序": "-- Sort",
    "-- 分页": "-- Pagination",
    "-- 重建索引": "-- Rebuild indexes",
    "-- 清理旧索引": "-- Clear old indexes",
    "-- 这里需要根据实体类型定义索引规则": "-- Here we need to define index rules based on entity type",
    "-- 状态机定义（可以从配置中读取）": "-- State machine definition (can be read from config)",
    "-- 获取实体类型": "-- Get entity type",
    "-- 验证当前状态": "-- Verify current state",
    "-- 验证转换": "-- Verify transition",
    "-- 执行转换": "-- Execute transition",
    "-- 记录状态历史": "-- Record state history",
    "-- 保留最近100条": "-- Keep last 100 records",
    "-- 发布状态变更事件": "-- Publish state change event",
    "-- 单字段索引": "-- Single field index",
    "-- 复合索引": "-- Composite index",
    "-- 排序索引": "-- Sorted index",
    "-- 删除所有相关索引": "-- Delete all related indexes",
    "-- 多条件交集查询": "-- Multi-condition intersection query",
    "-- 多条件并集查询": "-- Multi-condition union query",
    "-- 范围查询": "-- Range query",
    "-- 评估单个条件": "-- Evaluate single condition",
    "-- 评估条件组": "-- Evaluate condition group",
    "-- 嵌套条件组": "-- Nested condition group",
    "-- 单个条件": "-- Single condition",
    "-- 收集数据": "-- Collect data",
    "-- 直接指定的键": "-- Directly specified keys",
    "-- 模式匹配": "-- Pattern matching",
    "-- 从索引收集": "-- Collect from index",
    "-- 应用转换": "-- Apply transformation",
    "-- 将hash扁平化为对象": "-- Flatten hash to object",
    "-- 聚合": "-- Aggregate",
    "-- 假设数据已经被扁平化": "-- Assume data has been flattened",
    "-- 存储事件": "-- Store event",
    "-- 添加到事件流": "-- Add to event stream",
    "-- 发布到频道": "-- Publish to channel",
    "-- 增量统计": "-- Incremental statistics",
    "-- 简单的直方图实现": "-- Simple histogram implementation",
    "-- 时间窗口统计": "-- Time window statistics",
    "-- 获取统计": "-- Get statistics",
    "-- 获取分布": "-- Get distribution",
    "-- 重置统计": "-- Reset statistics",
    "-- 删除时间窗口数据": "-- Delete time window data",
    "-- 构建Hash key": "-- Build Hash key",
    "-- 设置过期时间（如果指定）": "-- Set expiration time (if specified)",
    "-- 返回初始化结果": "-- Return initialization result",
    # Additional translations for other files
    "-- VoltageEMS 特定函数库": "-- VoltageEMS Specific Function Library",
    "-- DAG执行器": "-- DAG Executor",
    "-- 创建执行节点映射": "-- Create execution node mapping",
    "-- 递归执行函数": "-- Recursive execution function",
    "-- 检查所有依赖是否已完成": "-- Check if all dependencies are completed",
    "-- 执行节点": "-- Execute node",
    "-- 处理参数中的变量引用": "-- Process variable references in parameters",
    "-- 替换变量引用": "-- Replace variable references",
    "-- 调用实际函数": "-- Call actual function",
    "-- 存储结果": "-- Store result",
    "-- 标记为已完成": "-- Mark as completed",
    "-- 执行依赖它的节点": "-- Execute nodes that depend on it",
    "-- 从没有依赖的节点开始": "-- Start from nodes without dependencies",
    "-- 返回执行结果": "-- Return execution result",
    "-- 行协议转换器": "-- Line Protocol Converter",
    "-- 支持的数据类型": "-- Supported data types",
    "-- 转换为Line Protocol格式": "-- Convert to Line Protocol format",
    "-- 解析标签": "-- Parse tags",
    "-- 构建字段": "-- Build fields",
    "-- 添加时间戳": "-- Add timestamp",
    "-- 拼接行协议": "-- Concatenate line protocol",
    "-- 处理特殊字符": "-- Process special characters",
    # Domain translations
    "-- VoltageEMS 领域函数库": "-- VoltageEMS Domain Function Library",
    "-- 告警管理函数": "-- Alarm Management Functions",
    "-- 存储告警信息": "-- Store alarm information",
    "-- 创建告警ID": "-- Create alarm ID",
    "-- 解析告警数据": "-- Parse alarm data",
    "-- 主索引存储": "-- Store main index",
    "-- 多维索引": "-- Multi-dimensional indexes",
    "-- 按状态索引": "-- Index by status",
    "-- 按设备索引": "-- Index by device",
    "-- 按级别索引": "-- Index by level",
    "-- 按类型索引": "-- Index by type",
    "-- 按时间索引": "-- Index by time",
    "-- 发布告警事件": "-- Publish alarm event",
    "-- 确认告警": "-- Acknowledge alarm",
    "-- 检查告警存在": "-- Check alarm exists",
    "-- 检查状态": "-- Check status",
    "-- 更新告警状态": "-- Update alarm status",
    "-- 解决告警": "-- Resolve alarm",
    "-- 删除过期告警": "-- Delete expired alarms",
    "-- 查询告警": "-- Query alarms",
    "-- 构建查询键": "-- Build query key",
    "-- 应用额外过滤器": "-- Apply additional filters",
    "-- 按时间排序": "-- Sort by time",
    # Services translations
    "-- VoltageEMS 服务函数库": "-- VoltageEMS Service Function Library",
    "-- HisSrv函数": "-- HisSrv Functions",
    "-- ModSrv函数": "-- ModSrv Functions",
    "-- NetSrv函数": "-- NetSrv Functions",
    "-- RuleSrv函数": "-- RuleSrv Functions",
    "-- AlarmSrv函数": "-- AlarmSrv Functions",
    "-- 收集数据准备写入InfluxDB": "-- Collect data for writing to InfluxDB",
    "-- 扫描匹配的键": "-- Scan matching keys",
    "-- 转换为InfluxDB行协议": "-- Convert to InfluxDB line protocol",
    "-- 批量数据队列": "-- Batch data queue",
    "-- 获取批次数据": "-- Get batch data",
    "-- 确认批次": "-- Acknowledge batch",
    "-- 获取批次行数据": "-- Get batch line data",
    "-- 初始化映射关系": "-- Initialize mappings",
    "-- 同步测量数据": "-- Sync measurement data",
    "-- 发送控制命令": "-- Send control command",
    "-- 转发数据": "-- Forward data",
    "-- 统计信息": "-- Statistics",
    "-- 配置路由": "-- Configure route",
    "-- 获取路由": "-- Get routes",
    "-- 清理队列": "-- Clear queues",
    "-- 执行DAG规则": "-- Execute DAG rule",
    # Additional common translations
    "-- 创建唯一ID": "-- Create unique ID",
    "-- 验证参数": "-- Validate parameters",
    "-- 检查权限": "-- Check permissions",
    "-- 更新状态": "-- Update status",
    "-- 删除记录": "-- Delete record",
    "-- 清理数据": "-- Clean data",
    "-- 构建索引": "-- Build index",
    "-- 计算统计": "-- Calculate statistics",
    "-- 生成报告": "-- Generate report",
    "-- 触发事件": "-- Trigger event",
    "-- 记录日志": "-- Log record",
    "-- 缓存结果": "-- Cache result",
    "-- 验证数据": "-- Validate data",
    "-- 格式化输出": "-- Format output",
    "-- 处理错误": "-- Handle error",
    "-- 重试操作": "-- Retry operation",
    "-- 获取配置": "-- Get configuration",
    "-- 保存配置": "-- Save configuration",
    "-- 初始化服务": "-- Initialize service",
    "-- 停止服务": "-- Stop service",
}


def translate_file(filepath):
    with open(filepath, "r", encoding="utf-8") as f:
        content = f.read()

    # Replace all known translations
    for chinese, english in translations.items():
        content = content.replace(chinese, english)

    # Find any remaining Chinese characters
    chinese_pattern = re.compile(r"[\u4e00-\u9fa5]+")
    remaining = chinese_pattern.findall(content)

    if remaining:
        print(f"Warning: Found untranslated Chinese text in {filepath}:")
        for text in set(remaining):
            print(f"  - {text}")

    return content


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python translate_chinese.py <file>")
        sys.exit(1)

    filepath = sys.argv[1]
    translated = translate_file(filepath)

    # Write back to file
    with open(filepath, "w", encoding="utf-8") as f:
        f.write(translated)

    print(f"Translated {filepath}")
