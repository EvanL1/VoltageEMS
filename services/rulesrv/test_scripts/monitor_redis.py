#!/usr/bin/env python3
"""
Redis监控工具

实时监控Redis中的：
1. 通道消息
2. 键值变化
3. 规则执行
4. 控制命令
"""

import redis
import json
import time
import argparse
import threading
from datetime import datetime
from collections import defaultdict
from colorama import init, Fore, Back, Style

# 初始化颜色
init()

# 配置
REDIS_HOST = "localhost"
REDIS_PORT = 6379

class RedisMonitor:
    def __init__(self, redis_host=REDIS_HOST, redis_port=REDIS_PORT):
        """初始化Redis监控器"""
        self.r = redis.Redis(host=redis_host, port=redis_port, decode_responses=True)
        self.running = False
        self.stats = defaultdict(int)
        
        # 测试连接
        try:
            self.r.ping()
            print(f"{Fore.GREEN}✓ 已连接到Redis {redis_host}:{redis_port}{Style.RESET_ALL}")
        except Exception as e:
            print(f"{Fore.RED}✗ Redis连接失败: {e}{Style.RESET_ALL}")
            raise

    def format_timestamp(self):
        """格式化时间戳"""
        return datetime.now().strftime("%H:%M:%S.%f")[:-3]

    def format_channel_name(self, channel):
        """格式化通道名称（带颜色）"""
        if 'point:update' in channel:
            return f"{Fore.GREEN}{channel}{Style.RESET_ALL}"
        elif 'modsrv' in channel:
            return f"{Fore.BLUE}{channel}{Style.RESET_ALL}"
        elif 'alarm' in channel:
            return f"{Fore.YELLOW}{channel}{Style.RESET_ALL}"
        elif 'cmd' in channel:
            return f"{Fore.MAGENTA}{channel}{Style.RESET_ALL}"
        elif 'rule' in channel:
            return f"{Fore.CYAN}{channel}{Style.RESET_ALL}"
        else:
            return channel

    def format_value(self, value, max_length=100):
        """格式化值（截断长内容）"""
        if len(value) > max_length:
            return value[:max_length] + "..."
        return value

    def monitor_channels(self, patterns):
        """监控Redis通道"""
        pubsub = self.r.pubsub()
        
        # 订阅通道
        for pattern in patterns:
            pubsub.psubscribe(pattern)
            print(f"{Fore.CYAN}已订阅: {pattern}{Style.RESET_ALL}")
        
        print(f"\n{Fore.YELLOW}开始监控通道消息...{Style.RESET_ALL}\n")
        
        while self.running:
            try:
                message = pubsub.get_message(timeout=1)
                if message and message['type'] in ['pmessage', 'message']:
                    timestamp = self.format_timestamp()
                    channel = message['channel']
                    data = message['data']
                    
                    # 更新统计
                    self.stats[channel] += 1
                    
                    # 显示消息
                    print(f"[{timestamp}] {self.format_channel_name(channel)}")
                    
                    # 尝试解析JSON
                    try:
                        json_data = json.loads(data)
                        print(f"  {json.dumps(json_data, indent=2, ensure_ascii=False)}")
                    except:
                        print(f"  {self.format_value(data)}")
                    
                    print()
                    
            except KeyboardInterrupt:
                break
            except Exception as e:
                print(f"{Fore.RED}错误: {e}{Style.RESET_ALL}")
        
        pubsub.close()

    def monitor_keys(self, patterns, interval=1):
        """监控键值变化"""
        print(f"\n{Fore.YELLOW}开始监控键值变化...{Style.RESET_ALL}\n")
        
        # 初始快照
        key_snapshots = {}
        
        while self.running:
            try:
                for pattern in patterns:
                    # 扫描匹配的键
                    cursor = 0
                    while True:
                        cursor, keys = self.r.scan(cursor, match=pattern, count=100)
                        
                        for key in keys:
                            current_value = self.r.get(key)
                            
                            # 检查是否有变化
                            if key not in key_snapshots:
                                # 新键
                                key_snapshots[key] = current_value
                                print(f"[{self.format_timestamp()}] {Fore.GREEN}NEW{Style.RESET_ALL} {key}")
                                print(f"  值: {self.format_value(current_value)}")
                                print()
                            elif key_snapshots[key] != current_value:
                                # 值变化
                                old_value = key_snapshots[key]
                                key_snapshots[key] = current_value
                                print(f"[{self.format_timestamp()}] {Fore.YELLOW}CHANGE{Style.RESET_ALL} {key}")
                                print(f"  旧值: {self.format_value(old_value)}")
                                print(f"  新值: {self.format_value(current_value)}")
                                print()
                        
                        if cursor == 0:
                            break
                
                # 检查删除的键
                current_keys = set()
                for pattern in patterns:
                    cursor = 0
                    while True:
                        cursor, keys = self.r.scan(cursor, match=pattern, count=100)
                        current_keys.update(keys)
                        if cursor == 0:
                            break
                
                for key in list(key_snapshots.keys()):
                    if key not in current_keys:
                        print(f"[{self.format_timestamp()}] {Fore.RED}DELETE{Style.RESET_ALL} {key}")
                        print()
                        del key_snapshots[key]
                
                time.sleep(interval)
                
            except KeyboardInterrupt:
                break
            except Exception as e:
                print(f"{Fore.RED}错误: {e}{Style.RESET_ALL}")

    def monitor_rules(self):
        """监控规则执行"""
        print(f"\n{Fore.YELLOW}监控规则执行...{Style.RESET_ALL}\n")
        
        # 监控执行记录
        execution_keys = set()
        
        while self.running:
            try:
                # 扫描执行记录
                pattern = "ems:rule:execution:*"
                cursor = 0
                
                while True:
                    cursor, keys = self.r.scan(cursor, match=pattern, count=100)
                    
                    for key in keys:
                        if key not in execution_keys:
                            execution_keys.add(key)
                            
                            # 读取执行记录
                            record_data = self.r.get(key)
                            if record_data:
                                record = json.loads(record_data)
                                
                                timestamp = record.get('timestamp', 'N/A')
                                rule_id = record.get('rule_id', 'N/A')
                                status = record.get('status', 'unknown')
                                duration = record.get('duration_ms', 0)
                                
                                # 状态着色
                                if status == 'completed':
                                    status_color = Fore.GREEN
                                else:
                                    status_color = Fore.RED
                                
                                print(f"[{self.format_timestamp()}] {Fore.CYAN}规则执行{Style.RESET_ALL}")
                                print(f"  规则ID: {rule_id}")
                                print(f"  状态: {status_color}{status}{Style.RESET_ALL}")
                                print(f"  耗时: {duration}ms")
                                print(f"  时间: {timestamp}")
                                
                                if record.get('error'):
                                    print(f"  错误: {Fore.RED}{record['error']}{Style.RESET_ALL}")
                                
                                print()
                    
                    if cursor == 0:
                        break
                
                time.sleep(1)
                
            except KeyboardInterrupt:
                break
            except Exception as e:
                print(f"{Fore.RED}错误: {e}{Style.RESET_ALL}")

    def show_statistics(self):
        """显示统计信息"""
        print(f"\n{Fore.CYAN}{'=' * 60}{Style.RESET_ALL}")
        print(f"{Fore.CYAN}监控统计{Style.RESET_ALL}")
        print(f"{Fore.CYAN}{'=' * 60}{Style.RESET_ALL}\n")
        
        if self.stats:
            print("通道消息统计:")
            for channel, count in sorted(self.stats.items(), key=lambda x: x[1], reverse=True):
                print(f"  {channel}: {count} 条")
        else:
            print("没有收到任何消息")

    def start_monitoring(self, mode='all'):
        """启动监控"""
        self.running = True
        threads = []
        
        try:
            if mode in ['all', 'channels']:
                # 监控通道
                patterns = [
                    "*",  # 监控所有通道
                ]
                t = threading.Thread(target=self.monitor_channels, args=(patterns,))
                t.daemon = True
                t.start()
                threads.append(t)
            
            if mode in ['all', 'keys']:
                # 监控键值
                patterns = [
                    "rule:*",
                    "ems:rule:*",
                    "test:*:*",
                    "1001:*:*",  # 通道数据
                    "modsrv:*",
                ]
                t = threading.Thread(target=self.monitor_keys, args=(patterns,))
                t.daemon = True
                t.start()
                threads.append(t)
            
            if mode in ['all', 'rules']:
                # 监控规则执行
                t = threading.Thread(target=self.monitor_rules)
                t.daemon = True
                t.start()
                threads.append(t)
            
            # 等待中断
            while True:
                time.sleep(1)
                
        except KeyboardInterrupt:
            print(f"\n{Fore.YELLOW}停止监控...{Style.RESET_ALL}")
            self.running = False
            
            # 等待线程结束
            for t in threads:
                t.join(timeout=2)
            
            # 显示统计
            self.show_statistics()

    def show_current_state(self):
        """显示当前状态快照"""
        print(f"\n{Fore.CYAN}{'=' * 60}{Style.RESET_ALL}")
        print(f"{Fore.CYAN}Redis当前状态{Style.RESET_ALL}")
        print(f"{Fore.CYAN}{'=' * 60}{Style.RESET_ALL}\n")
        
        # 显示规则列表
        print(f"{Fore.YELLOW}已配置的规则:{Style.RESET_ALL}")
        rule_ids = self.r.smembers("rulesrv:rules")
        if rule_ids:
            for rule_id in sorted(rule_ids):
                rule_data = self.r.get(f"rule:{rule_id}")
                if rule_data:
                    rule = json.loads(rule_data)
                    status = "✓" if rule.get('enabled') else "✗"
                    print(f"  {status} {rule['name']} ({rule_id})")
        else:
            print("  (无)")
        
        # 显示最近的执行记录
        print(f"\n{Fore.YELLOW}最近的执行记录:{Style.RESET_ALL}")
        pattern = "ems:rule:execution:*"
        cursor = 0
        executions = []
        
        while True:
            cursor, keys = self.r.scan(cursor, match=pattern, count=100)
            for key in keys:
                record_data = self.r.get(key)
                if record_data:
                    record = json.loads(record_data)
                    executions.append(record)
            if cursor == 0:
                break
        
        # 按时间排序
        executions.sort(key=lambda x: x.get('timestamp', ''), reverse=True)
        
        for record in executions[:5]:  # 显示最近5条
            timestamp = record.get('timestamp', 'N/A')
            rule_id = record.get('rule_id', 'N/A')
            status = record.get('status', 'unknown')
            icon = "✓" if status == 'completed' else "✗"
            print(f"  {icon} {timestamp} - {rule_id}")
        
        if not executions:
            print("  (无)")
        
        # 显示当前数据点
        print(f"\n{Fore.YELLOW}当前数据点:{Style.RESET_ALL}")
        patterns = ["test:*:*", "1001:*:*"]
        
        for pattern in patterns:
            cursor = 0
            while True:
                cursor, keys = self.r.scan(cursor, match=pattern, count=20)
                for key in sorted(keys)[:10]:  # 限制显示数量
                    value = self.r.get(key)
                    print(f"  {key} = {value}")
                if cursor == 0:
                    break

def main():
    parser = argparse.ArgumentParser(description='Redis监控工具')
    parser.add_argument('--mode', choices=['all', 'channels', 'keys', 'rules', 'status'],
                        default='all', help='监控模式')
    parser.add_argument('--redis-host', default=REDIS_HOST, help='Redis主机')
    parser.add_argument('--redis-port', type=int, default=REDIS_PORT, help='Redis端口')
    
    args = parser.parse_args()
    
    # 创建监控器
    monitor = RedisMonitor(args.redis_host, args.redis_port)
    
    if args.mode == 'status':
        # 显示当前状态
        monitor.show_current_state()
    else:
        # 启动监控
        print(f"\n{Fore.GREEN}Redis监控已启动{Style.RESET_ALL}")
        print(f"模式: {args.mode}")
        print(f"按 Ctrl+C 停止监控\n")
        
        monitor.start_monitoring(args.mode)

if __name__ == "__main__":
    main()