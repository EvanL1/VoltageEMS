#!/usr/bin/env python3
import requests
import json
import sys
import random
import time

BASE_URL = "http://localhost:8000/api"

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

def test_rules_endpoints():
    print("\nTesting rules endpoints...")
    # First check if any rules exist
    response = requests.get(f"{BASE_URL}/rules")
    print(f"Rules list status code: {response.status_code}")
    
    if response.status_code == 200:
        rules_count = len(response.json().get("rules", []))
        print(f"Found {rules_count} existing rules")
        
        # Create a simple rule for testing
        rule_id = f"api_test_rule_{int(time.time())}"
        test_rule = {
            "id": rule_id,
            "name": "API Test Rule",
            "description": "Rule created by the API test script",
            "enabled": True,
            "priority": 1,
            "nodes": [
                {
                    "id": "input1",
                    "name": "Test Input",
                    "node_type": "Input",
                    "config": {
                        "device_id": "test_device",
                        "data_points": ["status"]
                    }
                },
                {
                    "id": "action1",
                    "name": "Test Action",
                    "node_type": "Action",
                    "config": {
                        "type": "notify",
                        "target": "system",
                        "message": "Test notification"
                    }
                }
            ],
            "edges": [
                {
                    "from": "input1",
                    "to": "action1"
                }
            ]
        }
        
        # Create the rule
        print(f"Creating test rule with ID: {rule_id}")
        create_response = requests.post(f"{BASE_URL}/rules", json=test_rule)
        print(f"Create rule status code: {create_response.status_code}")
        
        if create_response.status_code == 201:
            # Get the rule
            get_response = requests.get(f"{BASE_URL}/rules/{rule_id}")
            print(f"Get rule status code: {get_response.status_code}")
            
            # Delete the rule to clean up
            delete_response = requests.delete(f"{BASE_URL}/rules/{rule_id}")
            print(f"Delete rule status code: {delete_response.status_code}")
            
            return create_response.status_code == 201 and get_response.status_code == 200 and delete_response.status_code == 200
        else:
            print(f"Error creating rule: {create_response.text}")
            return False
    else:
        print(f"Error listing rules: {response.text}")
        return False

def main():
    print("ModSrv API Test Script")
    print("======================")
    
    tests = [
        ("Health Check", test_health),
        ("Templates List", test_templates),
        ("Instance Creation", test_create_instance),
        ("Control Operations", test_control_operations),
        ("Rules Endpoints", test_rules_endpoints)
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