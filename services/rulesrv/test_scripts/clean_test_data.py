#!/usr/bin/env python3
"""
测试数据清理脚本

清理测试过程中产生的：
1. 测试规则
2. 执行记录
3. 测试数据点
4. 控制命令
"""

import redis
import json
import argparse
from datetime import datetime
from colorama import init, Fore, Style

# 初始化颜色
init()

# 配置
REDIS_HOST = "localhost"
REDIS_PORT = 6379

class TestDataCleaner:
    def __init__(self, redis_host=REDIS_HOST, redis_port=REDIS_PORT):
        """初始化清理器"""
        self.r = redis.Redis(host=redis_host, port=redis_port, decode_responses=True)
        self.dry_run = False
        self.cleaned_count = {
            'rules': 0,
            'executions': 0,
            'data_points': 0,
            'commands': 0,
            'model_outputs': 0
        }
        
        # 测试连接
        try:
            self.r.ping()
            print(f"{Fore.GREEN}✓ 已连接到Redis {redis_host}:{redis_port}{Style.RESET_ALL}")
        except Exception as e:
            print(f"{Fore.RED}✗ Redis连接失败: {e}{Style.RESET_ALL}")
            raise

    def print_info(self, msg):
        """打印信息"""
        print(f"{Fore.BLUE}ℹ {msg}{Style.RESET_ALL}")

    def print_warning(self, msg):
        """打印警告"""
        print(f"{Fore.YELLOW}⚠ {msg}{Style.RESET_ALL}")

    def print_success(self, msg):
        """打印成功"""
        print(f"{Fore.GREEN}✓ {msg}{Style.RESET_ALL}")

    def clean_test_rules(self, pattern="test_*"):
        """清理测试规则"""
        self.print_info(f"清理测试规则 (模式: {pattern})")
        
        # 获取所有规则ID
        rule_ids = self.r.smembers("rulesrv:rules")
        
        for rule_id in rule_ids:
            if pattern == "*" or rule_id.startswith(pattern.rstrip('*')):
                rule_key = f"rule:{rule_id}"
                rule_data = self.r.get(rule_key)
                
                if rule_data:
                    rule = json.loads(rule_data)
                    print(f"  - {rule['name']} ({rule_id})")
                    
                    if not self.dry_run:
                        # 删除规则
                        self.r.delete(rule_key)
                        self.r.srem("rulesrv:rules", rule_id)
                        self.cleaned_count['rules'] += 1

    def clean_execution_records(self, rule_pattern=None, age_hours=None):
        """清理执行记录"""
        self.print_info("清理执行记录")
        
        pattern = "ems:rule:execution:*"
        cursor = 0
        
        while True:
            cursor, keys = self.r.scan(cursor, match=pattern, count=100)
            
            for key in keys:
                record_data = self.r.get(key)
                if record_data:
                    record = json.loads(record_data)
                    rule_id = record.get('rule_id', '')
                    timestamp = record.get('timestamp', '')
                    
                    should_delete = False
                    
                    # 按规则ID过滤
                    if rule_pattern:
                        if rule_pattern == "*" or rule_id.startswith(rule_pattern.rstrip('*')):
                            should_delete = True
                    else:
                        should_delete = True
                    
                    # 按时间过滤
                    if age_hours and should_delete:
                        try:
                            exec_time = datetime.fromisoformat(timestamp.replace('Z', '+00:00'))
                            age = (datetime.now(exec_time.tzinfo) - exec_time).total_seconds() / 3600
                            if age < age_hours:
                                should_delete = False
                        except:
                            pass
                    
                    if should_delete:
                        print(f"  - {key} (规则: {rule_id})")
                        if not self.dry_run:
                            self.r.delete(key)
                            self.cleaned_count['executions'] += 1
            
            if cursor == 0:
                break

    def clean_data_points(self, patterns):
        """清理数据点"""
        self.print_info("清理数据点")
        
        for pattern in patterns:
            cursor = 0
            while True:
                cursor, keys = self.r.scan(cursor, match=pattern, count=100)
                
                for key in keys:
                    value = self.r.get(key)
                    print(f"  - {key} = {value}")
                    
                    if not self.dry_run:
                        self.r.delete(key)
                        self.cleaned_count['data_points'] += 1
                
                if cursor == 0:
                    break

    def clean_control_commands(self):
        """清理控制命令"""
        self.print_info("清理控制命令队列")
        
        # 清理命令队列
        queue_length = self.r.llen("ems:control:queue")
        if queue_length > 0:
            print(f"  - 命令队列: {queue_length} 条")
            if not self.dry_run:
                self.r.delete("ems:control:queue")
                self.cleaned_count['commands'] += queue_length
        
        # 清理命令记录
        pattern = "ems:control:cmd:*"
        cursor = 0
        
        while True:
            cursor, keys = self.r.scan(cursor, match=pattern, count=100)
            
            for key in keys:
                if not self.dry_run:
                    self.r.delete(key)
                    self.cleaned_count['commands'] += 1
            
            if cursor == 0:
                break

    def clean_model_outputs(self, patterns):
        """清理模型输出"""
        self.print_info("清理模型输出")
        
        for pattern in patterns:
            cursor = 0
            while True:
                cursor, keys = self.r.scan(cursor, match=pattern, count=100)
                
                for key in keys:
                    if key.startswith("modsrv:output:"):
                        # Hash类型
                        fields = self.r.hkeys(key)
                        print(f"  - {key} ({len(fields)} 个字段)")
                        if not self.dry_run:
                            self.r.delete(key)
                            self.cleaned_count['model_outputs'] += 1
                    else:
                        # String类型
                        value = self.r.get(key)
                        print(f"  - {key} = {value}")
                        if not self.dry_run:
                            self.r.delete(key)
                            self.cleaned_count['model_outputs'] += 1
                
                if cursor == 0:
                    break

    def show_summary(self):
        """显示清理摘要"""
        print(f"\n{Fore.CYAN}{'=' * 40}{Style.RESET_ALL}")
        print(f"{Fore.CYAN}清理摘要{Style.RESET_ALL}")
        print(f"{Fore.CYAN}{'=' * 40}{Style.RESET_ALL}\n")
        
        if self.dry_run:
            print(f"{Fore.YELLOW}模拟运行 - 未实际删除数据{Style.RESET_ALL}\n")
        
        total = sum(self.cleaned_count.values())
        
        for category, count in self.cleaned_count.items():
            if count > 0:
                print(f"{category}: {count} 条")
        
        print(f"\n总计: {total} 条")

    def clean_all(self):
        """清理所有测试数据"""
        # 清理测试规则
        self.clean_test_rules("test_*")
        
        # 清理执行记录
        self.clean_execution_records("test_*")
        
        # 清理测试数据点
        test_patterns = [
            "test:*:*",      # 测试数据点
            "temp_test:*",   # 临时测试数据
            "mock:*",        # 模拟数据
        ]
        self.clean_data_points(test_patterns)
        
        # 清理控制命令
        self.clean_control_commands()
        
        # 清理模型输出
        model_patterns = [
            "modsrv:output:test_*",
            "modsrv:test:*"
        ]
        self.clean_model_outputs(model_patterns)

    def interactive_clean(self):
        """交互式清理"""
        print(f"\n{Fore.CYAN}交互式清理模式{Style.RESET_ALL}\n")
        
        # 显示当前状态
        rule_count = self.r.scard("rulesrv:rules")
        print(f"当前规则数: {rule_count}")
        
        # 统计执行记录
        exec_pattern = "ems:rule:execution:*"
        exec_count = 0
        cursor = 0
        while True:
            cursor, keys = self.r.scan(cursor, match=exec_pattern, count=100)
            exec_count += len(keys)
            if cursor == 0:
                break
        print(f"执行记录数: {exec_count}")
        
        # 用户选择
        print(f"\n清理选项:")
        print("1. 清理所有测试数据")
        print("2. 仅清理测试规则")
        print("3. 仅清理执行记录")
        print("4. 清理指定模式的数据")
        print("5. 退出")
        
        choice = input("\n请选择 (1-5): ")
        
        if choice == '1':
            confirm = input(f"{Fore.YELLOW}确定要清理所有测试数据吗？(y/N): {Style.RESET_ALL}")
            if confirm.lower() == 'y':
                self.clean_all()
            else:
                print("已取消")
                
        elif choice == '2':
            pattern = input("输入规则ID模式 (默认: test_*): ") or "test_*"
            self.clean_test_rules(pattern)
            
        elif choice == '3':
            rule_pattern = input("规则ID模式 (留空清理所有): ")
            age_str = input("保留最近N小时的记录 (留空清理所有): ")
            age_hours = int(age_str) if age_str else None
            self.clean_execution_records(rule_pattern, age_hours)
            
        elif choice == '4':
            pattern = input("输入键模式 (如: test:*): ")
            if pattern:
                self.clean_data_points([pattern])
            
        elif choice == '5':
            print("退出")
            return
        
        self.show_summary()

def main():
    parser = argparse.ArgumentParser(description='测试数据清理工具')
    parser.add_argument('--mode', choices=['all', 'rules', 'executions', 'data', 'interactive'],
                        default='interactive', help='清理模式')
    parser.add_argument('--pattern', help='数据模式')
    parser.add_argument('--dry-run', action='store_true', help='模拟运行，不实际删除')
    parser.add_argument('--redis-host', default=REDIS_HOST, help='Redis主机')
    parser.add_argument('--redis-port', type=int, default=REDIS_PORT, help='Redis端口')
    parser.add_argument('--age-hours', type=int, help='保留最近N小时的数据')
    
    args = parser.parse_args()
    
    # 创建清理器
    cleaner = TestDataCleaner(args.redis_host, args.redis_port)
    cleaner.dry_run = args.dry_run
    
    if args.dry_run:
        print(f"{Fore.YELLOW}模拟模式 - 不会实际删除数据{Style.RESET_ALL}\n")
    
    if args.mode == 'all':
        # 清理所有
        cleaner.clean_all()
        cleaner.show_summary()
        
    elif args.mode == 'rules':
        # 清理规则
        pattern = args.pattern or "test_*"
        cleaner.clean_test_rules(pattern)
        cleaner.show_summary()
        
    elif args.mode == 'executions':
        # 清理执行记录
        cleaner.clean_execution_records(args.pattern, args.age_hours)
        cleaner.show_summary()
        
    elif args.mode == 'data':
        # 清理数据点
        patterns = [args.pattern] if args.pattern else ["test:*:*"]
        cleaner.clean_data_points(patterns)
        cleaner.show_summary()
        
    elif args.mode == 'interactive':
        # 交互式清理
        cleaner.interactive_clean()

if __name__ == "__main__":
    main()