#!/usr/bin/env python3
"""
通信服务负载测试脚本
用于对comsrv通信服务进行压力测试
"""

import requests
import time
import logging
import threading
import statistics
import argparse
from typing import Dict, List, Any, Tuple
from concurrent.futures import ThreadPoolExecutor

# 配置日志
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger("LoadTest")

# 默认配置
DEFAULT_API_URL = "http://localhost:8080/api"
DEFAULT_THREADS = 10
DEFAULT_REQUESTS = 1000
DEFAULT_TIMEOUT = 5  # 秒
DEFAULT_READ_RATIO = 80  # 读取操作百分比

class LoadTester:
    """负载测试器类"""
    
    def __init__(self, 
                 api_url: str = DEFAULT_API_URL,
                 num_threads: int = DEFAULT_THREADS, 
                 num_requests: int = DEFAULT_REQUESTS,
                 timeout: int = DEFAULT_TIMEOUT,
                 read_ratio: int = DEFAULT_READ_RATIO):
        """
        初始化负载测试器
        
        Args:
            api_url: API基础URL
            num_threads: 并发线程数
            num_requests: 总请求数
            timeout: 请求超时时间（秒）
            read_ratio: 读取操作的百分比（0-100）
        """
        self.api_url = api_url
        self.num_threads = num_threads
        self.num_requests = num_requests
        self.timeout = timeout
        self.read_ratio = read_ratio
        
        # API端点
        self.health_api = f"{api_url}/health"
        self.channels_api = f"{api_url}/v1/channels"
        self.points_api = f"{api_url}/v1/channels"
        self.values_api = f"{api_url}/v1/channels"
        
        # 测试数据
        self.channels = []
        self.points_by_channel = {}
        
        # 结果统计
        self.response_times = []
        self.success_count = 0
        self.error_count = 0
        self.lock = threading.Lock()
        
    def fetch_test_data(self) -> bool:
        """
        获取测试所需的通道和点位数据
        
        Returns:
            bool: 是否成功获取数据
        """
        try:
            # 获取通道列表
            response = requests.get(self.channels_api, timeout=self.timeout)
            response.raise_for_status()
            self.channels = response.json().get("data", [])
            
            if not self.channels:
                logger.error("没有可用的通道数据进行测试")
                return False
                
            # 获取各通道的点位列表
            for channel in self.channels:
                channel_id = channel.get("id")
                if not channel_id:
                    continue
                
                # 模拟一些点位数据用于测试
                # 实际环境中应通过API获取
                self.points_by_channel[channel_id] = [
                    {"name": f"point_{i}", "table": "default", "dataType": "float", "writable": i % 2 == 0}
                    for i in range(1, 6)
                ]
            
            if not self.points_by_channel:
                logger.error("没有可用的点位数据进行测试")
                return False
                
            logger.info(f"已获取 {len(self.channels)} 个通道和 {sum(len(points) for points in self.points_by_channel.values())} 个点位数据")
            return True
            
        except Exception as e:
            logger.error(f"获取测试数据失败: {e}")
            return False
            
    def get_random_read_request(self) -> Tuple[str, Dict]:
        """
        生成随机读取请求
        
        Returns:
            Tuple[str, Dict]: (URL, 请求参数)
        """
        import random
        
        # 随机选择一个通道
        channel_ids = list(self.points_by_channel.keys())
        if not channel_ids:
            return self.health_api, {}
            
        channel_id = random.choice(channel_ids)
        
        # 随机选择一个点位
        points = self.points_by_channel.get(channel_id, [])
        if not points:
            return f"{self.channels_api}/{channel_id}", {}
            
        point = random.choice(points)
        point_table = point.get("table", "default")
        point_name = point.get("name")
        
        # 返回读取点位值的请求
        return f"{self.points_api}/{channel_id}/points/{point_table}/{point_name}", {}
        
    def get_random_write_request(self) -> Tuple[str, Dict]:
        """
        生成随机写入请求
        
        Returns:
            Tuple[str, Dict]: (URL, 请求参数)
        """
        import random
        
        # 随机选择一个通道
        channel_ids = list(self.points_by_channel.keys())
        if not channel_ids:
            return self.health_api, {}
            
        channel_id = random.choice(channel_ids)
        
        # 随机选择一个可写点位
        points = self.points_by_channel.get(channel_id, [])
        writable_points = [p for p in points if p.get("writable", False)]
        
        if not writable_points:
            # 如果没有可写点位，退化为读请求
            return self.get_random_read_request()
            
        point = random.choice(writable_points)
        point_table = point.get("table", "default")
        point_name = point.get("name")
        
        # 根据点位类型生成随机值
        data_type = point.get("dataType", "float")
        if data_type == "bool":
            value = random.choice([True, False])
        elif data_type == "int":
            value = random.randint(0, 100)
        elif data_type == "float":
            value = random.uniform(0, 100)
        else:
            value = str(random.randint(0, 100))
            
        # 返回写入点位值的请求
        return f"{self.points_api}/{channel_id}/points/{point_table}/{point_name}", {"value": value}
        
    def make_request(self, is_read: bool) -> bool:
        """
        执行单个测试请求
        
        Args:
            is_read: 是否为读取请求
            
        Returns:
            bool: 请求是否成功
        """
        # 获取请求参数
        if is_read:
            url, params = self.get_random_read_request()
            method = "GET"
        else:
            url, params = self.get_random_write_request()
            method = "PUT"  # 写点位使用PUT请求
            
        # 发送请求并计时
        start_time = time.time()
        success = False
        
        try:
            if method == "GET":
                response = requests.get(url, timeout=self.timeout)
            elif method == "PUT":
                response = requests.put(url, json=params, timeout=self.timeout)
            else:
                response = requests.post(url, json=params, timeout=self.timeout)
                
            response.raise_for_status()
            success = True
            
        except Exception as e:
            logger.debug(f"请求失败: {url} - {e}")
            
        finally:
            elapsed_time = time.time() - start_time
            
            # 更新统计数据
            with self.lock:
                self.response_times.append(elapsed_time)
                if success:
                    self.success_count += 1
                else:
                    self.error_count += 1
                    
            return success
            
    def worker(self, request_count: int):
        """
        工作线程函数
        
        Args:
            request_count: 需要执行的请求数
        """
        import random
        
        for _ in range(request_count):
            # 根据读写比例决定请求类型
            is_read = random.randint(1, 100) <= self.read_ratio
            self.make_request(is_read)
            
    def run(self) -> Dict[str, Any]:
        """
        运行负载测试
        
        Returns:
            Dict[str, Any]: 测试结果统计
        """
        logger.info(f"开始负载测试: {self.num_threads}个线程, {self.num_requests}个请求, 读比例{self.read_ratio}%")
        
        # 获取测试数据
        if not self.fetch_test_data():
            return {
                "success": False,
                "error": "无法获取测试数据"
            }
            
        # 计算每个线程的请求数
        requests_per_thread = self.num_requests // self.num_threads
        remaining_requests = self.num_requests % self.num_threads
        
        # 启动线程池
        start_time = time.time()
        with ThreadPoolExecutor(max_workers=self.num_threads) as executor:
            # 提交任务
            futures = []
            for i in range(self.num_threads):
                # 分配请求数，处理余数
                thread_requests = requests_per_thread + (1 if i < remaining_requests else 0)
                futures.append(executor.submit(self.worker, thread_requests))
                
            # 等待所有任务完成
            for future in futures:
                future.result()
                
        total_time = time.time() - start_time
        
        # 计算结果统计
        results = {
            "success": True,
            "total_requests": self.num_requests,
            "successful_requests": self.success_count,
            "failed_requests": self.error_count,
            "total_time": total_time,
            "requests_per_second": self.num_requests / total_time if total_time > 0 else 0,
        }
        
        # 添加响应时间统计
        if self.response_times:
            results.update({
                "min_response_time": min(self.response_times),
                "max_response_time": max(self.response_times),
                "avg_response_time": statistics.mean(self.response_times),
                "median_response_time": statistics.median(self.response_times),
                "p95_response_time": sorted(self.response_times)[int(len(self.response_times) * 0.95)],
                "p99_response_time": sorted(self.response_times)[int(len(self.response_times) * 0.99)]
            })
            
        return results

def main():
    """主函数"""
    # 解析命令行参数
    parser = argparse.ArgumentParser(description="通信服务负载测试工具")
    parser.add_argument("-u", "--url", default=DEFAULT_API_URL, help=f"API基础URL (默认: {DEFAULT_API_URL})")
    parser.add_argument("-t", "--threads", type=int, default=DEFAULT_THREADS, help=f"并发线程数 (默认: {DEFAULT_THREADS})")
    parser.add_argument("-n", "--requests", type=int, default=DEFAULT_REQUESTS, help=f"总请求数 (默认: {DEFAULT_REQUESTS})")
    parser.add_argument("--timeout", type=int, default=DEFAULT_TIMEOUT, help=f"请求超时时间(秒) (默认: {DEFAULT_TIMEOUT})")
    parser.add_argument("-r", "--read-ratio", type=int, default=DEFAULT_READ_RATIO, help="读取操作的百分比 (默认: {}%%)"
        .format(DEFAULT_READ_RATIO))
    
    args = parser.parse_args()
    
    # 创建并运行测试器
    tester = LoadTester(
        api_url=args.url,
        num_threads=args.threads,
        num_requests=args.requests,
        timeout=args.timeout,
        read_ratio=args.read_ratio
    )
    
    results = tester.run()
    
    # 输出结果
    if not results.get("success", False):
        logger.error(f"测试失败: {results.get('error', '未知错误')}")
        return
        
    logger.info("===== 负载测试结果 =====")
    logger.info(f"总请求数: {results['total_requests']}")
    logger.info(f"成功请求: {results['successful_requests']} ({results['successful_requests']/results['total_requests']*100:.2f}%)")
    logger.info(f"失败请求: {results['failed_requests']} ({results['failed_requests']/results['total_requests']*100:.2f}%)")
    logger.info(f"总耗时: {results['total_time']:.2f} 秒")
    logger.info(f"请求速率: {results['requests_per_second']:.2f} 请求/秒")
    
    if "avg_response_time" in results:
        logger.info(f"最小响应时间: {results['min_response_time']*1000:.2f} ms")
        logger.info(f"最大响应时间: {results['max_response_time']*1000:.2f} ms")
        logger.info(f"平均响应时间: {results['avg_response_time']*1000:.2f} ms")
        logger.info(f"中位响应时间: {results['median_response_time']*1000:.2f} ms")
        logger.info(f"95%响应时间: {results['p95_response_time']*1000:.2f} ms")
        logger.info(f"99%响应时间: {results['p99_response_time']*1000:.2f} ms")
        
    logger.info("=======================")

if __name__ == "__main__":
    main()