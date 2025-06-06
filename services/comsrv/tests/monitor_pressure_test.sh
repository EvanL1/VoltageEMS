#!/bin/bash

# COMSRV 压力测试监控脚本
# 实时监控comsrv在高负载下的日志输出

echo "🔍 COMSRV 压力测试日志监控"
echo "=" 
echo "此脚本将实时监控comsrv的日志输出"
echo "按 Ctrl+C 停止监控"
echo ""

# 创建监控会话
LOG_DIR="./logs"
CHANNELS_DIR="$LOG_DIR/channels"
MAIN_LOG="$LOG_DIR/comsrv_pressure.log"

# 检查日志目录是否存在
if [ ! -d "$LOG_DIR" ]; then
    echo "⚠️  日志目录不存在，创建中..."
    mkdir -p "$LOG_DIR"
fi

echo "📁 监控目录: $LOG_DIR"
echo "📊 主日志文件: $MAIN_LOG"
echo "📂 通道日志目录: $CHANNELS_DIR"
echo ""

# 启动多个监控进程
monitor_main_log() {
    echo "🔍 [主日志监控] 开始监控主日志文件..."
    if [ -f "$MAIN_LOG" ]; then
        tail -f "$MAIN_LOG" | while read line; do
            echo "[主日志] $line"
        done
    else
        echo "⚠️  主日志文件尚未创建: $MAIN_LOG"
        while [ ! -f "$MAIN_LOG" ]; do
            sleep 1
        done
        echo "✅ 主日志文件已创建，开始监控..."
        tail -f "$MAIN_LOG" | while read line; do
            echo "[主日志] $line"
        done
    fi
}

monitor_channel_logs() {
    echo "🔍 [通道日志监控] 开始监控通道日志..."
    
    # 等待通道目录创建
    while [ ! -d "$CHANNELS_DIR" ]; do
        sleep 1
    done
    
    # 监控所有通道的今日日志
    TODAY=$(date +"%Y-%m-%d")
    
    # 使用inotify监控新文件创建和修改
    if command -v fswatch >/dev/null 2>&1; then
        # macOS 使用 fswatch
        fswatch -o "$CHANNELS_DIR" | while read f; do
            echo "📝 通道日志有更新..."
            find "$CHANNELS_DIR" -name "*$TODAY.log" -newer /tmp/last_check 2>/dev/null | while read logfile; do
                channel_name=$(basename $(dirname "$logfile"))
                tail -n 1 "$logfile" | sed "s/^/[通道:$channel_name] /"
            done
            touch /tmp/last_check
        done
    else
        # 回退到轮询方式
        while true; do
            find "$CHANNELS_DIR" -name "*$TODAY.log" -type f 2>/dev/null | while read logfile; do
                if [ -f "$logfile" ]; then
                    channel_name=$(basename $(dirname "$logfile"))
                    tail -n 5 "$logfile" | tail -n 1 | sed "s/^/[通道:$channel_name] /"
                fi
            done
            sleep 2
        done
    fi
}

show_pressure_stats() {
    echo "📊 [统计监控] 开始性能统计..."
    
    while true; do
        sleep 10
        
        echo ""
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        echo "📊 压力测试统计 ($(date))"
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        
        # 统计进程信息
        if pgrep -f "comsrv.*pressure_test_config" > /dev/null; then
            comsrv_pid=$(pgrep -f "comsrv.*pressure_test_config")
            echo "🟢 COMSRV 进程状态: 运行中 (PID: $comsrv_pid)"
            
            # 内存使用
            if command -v ps >/dev/null 2>&1; then
                memory_usage=$(ps -p $comsrv_pid -o rss= 2>/dev/null | awk '{print $1/1024}')
                if [ ! -z "$memory_usage" ]; then
                    echo "💾 内存使用: ${memory_usage} MB"
                fi
            fi
        else
            echo "🔴 COMSRV 进程状态: 未运行"
        fi
        
        # 统计通道日志数量
        if [ -d "$CHANNELS_DIR" ]; then
            channel_count=$(find "$CHANNELS_DIR" -maxdepth 1 -type d | wc -l)
            channel_count=$((channel_count - 1))  # 减去父目录
            echo "📂 活跃通道数量: $channel_count"
            
            # 统计今日日志条目
            TODAY=$(date +"%Y-%m-%d")
            total_lines=0
            find "$CHANNELS_DIR" -name "*$TODAY.log" -type f 2>/dev/null | while read logfile; do
                lines=$(wc -l < "$logfile" 2>/dev/null || echo "0")
                total_lines=$((total_lines + lines))
            done
            echo "📝 今日日志条目: 正在统计..."
        fi
        
        # 统计主日志大小
        if [ -f "$MAIN_LOG" ]; then
            log_size=$(du -h "$MAIN_LOG" 2>/dev/null | cut -f1)
            echo "📋 主日志文件大小: $log_size"
        fi
        
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        echo ""
    done
}

# 捕获退出信号
cleanup() {
    echo ""
    echo "🛑 停止监控..."
    kill $(jobs -p) 2>/dev/null
    exit 0
}

trap cleanup SIGINT SIGTERM

# 启动所有监控进程
echo "🚀 启动监控进程..."

# 后台启动各种监控
show_pressure_stats &
STATS_PID=$!

# 前台监控通道日志（主要输出）
monitor_channel_logs &
CHANNEL_PID=$!

# 等待用户中断
wait $CHANNEL_PID $STATS_PID 