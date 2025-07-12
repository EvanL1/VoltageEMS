#!/usr/bin/env python3
"""
Performance test script for Redis Hash structure optimization.
Tests the performance of old String-based vs new Hash-based storage.
"""

import redis
import time
import json
import random
from datetime import datetime

# Redis connection
r = redis.Redis(host='localhost', port=6379, decode_responses=True)

def generate_test_data(num_points=100):
    """Generate test point data"""
    data = {}
    for i in range(num_points):
        point_id = f"point_{i}"
        data[point_id] = json.dumps({
            "id": point_id,
            "value": round(random.uniform(0, 100), 2),
            "timestamp": datetime.now().isoformat(),
            "quality": "good",
            "telemetry_type": random.choice(["Measurement", "Signal", "Control", "Adjustment"])
        })
    return data

def test_old_string_method(channel_id, data):
    """Test old method: individual String keys"""
    start_time = time.time()
    
    # Write using old method
    for point_id, value in data.items():
        key = f"point:{channel_id}:{point_id}"
        r.set(key, value)
    
    write_time = time.time() - start_time
    
    # Read using old method
    start_time = time.time()
    results = {}
    for point_id in data.keys():
        key = f"point:{channel_id}:{point_id}"
        results[point_id] = r.get(key)
    
    read_time = time.time() - start_time
    
    # Cleanup
    for point_id in data.keys():
        key = f"point:{channel_id}:{point_id}"
        r.delete(key)
    
    return write_time, read_time

def test_new_hash_method(channel_id, data):
    """Test new method: Hash structure"""
    start_time = time.time()
    
    # Write using new method
    hash_key = f"comsrv:realtime:channel:{channel_id}"
    r.hset(hash_key, mapping=data)
    
    write_time = time.time() - start_time
    
    # Read using new method
    start_time = time.time()
    results = r.hgetall(hash_key)
    
    read_time = time.time() - start_time
    
    # Cleanup
    r.delete(hash_key)
    
    return write_time, read_time

def test_batch_operations():
    """Test batch operations performance"""
    print("\n=== Batch Operations Test ===")
    
    # Test with different batch sizes
    batch_sizes = [10, 50, 100, 500, 1000]
    
    for size in batch_sizes:
        data = generate_test_data(size)
        channel_id = 1
        
        # Test old method
        old_write, old_read = test_old_string_method(channel_id, data)
        
        # Test new method
        new_write, new_read = test_new_hash_method(channel_id, data)
        
        print(f"\nBatch size: {size} points")
        print(f"Old method - Write: {old_write:.4f}s, Read: {old_read:.4f}s")
        print(f"New method - Write: {new_write:.4f}s, Read: {new_read:.4f}s")
        print(f"Write speedup: {old_write/new_write:.2f}x")
        print(f"Read speedup: {old_read/new_read:.2f}x")

def test_query_patterns():
    """Test different query patterns"""
    print("\n=== Query Patterns Test ===")
    
    # Setup test data
    channels = 5
    points_per_channel = 100
    
    # Create test data for multiple channels
    print("\nSetting up test data...")
    for channel_id in range(1, channels + 1):
        data = generate_test_data(points_per_channel)
        hash_key = f"comsrv:realtime:channel:{channel_id}"
        r.hset(hash_key, mapping=data)
    
    # Test 1: Get all points from one channel
    start_time = time.time()
    channel_data = r.hgetall("comsrv:realtime:channel:1")
    query1_time = time.time() - start_time
    print(f"\nQuery 1 - Get all points from one channel: {query1_time:.4f}s ({len(channel_data)} points)")
    
    # Test 2: Get specific point from multiple channels
    start_time = time.time()
    for channel_id in range(1, channels + 1):
        value = r.hget(f"comsrv:realtime:channel:{channel_id}", "point_10")
    query2_time = time.time() - start_time
    print(f"Query 2 - Get specific point from {channels} channels: {query2_time:.4f}s")
    
    # Test 3: Get multiple specific points from one channel
    start_time = time.time()
    points = ["point_10", "point_20", "point_30", "point_40", "point_50"]
    values = r.hmget("comsrv:realtime:channel:1", points)
    query3_time = time.time() - start_time
    print(f"Query 3 - Get {len(points)} specific points from one channel: {query3_time:.4f}s")
    
    # Cleanup
    for channel_id in range(1, channels + 1):
        r.delete(f"comsrv:realtime:channel:{channel_id}")

def test_netsrv_cloud_status():
    """Test netsrv cloud status Hash storage"""
    print("\n=== NetSrv Cloud Status Test ===")
    
    networks = ["aws-iot", "aliyun-mqtt", "azure-iothub", "http-webhook"]
    
    # Write cloud status
    start_time = time.time()
    for network in networks:
        status_key = f"netsrv:cloud:status:{network}"
        status_data = {
            "connected": "true",
            "last_sync_time": datetime.now().isoformat(),
            "messages_sent": str(random.randint(1000, 10000)),
            "messages_failed": str(random.randint(0, 100)),
            "queue_size": str(random.randint(0, 50)),
            "updated_at": datetime.now().isoformat()
        }
        r.hset(status_key, mapping=status_data)
    
    write_time = time.time() - start_time
    print(f"\nWrite {len(networks)} network statuses: {write_time:.4f}s")
    
    # Read all statuses
    start_time = time.time()
    all_statuses = {}
    for network in networks:
        status_key = f"netsrv:cloud:status:{network}"
        all_statuses[network] = r.hgetall(status_key)
    
    read_time = time.time() - start_time
    print(f"Read all network statuses: {read_time:.4f}s")
    
    # Display summary
    total_sent = sum(int(status.get("messages_sent", "0")) for status in all_statuses.values())
    total_failed = sum(int(status.get("messages_failed", "0")) for status in all_statuses.values())
    
    print(f"\nCloud Status Summary:")
    print(f"Total messages sent: {total_sent}")
    print(f"Total messages failed: {total_failed}")
    print(f"Success rate: {(total_sent/(total_sent+total_failed)*100):.2f}%")
    
    # Cleanup
    for network in networks:
        r.delete(f"netsrv:cloud:status:{network}")

def test_modsrv_module_data():
    """Test modsrv module data Hash storage"""
    print("\n=== ModSrv Module Data Test ===")
    
    modules = ["calc_module_1", "calc_module_2", "calc_module_3"]
    points_per_module = 50
    
    # Write module data
    start_time = time.time()
    for module_id in modules:
        module_key = f"modsrv:realtime:module:{module_id}"
        module_data = {}
        for i in range(points_per_module):
            point_id = f"calc_point_{i}"
            module_data[point_id] = json.dumps({
                "value": round(random.uniform(0, 1000), 2),
                "formula": f"point_a * {i} + point_b",
                "timestamp": datetime.now().isoformat()
            })
        r.hset(module_key, mapping=module_data)
    
    write_time = time.time() - start_time
    print(f"\nWrite {len(modules)} modules ({points_per_module} points each): {write_time:.4f}s")
    
    # Read module data
    start_time = time.time()
    for module_id in modules:
        module_key = f"modsrv:realtime:module:{module_id}"
        module_data = r.hgetall(module_key)
    
    read_time = time.time() - start_time
    print(f"Read all module data: {read_time:.4f}s")
    
    # Cleanup
    for module_id in modules:
        r.delete(f"modsrv:realtime:module:{module_id}")

def main():
    print("Redis Hash Structure Performance Test")
    print("=" * 50)
    
    # Check Redis connection
    try:
        r.ping()
        print("✓ Redis connection successful")
    except Exception as e:
        print(f"✗ Redis connection failed: {e}")
        return
    
    # Run tests
    test_batch_operations()
    test_query_patterns()
    test_netsrv_cloud_status()
    test_modsrv_module_data()
    
    print("\n" + "=" * 50)
    print("Performance test completed!")

if __name__ == "__main__":
    main()