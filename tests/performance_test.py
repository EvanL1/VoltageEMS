#!/usr/bin/env python3
# performance_test.py - VoltageEMS扁平化存储性能测试

import redis
import time
import random
from concurrent.futures import ThreadPoolExecutor
import statistics

# 连接Redis
r = redis.Redis(host='localhost', port=6379, decode_responses=True)

def write_points(channel_id, start_id, count):
    """批量写入测试数据"""
    pipe = r.pipeline()
    timestamp = int(time.time() * 1000)
    
    for i in range(count):
        point_id = start_id + i
        value = round(random.uniform(0, 100), 2)
        key = f"{channel_id}:m:{point_id}"
        pipe.set(key, f"{value}:{timestamp}")
    
    start = time.time()
    pipe.execute()
    elapsed = time.time() - start
    
    print(f"Channel {channel_id}: 写入 {count} 个点，耗时 {elapsed:.3f}秒")
    return elapsed

def read_points(channel_id, start_id, count):
    """批量读取测试"""
    keys = [f"{channel_id}:m:{start_id+i}" for i in range(count)]
    
    start = time.time()
    values = r.mget(keys)
    elapsed = time.time() - start
    
    valid_count = sum(1 for v in values if v is not None)
    print(f"Channel {channel_id}: 读取 {count} 个点，耗时 {elapsed:.3f}秒，有效数据 {valid_count} 个")
    return elapsed

def test_single_point_latency(iterations=1000):
    """测试单点读写延迟"""
    write_times = []
    read_times = []
    
    for i in range(iterations):
        # 写入测试
        key = f"test:m:{i}"
        value = f"{random.uniform(0, 100):.2f}:{int(time.time() * 1000)}"
        
        start = time.time()
        r.set(key, value)
        write_times.append((time.time() - start) * 1000)  # 转换为毫秒
        
        # 读取测试
        start = time.time()
        r.get(key)
        read_times.append((time.time() - start) * 1000)  # 转换为毫秒
    
    # 清理测试数据
    pipe = r.pipeline()
    for i in range(iterations):
        pipe.delete(f"test:m:{i}")
    pipe.execute()
    
    # 计算统计数据
    write_p99 = sorted(write_times)[int(len(write_times) * 0.99)]
    read_p99 = sorted(read_times)[int(len(read_times) * 0.99)]
    
    print(f"\n单点操作延迟统计 ({iterations} 次迭代):")
    print(f"  写入 - 平均: {statistics.mean(write_times):.2f}ms, P99: {write_p99:.2f}ms")
    print(f"  读取 - 平均: {statistics.mean(read_times):.2f}ms, P99: {read_p99:.2f}ms")

def test_concurrent_access(num_threads=10, points_per_thread=1000):
    """测试并发访问性能"""
    print(f"\n并发测试: {num_threads} 线程，每线程 {points_per_thread} 点")
    
    with ThreadPoolExecutor(max_workers=num_threads) as executor:
        # 并发写入
        print("\n并发写入测试:")
        start_time = time.time()
        write_futures = []
        for i in range(num_threads):
            channel_id = 3001 + i
            future = executor.submit(write_points, channel_id, 10001, points_per_thread)
            write_futures.append(future)
        
        write_times = [f.result() for f in write_futures]
        total_write_time = time.time() - start_time
        total_points = num_threads * points_per_thread
        write_qps = total_points / total_write_time
        
        print(f"总耗时: {total_write_time:.3f}秒")
        print(f"写入QPS: {write_qps:.0f} 点/秒")
        
        # 并发读取
        print("\n并发读取测试:")
        start_time = time.time()
        read_futures = []
        for i in range(num_threads):
            channel_id = 3001 + i
            future = executor.submit(read_points, channel_id, 10001, points_per_thread)
            read_futures.append(future)
        
        read_times = [f.result() for f in read_futures]
        total_read_time = time.time() - start_time
        read_qps = total_points / total_read_time
        
        print(f"总耗时: {total_read_time:.3f}秒")
        print(f"读取QPS: {read_qps:.0f} 点/秒")
    
    # 清理测试数据
    print("\n清理测试数据...")
    for i in range(num_threads):
        channel_id = 3001 + i
        pattern = f"{channel_id}:*"
        cursor = 0
        while True:
            cursor, keys = r.scan(cursor, match=pattern, count=1000)
            if keys:
                r.delete(*keys)
            if cursor == 0:
                break

def test_memory_usage():
    """测试内存使用情况"""
    print("\n内存使用测试:")
    
    # 获取初始内存使用
    info = r.info('memory')
    initial_memory = info['used_memory']
    
    # 写入10000个点
    channel_id = 4001
    timestamp = int(time.time() * 1000)
    pipe = r.pipeline()
    
    for i in range(10000):
        key = f"{channel_id}:m:{10001+i}"
        value = f"{random.uniform(0, 100):.2f}:{timestamp}"
        pipe.set(key, value)
    
    pipe.execute()
    
    # 获取写入后的内存使用
    info = r.info('memory')
    final_memory = info['used_memory']
    memory_per_point = (final_memory - initial_memory) / 10000
    
    print(f"初始内存: {initial_memory / 1024 / 1024:.2f} MB")
    print(f"最终内存: {final_memory / 1024 / 1024:.2f} MB")
    print(f"10000点占用: {(final_memory - initial_memory) / 1024 / 1024:.2f} MB")
    print(f"每点占用: {memory_per_point:.0f} 字节")
    
    # 清理测试数据
    pattern = f"{channel_id}:*"
    cursor = 0
    while True:
        cursor, keys = r.scan(cursor, match=pattern, count=1000)
        if keys:
            r.delete(*keys)
        if cursor == 0:
            break

def main():
    print("=== VoltageEMS 扁平化存储性能测试 ===")
    print(f"开始时间: {time.strftime('%Y-%m-%d %H:%M:%S')}")
    
    # 检查Redis连接
    try:
        r.ping()
        print("Redis连接正常")
    except Exception as e:
        print(f"Redis连接失败: {e}")
        return
    
    # 执行各项测试
    test_single_point_latency()
    test_concurrent_access()
    test_memory_usage()
    
    print(f"\n测试完成时间: {time.strftime('%Y-%m-%d %H:%M:%S')}")

if __name__ == "__main__":
    main()