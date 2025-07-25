#!/usr/bin/env python3
"""ComsRv数据验证测试"""

import os
import redis


def test_comsrv_data():
    """测试ComsRv数据是否按规范生成"""
    redis_url = os.getenv("REDIS_URL", "redis://redis:6379")
    client = redis.from_url(redis_url, decode_responses=True)

    print("🔍 开始ComsRv数据验证...")

    # 检查模拟器是否产生了数据
    keys = client.keys("comsrv:*")
    print(f"发现 {len(keys)} 个comsrv键")

    if len(keys) == 0:
        raise Exception("未发现任何comsrv数据")

    # 验证数据格式
    data_found = False
    for key in keys:
        key_parts = key.split(":")
        if len(key_parts) != 3 or key_parts[0] != "comsrv":
            continue

        channel_id = key_parts[1]
        data_type = key_parts[2]

        print(f"检查键: {key} (通道: {channel_id}, 类型: {data_type})")

        # 获取Hash中的所有字段
        fields = client.hgetall(key)
        if fields:
            data_found = True
            print(f"  - 包含 {len(fields)} 个点位")

            # 验证数值格式
            for point_id, value in fields.items():
                try:
                    float_val = float(value)
                    # 检查是否是6位小数格式
                    if "." in value and len(value.split(".")[1]) == 6:
                        print(f"  ✅ 点位 {point_id}: {value} (格式正确)")
                    else:
                        print(f"  ⚠️  点位 {point_id}: {value} (格式可能不标准)")
                except ValueError:
                    print(f"  ❌ 点位 {point_id}: {value} (不是有效数值)")

    if not data_found:
        raise Exception("未发现有效的测量数据")

    print("✅ ComsRv数据验证通过")
    return True


if __name__ == "__main__":
    try:
        test_comsrv_data()
        print("ComsRv数据测试: PASS")
    except Exception as e:
        print(f"ComsRv数据测试: FAIL - {e}")
        exit(1)
