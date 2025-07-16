#!/usr/bin/env python3
"""
规则触发测试脚本

通过API触发规则执行，监控执行结果。
支持：
1. 手动触发指定规则
2. 提供输入数据
3. 查看执行历史
4. 监控规则执行状态
"""

import json
import requests
import redis
import time
import argparse
from datetime import datetime
from tabulate import tabulate

# 默认配置
API_BASE_URL = "http://localhost:8083"  # Default is no prefix, use rules directly
REDIS_HOST = "localhost"
REDIS_PORT = 6379

class RuleTriggerClient:
    def __init__(self, api_url=API_BASE_URL, redis_host=REDIS_HOST, redis_port=REDIS_PORT):
        """初始化规则触发客户端"""
        self.api_url = api_url
        self.r = redis.Redis(host=redis_host, port=redis_port, decode_responses=True)
        
        # 测试连接
        try:
            self.r.ping()
            print(f"✓ 已连接到Redis {redis_host}:{redis_port}")
        except Exception as e:
            print(f"✗ Redis连接失败: {e}")
            raise
            
        # 测试API连接
        try:
            resp = requests.get(f"{self.api_url}/health", timeout=5)
            if resp.status_code == 200:
                print(f"✓ 已连接到Rules API {api_url}")
        except:
            print(f"⚠ 无法连接到Rules API {api_url}，某些功能可能不可用")

    def list_rules(self):
        """列出所有规则"""
        try:
            resp = requests.get(f"{self.api_url}/rules")
            if resp.status_code == 200:
                rules = resp.json()
                return rules
        except Exception as e:
            print(f"API调用失败: {e}")
        
        # 从Redis直接读取
        print("尝试从Redis读取规则...")
        rules = []
        rule_ids = self.r.smembers("rulesrv:rules")
        
        for rule_id in rule_ids:
            rule_data = self.r.get(f"rule:{rule_id}")
            if rule_data:
                rule = json.loads(rule_data)
                rules.append(rule)
        
        return rules

    def get_rule(self, rule_id):
        """获取规则详情"""
        try:
            resp = requests.get(f"{self.api_url}/rules/{rule_id}")
            if resp.status_code == 200:
                return resp.json()
        except:
            pass
        
        # 从Redis读取
        rule_data = self.r.get(f"rule:{rule_id}")
        if rule_data:
            return json.loads(rule_data)
        return None

    def trigger_rule(self, rule_id, input_data=None):
        """触发规则执行"""
        url = f"{self.api_url}/rules/{rule_id}/execute"
        
        payload = {}
        if input_data:
            payload = {"input": input_data}
        
        print(f"\n触发规则: {rule_id}")
        if input_data:
            print(f"输入数据: {json.dumps(input_data, indent=2)}")
        
        try:
            resp = requests.post(url, json=payload, timeout=30)
            
            if resp.status_code == 200:
                result = resp.json()
                print(f"✓ 规则执行成功")
                return result
            else:
                print(f"✗ 规则执行失败: {resp.status_code}")
                print(f"响应: {resp.text}")
                return None
                
        except requests.exceptions.Timeout:
            print("✗ 规则执行超时")
            return None
        except Exception as e:
            print(f"✗ API调用失败: {e}")
            return None

    def get_execution_history(self, rule_id, limit=10):
        """获取规则执行历史"""
        history = []
        
        # 从Redis获取执行记录
        pattern = f"ems:rule:execution:*"
        cursor = 0
        
        while True:
            cursor, keys = self.r.scan(cursor, match=pattern, count=100)
            
            for key in keys:
                record_data = self.r.get(key)
                if record_data:
                    record = json.loads(record_data)
                    if record.get('rule_id') == rule_id:
                        history.append(record)
            
            if cursor == 0:
                break
        
        # 按时间戳排序
        history.sort(key=lambda x: x.get('timestamp', ''), reverse=True)
        
        return history[:limit]

    def monitor_rule_execution(self, rule_id, timeout=60):
        """监控规则执行状态"""
        print(f"\n监控规则执行状态: {rule_id}")
        print(f"超时时间: {timeout}秒")
        print("-" * 60)
        
        start_time = time.time()
        last_execution_id = None
        
        while time.time() - start_time < timeout:
            # 获取最新执行记录
            history = self.get_execution_history(rule_id, limit=1)
            
            if history and history[0].get('execution_id') != last_execution_id:
                execution = history[0]
                last_execution_id = execution['execution_id']
                
                timestamp = execution.get('timestamp', 'N/A')
                status = execution.get('status', 'unknown')
                duration = execution.get('duration_ms', 0)
                
                status_icon = "✓" if status == "completed" else "✗"
                
                print(f"\n[{timestamp}] {status_icon} 执行ID: {execution['execution_id']}")
                print(f"  状态: {status}")
                print(f"  耗时: {duration}ms")
                
                if execution.get('output'):
                    print(f"  输出: {json.dumps(execution['output'], indent=4)}")
                
                if status == "failed" and execution.get('error'):
                    print(f"  错误: {execution['error']}")
            
            time.sleep(1)
        
        print("\n监控超时")

    def test_rule_with_data(self, rule_id):
        """使用预设数据测试规则"""
        rule = self.get_rule(rule_id)
        if not rule:
            print(f"规则不存在: {rule_id}")
            return
        
        print(f"\n测试规则: {rule['name']}")
        print(f"描述: {rule.get('description', 'N/A')}")
        
        # 根据规则类型准备测试数据
        test_data = {}
        
        if rule_id == "temp_monitor_001":
            test_data = {
                "input_temp": 35.0  # 高温触发
            }
        elif rule_id == "power_limit_001":
            test_data = {
                "input_power": 150000  # 150kW触发
            }
        elif rule_id == "aggregate_alarm_001":
            test_data = {
                "input_temp": 40.0,
                "input_power": 130000
            }
        elif rule_id == "modsrv_output_001":
            test_data = {
                "input_model": 0.80  # 低效率触发
            }
        
        # 触发规则
        result = self.trigger_rule(rule_id, test_data)
        
        if result:
            print("\n执行结果:")
            print(json.dumps(result, indent=2, ensure_ascii=False))

    def display_rules_table(self, rules):
        """以表格形式显示规则"""
        if not rules:
            print("没有找到规则")
            return
        
        headers = ["ID", "名称", "状态", "优先级", "节点数", "描述"]
        rows = []
        
        for rule in rules:
            status = "✓ 启用" if rule.get('enabled', False) else "✗ 禁用"
            node_count = len(rule.get('nodes', []))
            description = rule.get('description', '')[:40] + "..." if len(rule.get('description', '')) > 40 else rule.get('description', '')
            
            rows.append([
                rule['id'],
                rule['name'],
                status,
                rule.get('priority', 0),
                node_count,
                description
            ])
        
        print(tabulate(rows, headers=headers, tablefmt="grid"))

    def batch_trigger(self, rule_ids):
        """批量触发多个规则"""
        print(f"\n批量触发 {len(rule_ids)} 个规则")
        print("=" * 60)
        
        results = []
        
        for rule_id in rule_ids:
            rule = self.get_rule(rule_id)
            if not rule:
                print(f"\n✗ 规则不存在: {rule_id}")
                continue
            
            print(f"\n触发规则: {rule['name']} ({rule_id})")
            result = self.trigger_rule(rule_id)
            
            results.append({
                'rule_id': rule_id,
                'rule_name': rule['name'],
                'success': result is not None,
                'result': result
            })
            
            time.sleep(1)  # 避免过快触发
        
        # 显示汇总
        print("\n" + "=" * 60)
        print("批量触发结果汇总:")
        success_count = sum(1 for r in results if r['success'])
        print(f"成功: {success_count}/{len(results)}")
        
        for r in results:
            icon = "✓" if r['success'] else "✗"
            print(f"{icon} {r['rule_name']} ({r['rule_id']})")

def main():
    parser = argparse.ArgumentParser(description='规则触发测试脚本')
    parser.add_argument('--action', choices=['list', 'trigger', 'test', 'monitor', 'batch', 'history'],
                        default='list', help='操作类型')
    parser.add_argument('--rule-id', help='规则ID')
    parser.add_argument('--input', help='输入数据（JSON格式）')
    parser.add_argument('--api-url', default=API_BASE_URL, help='API基础URL')
    parser.add_argument('--redis-host', default=REDIS_HOST, help='Redis主机')
    parser.add_argument('--redis-port', type=int, default=REDIS_PORT, help='Redis端口')
    parser.add_argument('--timeout', type=int, default=60, help='监控超时时间（秒）')
    
    args = parser.parse_args()
    
    # 创建客户端
    client = RuleTriggerClient(args.api_url, args.redis_host, args.redis_port)
    
    if args.action == 'list':
        # 列出所有规则
        rules = client.list_rules()
        client.display_rules_table(rules)
        
    elif args.action == 'trigger':
        # 触发单个规则
        if not args.rule_id:
            print("错误：需要指定 --rule-id")
            return
        
        input_data = None
        if args.input:
            try:
                input_data = json.loads(args.input)
            except json.JSONDecodeError:
                print(f"错误：输入数据不是有效的JSON: {args.input}")
                return
        
        result = client.trigger_rule(args.rule_id, input_data)
        if result:
            print("\n完整响应:")
            print(json.dumps(result, indent=2, ensure_ascii=False))
            
    elif args.action == 'test':
        # 使用预设数据测试
        if not args.rule_id:
            # 测试所有规则
            rules = client.list_rules()
            for rule in rules:
                if rule.get('enabled', False):
                    client.test_rule_with_data(rule['id'])
                    print("\n" + "-" * 60)
                    time.sleep(2)
        else:
            client.test_rule_with_data(args.rule_id)
            
    elif args.action == 'monitor':
        # 监控规则执行
        if not args.rule_id:
            print("错误：monitor操作需要指定 --rule-id")
            return
        client.monitor_rule_execution(args.rule_id, args.timeout)
        
    elif args.action == 'batch':
        # 批量触发所有启用的规则
        rules = client.list_rules()
        enabled_rules = [r['id'] for r in rules if r.get('enabled', False)]
        client.batch_trigger(enabled_rules)
        
    elif args.action == 'history':
        # 查看执行历史
        if not args.rule_id:
            print("错误：history操作需要指定 --rule-id")
            return
            
        history = client.get_execution_history(args.rule_id)
        if not history:
            print(f"没有找到规则 {args.rule_id} 的执行历史")
            return
        
        print(f"\n规则 {args.rule_id} 的执行历史:")
        print("=" * 80)
        
        headers = ["时间", "执行ID", "状态", "耗时(ms)", "输出"]
        rows = []
        
        for record in history:
            output = str(record.get('output', ''))[:50] + "..." if len(str(record.get('output', ''))) > 50 else str(record.get('output', ''))
            rows.append([
                record.get('timestamp', 'N/A'),
                record.get('execution_id', 'N/A')[:8],
                record.get('status', 'unknown'),
                record.get('duration_ms', 0),
                output
            ])
        
        print(tabulate(rows, headers=headers, tablefmt="grid"))

if __name__ == "__main__":
    main()