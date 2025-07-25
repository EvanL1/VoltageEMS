#!/usr/bin/env python3
"""负载测试"""

import os
import requests
import time
import threading
import statistics
import redis


def test_load():
    """测试系统负载能力"""
    modsrv_url = os.getenv("MODSRV_URL", "http://modsrv:8082")
    redis_url = os.getenv("REDIS_URL", "redis://redis:6379")

    print("🔍 开始负载测试...")

    # 连接Redis监控数据
    redis_client = redis.from_url(redis_url, decode_responses=True)

    # 测试配置
    load_config = {
        "concurrent_users": 20,
        "requests_per_user": 10,
        "test_duration": 30,  # 秒
        "ramp_up_time": 5,  # 秒
    }

    print("负载测试配置:")
    print(f"  - 并发用户数: {load_config['concurrent_users']}")
    print(f"  - 每用户请求数: {load_config['requests_per_user']}")
    print(f"  - 测试持续时间: {load_config['test_duration']}秒")
    print(f"  - 启动时间: {load_config['ramp_up_time']}秒")

    # 跳过实例创建，直接使用预定义模型
    print("1. 使用预定义模型进行测试...")
    test_models = ["power_meter_demo", "transformer_demo"]
    print(f"   使用模型: {test_models}")

    # 验证模型是否可访问
    for model_id in test_models:
        try:
            response = requests.get(f"{modsrv_url}/models/{model_id}", timeout=5)
            if response.status_code == 200:
                print(f"   ✅ 模型 {model_id} 可访问")
            else:
                print(f"   ⚠️  模型 {model_id} 访问失败: {response.status_code}")
        except Exception as e:
            print(f"   ❌ 模型 {model_id} 验证异常: {e}")

    time.sleep(1)  # 等待验证完成

    # 定义负载测试操作
    def api_operations():
        """返回API操作列表"""
        return [
            ("GET", "/health", None, "健康检查"),
            ("GET", "/models", None, "模型列表"),
            ("GET", "/models/power_meter_demo", None, "电表模型详情"),
            ("GET", "/models/transformer_demo", None, "变压器模型详情"),
            (
                "POST",
                "/models/power_meter_demo/control/power_limit",
                {"value": 100.0},
                "电表功率限制控制",
            ),
            (
                "POST",
                "/models/transformer_demo/control/main_breaker",
                {"value": 1.0},
                "变压器断路器控制",
            ),
        ]

    # 负载测试工作线程
    def load_test_worker(worker_id, results_queue, barrier, stop_event):
        """负载测试工作线程"""
        operations = api_operations()
        local_results = []

        # 等待所有线程就绪
        barrier.wait()

        request_count = 0
        start_time = time.time()

        while not stop_event.is_set():
            for method, endpoint, data, description in operations:
                if stop_event.is_set():
                    break

                try:
                    request_start = time.time()

                    if method == "GET":
                        response = requests.get(f"{modsrv_url}{endpoint}", timeout=10)
                    elif method == "POST":
                        response = requests.post(
                            f"{modsrv_url}{endpoint}",
                            json=data,
                            headers={"Content-Type": "application/json"},
                            timeout=10,
                        )

                    request_end = time.time()
                    response_time = (request_end - request_start) * 1000  # 毫秒

                    local_results.append(
                        {
                            "worker_id": worker_id,
                            "operation": description,
                            "method": method,
                            "endpoint": endpoint,
                            "status_code": response.status_code,
                            "response_time": response_time,
                            "timestamp": request_start,
                            "success": 200 <= response.status_code < 400,
                        }
                    )

                except Exception as e:
                    request_end = time.time()
                    response_time = (request_end - request_start) * 1000

                    local_results.append(
                        {
                            "worker_id": worker_id,
                            "operation": description,
                            "method": method,
                            "endpoint": endpoint,
                            "status_code": 0,
                            "response_time": response_time,
                            "timestamp": request_start,
                            "success": False,
                            "error": str(e),
                        }
                    )

                request_count += 1

                # 极小的延迟，仅用于避免过度占用CPU
                time.sleep(0.001)  # 1ms延迟

        results_queue.put(local_results)

    # 执行负载测试
    print("2. 开始负载测试...")

    import queue

    results_queue = queue.Queue()
    barrier = threading.Barrier(
        load_config["concurrent_users"] + 1
    )  # +1 for main thread
    stop_event = threading.Event()

    # 启动工作线程
    threads = []
    for i in range(load_config["concurrent_users"]):
        thread = threading.Thread(
            target=load_test_worker, args=(i, results_queue, barrier, stop_event)
        )
        threads.append(thread)
        thread.start()

        # 渐进式启动
        time.sleep(load_config["ramp_up_time"] / load_config["concurrent_users"])

    # 开始测试
    test_start_time = time.time()
    barrier.wait()  # 等待所有线程就绪
    print(f"   📊 负载测试开始，{load_config['concurrent_users']} 个并发用户")

    # 监控测试进程
    monitor_interval = 5
    next_monitor = test_start_time + monitor_interval

    while time.time() - test_start_time < load_config["test_duration"]:
        current_time = time.time()

        if current_time >= next_monitor:
            elapsed = current_time - test_start_time
            remaining = load_config["test_duration"] - elapsed
            print(
                f"   ⏱️  测试进行中: {elapsed:.1f}s / {load_config['test_duration']}s (剩余: {remaining:.1f}s)"
            )
            next_monitor = current_time + monitor_interval

        time.sleep(1)

    # 停止测试
    print("   🛑 停止负载测试...")
    stop_event.set()

    # 等待所有线程完成
    for thread in threads:
        thread.join(timeout=10)

    test_end_time = time.time()
    actual_duration = test_end_time - test_start_time

    # 收集结果
    print("3. 收集测试结果...")
    all_results = []

    while not results_queue.empty():
        worker_results = results_queue.get()
        all_results.extend(worker_results)

    # 分析结果
    print("4. 分析测试结果...")

    if not all_results:
        print("   ❌ 没有收集到测试结果")
        return False

    # 基本统计
    total_requests = len(all_results)
    successful_requests = sum(1 for r in all_results if r["success"])
    failed_requests = total_requests - successful_requests
    success_rate = successful_requests / total_requests * 100

    # 响应时间统计
    response_times = [r["response_time"] for r in all_results if r["success"]]

    if response_times:
        avg_response_time = statistics.mean(response_times)
        median_response_time = statistics.median(response_times)
        p95_response_time = sorted(response_times)[int(len(response_times) * 0.95)]
        p99_response_time = sorted(response_times)[int(len(response_times) * 0.99)]
        min_response_time = min(response_times)
        max_response_time = max(response_times)
    else:
        avg_response_time = median_response_time = p95_response_time = (
            p99_response_time
        ) = 0
        min_response_time = max_response_time = 0

    # 吞吐量统计
    throughput = total_requests / actual_duration  # 请求/秒

    # 错误分析
    error_types = {}
    for result in all_results:
        if not result["success"]:
            status = result["status_code"]
            error_key = f"HTTP_{status}" if status > 0 else "Connection_Error"
            error_types[error_key] = error_types.get(error_key, 0) + 1

    # 输出结果
    print("\n📊 负载测试结果:")
    print(f"   总请求数: {total_requests}")
    print(f"   成功请求: {successful_requests}")
    print(f"   失败请求: {failed_requests}")
    print(f"   成功率: {success_rate:.2f}%")
    print(f"   实际测试时间: {actual_duration:.2f}秒")
    print(f"   平均吞吐量: {throughput:.2f} 请求/秒")

    print("\n⏱️  响应时间统计 (毫秒):")
    print(f"   平均响应时间: {avg_response_time:.2f}ms")
    print(f"   中位数响应时间: {median_response_time:.2f}ms")
    print(f"   95%响应时间: {p95_response_time:.2f}ms")
    print(f"   99%响应时间: {p99_response_time:.2f}ms")
    print(f"   最小响应时间: {min_response_time:.2f}ms")
    print(f"   最大响应时间: {max_response_time:.2f}ms")

    if error_types:
        print("\n❌ 错误统计:")
        for error_type, count in error_types.items():
            print(f"   {error_type}: {count} 次")

    # 评估测试结果
    performance_issues = []

    if success_rate < 95:
        performance_issues.append(f"成功率过低: {success_rate:.2f}%")

    if avg_response_time > 1000:  # 1秒
        performance_issues.append(f"平均响应时间过长: {avg_response_time:.2f}ms")

    if p95_response_time > 2000:  # 2秒
        performance_issues.append(f"95%响应时间过长: {p95_response_time:.2f}ms")

    if throughput < 10:  # 10请求/秒
        performance_issues.append(f"吞吐量过低: {throughput:.2f} 请求/秒")

    if performance_issues:
        print("\n⚠️  性能问题:")
        for issue in performance_issues:
            print(f"   - {issue}")
    else:
        print("\n✅ 系统性能表现良好")

    # Redis数据检查
    print("5. 检查Redis数据状态...")
    try:
        info = redis_client.info()
        memory_usage = info.get("used_memory_human", "N/A")
        connected_clients = info.get("connected_clients", "N/A")
        total_commands = info.get("total_commands_processed", "N/A")

        print(f"   Redis内存使用: {memory_usage}")
        print(f"   连接客户端数: {connected_clients}")
        print(f"   总命令数: {total_commands}")

    except Exception as e:
        print(f"   ⚠️  Redis状态检查失败: {e}")

    # 判断测试是否通过
    test_passed = success_rate >= 90 and avg_response_time <= 2000

    if test_passed:
        print("\n✅ 负载测试通过")
    else:
        print("\n❌ 负载测试未达到预期标准")

    return test_passed


if __name__ == "__main__":
    try:
        if test_load():
            print("负载测试: PASS")
        else:
            print("负载测试: FAIL - 性能不达标")
            exit(1)
    except Exception as e:
        print(f"负载测试: FAIL - {e}")
        exit(1)
