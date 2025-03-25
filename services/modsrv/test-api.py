#!/usr/bin/env python3
"""
ModSrv API综合测试脚本（Docker环境适配版）

这个脚本用于测试ModSrv服务的所有API端点，主要功能包括：
1. 健康检查API测试
2. 规则管理API测试（列表、创建、获取、更新、删除）
3. 规则执行API测试，包括简单规则和复杂DAG结构规则
4. 模板API和控制操作API测试（如果可用）

特别关注DAG结构规则，用不同场景测试规则执行的正确性。

Docker环境增强功能：
1. 支持通过环境变量配置主机和端口
2. 增加服务可用性检测和重试机制
3. 增加测试统计和结果输出
4. 支持在CI/CD流程中的集成

运行方式：
$ python test-api.py

在Docker中运行：
$ docker run -e MODSRV_HOST=modsrv -e MODSRV_PORT=8000 --network=voltageems_network voltageems/modsrv-tester

注意：
- 确保ModSrv服务已经启动并可访问
- 这个测试会创建多个测试规则并在完成后清理
"""
import requests
import json
import sys
import random
import time
import os
import socket
from datetime import datetime
from requests.exceptions import RequestException

# 从环境变量获取主机和端口配置，允许在Docker中通过环境变量指定
MODSRV_HOST = os.environ.get('MODSRV_HOST', 'localhost')
MODSRV_PORT = os.environ.get('MODSRV_PORT', '8000')
BASE_URL = f"http://{MODSRV_HOST}:{MODSRV_PORT}"

# 测试配置
MAX_RETRIES = 5  # 最大重试次数
RETRY_INTERVAL = 2  # 重试间隔（秒）
STARTUP_WAIT_TIME = 3  # 服务启动等待时间（秒）
REQUEST_TIMEOUT = 10  # 请求超时时间（秒）

# 测试计数器
test_results = {
    "passed": 0,
    "failed": 0,
    "skipped": 0,
    "total": 0
}

def log_test_result(test_name, success, error_msg=None):
    """记录测试结果并更新统计"""
    test_results["total"] += 1
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    
    if success:
        test_results["passed"] += 1
        print(f"[PASS] {timestamp} - {test_name}")
    else:
        test_results["failed"] += 1
        msg = f" - Error: {error_msg}" if error_msg else ""
        print(f"[FAIL] {timestamp} - {test_name}{msg}")

def wait_for_service():
    """等待服务可用，适用于Docker容器刚启动的情况"""
    print(f"Waiting for ModSrv service at {MODSRV_HOST}:{MODSRV_PORT}...")
    start_time = time.time()
    max_wait_time = 60  # 最大等待时间（秒）
    
    while time.time() - start_time < max_wait_time:
        try:
            # 尝试连接服务
            sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            sock.settimeout(1)
            result = sock.connect_ex((MODSRV_HOST, int(MODSRV_PORT)))
            sock.close()
            
            if result == 0:
                print(f"Service is available at {MODSRV_HOST}:{MODSRV_PORT}")
                # 服务端口已开放，再等待几秒确保服务完全启动
                time.sleep(STARTUP_WAIT_TIME)
                return True
        except Exception as e:
            print(f"Exception during connection check: {e}")
        
        print(f"Waiting for service to be available... ({int(time.time() - start_time)}s)")
        time.sleep(2)
    
    print(f"Service not available after {max_wait_time} seconds")
    return False

def make_request(method, url, json_data=None, retry=True):
    """发送HTTP请求，支持重试和错误处理"""
    retries = 0
    while retries < MAX_RETRIES:
        try:
            if method.lower() == 'get':
                response = requests.get(url, timeout=REQUEST_TIMEOUT)
            elif method.lower() == 'post':
                response = requests.post(url, json=json_data, timeout=REQUEST_TIMEOUT)
            elif method.lower() == 'put':
                response = requests.put(url, json=json_data, timeout=REQUEST_TIMEOUT)
            elif method.lower() == 'delete':
                response = requests.delete(url, timeout=REQUEST_TIMEOUT)
            else:
                raise ValueError(f"Unsupported HTTP method: {method}")
            
            return response
        except RequestException as e:
            retries += 1
            if not retry or retries >= MAX_RETRIES:
                raise
            print(f"Request failed. Retrying ({retries}/{MAX_RETRIES})... Error: {e}")
            time.sleep(RETRY_INTERVAL)
    
    raise Exception(f"Failed after {MAX_RETRIES} retries")

def test_health():
    """测试健康检查端点"""
    print("\nTesting health endpoint...")
    
    # 尝试多个可能的健康检查路径
    health_paths = [
        "/health",           # 直接在基础URL后的health
        "",                  # 基础URL本身
        "/api/health",       # 标准的API health路径
        "/health/status",    # 一些API使用的health路径
        "/v1/health"         # 版本化的API路径
    ]
    
    for path in health_paths:
        try:
            print(f"尝试健康检查路径: {path}")
            response = make_request('get', f"{BASE_URL}{path}", retry=False)
            print(f"Status code: {response.status_code}")
            
            # 如果是200或任何成功的状态，则通过
            if 200 <= response.status_code < 300:
                print(f"Response: {response.json() if response.headers.get('content-type', '').startswith('application/json') else response.text}")
                log_test_result("Health endpoint", True)
                return True
            else:
                print(f"Response: {response.text}")
        except Exception as e:
            print(f"Error: {str(e)}")
    
    # 如果所有路径都失败，则记录失败
    log_test_result("Health endpoint", False, "所有健康检查路径均失败")
    return False

def test_templates():
    """测试模板列表端点"""
    print("\nTesting templates endpoint...")
    try:
        response = make_request('get', f"{BASE_URL}/api/templates")
        print(f"Status code: {response.status_code}")
        
        if response.status_code == 200:
            templates = response.json().get("templates", [])
            print(f"Found {len(templates)} templates:")
            for template in templates:
                print(f"  - {template['name']}: {template['description']}")
            log_test_result("Templates endpoint", True)
            return True
        else:
            print(f"Error: Templates endpoint not available or returned error {response.status_code}")
            log_test_result("Templates endpoint", False, f"HTTP {response.status_code}")
            return False
    except requests.exceptions.JSONDecodeError as e:
        log_test_result("Templates endpoint", False, f"JSON decode error: {str(e)}")
        return False
    except Exception as e:
        log_test_result("Templates endpoint", False, str(e))
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
    try:
        response = make_request('post', f"{BASE_URL}/api/instances", payload)
        print(f"Status code: {response.status_code}")
        if response.status_code == 200 or response.status_code == 201:
            print(f"Response: {response.json()}")
            log_test_result("Instance creation", True)
            return True
        elif response.status_code == 404:
            # 实例创建功能可能尚未实现
            print("Instance creation API not implemented, skipping test")
            test_results["skipped"] += 1
            return False
        else:
            print(f"Error: {response.text}")
            log_test_result("Instance creation", False, f"HTTP {response.status_code}")
            return False
    except Exception as e:
        log_test_result("Instance creation", False, str(e))
        return False

def test_control_operations():
    """测试控制操作端点"""
    print("\nTesting control operations...")
    try:
        operations = make_request('get', f"{BASE_URL}/api/control/operations")
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
                try:
                    response = make_request('post', f"{BASE_URL}/api/control/execute/{op_name}", payload)
                    print(f"Status code: {response.status_code}")
                    print(f"Response: {response.json() if response.status_code == 200 else response.text}")
                    success = response.status_code == 200
                    log_test_result(f"Execute operation {op_name}", success)
                    return success
                except Exception as e:
                    log_test_result(f"Execute operation {op_name}", False, str(e))
                    return False
            else:
                print("No operations available to test.")
                log_test_result("Control operations - no operations", True)
                return True  # Return true because the API is working as expected
        elif operations.status_code == 404:
            # 控制操作API可能尚未实现
            print("Control operations API not implemented, skipping test")
            test_results["skipped"] += 1
            return False
        else:
            log_test_result("Control operations", False, f"HTTP {operations.status_code}")
            return False
    except Exception as e:
        log_test_result("Control operations", False, str(e))
        return False

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
    try:
        response = make_request('get', f"{BASE_URL}/api/rules")
        print(f"Status code: {response.status_code}")
        
        if response.status_code == 200:
            rules = response.json().get("rules", [])
            print(f"Found {len(rules)} rules")
            for rule in rules:
                print(f"  - {rule['id']}: {rule['name']}")
            log_test_result("Rules list endpoint", True)
            return True
        else:
            print(f"Error: {response.text}")
            log_test_result("Rules list endpoint", False, f"HTTP {response.status_code}")
            return False
    except Exception as e:
        log_test_result("Rules list endpoint", False, str(e))
        return False

def test_rule_creation(rule_id, rule_data):
    """测试创建规则端点"""
    print(f"\nTesting rule creation: {rule_id}")
    try:
        response = make_request('post', f"{BASE_URL}/api/rules", rule_data)
        print(f"Status code: {response.status_code}")
        
        if response.status_code == 201 or response.status_code == 200:
            print("Rule created successfully")
            log_test_result(f"Rule creation: {rule_id}", True)
            return True
        else:
            print(f"Error: {response.text}")
            log_test_result(f"Rule creation: {rule_id}", False, f"HTTP {response.status_code}")
            return False
    except Exception as e:
        log_test_result(f"Rule creation: {rule_id}", False, str(e))
        return False

def test_get_rule(rule_id):
    """测试获取规则端点"""
    print(f"\nTesting get rule endpoint: {rule_id}")
    try:
        response = make_request('get', f"{BASE_URL}/api/rules/{rule_id}")
        print(f"Status code: {response.status_code}")
        
        if response.status_code == 200:
            rule = response.json().get("rule", {})
            print(f"Rule name: {rule.get('name')}")
            print(f"Nodes count: {len(rule.get('nodes', []))}")
            print(f"Edges count: {len(rule.get('edges', []))}")
            log_test_result(f"Get rule: {rule_id}", True)
            return rule
        else:
            print(f"Error: {response.text}")
            log_test_result(f"Get rule: {rule_id}", False, f"HTTP {response.status_code}")
            return None
    except Exception as e:
        log_test_result(f"Get rule: {rule_id}", False, str(e))
        return None

def test_update_rule(rule_id, rule_data):
    """测试更新规则端点"""
    print(f"\nTesting update rule endpoint: {rule_id}")
    
    # 修改规则描述
    rule_data["description"] = f"Updated description at {time.time()}"
    
    try:
        response = make_request('put', f"{BASE_URL}/api/rules/{rule_id}", rule_data)
        print(f"Status code: {response.status_code}")
        
        if response.status_code == 200:
            print("Rule updated successfully")
            log_test_result(f"Update rule: {rule_id}", True)
            return True
        else:
            print(f"Error: {response.text}")
            log_test_result(f"Update rule: {rule_id}", False, f"HTTP {response.status_code}")
            return False
    except Exception as e:
        log_test_result(f"Update rule: {rule_id}", False, str(e))
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
    
    try:
        response = make_request('post', f"{BASE_URL}/api/rules/{rule_id}/execute", test_data)
        print(f"Status code: {response.status_code}")
        
        if response.status_code == 200:
            result = response.json()
            print(f"Execution result (summary): {json.dumps(result, indent=2)[:500]}...")
            log_test_result(f"Execute rule: {rule_id}", True)
            return True
        else:
            print(f"Error: {response.text}")
            log_test_result(f"Execute rule: {rule_id}", False, f"HTTP {response.status_code}")
            return False
    except Exception as e:
        log_test_result(f"Execute rule: {rule_id}", False, str(e))
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
        try:
            response = make_request('post', f"{BASE_URL}/api/rules/{rule_id}/execute", scenario)
            if response.status_code == 200:
                result = response.json()
                print(f"Result: {json.dumps(result, indent=2)[:300]}...")
                results.append((name, True, result))
                log_test_result(f"DAG execution - {name}", True)
            else:
                print(f"Error: {response.text}")
                results.append((name, False, None))
                log_test_result(f"DAG execution - {name}", False, f"HTTP {response.status_code}")
        except Exception as e:
            print(f"Error executing {name}: {e}")
            results.append((name, False, None))
            log_test_result(f"DAG execution - {name}", False, str(e))
    
    print("\nDAG Execution Scenarios Summary:")
    for name, success, _ in results:
        print(f"  - {name}: {'SUCCESS' if success else 'FAILED'}")
    
    all_passed = all(success for _, success, _ in results)
    log_test_result(f"DAG execution scenarios summary", all_passed)
    return all_passed

def test_delete_rule(rule_id):
    """测试删除规则端点"""
    print(f"\nTesting delete rule endpoint: {rule_id}")
    try:
        response = make_request('delete', f"{BASE_URL}/api/rules/{rule_id}")
        print(f"Status code: {response.status_code}")
        
        if response.status_code == 200:
            print("Rule deleted successfully")
            
            # 验证规则是否已被删除（尝试获取规则）
            verify = make_request('get', f"{BASE_URL}/api/rules/{rule_id}", retry=False)
            print(f"Verification status code: {verify.status_code}")
            
            # 检查返回的内容，可能是404或200但内容为空或错误消息
            if verify.status_code == 404:
                print("Verified: Rule no longer exists (404 Not Found)")
                log_test_result(f"Delete rule: {rule_id}", True)
                return True
            elif verify.status_code == 200:
                # 检查响应中是否包含错误信息
                try:
                    response_json = verify.json()
                    if "error" in response_json.get("status", "").lower() or "not found" in response_json.get("message", "").lower():
                        print("Verified: Rule no longer exists (status=error)")
                        log_test_result(f"Delete rule: {rule_id}", True)
                        return True
                    else:
                        print(f"Warning: Rule still exists with response: {response_json}")
                        log_test_result(f"Delete rule: {rule_id}", False, "Rule still exists after deletion")
                        return False
                except:
                    print(f"Warning: Could not parse response JSON")
                    log_test_result(f"Delete rule: {rule_id}", False, "Could not parse verification response")
                    return False
            else:
                print(f"Warning: Unexpected status code: {verify.status_code}")
                log_test_result(f"Delete rule: {rule_id}", False, f"Unexpected verification status: {verify.status_code}")
                return False
        else:
            print(f"Error: {response.text}")
            log_test_result(f"Delete rule: {rule_id}", False, f"HTTP {response.status_code}")
            return False
    except Exception as e:
        log_test_result(f"Delete rule: {rule_id}", False, str(e))
        return False

def print_summary():
    """打印测试摘要"""
    print("\n====================================")
    print("           TEST SUMMARY")
    print("====================================")
    print(f"Total tests:  {test_results['total']}")
    print(f"Passed:       {test_results['passed']}")
    print(f"Failed:       {test_results['failed']}")
    print(f"Skipped:      {test_results['skipped']}")
    print(f"Success rate: {test_results['passed'] / test_results['total'] * 100:.1f}%")
    print("====================================")
    
    return test_results["failed"] == 0

def main():
    print("ModSrv Comprehensive API Test Script (Docker Edition)")
    print("====================================================")
    print(f"Target service: {BASE_URL}")
    print(f"Date: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print("====================================================")
    
    # 等待服务可用，适用于Docker环境
    if not wait_for_service():
        print("Service not available, aborting tests.")
        sys.exit(1)
    
    # 健康检查
    if not test_health():
        print("Health check failed, aborting further tests.")
        print_summary()
        return 1
    
    # 测试规则API
    print("\n## Testing Rules API ##")
    
    # 列出现有规则
    test_rules_list()
    
    # 创建并测试一个简单规则
    simple_rule_id = None
    simple_rule_data = None
    try:
        simple_rule_id, simple_rule_data = create_simple_rule()
        if test_rule_creation(simple_rule_id, simple_rule_data):
            simple_rule = test_get_rule(simple_rule_id)
            if simple_rule:
                test_update_rule(simple_rule_id, simple_rule)
                test_execute_rule(simple_rule_id)
    except Exception as e:
        print(f"Error during simple rule tests: {e}")
        log_test_result("Simple rule tests", False, str(e))
    
    # 创建并测试一个复杂DAG规则
    dag_rule_id = None
    dag_rule_data = None
    try:
        dag_rule_id, dag_rule_data = create_complex_dag_rule()
        if test_rule_creation(dag_rule_id, dag_rule_data):
            dag_rule = test_get_rule(dag_rule_id)
            if dag_rule:
                print("\n## Testing DAG Structure Rule ##")
                test_update_rule(dag_rule_id, dag_rule)
                test_execute_rule(dag_rule_id)
                test_dag_execution_scenarios(dag_rule_id)
    except Exception as e:
        print(f"Error during DAG rule tests: {e}")
        log_test_result("DAG rule tests", False, str(e))
            
    # 测试模板和实例API (如果可用)
    print("\n## Testing Templates and Instances API ##")
    templates_available = test_templates()
    if templates_available:
        test_create_instance()
    else:
        print("Skipping instance creation test as templates API is not available")
        test_results["skipped"] += 1
    
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
    
    # 打印测试摘要
    all_passed = print_summary()
    
    return 0 if all_passed else 1

if __name__ == "__main__":
    try:
        exit_code = main()
        sys.exit(exit_code)
    except KeyboardInterrupt:
        print("\nTest interrupted by user")
        print_summary()
        sys.exit(130)
    except Exception as e:
        print(f"\nUnexpected error: {e}")
        print_summary()
        sys.exit(1) 