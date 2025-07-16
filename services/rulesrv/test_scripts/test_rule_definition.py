#!/usr/bin/env python3
"""
规则定义和存储测试脚本

这个脚本演示如何创建不同类型的规则并存储到Redis中。
支持两种规则格式：
1. DAG规则 - 使用有向无环图结构
2. 简单规则 - 传统的条件-动作格式
"""

import json
import redis
import sys
from datetime import datetime
import argparse

# Redis连接配置
REDIS_HOST = "localhost"
REDIS_PORT = 6379
REDIS_DB = 0

def create_redis_connection():
    """创建Redis连接"""
    try:
        r = redis.Redis(host=REDIS_HOST, port=REDIS_PORT, db=REDIS_DB, decode_responses=True)
        r.ping()
        print(f"✓ 已连接到Redis {REDIS_HOST}:{REDIS_PORT}")
        return r
    except Exception as e:
        print(f"✗ Redis连接失败: {e}")
        sys.exit(1)

def create_temperature_monitor_rule():
    """
    创建温度监控规则（DAG格式）
    当温度超过30°C时，开启冷却设备
    """
    rule = {
        "id": "temp_monitor_001",
        "name": "温度监控规则",
        "description": "当温度超过30°C时自动开启冷却设备",
        "enabled": True,
        "priority": 10,
        "nodes": [
            {
                "id": "input_temp",
                "name": "读取温度",
                "type": "Input",
                "config": {
                    "source": "1001:m:10001"  # 通道1001的测量点10001
                }
            },
            {
                "id": "check_temp",
                "name": "检查温度阈值",
                "type": "Condition",
                "config": {
                    "expression": "$input_temp > 30"
                }
            },
            {
                "id": "action_cooling",
                "name": "开启冷却设备",
                "type": "Action",
                "config": {
                    "action_type": "control",
                    "channel_id": "1001",
                    "point_type": "c",
                    "point_id": "30001",
                    "value": 1,
                    "description": "开启冷却设备"
                }
            }
        ],
        "edges": [
            {
                "from": "input_temp",
                "to": "check_temp",
                "condition": None
            },
            {
                "from": "check_temp",
                "to": "action_cooling",
                "condition": "$check_temp == true"
            }
        ]
    }
    return rule

def create_power_limit_rule():
    """
    创建功率限制规则（DAG格式）
    当功率超过阈值时，降低负载
    """
    rule = {
        "id": "power_limit_001",
        "name": "功率限制规则",
        "description": "当总功率超过阈值时自动降低负载",
        "enabled": True,
        "priority": 20,
        "nodes": [
            {
                "id": "input_power",
                "name": "读取功率",
                "type": "Input",
                "config": {
                    "source": "1001:m:10002"  # 功率测量点
                }
            },
            {
                "id": "scale_power",
                "name": "功率单位转换",
                "type": "Transform",
                "config": {
                    "transform_type": "scale",
                    "input": {
                        "value_expr": "$input_power",
                        "factor": 0.001  # W转kW
                    }
                }
            },
            {
                "id": "check_threshold",
                "name": "检查功率阈值",
                "type": "Transform",
                "config": {
                    "transform_type": "threshold",
                    "input": {
                        "value_expr": "$scale_power",
                        "threshold": 100  # 100kW
                    }
                }
            },
            {
                "id": "action_reduce",
                "name": "降低负载",
                "type": "Action",
                "config": {
                    "action_type": "control",
                    "channel_id": "1001",
                    "point_type": "a",  # 调节点
                    "point_id": "40001",
                    "value": 80,  # 降到80%
                    "description": "降低负载到80%"
                }
            }
        ],
        "edges": [
            {
                "from": "input_power",
                "to": "scale_power"
            },
            {
                "from": "scale_power",
                "to": "check_threshold"
            },
            {
                "from": "check_threshold",
                "to": "action_reduce",
                "condition": "$check_threshold == true"
            }
        ]
    }
    return rule

def create_aggregate_alarm_rule():
    """
    创建聚合告警规则（DAG格式）
    多个条件同时满足时触发告警
    """
    rule = {
        "id": "aggregate_alarm_001",
        "name": "综合告警规则",
        "description": "温度高且功率大时触发告警",
        "enabled": True,
        "priority": 30,
        "nodes": [
            # 输入节点
            {
                "id": "input_temp",
                "name": "读取温度",
                "type": "Input",
                "config": {
                    "source": "1001:m:10001"
                }
            },
            {
                "id": "input_power",
                "name": "读取功率",
                "type": "Input",
                "config": {
                    "source": "1001:m:10002"
                }
            },
            # 条件节点
            {
                "id": "check_temp_high",
                "name": "温度过高",
                "type": "Condition",
                "config": {
                    "expression": "$input_temp > 35"
                }
            },
            {
                "id": "check_power_high",
                "name": "功率过高",
                "type": "Condition",
                "config": {
                    "expression": "$input_power > 120000"  # 120kW
                }
            },
            # 聚合节点
            {
                "id": "aggregate_conditions",
                "name": "条件聚合",
                "type": "Aggregate",
                "config": {
                    "aggregation_type": "and",
                    "inputs": ["check_temp_high", "check_power_high"]
                }
            },
            # 动作节点
            {
                "id": "action_alarm",
                "name": "触发告警",
                "type": "Action",
                "config": {
                    "action_type": "alarm",
                    "alarm_level": "critical",
                    "message": "温度和功率同时超限",
                    "device_id": "device_001"
                }
            }
        ],
        "edges": [
            {"from": "input_temp", "to": "check_temp_high"},
            {"from": "input_power", "to": "check_power_high"},
            {"from": "check_temp_high", "to": "aggregate_conditions"},
            {"from": "check_power_high", "to": "aggregate_conditions"},
            {
                "from": "aggregate_conditions",
                "to": "action_alarm",
                "condition": "$aggregate_conditions == true"
            }
        ]
    }
    return rule

def create_modsrv_output_rule():
    """
    创建基于modsrv输出的规则
    监听模型计算结果并执行动作
    """
    rule = {
        "id": "modsrv_output_001",
        "name": "模型输出响应规则",
        "description": "根据modsrv模型输出执行控制",
        "enabled": True,
        "priority": 15,
        "nodes": [
            {
                "id": "input_model",
                "name": "读取模型输出",
                "type": "Input",
                "config": {
                    "source": "modsrv:model1:efficiency"  # modsrv模型输出
                }
            },
            {
                "id": "check_efficiency",
                "name": "检查效率",
                "type": "Condition",
                "config": {
                    "expression": "$input_model < 0.85"  # 效率低于85%
                }
            },
            {
                "id": "action_optimize",
                "name": "优化运行参数",
                "type": "Action",
                "config": {
                    "action_type": "control",
                    "control_id": "optimize_params",
                    "parameters": {
                        "mode": "efficiency",
                        "target": 0.90
                    }
                }
            }
        ],
        "edges": [
            {"from": "input_model", "to": "check_efficiency"},
            {
                "from": "check_efficiency",
                "to": "action_optimize",
                "condition": "$check_efficiency == true"
            }
        ]
    }
    return rule

def save_rule_to_redis(r, rule):
    """保存规则到Redis"""
    rule_key = f"rule:{rule['id']}"
    rule_json = json.dumps(rule, ensure_ascii=False)
    
    try:
        # 保存规则
        r.set(rule_key, rule_json)
        
        # 添加到规则列表
        r.sadd("rulesrv:rules", rule['id'])
        
        print(f"✓ 已保存规则: {rule['name']} (ID: {rule['id']})")
        print(f"  键: {rule_key}")
        print(f"  节点数: {len(rule['nodes'])}")
        print(f"  边数: {len(rule['edges'])}")
        
    except Exception as e:
        print(f"✗ 保存规则失败: {e}")

def list_saved_rules(r):
    """列出已保存的规则"""
    print("\n已保存的规则列表:")
    print("-" * 60)
    
    rule_ids = r.smembers("rulesrv:rules")
    if not rule_ids:
        print("没有找到任何规则")
        return
    
    for rule_id in sorted(rule_ids):
        rule_key = f"rule:{rule_id}"
        rule_json = r.get(rule_key)
        if rule_json:
            rule = json.loads(rule_json)
            status = "启用" if rule.get('enabled', False) else "禁用"
            print(f"- {rule['name']} (ID: {rule_id}) - {status}")
            print(f"  描述: {rule.get('description', 'N/A')}")
            print(f"  优先级: {rule.get('priority', 0)}")

def delete_rule(r, rule_id):
    """删除规则"""
    rule_key = f"rule:{rule_id}"
    
    if r.exists(rule_key):
        r.delete(rule_key)
        r.srem("rulesrv:rules", rule_id)
        print(f"✓ 已删除规则: {rule_id}")
    else:
        print(f"✗ 规则不存在: {rule_id}")

def main():
    parser = argparse.ArgumentParser(description='规则定义和存储测试脚本')
    parser.add_argument('--action', choices=['create', 'list', 'delete', 'create-all'], 
                        default='create-all', help='操作类型')
    parser.add_argument('--rule-id', help='规则ID（用于删除操作）')
    parser.add_argument('--redis-host', default=REDIS_HOST, help='Redis主机')
    parser.add_argument('--redis-port', type=int, default=REDIS_PORT, help='Redis端口')
    
    args = parser.parse_args()
    
    # 更新Redis配置
    global REDIS_HOST, REDIS_PORT
    REDIS_HOST = args.redis_host
    REDIS_PORT = args.redis_port
    
    # 创建Redis连接
    r = create_redis_connection()
    
    if args.action == 'create-all':
        print("\n创建示例规则...")
        print("=" * 60)
        
        # 创建所有示例规则
        rules = [
            create_temperature_monitor_rule(),
            create_power_limit_rule(),
            create_aggregate_alarm_rule(),
            create_modsrv_output_rule()
        ]
        
        for rule in rules:
            save_rule_to_redis(r, rule)
            print()
        
        list_saved_rules(r)
        
    elif args.action == 'list':
        list_saved_rules(r)
        
    elif args.action == 'delete':
        if not args.rule_id:
            print("错误：删除操作需要指定 --rule-id")
            sys.exit(1)
        delete_rule(r, args.rule_id)
        
    print("\n规则访问方式:")
    print("- 通过Redis CLI查看: redis-cli get rule:<rule_id>")
    print("- 通过API查看: GET http://localhost:8086/api/v1/rules/<rule_id>")
    print("- 触发执行: POST http://localhost:8086/api/v1/rules/<rule_id>/execute")

if __name__ == "__main__":
    main()