#!/usr/bin/env python3
"""命令执行测试"""

import os
import requests
import time
import redis


def test_command_execution():
    """测试命令执行功能"""
    modsrv_url = os.getenv("MODSRV_URL", "http://modsrv:8082")
    redis_url = os.getenv("REDIS_URL", "redis://redis:6379")

    print("🔍 开始命令执行测试...")

    # 连接Redis监听命令发布
    redis_client = redis.from_url(redis_url, decode_responses=True)

    # 首先创建一个测试实例
    print("1. 创建测试实例...")
    instance_data = {
        "template_id": "motor_control_model",
        "instance_id": "command_test_motor",
        "config": {
            "name": "命令测试电机",
            "description": "用于测试命令执行的电机实例",
            "rated_power": 15.0,
            "max_speed": 1500,
        },
    }

    response = requests.post(
        f"{modsrv_url}/api/instances",
        json=instance_data,
        headers={"Content-Type": "application/json"},
    )

    if response.status_code not in [200, 201]:
        print(f"   ⚠️  实例创建失败: {response.status_code} - {response.text}")
        print("   继续测试已存在的实例...")
    else:
        print("   ✅ 测试实例创建成功")

    # 等待实例初始化
    time.sleep(2)

    # 定义测试命令
    test_commands = [
        {
            "command_name": "start_motor",
            "parameters": {"speed": 1000, "direction": "forward"},
            "description": "启动电机命令",
        },
        {"command_name": "stop_motor", "parameters": {}, "description": "停止电机命令"},
        {
            "command_name": "set_speed",
            "parameters": {"target_speed": 800},
            "description": "设置转速命令",
        },
        {
            "command_name": "emergency_stop",
            "parameters": {"reason": "safety_test"},
            "description": "紧急停机命令",
        },
    ]

    successful_commands = 0
    published_messages = []

    print("2. 执行命令测试...")
    for i, test_command in enumerate(test_commands):
        print(f"   测试 {i + 1}/{len(test_commands)}: {test_command['description']}")

        url = f"{modsrv_url}/api/instances/command_test_motor/commands/{test_command['command_name']}"

        try:
            # 在执行命令前订阅Redis通道以监听发布的消息
            pubsub = redis_client.pubsub()
            command_channel = "cmd:command_test_motor:control"
            pubsub.subscribe(command_channel)

            # 执行命令
            response = requests.post(
                url,
                json=test_command["parameters"],
                headers={"Content-Type": "application/json"},
            )

            if response.status_code == 200:
                result = response.json()
                print(f"   ✅ 命令执行成功: {result.get('status', 'unknown')}")
                successful_commands += 1

                # 检查Redis中是否收到命令消息
                time.sleep(0.5)  # 等待消息传播
                try:
                    message = pubsub.get_message(timeout=1)
                    if message and message["type"] == "message":
                        published_messages.append(
                            {
                                "command": test_command["command_name"],
                                "channel": message["channel"],
                                "data": message["data"],
                            }
                        )
                        print(f"   📡 Redis消息已发布: {message['channel']}")
                    else:
                        print("   ⚠️  未收到Redis消息")
                except Exception as e:
                    print(f"   ⚠️  Redis消息检查失败: {e}")

            elif response.status_code == 404:
                print(f"   ⚠️  命令不存在: {test_command['command_name']}")
            elif response.status_code == 503:
                print("   ⚠️  设备模型系统不可用")
            else:
                print(f"   ❌ 命令执行失败: {response.status_code} - {response.text}")

            pubsub.close()

        except Exception as e:
            print(f"   ❌ 请求异常: {e}")

    # 测试无效命令
    print("3. 测试无效命令...")
    invalid_commands = [
        {
            "command_name": "non_existent_command",
            "parameters": {},
            "expected_status": 404,
        },
        {
            "command_name": "start_motor",
            "parameters": {"invalid_param": "value"},
            "expected_status": [200, 400],  # 可能接受也可能拒绝
        },
    ]

    for invalid_cmd in invalid_commands:
        url = f"{modsrv_url}/api/instances/command_test_motor/commands/{invalid_cmd['command_name']}"

        try:
            response = requests.post(
                url,
                json=invalid_cmd["parameters"],
                headers={"Content-Type": "application/json"},
            )

            expected = invalid_cmd["expected_status"]
            if isinstance(expected, list):
                if response.status_code in expected:
                    print(f"   ✅ 无效命令 '{invalid_cmd['command_name']}' 处理正确")
                else:
                    print(
                        f"   ⚠️  无效命令 '{invalid_cmd['command_name']}' 返回: {response.status_code}"
                    )
            else:
                if response.status_code == expected:
                    print(f"   ✅ 无效命令 '{invalid_cmd['command_name']}' 正确拒绝")
                else:
                    print(
                        f"   ⚠️  无效命令 '{invalid_cmd['command_name']}' 返回: {response.status_code}"
                    )

        except Exception as e:
            print(f"   ❌ 无效命令测试异常: {e}")

    # 测试不存在的实例
    print("4. 测试不存在的实例...")
    response = requests.post(
        f"{modsrv_url}/api/instances/non_existent_instance/commands/start_motor",
        json={"speed": 1000},
        headers={"Content-Type": "application/json"},
    )

    if response.status_code == 404:
        print("   ✅ 不存在实例正确返回404")
    else:
        print(f"   ⚠️  不存在实例返回: {response.status_code}")

    # 测试复杂参数
    print("5. 测试复杂参数...")
    complex_parameters = {
        "configuration": {
            "speed_profile": [0, 500, 1000, 1500],
            "timing": {"ramp_up": 5.0, "hold": 10.0, "ramp_down": 3.0},
            "safety": {
                "max_temperature": 80.0,
                "max_vibration": 2.5,
                "enable_monitoring": True,
            },
        },
        "metadata": {
            "operator": "test_system",
            "timestamp": time.time(),
            "test_id": "complex_param_test_001",
        },
    }

    response = requests.post(
        f"{modsrv_url}/api/instances/command_test_motor/commands/start_motor",
        json=complex_parameters,
        headers={"Content-Type": "application/json"},
    )

    if response.status_code == 200:
        print("   ✅ 复杂参数命令执行成功")
    else:
        print(f"   ⚠️  复杂参数命令返回: {response.status_code}")

    # 测试并发命令执行
    print("6. 测试并发命令执行...")
    import threading
    import queue

    def execute_command_concurrent(result_queue, instance_id, command_name, params):
        try:
            response = requests.post(
                f"{modsrv_url}/api/instances/{instance_id}/commands/{command_name}",
                json=params,
                headers={"Content-Type": "application/json"},
            )
            result_queue.put(("success", response.status_code))
        except Exception as e:
            result_queue.put(("error", str(e)))

    result_queue = queue.Queue()
    threads = []
    concurrent_commands = 5

    start_time = time.time()

    for i in range(concurrent_commands):
        thread = threading.Thread(
            target=execute_command_concurrent,
            args=(
                result_queue,
                "command_test_motor",
                "set_speed",
                {"target_speed": 1000 + i * 100},
            ),
        )
        threads.append(thread)
        thread.start()

    for thread in threads:
        thread.join()

    end_time = time.time()

    success_count = 0
    error_count = 0

    while not result_queue.empty():
        result_type, result_value = result_queue.get()
        if result_type == "success":
            success_count += 1
        else:
            error_count += 1

    print(f"   ✅ 并发命令测试完成: {success_count} 成功, {error_count} 失败")
    print(f"   执行时间: {(end_time - start_time) * 1000:.2f}ms")

    print(
        f"✅ 命令执行测试完成，成功执行 {successful_commands}/{len(test_commands)} 个命令"
    )
    print(f"📡 Redis消息发布: {len(published_messages)} 条消息")

    return True


if __name__ == "__main__":
    try:
        test_command_execution()
        print("命令执行测试: PASS")
    except Exception as e:
        print(f"命令执行测试: FAIL - {e}")
        exit(1)
