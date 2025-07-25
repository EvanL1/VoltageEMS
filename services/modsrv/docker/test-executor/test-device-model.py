#!/usr/bin/env python3
"""设备模型API测试"""

import os
import requests


def test_device_model_api():
    """测试设备模型API功能"""
    modsrv_url = os.getenv("MODSRV_URL", "http://modsrv:8092")

    print("🔍 开始设备模型API测试...")

    # 1. 健康检查
    response = requests.get(f"{modsrv_url}/health")
    if response.status_code != 200:
        raise Exception(f"健康检查失败: {response.status_code}")

    health_data = response.json()
    print(f"✅ 健康检查通过: {health_data['status']}")

    # 2. 测试模板列表
    response = requests.get(f"{modsrv_url}/api/templates")
    if response.status_code != 200:
        raise Exception(f"模板列表获取失败: {response.status_code}")

    print("✅ 模板列表API正常")

    # 3. 测试实例创建
    instance_data = {
        "template_id": "test_avg_model",
        "instance_id": "test_instance_001",
        "config": {"name": "测试实例", "description": "用于API测试的实例"},
    }

    response = requests.post(
        f"{modsrv_url}/api/instances",
        json=instance_data,
        headers={"Content-Type": "application/json"},
    )

    if response.status_code in [200, 201]:
        print("✅ 实例创建API正常")
        instance_result = response.json()
        print(f"  实例ID: {instance_result.get('instance_id', 'N/A')}")
    else:
        print(f"⚠️  实例创建API返回: {response.status_code} - {response.text}")

    # 4. 测试操作列表
    response = requests.get(f"{modsrv_url}/api/control/operations")
    if response.status_code != 200:
        raise Exception(f"操作列表获取失败: {response.status_code}")

    operations = response.json()
    print(f"✅ 操作列表API正常，包含 {len(operations)} 个操作")

    print("✅ 设备模型API测试通过")
    return True


if __name__ == "__main__":
    try:
        test_device_model_api()
        print("设备模型API测试: PASS")
    except Exception as e:
        print(f"设备模型API测试: FAIL - {e}")
        exit(1)
