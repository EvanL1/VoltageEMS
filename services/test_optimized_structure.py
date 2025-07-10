#!/usr/bin/env python3
"""
Performance comparison test for old vs new Redis structure.
"""

import redis
import json
import time
import random
from datetime import datetime

# Test configuration
NUM_POINTS = 1000
NUM_ITERATIONS = 10

def generate_old_structure_data(num_points):
    """Generate test data in old structure"""
    data = {}
    for i in range(num_points):
        point_id = f"point_{i}"
        data[point_id] = json.dumps({
            "id": point_id,
            "name": f"Test Point {i}",
            "value": str(round(random.uniform(0, 100), 2)),
            "timestamp": datetime.now().isoformat() + "Z",
            "quality": "good",
            "unit": "kW",
            "telemetry_type": "Measurement",
            "description": f"Test point {i} description"
        })
    return data

def generate_new_structure_data(num_points):
    """Generate test data in new structure"""
    config_data = {}
    realtime_data = {}
    
    for i in range(num_points):
        point_id = f"point_{i}"
        
        # Configuration (static)
        config_data[point_id] = json.dumps({
            "name": f"Test Point {i}",
            "unit": "kW",
            "telemetry_type": "Measurement",
            "description": f"Test point {i} description",
            "scale": 0.1,
            "offset": 0,
            "address": f"1:3:{1000+i}"
        })
        
        # Realtime data
        raw_value = random.uniform(0, 1000)
        realtime_data[point_id] = json.dumps({
            "raw": raw_value,
            "value": raw_value * 0.1,  # Apply scale
            "ts": int(time.time() * 1000)
        })
    
    return config_data, realtime_data

def test_old_structure_write(r, channel_id, data):
    """Test writing with old structure"""
    key = f"test:old:channel:{channel_id}"
    start = time.time()
    
    # Use pipeline for fair comparison
    pipe = r.pipeline()
    for field, value in data.items():
        pipe.hset(key, field, value)
    pipe.execute()
    
    return time.time() - start

def test_new_structure_write(r, channel_id, config_data, realtime_data):
    """Test writing with new structure"""
    config_key = f"test:new:config:channel:{channel_id}:points"
    realtime_key = f"test:new:realtime:channel:{channel_id}"
    
    start = time.time()
    
    # Write config (only once in real scenario)
    pipe = r.pipeline()
    for field, value in config_data.items():
        pipe.hset(config_key, field, value)
    pipe.execute()
    
    # Write realtime data
    pipe = r.pipeline()
    for field, value in realtime_data.items():
        pipe.hset(realtime_key, field, value)
    pipe.execute()
    
    return time.time() - start

def test_old_structure_read(r, channel_id):
    """Test reading with old structure"""
    key = f"test:old:channel:{channel_id}"
    start = time.time()
    
    data = r.hgetall(key)
    # Parse all JSON
    parsed = {}
    for field, value in data.items():
        parsed[field] = json.loads(value)
    
    return time.time() - start, len(parsed)

def test_new_structure_read(r, channel_id):
    """Test reading with new structure"""
    config_key = f"test:new:config:channel:{channel_id}:points"
    realtime_key = f"test:new:realtime:channel:{channel_id}"
    
    start = time.time()
    
    # In real scenario, config would be cached
    config = r.hgetall(config_key)
    realtime = r.hgetall(realtime_key)
    
    # Parse data
    parsed = {}
    for field, value in realtime.items():
        rt_data = json.loads(value)
        if field in config:
            cfg_data = json.loads(config[field])
            parsed[field] = {
                "name": cfg_data["name"],
                "value": rt_data["value"],
                "raw": rt_data["raw"],
                "unit": cfg_data["unit"]
            }
    
    return time.time() - start, len(parsed)

def calculate_storage_size(r, pattern):
    """Calculate approximate storage size"""
    total_size = 0
    for key in r.scan_iter(match=pattern):
        total_size += r.memory_usage(key)
    return total_size

def main():
    # Connect to Redis
    r = redis.Redis(host='localhost', port=6379, decode_responses=True)
    
    try:
        r.ping()
        print("✓ Redis connection successful")
    except Exception as e:
        print(f"✗ Redis connection failed: {e}")
        return
    
    print(f"\nTesting with {NUM_POINTS} points, {NUM_ITERATIONS} iterations")
    print("="*60)
    
    # Generate test data
    print("\nGenerating test data...")
    old_data = generate_old_structure_data(NUM_POINTS)
    config_data, realtime_data = generate_new_structure_data(NUM_POINTS)
    
    # Test writes
    print("\n--- WRITE PERFORMANCE ---")
    old_write_times = []
    new_write_times = []
    
    for i in range(NUM_ITERATIONS):
        # Old structure
        old_time = test_old_structure_write(r, i, old_data)
        old_write_times.append(old_time)
        
        # New structure (config + realtime)
        new_time = test_new_structure_write(r, i, config_data, realtime_data)
        new_write_times.append(new_time)
    
    avg_old_write = sum(old_write_times) / len(old_write_times)
    avg_new_write = sum(new_write_times) / len(new_write_times)
    
    print(f"Old structure avg write time: {avg_old_write*1000:.2f}ms")
    print(f"New structure avg write time: {avg_new_write*1000:.2f}ms")
    print(f"Write performance ratio: {avg_old_write/avg_new_write:.2f}x")
    
    # Test reads
    print("\n--- READ PERFORMANCE ---")
    old_read_times = []
    new_read_times = []
    
    for i in range(NUM_ITERATIONS):
        # Old structure
        old_time, _ = test_old_structure_read(r, 0)
        old_read_times.append(old_time)
        
        # New structure
        new_time, _ = test_new_structure_read(r, 0)
        new_read_times.append(new_time)
    
    avg_old_read = sum(old_read_times) / len(old_read_times)
    avg_new_read = sum(new_read_times) / len(new_read_times)
    
    print(f"Old structure avg read time: {avg_old_read*1000:.2f}ms")
    print(f"New structure avg read time: {avg_new_read*1000:.2f}ms")
    print(f"Read performance ratio: {avg_old_read/avg_new_read:.2f}x")
    
    # Storage comparison
    print("\n--- STORAGE COMPARISON ---")
    old_size = calculate_storage_size(r, "test:old:*")
    new_size = calculate_storage_size(r, "test:new:*")
    
    if old_size > 0 and new_size > 0:
        print(f"Old structure size: {old_size:,} bytes")
        print(f"New structure size: {new_size:,} bytes")
        print(f"Storage savings: {(1 - new_size/old_size)*100:.1f}%")
    
    # Real-world simulation - frequent updates
    print("\n--- REAL-WORLD SIMULATION ---")
    print("Simulating frequent realtime updates (config cached)...")
    
    # Old structure - must update entire JSON
    start = time.time()
    for _ in range(100):
        point_id = f"point_{random.randint(0, NUM_POINTS-1)}"
        # Must read, modify, write entire JSON
        data = json.loads(old_data[point_id])
        data["value"] = str(round(random.uniform(0, 100), 2))
        data["timestamp"] = datetime.now().isoformat() + "Z"
        r.hset("test:old:channel:0", point_id, json.dumps(data))
    old_update_time = time.time() - start
    
    # New structure - only update realtime value
    start = time.time()
    for _ in range(100):
        point_id = f"point_{random.randint(0, NUM_POINTS-1)}"
        raw_value = random.uniform(0, 1000)
        rt_data = {
            "raw": raw_value,
            "value": raw_value * 0.1,
            "ts": int(time.time() * 1000)
        }
        r.hset("test:new:realtime:channel:0", point_id, json.dumps(rt_data))
    new_update_time = time.time() - start
    
    print(f"Old structure 100 updates: {old_update_time*1000:.2f}ms")
    print(f"New structure 100 updates: {new_update_time*1000:.2f}ms")
    print(f"Update performance ratio: {old_update_time/new_update_time:.2f}x")
    
    # Cleanup
    print("\nCleaning up test data...")
    for key in r.scan_iter(match="test:*"):
        r.delete(key)
    
    print("\n✓ Test completed!")

if __name__ == "__main__":
    main()