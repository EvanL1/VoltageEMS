#!/usr/bin/env python3
"""数据持续性测试"""

import os
import redis
import time
import requests


def test_data_persistence():
    """测试数据持续性和一致性"""
    redis_url = os.getenv("REDIS_URL", "redis://redis:6379")
    modsrv_url = os.getenv("MODSRV_URL", "http://modsrv:8082")

    print("🔍 开始数据持续性测试...")

    # 连接Redis
    redis_client = redis.from_url(redis_url, decode_responses=True)

    # 1. 测试ComsRv数据持续性
    print("1. 测试ComsRv数据持续性...")

    # 记录初始数据快照
    initial_snapshot = {}
    comsrv_keys = redis_client.keys("comsrv:*")

    for key in comsrv_keys[:10]:  # 只测试前10个键
        initial_snapshot[key] = redis_client.hgetall(key)

    print(f"   记录初始快照: {len(initial_snapshot)} 个键")

    # 等待一段时间让数据更新
    wait_time = 10
    print(f"   等待 {wait_time} 秒观察数据变化...")
    time.sleep(wait_time)

    # 检查数据变化
    changed_keys = 0
    unchanged_keys = 0
    data_consistency_issues = 0

    for key, initial_data in initial_snapshot.items():
        current_data = redis_client.hgetall(key)

        if current_data != initial_data:
            changed_keys += 1

            # 检查数据格式一致性
            for point_id, value in current_data.items():
                try:
                    float_val = float(value)
                    # 检查6位小数格式
                    if "." not in value or len(value.split(".")[1]) != 6:
                        data_consistency_issues += 1
                        print(f"   ⚠️  数据格式问题 {key}.{point_id}: {value}")
                except ValueError:
                    data_consistency_issues += 1
                    print(f"   ❌ 非数值数据 {key}.{point_id}: {value}")
        else:
            unchanged_keys += 1

    print(f"   数据变化统计: {changed_keys} 个键有变化, {unchanged_keys} 个键未变化")
    print(f"   数据一致性问题: {data_consistency_issues} 个")

    # 2. 测试数据丢失情况
    print("2. 测试数据完整性...")

    # 记录所有键
    all_keys_before = set(redis_client.keys("*"))

    # 等待一段时间
    time.sleep(5)

    # 检查键是否丢失
    all_keys_after = set(redis_client.keys("*"))

    lost_keys = all_keys_before - all_keys_after
    new_keys = all_keys_after - all_keys_before

    if lost_keys:
        print(f"   ⚠️  丢失了 {len(lost_keys)} 个键")
        for key in list(lost_keys)[:5]:  # 只显示前5个
            print(f"      - {key}")
    else:
        print("   ✅ 没有键丢失")

    if new_keys:
        print(f"   📈 新增了 {len(new_keys)} 个键")
        for key in list(new_keys)[:5]:  # 只显示前5个
            print(f"      + {key}")

    # 3. 测试Redis连接稳定性
    print("3. 测试Redis连接稳定性...")

    connection_tests = 20
    successful_connections = 0

    for i in range(connection_tests):
        try:
            test_client = redis.from_url(redis_url, decode_responses=True)
            result = test_client.ping()
            if result:
                successful_connections += 1
            test_client.close()
        except Exception as e:
            print(f"   ❌ 连接测试 {i + 1} 失败: {e}")

        time.sleep(0.1)  # 短暂间隔

    connection_rate = successful_connections / connection_tests * 100
    print(
        f"   连接成功率: {connection_rate:.1f}% ({successful_connections}/{connection_tests})"
    )

    # 4. 测试数据读写性能持续性
    print("4. 测试数据读写性能...")

    # 写入测试数据
    test_key = "test:persistence:data"
    test_data = {f"point_{i}": f"{i * 1.234567:.6f}" for i in range(100)}

    write_times = []
    read_times = []

    # 执行多次读写测试
    for i in range(10):
        # 写入测试
        start_time = time.time()
        redis_client.hset(test_key, mapping=test_data)
        write_time = (time.time() - start_time) * 1000
        write_times.append(write_time)

        # 读取测试
        start_time = time.time()
        read_data = redis_client.hgetall(test_key)
        read_time = (time.time() - start_time) * 1000
        read_times.append(read_time)

        # 验证数据完整性
        if len(read_data) != len(test_data):
            print(
                f"   ❌ 数据完整性问题: 期望 {len(test_data)} 个字段, 实际 {len(read_data)} 个"
            )

        time.sleep(0.5)

    avg_write_time = sum(write_times) / len(write_times)
    avg_read_time = sum(read_times) / len(read_times)

    print(f"   平均写入时间: {avg_write_time:.2f}ms")
    print(f"   平均读取时间: {avg_read_time:.2f}ms")

    # 清理测试数据
    redis_client.delete(test_key)

    # 5. 测试ModSrv API持续性
    print("5. 测试ModSrv API持续性...")

    api_tests = 10
    successful_api_calls = 0
    api_response_times = []

    for i in range(api_tests):
        try:
            start_time = time.time()
            response = requests.get(f"{modsrv_url}/health", timeout=5)
            response_time = (time.time() - start_time) * 1000
            api_response_times.append(response_time)

            if response.status_code == 200:
                successful_api_calls += 1
            else:
                print(f"   ⚠️  API测试 {i + 1} 返回状态码: {response.status_code}")

        except Exception as e:
            print(f"   ❌ API测试 {i + 1} 失败: {e}")

        time.sleep(1)

    api_success_rate = successful_api_calls / api_tests * 100
    avg_api_response_time = (
        sum(api_response_times) / len(api_response_times) if api_response_times else 0
    )

    print(f"   API成功率: {api_success_rate:.1f}% ({successful_api_calls}/{api_tests})")
    print(f"   平均API响应时间: {avg_api_response_time:.2f}ms")

    # 6. 测试长期运行稳定性
    print("6. 测试长期运行稳定性...")

    # 模拟长期运行场景
    stability_test_duration = 30  # 秒
    check_interval = 5  # 秒
    stability_checks = []

    start_time = time.time()
    next_check = start_time + check_interval

    while time.time() - start_time < stability_test_duration:
        current_time = time.time()

        if current_time >= next_check:
            try:
                # 检查Redis状态
                redis_info = redis_client.info()
                memory_usage = redis_info.get("used_memory", 0)
                client_count = redis_info.get("connected_clients", 0)

                # 检查API状态
                api_response = requests.get(f"{modsrv_url}/health", timeout=3)
                api_ok = api_response.status_code == 200

                # 检查数据键数量
                key_count = len(redis_client.keys("*"))

                stability_checks.append(
                    {
                        "timestamp": current_time,
                        "memory_usage": memory_usage,
                        "client_count": client_count,
                        "api_ok": api_ok,
                        "key_count": key_count,
                    }
                )

                elapsed = current_time - start_time
                remaining = stability_test_duration - elapsed
                print(
                    f"   稳定性检查: {elapsed:.1f}s / {stability_test_duration}s (剩余: {remaining:.1f}s)"
                )

            except Exception as e:
                print(f"   ⚠️  稳定性检查异常: {e}")

            next_check = current_time + check_interval

        time.sleep(1)

    # 分析稳定性结果
    if stability_checks:
        memory_values = [check["memory_usage"] for check in stability_checks]
        client_values = [check["client_count"] for check in stability_checks]
        key_values = [check["key_count"] for check in stability_checks]
        api_success_count = sum(1 for check in stability_checks if check["api_ok"])

        memory_growth = max(memory_values) - min(memory_values)
        client_variance = max(client_values) - min(client_values)
        key_variance = max(key_values) - min(key_values)
        api_stability = api_success_count / len(stability_checks) * 100

        print(f"   内存增长: {memory_growth} bytes")
        print(f"   客户端连接变化: {client_variance}")
        print(f"   键数量变化: {key_variance}")
        print(f"   API稳定性: {api_stability:.1f}%")

    # 7. 综合评估
    print("7. 综合评估...")

    issues = []

    if data_consistency_issues > 0:
        issues.append(f"数据一致性问题: {data_consistency_issues} 个")

    if len(lost_keys) > 0:
        issues.append(f"数据丢失: {len(lost_keys)} 个键")

    if connection_rate < 95:
        issues.append(f"连接稳定性不足: {connection_rate:.1f}%")

    if avg_write_time > 100:  # 100ms
        issues.append(f"写入性能过慢: {avg_write_time:.2f}ms")

    if avg_read_time > 50:  # 50ms
        issues.append(f"读取性能过慢: {avg_read_time:.2f}ms")

    if api_success_rate < 95:
        issues.append(f"API稳定性不足: {api_success_rate:.1f}%")

    if issues:
        print("   ❌ 发现问题:")
        for issue in issues:
            print(f"      - {issue}")
        print("   数据持续性测试未完全通过")
        return False
    else:
        print("   ✅ 所有持续性测试通过")
        print("   系统数据持续性表现良好")
        return True


if __name__ == "__main__":
    try:
        if test_data_persistence():
            print("数据持续性测试: PASS")
        else:
            print("数据持续性测试: FAIL - 存在持续性问题")
            exit(1)
    except Exception as e:
        print(f"数据持续性测试: FAIL - {e}")
        exit(1)
