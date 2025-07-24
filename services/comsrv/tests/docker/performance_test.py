#!/usr/bin/env python3
"""
COMSRV性能测试
测试系统在高负载下的性能表现
"""

import os
import time
import statistics
import redis
import requests
import logging
import asyncio
import aiohttp
from concurrent.futures import ThreadPoolExecutor, as_completed

# 配置日志
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[
        logging.FileHandler("/app/logs/performance_test.log"),
        logging.StreamHandler(),
    ],
)
logger = logging.getLogger(__name__)

# 测试配置
REDIS_URL = os.getenv("REDIS_URL", "redis://:testpass123@redis:6379")
COMSRV_URL = os.getenv("COMSRV_URL", "http://comsrv:3000")
MODBUS_HOST = os.getenv("MODBUS_HOST", "modbus-simulator")
MODBUS_PORT = int(os.getenv("MODBUS_PORT", 502))


class PerformanceTest:
    """性能测试类"""

    def __init__(self):
        self.api_base_url = os.getenv("COMSRV_URL", "http://comsrv:3000")
        self.redis_url = os.getenv("REDIS_URL", "redis://localhost:6379")
        self.results = {"response_times": [], "error_count": 0, "success_count": 0}

    async def make_async_request(self, session, endpoint):
        """发送异步请求并记录响应时间"""
        start_time = time.time()
        try:
            async with session.get(f"{self.api_base_url}{endpoint}") as response:
                await response.text()
                elapsed = time.time() - start_time
                self.results["response_times"].append(elapsed)
                self.results["success_count"] += 1
                return response.status
        except Exception as e:
            self.results["error_count"] += 1
            print(f"Request error: {e}")
            return None

    async def load_test_async(self, concurrent_users=50, requests_per_user=20):
        """异步负载测试"""
        print("\n=== 异步负载测试 ===")
        print(f"并发用户数: {concurrent_users}")
        print(f"每用户请求数: {requests_per_user}")

        endpoints = [
            "/api/health",
            "/api/channels",
        ]

        async with aiohttp.ClientSession() as session:
            tasks = []
            for _ in range(concurrent_users):
                for i in range(requests_per_user):
                    endpoint = endpoints[i % len(endpoints)]
                    task = self.make_async_request(session, endpoint)
                    tasks.append(task)

            start_time = time.time()
            await asyncio.gather(*tasks)
            total_time = time.time() - start_time

        self._print_results(total_time)

    def throughput_test(self, duration_seconds=30):
        """吞吐量测试"""
        print(f"\n=== 吞吐量测试 ({duration_seconds}秒) ===")

        session = requests.Session()

        start_time = time.time()
        request_count = 0

        while time.time() - start_time < duration_seconds:
            try:
                response = session.get(f"{self.api_base_url}/api/health")
                if response.status_code == 200:
                    request_count += 1
            except Exception as e:
                print(f"Error: {e}")

        elapsed = time.time() - start_time
        rps = request_count / elapsed

        print(f"总请求数: {request_count}")
        print(f"总时间: {elapsed:.2f}秒")
        print(f"吞吐量: {rps:.2f} 请求/秒")

        return rps

    def latency_test(self, iterations=100):
        """延迟测试"""
        print(f"\n=== 延迟测试 ({iterations}次迭代) ===")

        latencies = []

        for _ in range(iterations):
            start = time.time()
            try:
                response = requests.get(f"{self.api_base_url}/api/health")
                if response.status_code == 200:
                    latency = (time.time() - start) * 1000  # 转换为毫秒
                    latencies.append(latency)
            except Exception as e:
                print(f"Error: {e}")

        if latencies:
            avg_latency = statistics.mean(latencies)
            min_latency = min(latencies)
            max_latency = max(latencies)
            p50 = statistics.median(latencies)
            p95 = (
                statistics.quantiles(latencies, n=20)[18]
                if len(latencies) > 20
                else max_latency
            )
            p99 = (
                statistics.quantiles(latencies, n=100)[98]
                if len(latencies) > 100
                else max_latency
            )

            print(f"平均延迟: {avg_latency:.2f}ms")
            print(f"最小延迟: {min_latency:.2f}ms")
            print(f"最大延迟: {max_latency:.2f}ms")
            print(f"P50延迟: {p50:.2f}ms")
            print(f"P95延迟: {p95:.2f}ms")
            print(f"P99延迟: {p99:.2f}ms")

        return latencies

    def redis_performance_test(self):
        """Redis性能测试"""
        print("\n=== Redis性能测试 ===")

        r = redis.from_url(self.redis_url)

        # 写入性能测试
        write_times = []
        for i in range(1000):
            start = time.time()
            r.hset("comsrv:test:m", f"point_{i}", f"{i},123456789")
            write_times.append(time.time() - start)

        # 读取性能测试
        read_times = []
        for i in range(1000):
            start = time.time()
            r.hget("comsrv:test:m", f"point_{i}")
            read_times.append(time.time() - start)

        # 清理测试数据
        r.delete("comsrv:test:m")

        avg_write = statistics.mean(write_times) * 1000
        avg_read = statistics.mean(read_times) * 1000

        print(f"平均写入时间: {avg_write:.2f}ms")
        print(f"平均读取时间: {avg_read:.2f}ms")
        print(f"写入吞吐量: {1000 / sum(write_times):.2f} ops/s")
        print(f"读取吞吐量: {1000 / sum(read_times):.2f} ops/s")

    def stress_test(self, max_concurrent=200, ramp_up_time=10):
        """压力测试"""
        print("\n=== 压力测试 ===")
        print(f"最大并发数: {max_concurrent}")
        print(f"递增时间: {ramp_up_time}秒")

        def make_request():
            try:
                response = requests.get(f"{self.api_base_url}/api/health", timeout=5)
                return response.status_code == 200
            except:
                return False

        results = []
        step = max_concurrent // ramp_up_time

        for current_load in range(step, max_concurrent + 1, step):
            print(f"\n当前并发数: {current_load}")

            start_time = time.time()
            success_count = 0

            with ThreadPoolExecutor(max_workers=current_load) as executor:
                futures = [
                    executor.submit(make_request) for _ in range(current_load * 10)
                ]
                for future in as_completed(futures):
                    if future.result():
                        success_count += 1

            elapsed = time.time() - start_time
            success_rate = (success_count / (current_load * 10)) * 100
            rps = success_count / elapsed

            results.append(
                {"concurrent": current_load, "success_rate": success_rate, "rps": rps}
            )

            print(f"成功率: {success_rate:.1f}%")
            print(f"吞吐量: {rps:.1f} req/s")

            # 如果成功率低于80%，停止测试
            if success_rate < 80:
                print("成功率低于80%，停止压力测试")
                break

            time.sleep(1)  # 短暂休息

        return results

    def _print_results(self, total_time):
        """打印测试结果"""
        total_requests = len(self.results["response_times"])

        if total_requests > 0:
            avg_response_time = statistics.mean(self.results["response_times"])
            min_response_time = min(self.results["response_times"])
            max_response_time = max(self.results["response_times"])
            p95_response_time = statistics.quantiles(
                self.results["response_times"], n=20
            )[18]
            requests_per_second = total_requests / total_time

            print("\n测试结果:")
            print(f"总请求数: {total_requests}")
            print(f"成功请求: {self.results['success_count']}")
            print(f"失败请求: {self.results['error_count']}")
            print(f"总时间: {total_time:.2f}秒")
            print(f"吞吐量: {requests_per_second:.2f} req/s")
            print(f"平均响应时间: {avg_response_time * 1000:.2f}ms")
            print(f"最小响应时间: {min_response_time * 1000:.2f}ms")
            print(f"最大响应时间: {max_response_time * 1000:.2f}ms")
            print(f"P95响应时间: {p95_response_time * 1000:.2f}ms")


async def main():
    """主测试函数"""
    test = PerformanceTest()

    # 1. 延迟测试
    test.latency_test(iterations=100)

    # 2. 吞吐量测试
    test.throughput_test(duration_seconds=10)

    # 3. 异步负载测试
    await test.load_test_async(concurrent_users=20, requests_per_user=50)

    # 4. Redis性能测试
    test.redis_performance_test()

    # 5. 压力测试
    test.stress_test(max_concurrent=100, ramp_up_time=5)

    print("\n=== 性能测试完成 ===")


if __name__ == "__main__":
    asyncio.run(main())
