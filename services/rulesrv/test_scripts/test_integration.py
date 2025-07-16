#!/usr/bin/env python3
"""
集成测试脚本

完整的端到端测试流程：
1. 创建测试规则
2. 发布测试数据
3. 等待规则触发
4. 验证执行结果
5. 检查控制命令
"""

import json
import redis
import time
import sys
import argparse
from datetime import datetime
from colorama import init, Fore, Style

# 初始化颜色输出
init()

# 配置
REDIS_HOST = "localhost"
REDIS_PORT = 6379
API_URL = "http://localhost:8086/api/v1"

class IntegrationTester:
    def __init__(self, redis_host=REDIS_HOST, redis_port=REDIS_PORT):
        """初始化集成测试器"""
        self.r = redis.Redis(host=redis_host, port=redis_port, decode_responses=True)
        self.test_results = []
        
        # 测试连接
        try:
            self.r.ping()
            self.print_success(f"已连接到Redis {redis_host}:{redis_port}")
        except Exception as e:
            self.print_error(f"Redis连接失败: {e}")
            sys.exit(1)

    def print_success(self, msg):
        """打印成功消息"""
        print(f"{Fore.GREEN}✓ {msg}{Style.RESET_ALL}")

    def print_error(self, msg):
        """打印错误消息"""
        print(f"{Fore.RED}✗ {msg}{Style.RESET_ALL}")

    def print_info(self, msg):
        """打印信息消息"""
        print(f"{Fore.BLUE}ℹ {msg}{Style.RESET_ALL}")

    def print_warning(self, msg):
        """打印警告消息"""
        print(f"{Fore.YELLOW}⚠ {msg}{Style.RESET_ALL}")

    def create_test_rule(self, test_name):
        """创建测试规则"""
        self.print_info(f"创建测试规则: {test_name}")
        
        rule = {
            "id": f"test_{test_name}_{int(time.time())}",
            "name": f"测试规则 - {test_name}",
            "description": f"集成测试规则 - {test_name}",
            "enabled": True,
            "priority": 100,
            "nodes": [],
            "edges": []
        }
        
        if test_name == "simple_control":
            # 简单控制规则
            rule["nodes"] = [
                {
                    "id": "input1",
                    "name": "读取温度",
                    "type": "Input",
                    "config": {"source": "test:m:temp"}
                },
                {
                    "id": "condition1",
                    "name": "检查阈值",
                    "type": "Condition",
                    "config": {"expression": "$input1 > 30"}
                },
                {
                    "id": "action1",
                    "name": "控制输出",
                    "type": "Action",
                    "config": {
                        "action_type": "control",
                        "channel_id": "test",
                        "point_type": "c",
                        "point_id": "ctrl1",
                        "value": 1
                    }
                }
            ]
            rule["edges"] = [
                {"from": "input1", "to": "condition1"},
                {"from": "condition1", "to": "action1", "condition": "$condition1 == true"}
            ]
            
        elif test_name == "model_trigger":
            # 模型触发规则
            rule["nodes"] = [
                {
                    "id": "model_input",
                    "name": "模型输出",
                    "type": "Input",
                    "config": {"source": "modsrv:test_model:output"}
                },
                {
                    "id": "transform1",
                    "name": "阈值检查",
                    "type": "Transform",
                    "config": {
                        "transform_type": "threshold",
                        "input": {
                            "value_expr": "$model_input",
                            "threshold": 0.85
                        }
                    }
                },
                {
                    "id": "action1",
                    "name": "调整参数",
                    "type": "Action",
                    "config": {
                        "action_type": "control",
                        "control_id": "adjust_params"
                    }
                }
            ]
            rule["edges"] = [
                {"from": "model_input", "to": "transform1"},
                {"from": "transform1", "to": "action1", "condition": "$transform1 == false"}
            ]
        
        # 保存规则
        rule_key = f"rule:{rule['id']}"
        self.r.set(rule_key, json.dumps(rule))
        self.r.sadd("rulesrv:rules", rule['id'])
        
        self.print_success(f"规则已创建: {rule['id']}")
        return rule

    def publish_test_data(self, test_name, rule_id):
        """发布测试数据"""
        self.print_info(f"发布测试数据: {test_name}")
        
        if test_name == "simple_control":
            # 发布触发条件的数据
            self.r.set("test:m:temp", "35:{}".format(int(time.time() * 1000)))
            self.print_success("已发布温度数据: 35°C (应触发规则)")
            
            # 等待一下再发布不触发的数据
            time.sleep(2)
            self.r.set("test:m:temp", "25:{}".format(int(time.time() * 1000)))
            self.print_info("已发布温度数据: 25°C (不应触发规则)")
            
        elif test_name == "model_trigger":
            # 发布模型输出
            self.r.hset("modsrv:output:test_model", "output", "0.82")
            
            # 发布到通道
            channel = "modsrv:outputs:test_model"
            message = json.dumps({
                "model_id": "test_model",
                "output": "output",
                "value": 0.82,
                "timestamp": datetime.now().isoformat()
            })
            self.r.publish(channel, message)
            self.print_success("已发布模型输出: 0.82 (应触发规则)")

    def verify_execution(self, rule_id, wait_time=5):
        """验证规则执行"""
        self.print_info(f"等待 {wait_time} 秒后验证执行结果...")
        time.sleep(wait_time)
        
        # 查找执行记录
        pattern = f"ems:rule:execution:*"
        found_execution = False
        
        cursor = 0
        while True:
            cursor, keys = self.r.scan(cursor, match=pattern, count=100)
            
            for key in keys:
                record_data = self.r.get(key)
                if record_data:
                    record = json.loads(record_data)
                    if record.get('rule_id') == rule_id:
                        found_execution = True
                        self.print_success(f"找到执行记录: {record['execution_id']}")
                        self.print_info(f"执行状态: {record.get('status')}")
                        self.print_info(f"执行时间: {record.get('duration_ms')}ms")
                        
                        if record.get('status') == 'completed':
                            return True
                        else:
                            self.print_error(f"执行失败: {record.get('error', 'Unknown error')}")
                            return False
            
            if cursor == 0:
                break
        
        if not found_execution:
            self.print_warning("未找到执行记录")
        
        return found_execution

    def verify_control_command(self, test_name):
        """验证控制命令"""
        self.print_info("验证控制命令...")
        
        if test_name == "simple_control":
            # 检查控制命令队列
            commands = self.r.lrange("ems:control:queue", 0, -1)
            
            if commands:
                self.print_success(f"找到 {len(commands)} 个控制命令")
                for cmd_json in commands:
                    cmd = json.loads(cmd_json)
                    self.print_info(f"命令ID: {cmd.get('id')}")
                    self.print_info(f"目标: {cmd.get('target')}")
                    self.print_info(f"操作: {cmd.get('operation')}")
                return True
            else:
                self.print_warning("未找到控制命令")
                
            # 检查控制点状态
            ctrl_value = self.r.get("test:c:ctrl1")
            if ctrl_value:
                self.print_success(f"控制点已更新: {ctrl_value}")
                return True
                
        return False

    def cleanup_test_data(self, rule_id):
        """清理测试数据"""
        self.print_info("清理测试数据...")
        
        # 删除规则
        self.r.delete(f"rule:{rule_id}")
        self.r.srem("rulesrv:rules", rule_id)
        
        # 删除测试数据
        self.r.delete("test:m:temp")
        self.r.delete("test:c:ctrl1")
        self.r.hdel("modsrv:output:test_model", "output")
        
        # 清理执行记录
        pattern = f"ems:rule:execution:*"
        cursor = 0
        while True:
            cursor, keys = self.r.scan(cursor, match=pattern, count=100)
            for key in keys:
                record_data = self.r.get(key)
                if record_data:
                    record = json.loads(record_data)
                    if record.get('rule_id') == rule_id:
                        self.r.delete(key)
            if cursor == 0:
                break
        
        self.print_success("测试数据已清理")

    def run_test_scenario(self, test_name):
        """运行测试场景"""
        print(f"\n{Fore.CYAN}=== 运行测试场景: {test_name} ==={Style.RESET_ALL}\n")
        
        test_start = time.time()
        success = True
        rule_id = None
        
        try:
            # 1. 创建规则
            rule = self.create_test_rule(test_name)
            rule_id = rule['id']
            
            # 2. 等待规则加载
            time.sleep(2)
            
            # 3. 发布测试数据
            self.publish_test_data(test_name, rule_id)
            
            # 4. 验证执行
            if not self.verify_execution(rule_id):
                success = False
                self.print_error("规则执行验证失败")
            
            # 5. 验证结果
            if success and not self.verify_control_command(test_name):
                success = False
                self.print_error("控制命令验证失败")
            
        except Exception as e:
            success = False
            self.print_error(f"测试异常: {e}")
            
        finally:
            # 清理
            if rule_id:
                self.cleanup_test_data(rule_id)
        
        test_duration = time.time() - test_start
        
        # 记录结果
        self.test_results.append({
            'test_name': test_name,
            'success': success,
            'duration': test_duration
        })
        
        if success:
            self.print_success(f"测试通过 (耗时: {test_duration:.2f}秒)")
        else:
            self.print_error(f"测试失败 (耗时: {test_duration:.2f}秒)")
        
        return success

    def run_all_tests(self):
        """运行所有测试"""
        print(f"\n{Fore.CYAN}{'=' * 60}{Style.RESET_ALL}")
        print(f"{Fore.CYAN}开始集成测试{Style.RESET_ALL}")
        print(f"{Fore.CYAN}{'=' * 60}{Style.RESET_ALL}\n")
        
        test_scenarios = [
            "simple_control",
            "model_trigger"
        ]
        
        for scenario in test_scenarios:
            self.run_test_scenario(scenario)
            time.sleep(3)  # 测试间隔
        
        # 显示测试报告
        self.show_test_report()

    def show_test_report(self):
        """显示测试报告"""
        print(f"\n{Fore.CYAN}{'=' * 60}{Style.RESET_ALL}")
        print(f"{Fore.CYAN}测试报告{Style.RESET_ALL}")
        print(f"{Fore.CYAN}{'=' * 60}{Style.RESET_ALL}\n")
        
        total_tests = len(self.test_results)
        passed_tests = sum(1 for r in self.test_results if r['success'])
        total_duration = sum(r['duration'] for r in self.test_results)
        
        print(f"总测试数: {total_tests}")
        print(f"通过: {Fore.GREEN}{passed_tests}{Style.RESET_ALL}")
        print(f"失败: {Fore.RED}{total_tests - passed_tests}{Style.RESET_ALL}")
        print(f"总耗时: {total_duration:.2f}秒")
        print(f"通过率: {passed_tests/total_tests*100:.1f}%")
        
        print("\n详细结果:")
        for result in self.test_results:
            icon = f"{Fore.GREEN}✓{Style.RESET_ALL}" if result['success'] else f"{Fore.RED}✗{Style.RESET_ALL}"
            print(f"{icon} {result['test_name']} ({result['duration']:.2f}秒)")

    def monitor_live_data(self, duration=30):
        """实时监控数据流"""
        print(f"\n{Fore.CYAN}实时数据监控 (持续{duration}秒){Style.RESET_ALL}\n")
        
        pubsub = self.r.pubsub()
        patterns = [
            "point:update:*",
            "modsrv:outputs:*",
            "alarm:event:*",
            "cmd:*:*"
        ]
        
        for pattern in patterns:
            pubsub.psubscribe(pattern)
        
        start_time = time.time()
        
        try:
            while time.time() - start_time < duration:
                message = pubsub.get_message(timeout=1)
                if message and message['type'] in ['pmessage', 'message']:
                    timestamp = datetime.now().strftime("%H:%M:%S.%f")[:-3]
                    channel = message['channel']
                    data = message['data']
                    
                    # 根据通道类型着色
                    if 'point:update' in channel:
                        color = Fore.GREEN
                    elif 'modsrv' in channel:
                        color = Fore.BLUE
                    elif 'alarm' in channel:
                        color = Fore.YELLOW
                    elif 'cmd' in channel:
                        color = Fore.MAGENTA
                    else:
                        color = Fore.WHITE
                    
                    print(f"[{timestamp}] {color}{channel}{Style.RESET_ALL}")
                    print(f"  {data}\n")
                    
        except KeyboardInterrupt:
            print("\n监控已停止")
        finally:
            pubsub.close()

def main():
    parser = argparse.ArgumentParser(description='规则服务集成测试')
    parser.add_argument('--mode', choices=['full', 'scenario', 'monitor'],
                        default='full', help='测试模式')
    parser.add_argument('--scenario', choices=['simple_control', 'model_trigger'],
                        help='单个测试场景')
    parser.add_argument('--redis-host', default=REDIS_HOST, help='Redis主机')
    parser.add_argument('--redis-port', type=int, default=REDIS_PORT, help='Redis端口')
    parser.add_argument('--monitor-duration', type=int, default=30, help='监控持续时间(秒)')
    
    args = parser.parse_args()
    
    # 创建测试器
    tester = IntegrationTester(args.redis_host, args.redis_port)
    
    if args.mode == 'full':
        # 运行所有测试
        tester.run_all_tests()
        
    elif args.mode == 'scenario':
        # 运行单个场景
        if not args.scenario:
            print("错误：scenario模式需要指定 --scenario")
            return
        tester.run_test_scenario(args.scenario)
        
    elif args.mode == 'monitor':
        # 实时监控
        tester.monitor_live_data(args.monitor_duration)

if __name__ == "__main__":
    main()