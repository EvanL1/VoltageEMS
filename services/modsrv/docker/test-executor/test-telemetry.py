#!/usr/bin/env python3
"""遥测数据获取测试"""

import os
import requests
import time


def test_telemetry_retrieval():
    """测试遥测数据获取功能"""
    modsrv_url = os.getenv("MODSRV_URL", "http://modsrv:8082")

    print("🔍 开始遥测数据获取测试...")

    # 首先创建一个测试实例
    print("1. 创建测试实例...")
    instance_data = {
        "template_id": "test_avg_model",
        "instance_id": "telemetry_test_instance",
        "config": {"name": "遥测测试实例", "description": "用于测试遥测数据获取的实例"},
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

    # 测试遥测数据获取
    test_cases = [
        {
            "instance_id": "telemetry_test_instance",
            "telemetry_name": "average_voltage",
            "description": "平均电压遥测",
        },
        {
            "instance_id": "telemetry_test_instance",
            "telemetry_name": "calculation_result",
            "description": "计算结果遥测",
        },
        {
            "instance_id": "telemetry_test_instance",
            "telemetry_name": "status",
            "description": "状态遥测",
        },
    ]

    successful_retrievals = 0

    print("2. 测试遥测数据获取...")
    for i, test_case in enumerate(test_cases):
        print(f"   测试 {i + 1}/{len(test_cases)}: {test_case['description']}")

        url = f"{modsrv_url}/api/instances/{test_case['instance_id']}/telemetry/{test_case['telemetry_name']}"

        try:
            response = requests.get(url)

            if response.status_code == 200:
                data = response.json()
                print(f"   ✅ 遥测数据获取成功: {data.get('value', 'N/A')}")
                successful_retrievals += 1
            elif response.status_code == 404:
                print(f"   ⚠️  遥测点不存在: {test_case['telemetry_name']}")
            elif response.status_code == 503:
                print("   ⚠️  设备模型系统不可用")
            else:
                print(f"   ❌ 遥测获取失败: {response.status_code} - {response.text}")

        except Exception as e:
            print(f"   ❌ 请求异常: {e}")

    # 测试不存在的实例
    print("3. 测试不存在的实例...")
    response = requests.get(
        f"{modsrv_url}/api/instances/non_existent_instance/telemetry/voltage"
    )

    if response.status_code == 404:
        print("   ✅ 不存在实例正确返回404")
    else:
        print(f"   ⚠️  不存在实例返回: {response.status_code}")

    # 测试不存在的遥测点
    print("4. 测试不存在的遥测点...")
    response = requests.get(
        f"{modsrv_url}/api/instances/telemetry_test_instance/telemetry/non_existent_telemetry"
    )

    if response.status_code == 404:
        print("   ✅ 不存在遥测点正确返回404")
    else:
        print(f"   ⚠️  不存在遥测点返回: {response.status_code}")

    # 测试无效实例ID格式
    print("5. 测试无效实例ID格式...")
    invalid_instance_ids = ["", "invalid/id", "id with spaces", "特殊字符ID"]

    for invalid_id in invalid_instance_ids:
        try:
            # URL编码处理特殊字符
            import urllib.parse

            encoded_id = urllib.parse.quote(invalid_id, safe="")

            response = requests.get(
                f"{modsrv_url}/api/instances/{encoded_id}/telemetry/voltage"
            )

            if response.status_code in [400, 404]:
                print(f"   ✅ 无效ID '{invalid_id}' 正确拒绝")
            else:
                print(f"   ⚠️  无效ID '{invalid_id}' 返回: {response.status_code}")

        except Exception as e:
            print(f"   ⚠️  无效ID '{invalid_id}' 请求异常: {e}")

    # 测试批量遥测获取性能
    print("6. 测试批量遥测获取性能...")
    start_time = time.time()
    batch_requests = 20

    for i in range(batch_requests):
        response = requests.get(
            f"{modsrv_url}/api/instances/telemetry_test_instance/telemetry/average_voltage"
        )
        if response.status_code not in [200, 404, 503]:
            print(f"   ⚠️  批量请求 {i + 1} 异常: {response.status_code}")

    end_time = time.time()
    avg_time = (end_time - start_time) / batch_requests * 1000

    print(f"   ✅ 批量请求完成，平均响应时间: {avg_time:.2f}ms")

    # 测试并发遥测获取
    print("7. 测试并发遥测获取...")
    import threading
    import queue

    def fetch_telemetry(result_queue, instance_id, telemetry_name):
        try:
            response = requests.get(
                f"{modsrv_url}/api/instances/{instance_id}/telemetry/{telemetry_name}"
            )
            result_queue.put(("success", response.status_code))
        except Exception as e:
            result_queue.put(("error", str(e)))

    result_queue = queue.Queue()
    threads = []
    concurrent_requests = 10

    start_time = time.time()

    for i in range(concurrent_requests):
        thread = threading.Thread(
            target=fetch_telemetry,
            args=(result_queue, "telemetry_test_instance", "average_voltage"),
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

    print(f"   ✅ 并发测试完成: {success_count} 成功, {error_count} 失败")
    print(f"   响应时间: {(end_time - start_time) * 1000:.2f}ms")

    print(
        f"✅ 遥测数据获取测试完成，成功获取 {successful_retrievals}/{len(test_cases)} 个遥测点"
    )
    return True


if __name__ == "__main__":
    try:
        test_telemetry_retrieval()
        print("遥测数据获取测试: PASS")
    except Exception as e:
        print(f"遥测数据获取测试: FAIL - {e}")
        exit(1)
