#!/usr/bin/env python3
"""
ModSrv API综合测试脚本

这个脚本用于测试ModSrv服务的所有API端点，主要功能包括：
1. 健康检查API测试
2. 规则管理API测试（列表、创建、获取、更新、删除）
3. 规则执行API测试，包括简单规则和复杂DAG结构规则
4. 模板API和控制操作API测试（如果可用）

特别关注DAG结构规则，用不同场景测试规则执行的正确性。

运行方式：
$ python test-api.py

注意：
- 确保ModSrv服务已经启动并在端口8000上运行
- 这个测试会创建多个测试规则并在完成后清理
"""
import requests
import json
import sys
import random
import time

BASE_URL = "http://localhost:8000/api"

def test_health():
    """测试健康检查端点"""
    print("Testing health endpoint...")
    response = requests.get(f"{BASE_URL}/health")
    print(f"Status code: {response.status_code}")
    print(f"Response: {response.json()}")
    return response.status_code == 200

def test_templates():
    """测试模板列表端点"""
    print("\nTesting templates endpoint...")
    try:
        response = requests.get(f"{BASE_URL}/templates")
        print(f"Status code: {response.status_code}")
        
        if response.status_code == 200:
            templates = response.json().get("templates", [])
            print(f"Found {len(templates)} templates:")
            for template in templates:
                print(f"  - {template['name']}: {template['description']}")
            return True
        else:
            print(f"Error: Templates endpoint not available or returned error {response.status_code}")
            return False
    except requests.exceptions.JSONDecodeError:
        print("Error: Could not decode JSON response from templates endpoint")
        return False
    except Exception as e:
        print(f"Error accessing templates endpoint: {str(e)}")
        return False

def test_create_instance():
    """测试创建实例端点"""
    print("\nTesting instance creation...")
    # Generate a unique instance ID using timestamp and random number
    instance_id = f"motor_{int(time.time())}_{random.randint(1000, 9999)}"
    
    payload = {
        "template_id": "stepper_motor_template",
        "instance_id": instance_id,
        "config": {
            "name": "Test Motor",
            "description": "A test stepper motor instance",
            "parameters": {
                "max_speed": 2000,
                "acceleration": 500
            }
        }
    }
    
    print(f"Creating instance with ID: {instance_id}")
    response = requests.post(f"{BASE_URL}/instances", json=payload)
    print(f"Status code: {response.status_code}")
    if response.status_code == 200 or response.status_code == 201:
        print(f"Response: {response.json()}")
        return True
    else:
        print(f"Error: {response.text}")
        return False

def test_control_operations():
    """测试控制操作端点"""
    print("\nTesting control operations...")
    operations = requests.get(f"{BASE_URL}/control/operations")
    print(f"Status code: {operations.status_code}")
    if operations.status_code == 200:
        ops = operations.json()
        print(f"Available operations: {ops}")
        
        if isinstance(ops, list) and len(ops) > 0:
            # Try to execute the first operation
            op_name = ops[0]
            print(f"\nExecuting operation: {op_name}")
            payload = {
                "instance_id": "motor002",
                "parameters": {
                    "speed": 1000
                }
            }
            response = requests.post(f"{BASE_URL}/control/execute/{op_name}", json=payload)
            print(f"Status code: {response.status_code}")
            print(f"Response: {response.json() if response.status_code == 200 else response.text}")
            return response.status_code == 200
        else:
            print("No operations available to test.")
            return True  # Return true because the API is working as expected
    return operations.status_code == 200

def create_simple_rule():
    """创建一个简单的规则"""
    rule_id = f"api_test_rule_{int(time.time())}_{random.randint(1000, 9999)}"
    
    test_rule = {
        "id": rule_id,
        "name": "API Test Rule",
        "description": "Rule created by the API test script",
        "enabled": True,
        "priority": 1,
        "type": "simple",
        "nodes": [
            {
                "id": "input1",
                "name": "Test Input",
                "type": "Input",
                "config": {
                    "device_id": "test_device",
                    "data_points": ["status"]
                }
            },
            {
                "id": "action1",
                "name": "Test Action",
                "type": "Action",
                "config": {
                    "type": "notify",
                    "target": "system",
                    "message": "Test notification"
                }
            }
        ],
        "edges": [
            {
                "from": "input1",
                "to": "action1"
            }
        ]
    }
    
    return rule_id, test_rule

def create_complex_dag_rule():
    """创建一个复杂的DAG结构规则"""
    rule_id = f"dag_test_rule_{int(time.time())}_{random.randint(1000, 9999)}"
    
    # 创建具有多输入、转换、条件和多分支的复杂DAG结构
    dag_rule = {
        "id": rule_id,
        "name": "Complex DAG Test Rule",
        "description": "A complex rule with DAG structure for comprehensive testing",
        "enabled": True,
        "priority": 2,
        "type": "complex_dag",
        "nodes": [
            # 输入节点
            {
                "id": "temp_input",
                "name": "Temperature Input",
                "type": "Input",
                "config": {
                    "device_id": "sensor_001",
                    "data_points": ["temperature"]
                }
            },
            {
                "id": "humidity_input",
                "name": "Humidity Input",
                "type": "Input",
                "config": {
                    "device_id": "sensor_002",
                    "data_points": ["humidity"]
                }
            },
            {
                "id": "pressure_input",
                "name": "Pressure Input",
                "type": "Input",
                "config": {
                    "device_id": "sensor_003",
                    "data_points": ["pressure"]
                }
            },
            
            # 转换节点
            {
                "id": "temp_transform",
                "name": "Temperature Transform",
                "type": "Transform",
                "config": {
                    "formula": "node.temp_input.result.temperature * 1.8 + 32" # 摄氏度转华氏度
                }
            },
            {
                "id": "humidity_transform",
                "name": "Humidity Classification",
                "type": "Transform",
                "config": {
                    "formula": "node.humidity_input.result.humidity > 70 ? 'HIGH' : 'NORMAL'"
                }
            },
            
            # 条件节点
            {
                "id": "temp_condition",
                "name": "High Temperature Condition",
                "type": "Condition",
                "config": {
                    "condition": "node.temp_transform.result > 85" # 华氏度温度检查
                }
            },
            {
                "id": "pressure_condition",
                "name": "Low Pressure Condition",
                "type": "Condition",
                "config": {
                    "condition": "node.pressure_input.result.pressure < 980"
                }
            },
            
            # 聚合节点
            {
                "id": "condition_aggregator",
                "name": "Condition Aggregator",
                "type": "Aggregate",
                "config": {
                    "type": "any",
                    "inputs": ["temp_condition", "pressure_condition"]
                }
            },
            
            # 动作节点
            {
                "id": "alert_action",
                "name": "Send Alert",
                "type": "Action",
                "config": {
                    "type": "notify",
                    "target": "admin",
                    "message": "Environmental alert triggered!"
                }
            },
            {
                "id": "log_action",
                "name": "Log Event",
                "type": "Action",
                "config": {
                    "type": "log",
                    "level": "warning",
                    "message": "Abnormal environmental conditions detected"
                }
            },
            {
                "id": "humidity_alert_action",
                "name": "Humidity Alert",
                "type": "Action",
                "config": {
                    "type": "notify",
                    "target": "operator",
                    "message": "High humidity level detected"
                }
            }
        ],
        "edges": [
            # 温度处理路径
            {"from": "temp_input", "to": "temp_transform"},
            {"from": "temp_transform", "to": "temp_condition"},
            {"from": "temp_condition", "to": "condition_aggregator"},
            
            # 湿度处理路径 - 直接到动作
            {"from": "humidity_input", "to": "humidity_transform"},
            {"from": "humidity_transform", "to": "humidity_alert_action"},
            
            # 压力处理路径
            {"from": "pressure_input", "to": "pressure_condition"},
            {"from": "pressure_condition", "to": "condition_aggregator"},
            
            # 聚合条件到动作
            {"from": "condition_aggregator", "to": "alert_action"},
            {"from": "condition_aggregator", "to": "log_action"}
        ]
    }
    
    return rule_id, dag_rule

def test_rules_list():
    """测试规则列表端点"""
    print("\nTesting rules list endpoint...")
    response = requests.get(f"{BASE_URL}/rules")
    print(f"Status code: {response.status_code}")
    
    if response.status_code == 200:
        rules = response.json().get("rules", [])
        print(f"Found {len(rules)} rules")
        for rule in rules:
            print(f"  - {rule['id']}: {rule['name']}")
        return True
    else:
        print(f"Error: {response.text}")
        return False

def test_rule_creation(rule_id, rule_data):
    """测试创建规则端点"""
    print(f"\nTesting rule creation: {rule_id}")
    response = requests.post(f"{BASE_URL}/rules", json=rule_data)
    print(f"Status code: {response.status_code}")
    
    if response.status_code == 201 or response.status_code == 200:
        print("Rule created successfully")
        return True
    else:
        print(f"Error: {response.text}")
        return False

def test_get_rule(rule_id):
    """测试获取规则端点"""
    print(f"\nTesting get rule endpoint: {rule_id}")
    response = requests.get(f"{BASE_URL}/rules/{rule_id}")
    print(f"Status code: {response.status_code}")
    
    if response.status_code == 200:
        rule = response.json().get("rule", {})
        print(f"Rule name: {rule.get('name')}")
        print(f"Nodes count: {len(rule.get('nodes', []))}")
        print(f"Edges count: {len(rule.get('edges', []))}")
        return rule
    else:
        print(f"Error: {response.text}")
        return None

def test_update_rule(rule_id, rule_data):
    """测试更新规则端点"""
    print(f"\nTesting update rule endpoint: {rule_id}")
    
    # 修改规则描述
    rule_data["description"] = f"Updated description at {time.time()}"
    
    response = requests.put(f"{BASE_URL}/rules/{rule_id}", json=rule_data)
    print(f"Status code: {response.status_code}")
    
    if response.status_code == 200:
        print("Rule updated successfully")
        return True
    else:
        print(f"Error: {response.text}")
        return False

def test_execute_rule(rule_id):
    """测试规则执行端点"""
    print(f"\nTesting rule execution endpoint: {rule_id}")
    
    # 创建测试数据
    test_data = {
        "device_data": {
            "sensor_001": {
                "temperature": 35  # 高温 -> 华氏度95度
            },
            "sensor_002": {
                "humidity": 75     # 高湿度
            },
            "sensor_003": {
                "pressure": 975    # 低气压
            },
            "test_device": {
                "status": "active"
            }
        }
    }
    
    response = requests.post(f"{BASE_URL}/rules/{rule_id}/execute", json=test_data)
    print(f"Status code: {response.status_code}")
    
    if response.status_code == 200:
        result = response.json()
        print(f"Execution result (summary): {json.dumps(result, indent=2)[:500]}...")
        return True
    else:
        print(f"Error: {response.text}")
        return False

def test_dag_execution_scenarios(rule_id):
    """测试DAG规则在不同输入场景下的执行"""
    print(f"\nTesting DAG rule with different execution scenarios: {rule_id}")
    
    # 场景1: 所有条件都满足
    print("\nScenario 1: All conditions met")
    scenario1 = {
        "device_data": {
            "sensor_001": {"temperature": 35},  # 高温 -> 华氏度95度
            "sensor_002": {"humidity": 75},     # 高湿度
            "sensor_003": {"pressure": 975}     # 低气压
        }
    }
    
    # 场景2: 只有温度条件满足
    print("\nScenario 2: Only temperature condition met")
    scenario2 = {
        "device_data": {
            "sensor_001": {"temperature": 35},  # 高温 -> 华氏度95度
            "sensor_002": {"humidity": 50},     # 正常湿度
            "sensor_003": {"pressure": 1010}    # 正常气压
        }
    }
    
    # 场景3: 只有湿度条件满足
    print("\nScenario 3: Only humidity condition met")
    scenario3 = {
        "device_data": {
            "sensor_001": {"temperature": 25},  # 正常温度 -> 华氏度77度
            "sensor_002": {"humidity": 85},     # 高湿度
            "sensor_003": {"pressure": 1010}    # 正常气压
        }
    }
    
    # 执行每个场景并验证
    scenarios = [
        ("Scenario 1", scenario1),
        ("Scenario 2", scenario2),
        ("Scenario 3", scenario3)
    ]
    
    results = []
    for name, scenario in scenarios:
        print(f"\nExecuting {name}")
        response = requests.post(f"{BASE_URL}/rules/{rule_id}/execute", json=scenario)
        if response.status_code == 200:
            result = response.json()
            print(f"Result: {json.dumps(result, indent=2)[:300]}...")
            results.append((name, True, result))
        else:
            print(f"Error: {response.text}")
            results.append((name, False, None))
    
    print("\nDAG Execution Scenarios Summary:")
    for name, success, _ in results:
        print(f"  - {name}: {'SUCCESS' if success else 'FAILED'}")
    
    return all(success for _, success, _ in results)

def test_delete_rule(rule_id):
    """测试删除规则端点"""
    print(f"\nTesting delete rule endpoint: {rule_id}")
    response = requests.delete(f"{BASE_URL}/rules/{rule_id}")
    print(f"Status code: {response.status_code}")
    
    if response.status_code == 200:
        print("Rule deleted successfully")
        
        # 验证规则是否已被删除（尝试获取规则）
        verify = requests.get(f"{BASE_URL}/rules/{rule_id}")
        print(f"Verification status code: {verify.status_code}")
        
        # 检查返回的内容，可能是404或200但内容为空或错误消息
        if verify.status_code == 404:
            print("Verified: Rule no longer exists (404 Not Found)")
            return True
        elif verify.status_code == 200:
            # 检查响应中是否包含错误信息
            try:
                response_json = verify.json()
                if "error" in response_json.get("status", "").lower() or "not found" in response_json.get("message", "").lower():
                    print("Verified: Rule no longer exists (status=error)")
                    return True
                else:
                    print(f"Warning: Rule still exists with response: {response_json}")
                    return False
            except:
                print(f"Warning: Could not parse response JSON")
                return False
        else:
            print(f"Warning: Unexpected status code: {verify.status_code}")
            return False
    else:
        print(f"Error: {response.text}")
        return False

def main():
    print("ModSrv Comprehensive API Test Script")
    print("=====================================")
    
    if not test_health():
        print("Health check failed, aborting further tests.")
        return 1
    
    # 测试规则API
    print("\n## Testing Rules API ##")
    
    # 列出现有规则
    test_rules_list()
    
    # 创建并测试一个简单规则
    simple_rule_id, simple_rule_data = create_simple_rule()
    if test_rule_creation(simple_rule_id, simple_rule_data):
        simple_rule = test_get_rule(simple_rule_id)
        if simple_rule:
            test_update_rule(simple_rule_id, simple_rule)
            test_execute_rule(simple_rule_id)
    
    # 创建并测试一个复杂DAG规则
    dag_rule_id, dag_rule_data = create_complex_dag_rule()
    if test_rule_creation(dag_rule_id, dag_rule_data):
        dag_rule = test_get_rule(dag_rule_id)
        if dag_rule:
            print("\n## Testing DAG Structure Rule ##")
            test_update_rule(dag_rule_id, dag_rule)
            test_execute_rule(dag_rule_id)
            test_dag_execution_scenarios(dag_rule_id)
            
    # 测试模板和实例API (如果可用)
    print("\n## Testing Templates and Instances API ##")
    templates_available = test_templates()
    if templates_available:
        test_create_instance()
    else:
        print("Skipping instance creation test as templates API is not available")
    
    # 测试控制操作API
    print("\n## Testing Control Operations API ##")
    test_control_operations()
    
    # 清理测试数据
    print("\n## Cleaning up test data ##")
    if simple_rule_id:
        test_delete_rule(simple_rule_id)
    if dag_rule_id:
        test_delete_rule(dag_rule_id)
    
    # 最终验证
    print("\n## Final verification ##")
    test_rules_list()
    
    print("\nAll tests completed!")
    return 0

if __name__ == "__main__":
    sys.exit(main()) 