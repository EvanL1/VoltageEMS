#!/usr/bin/env python3
"""
Test script to validate Redis Hash optimizations for VoltageEMS services
Tests the optimized storage structures for comsrv, modsrv, and alarmsrv
"""

import redis
import json
import time
import random
from datetime import datetime, timedelta

# Connect to Redis
r = redis.Redis(host='localhost', port=6379, decode_responses=True)

def test_comsrv_hash_structure():
    """Test comsrv Hash structure for real-time data"""
    print("\n=== Testing comsrv Hash Structure ===")
    
    # Simulate data for multiple channels
    channels = [1, 2, 3]
    telemetry_types = ["Measurement", "Signal", "Control", "Adjustment"]
    
    start_time = time.time()
    
    # Write test data
    for channel_id in channels:
        hash_key = f"comsrv:realtime:channel:{channel_id}"
        fields = {}
        
        for telemetry_type in telemetry_types:
            for point_id in range(1, 101):  # 100 points per type
                field = f"{telemetry_type}:{point_id}"
                value = {
                    "id": point_id,
                    "name": f"{telemetry_type}_Point_{point_id}",
                    "value": random.uniform(0, 100),
                    "unit": "kW" if telemetry_type == "Measurement" else "",
                    "timestamp": datetime.utcnow().isoformat(),
                    "telemetry_type": telemetry_type,
                    "channel_id": channel_id
                }
                fields[field] = json.dumps(value)
        
        # Use HMSET for batch update
        r.hset(hash_key, mapping=fields)
    
    write_time = time.time() - start_time
    print(f"Written {len(channels) * len(telemetry_types) * 100} points in {write_time:.3f}s")
    
    # Test single point query
    start_time = time.time()
    result = r.hget("comsrv:realtime:channel:1", "Measurement:50")
    single_query_time = time.time() - start_time
    print(f"Single point query time: {single_query_time * 1000:.3f}ms")
    
    # Test channel batch query
    start_time = time.time()
    all_values = r.hgetall("comsrv:realtime:channel:1")
    batch_query_time = time.time() - start_time
    print(f"Channel batch query ({len(all_values)} points): {batch_query_time * 1000:.3f}ms")
    
    # Test filtered query (by telemetry type)
    start_time = time.time()
    measurement_points = {k: v for k, v in all_values.items() if k.startswith("Measurement:")}
    filter_time = time.time() - start_time
    print(f"Filtered query ({len(measurement_points)} Measurement points): {filter_time * 1000:.3f}ms")

def test_modsrv_hash_structure():
    """Test modsrv Hash structure for module data"""
    print("\n=== Testing modsrv Hash Structure ===")
    
    # Simulate data for multiple modules
    modules = ["calc_module_1", "calc_module_2", "calc_module_3"]
    
    start_time = time.time()
    
    # Write test data
    for module_id in modules:
        hash_key = f"modsrv:realtime:module:{module_id}"
        fields = {}
        
        for point_id in range(1, 51):  # 50 calculated points per module
            value_data = {
                "value": random.uniform(0, 1000),
                "timestamp": datetime.utcnow().isoformat(),
                "quality": "good"
            }
            fields[f"calc_point_{point_id}"] = json.dumps(value_data)
        
        r.hset(hash_key, mapping=fields)
    
    write_time = time.time() - start_time
    print(f"Written {len(modules) * 50} calculated points in {write_time:.3f}s")
    
    # Test module query
    start_time = time.time()
    module_data = r.hgetall("modsrv:realtime:module:calc_module_1")
    query_time = time.time() - start_time
    print(f"Module query ({len(module_data)} points): {query_time * 1000:.3f}ms")

def test_alarmsrv_optimized_structure():
    """Test alarmsrv optimized structure with time-based sharding"""
    print("\n=== Testing alarmsrv Optimized Structure ===")
    
    # Create test alarms
    channels = ["channel_1", "channel_2", "channel_3"]
    levels = ["Critical", "Warning", "Info"]
    
    # Generate alarms across different time buckets
    current_time = datetime.utcnow()
    alarms_created = 0
    
    start_time = time.time()
    
    # Create alarms for the last 3 hours
    for hours_ago in range(3):
        alarm_time = current_time - timedelta(hours=hours_ago)
        hour_bucket = alarm_time.strftime("%Y%m%d%H")
        
        for i in range(20):  # 20 alarms per hour
            alarm_id = f"alarm_{hour_bucket}_{i}"
            channel = random.choice(channels)
            level = random.choice(levels)
            
            # Store in time-based shard
            shard_key = f"ems:alarms:shard:{hour_bucket}:{alarm_id}"
            alarm_data = {
                "id": alarm_id,
                "channel": channel,
                "level": level,
                "description": f"Test alarm {alarm_id}",
                "status": "Active",
                "created_at": alarm_time.isoformat(),
                "data": json.dumps({
                    "id": alarm_id,
                    "channel": channel,
                    "level": level,
                    "description": f"Test alarm {alarm_id}",
                    "created_at": alarm_time.isoformat()
                })
            }
            r.hset(shard_key, mapping=alarm_data)
            
            # Add to bucket index
            bucket_index_key = f"ems:alarms:buckets:{hour_bucket}"
            r.sadd(bucket_index_key, alarm_id)
            
            # Add to realtime hash
            realtime_key = "ems:alarms:realtime"
            field = f"{channel}:{alarm_id}"
            realtime_data = {
                "id": alarm_id,
                "channel": channel,
                "level": level,
                "description": f"Test alarm {alarm_id}",
                "status": "Active",
                "created_at": alarm_time.isoformat()
            }
            r.hset(realtime_key, field, json.dumps(realtime_data))
            
            alarms_created += 1
    
    write_time = time.time() - start_time
    print(f"Created {alarms_created} alarms in {write_time:.3f}s")
    
    # Test recent alarms query
    start_time = time.time()
    recent_alarms = r.hgetall("ems:alarms:realtime")
    recent_query_time = time.time() - start_time
    print(f"Recent alarms query ({len(recent_alarms)} alarms): {recent_query_time * 1000:.3f}ms")
    
    # Test time-range query (last hour)
    start_time = time.time()
    last_hour_bucket = current_time.strftime("%Y%m%d%H")
    bucket_alarms = r.smembers(f"ems:alarms:buckets:{last_hour_bucket}")
    time_range_query_time = time.time() - start_time
    print(f"Time-range query (last hour, {len(bucket_alarms)} alarms): {time_range_query_time * 1000:.3f}ms")

def compare_with_old_structure():
    """Compare performance with old String-based structure"""
    print("\n=== Performance Comparison ===")
    
    # Old structure: individual String keys
    print("Old structure (String keys):")
    start_time = time.time()
    for i in range(1000):
        key = f"point:{i}"
        value = json.dumps({"value": random.uniform(0, 100), "timestamp": datetime.utcnow().isoformat()})
        r.set(key, value)
    old_write_time = time.time() - start_time
    print(f"  Write 1000 points: {old_write_time:.3f}s")
    
    # Query all points
    start_time = time.time()
    keys = r.keys("point:*")
    values = []
    for key in keys[:100]:  # Get first 100
        values.append(r.get(key))
    old_query_time = time.time() - start_time
    print(f"  Query 100 points: {old_query_time * 1000:.3f}ms")
    
    # New structure: Hash
    print("\nNew structure (Hash):")
    start_time = time.time()
    hash_data = {}
    for i in range(1000):
        field = f"point_{i}"
        value = json.dumps({"value": random.uniform(0, 100), "timestamp": datetime.utcnow().isoformat()})
        hash_data[field] = value
    r.hset("optimized:points", mapping=hash_data)
    new_write_time = time.time() - start_time
    print(f"  Write 1000 points: {new_write_time:.3f}s")
    
    # Query all points
    start_time = time.time()
    all_values = r.hgetall("optimized:points")
    new_query_time = time.time() - start_time
    print(f"  Query all points: {new_query_time * 1000:.3f}ms")
    
    print(f"\nPerformance improvement:")
    print(f"  Write speed: {old_write_time/new_write_time:.2f}x faster")
    print(f"  Query speed: {old_query_time/new_query_time:.2f}x faster")

def cleanup_test_data():
    """Clean up test data"""
    print("\n=== Cleaning up test data ===")
    
    # Clean comsrv data
    for key in r.keys("comsrv:realtime:*"):
        r.delete(key)
    
    # Clean modsrv data
    for key in r.keys("modsrv:realtime:*"):
        r.delete(key)
    
    # Clean alarmsrv data
    for key in r.keys("ems:alarms:shard:*"):
        r.delete(key)
    for key in r.keys("ems:alarms:buckets:*"):
        r.delete(key)
    r.delete("ems:alarms:realtime")
    
    # Clean test data
    for key in r.keys("point:*"):
        r.delete(key)
    r.delete("optimized:points")
    
    print("Test data cleaned up")

if __name__ == "__main__":
    print("Redis Hash Optimization Test Suite for VoltageEMS")
    print("=" * 50)
    
    try:
        # Run tests
        test_comsrv_hash_structure()
        test_modsrv_hash_structure()
        test_alarmsrv_optimized_structure()
        compare_with_old_structure()
        
        # Cleanup
        cleanup_test_data()
        
        print("\n✅ All tests completed successfully!")
        
    except Exception as e:
        print(f"\n❌ Error during testing: {e}")
        cleanup_test_data()