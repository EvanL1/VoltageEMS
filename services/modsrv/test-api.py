#!/usr/bin/env python3
import requests
import json
import sys
import random
import time

BASE_URL = "http://localhost:8001/api/v1"

def test_health():
    print("Testing health endpoint...")
    response = requests.get(f"{BASE_URL}/health")
    print(f"Status code: {response.status_code}")
    print(f"Response: {response.json()}")
    return response.status_code == 200

def test_templates():
    print("\nTesting templates endpoint...")
    response = requests.get(f"{BASE_URL}/templates")
    print(f"Status code: {response.status_code}")
    templates = response.json().get("templates", [])
    print(f"Found {len(templates)} templates:")
    for template in templates:
        print(f"  - {template['name']}: {template['description']}")
    return response.status_code == 200

def test_create_instance():
    print("\nTesting instance creation...")
    # Generate a unique instance ID using timestamp and random number
    instance_id = f"motor_{int(time.time())}_{random.randint(1000, 9999)}"
    
    payload = {
        "template_id": "stepper_motor_template",
        "instance_id": instance_id,
        "config": {
            "name": "Test Motor",
            "description": "A test stepper motor instance",
            "parameters": {
                "max_speed": 2000,
                "acceleration": 500
            }
        }
    }
    
    print(f"Creating instance with ID: {instance_id}")
    response = requests.post(f"{BASE_URL}/instances", json=payload)
    print(f"Status code: {response.status_code}")
    if response.status_code == 200 or response.status_code == 201:
        print(f"Response: {response.json()}")
        return True
    else:
        print(f"Error: {response.text}")
        return False

def test_control_operations():
    print("\nTesting control operations...")
    operations = requests.get(f"{BASE_URL}/control/operations")
    print(f"Status code: {operations.status_code}")
    if operations.status_code == 200:
        ops = operations.json()
        print(f"Available operations: {ops}")
        
        if isinstance(ops, list) and len(ops) > 0:
            # Try to execute the first operation
            op_name = ops[0]
            print(f"\nExecuting operation: {op_name}")
            payload = {
                "instance_id": "motor002",
                "parameters": {
                    "speed": 1000
                }
            }
            response = requests.post(f"{BASE_URL}/control/execute/{op_name}", json=payload)
            print(f"Status code: {response.status_code}")
            print(f"Response: {response.json() if response.status_code == 200 else response.text}")
            return response.status_code == 200
        else:
            print("No operations available to test.")
            return True  # Return true because the API is working as expected
    return operations.status_code == 200

def main():
    print("ModSrv API Test Script")
    print("======================")
    
    tests = [
        ("Health Check", test_health),
        ("Templates List", test_templates),
        ("Instance Creation", test_create_instance),
        ("Control Operations", test_control_operations)
    ]
    
    results = []
    for name, test_func in tests:
        try:
            print(f"\n## {name} ##")
            result = test_func()
            results.append((name, result))
        except Exception as e:
            print(f"Error: {str(e)}")
            results.append((name, False))
    
    print("\n\nTest Summary")
    print("============")
    all_passed = True
    for name, result in results:
        status = "PASSED" if result else "FAILED"
        if not result:
            all_passed = False
        print(f"{name}: {status}")
    
    if all_passed:
        print("\nAll tests passed!")
        return 0
    else:
        print("\nSome tests failed.")
        return 1

if __name__ == "__main__":
    sys.exit(main()) 