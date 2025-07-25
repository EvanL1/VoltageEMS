#!/usr/bin/env python3
"""Redis数据格式验证测试"""

import os
import redis
import re


def test_redis_format():
    """验证Redis数据是否符合规范v3.2"""
    redis_url = os.getenv("REDIS_URL", "redis://redis:6379")
    client = redis.from_url(redis_url, decode_responses=True)

    print("🔍 开始Redis数据格式验证...")

    # 获取所有comsrv键
    comsrv_keys = client.keys("comsrv:*")
    print(f"发现 {len(comsrv_keys)} 个comsrv键")

    format_errors = []

    # 验证键格式: comsrv:{channelID}:{type}
    key_pattern = re.compile(r"^comsrv:\d+:[msca]$")

    for key in comsrv_keys:
        if not key_pattern.match(key):
            format_errors.append(f"键格式错误: {key}")
            continue

        # 验证值格式：6位小数
        fields = client.hgetall(key)
        for point_id, value in fields.items():
            try:
                float_val = float(value)
                # 检查小数位数
                if "." in value:
                    decimal_places = len(value.split(".")[1])
                    if decimal_places != 6:
                        format_errors.append(
                            f"{key}.{point_id}: 小数位数错误 ({decimal_places}位，应为6位)"
                        )
                else:
                    format_errors.append(f"{key}.{point_id}: 缺少小数点")
            except ValueError:
                format_errors.append(f"{key}.{point_id}: 非数值格式: {value}")

    # 检查是否有modsrv键（模型输出）
    modsrv_keys = client.keys("modsrv:*")
    print(f"发现 {len(modsrv_keys)} 个modsrv键")

    if format_errors:
        print("❌ 格式验证失败:")
        for error in format_errors[:10]:  # 只显示前10个错误
            print(f"  - {error}")
        if len(format_errors) > 10:
            print(f"  ... 还有 {len(format_errors) - 10} 个错误")
        raise Exception(f"发现 {len(format_errors)} 个格式错误")

    print("✅ Redis数据格式验证通过")
    return True


if __name__ == "__main__":
    try:
        test_redis_format()
        print("Redis格式测试: PASS")
    except Exception as e:
        print(f"Redis格式测试: FAIL - {e}")
        exit(1)
