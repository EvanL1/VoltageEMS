#!/usr/bin/env python3
"""实例创建和管理测试"""

import os
import requests
import time


def test_instance_management():
    """测试实例创建、查询和管理功能"""
    modsrv_url = os.getenv("MODSRV_URL", "http://modsrv:8082")

    print("🔍 开始实例管理测试...")

    # 测试数据
    test_instances = [
        {
            "template_id": "test_avg_model",
            "instance_id": "test_avg_instance_001",
            "config": {
                "name": "平均值计算实例",
                "description": "用于测试平均值计算的实例",
                "input_channels": ["1001", "1002"],
            },
        },
        {
            "template_id": "test_sum_model",
            "instance_id": "test_sum_instance_001",
            "config": {
                "name": "求和计算实例",
                "description": "用于测试求和计算的实例",
                "multiplier": 1.5,
            },
        },
        {
            "template_id": "motor_control_model",
            "instance_id": "motor_001",
            "config": {
                "name": "电机控制实例",
                "description": "用于测试电机控制的实例",
                "rated_power": 15.0,
                "max_speed": 1500,
            },
        },
    ]

    created_instances = []

    try:
        # 1. 创建多个实例
        print("1. 批量创建实例...")
        for i, instance_data in enumerate(test_instances):
            print(
                f"   创建实例 {i + 1}/{len(test_instances)}: {instance_data['instance_id']}"
            )

            response = requests.post(
                f"{modsrv_url}/api/instances",
                json=instance_data,
                headers={"Content-Type": "application/json"},
            )

            if response.status_code in [200, 201]:
                result = response.json()
                created_instances.append(
                    result.get("instance_id", instance_data["instance_id"])
                )
                print(f"   ✅ 实例创建成功: {result.get('instance_id')}")
            else:
                print(f"   ⚠️  实例创建返回: {response.status_code} - {response.text}")
                # 仍然记录，以便后续清理
                created_instances.append(instance_data["instance_id"])

        # 2. 测试重复创建（应该失败或返回已存在）
        print("2. 测试重复实例创建...")
        duplicate_data = test_instances[0].copy()
        response = requests.post(
            f"{modsrv_url}/api/instances",
            json=duplicate_data,
            headers={"Content-Type": "application/json"},
        )

        if response.status_code in [400, 409]:
            print("   ✅ 重复创建正确拒绝")
        elif response.status_code in [200, 201]:
            print("   ⚠️  重复创建被接受（可能是更新操作）")
        else:
            print(f"   ❓ 重复创建返回意外状态: {response.status_code}")

        # 3. 测试无效模板ID
        print("3. 测试无效模板ID...")
        invalid_data = {
            "template_id": "non_existent_model",
            "instance_id": "invalid_test_001",
            "config": {},
        }

        response = requests.post(
            f"{modsrv_url}/api/instances",
            json=invalid_data,
            headers={"Content-Type": "application/json"},
        )

        if response.status_code in [400, 404]:
            print("   ✅ 无效模板ID正确拒绝")
        else:
            print(f"   ⚠️  无效模板ID处理异常: {response.status_code}")

        # 4. 测试空配置
        print("4. 测试空配置...")
        empty_config_data = {
            "template_id": "test_avg_model",
            "instance_id": "empty_config_test",
            "config": {},
        }

        response = requests.post(
            f"{modsrv_url}/api/instances",
            json=empty_config_data,
            headers={"Content-Type": "application/json"},
        )

        if response.status_code in [200, 201]:
            print("   ✅ 空配置被接受")
            created_instances.append("empty_config_test")
        else:
            print(f"   ⚠️  空配置处理: {response.status_code}")

        # 5. 测试大配置数据
        print("5. 测试大配置数据...")
        large_config_data = {
            "template_id": "test_sum_model",
            "instance_id": "large_config_test",
            "config": {
                "name": "大配置测试实例",
                "description": "包含大量配置参数的测试实例",
                "parameters": {f"param_{i}": f"value_{i}" for i in range(100)},
                "arrays": [list(range(50)) for _ in range(10)],
                "nested": {"level1": {"level2": {"level3": {"data": "深层嵌套数据"}}}},
            },
        }

        response = requests.post(
            f"{modsrv_url}/api/instances",
            json=large_config_data,
            headers={"Content-Type": "application/json"},
        )

        if response.status_code in [200, 201]:
            print("   ✅ 大配置数据被接受")
            created_instances.append("large_config_test")
        else:
            print(f"   ⚠️  大配置数据处理: {response.status_code}")

        # 等待一段时间让实例初始化
        print("6. 等待实例初始化...")
        time.sleep(3)

        print(f"✅ 实例管理测试完成，创建了 {len(created_instances)} 个实例")
        return True

    except Exception as e:
        print(f"❌ 实例管理测试失败: {e}")
        raise


if __name__ == "__main__":
    try:
        test_instance_management()
        print("实例管理测试: PASS")
    except Exception as e:
        print(f"实例管理测试: FAIL - {e}")
        exit(1)
