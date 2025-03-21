#!/usr/bin/env python3
import requests
import json
import sys
import time
import random

BASE_URL = "http://localhost:8000/api"

def test_health():
    """Test the health endpoint to ensure the API is running"""
    print("Testing health endpoint...")
    response = requests.get(f"{BASE_URL}/health")
    print(f"Status code: {response.status_code}")
    print(f"Response: {response.json()}")
    return response.status_code == 200

def test_rules_list():
    """Test the rules listing endpoint"""
    print("\nTesting rules listing...")
    response = requests.get(f"{BASE_URL}/rules")
    print(f"Status code: {response.status_code}")
    
    if response.status_code == 200:
        data = response.json()
        rules = data.get("rules", [])
        print(f"Found {len(rules)} rules")
        for rule in rules:
            print(f"  - {rule['id']}: {rule['name']}")
        return True
    else:
        print(f"Error: {response.text}")
        return False

def create_simple_rule():
    """Create a simple rule with a few nodes and edges"""
    rule_id = f"test_rule_{int(time.time())}_{random.randint(1000, 9999)}"
    
    # Define a simple rule with a condition and action
    rule = {
        "id": rule_id,
        "name": "Test Temperature Alert Rule",
        "description": "Send an alert when temperature exceeds threshold",
        "enabled": True,
        "priority": 1,
        "nodes": [
            {
                "id": "input1",
                "name": "Temperature Sensor Input",
                "node_type": "Input",
                "config": {
                    "device_id": "temp_sensor_001",
                    "data_points": ["temperature", "humidity"]
                }
            },
            {
                "id": "condition1",
                "name": "Temperature Threshold Check",
                "node_type": "Condition",
                "config": {
                    "condition": "node.input1.result.temperature > 30"
                }
            },
            {
                "id": "action1",
                "name": "Send Alert",
                "node_type": "Action",
                "config": {
                    "type": "notify",
                    "target": "admin",
                    "message": "High temperature alert!"
                }
            }
        ],
        "edges": [
            {
                "from": "input1",
                "to": "condition1"
            },
            {
                "from": "condition1",
                "to": "action1"
            }
        ]
    }
    
    return rule_id, rule

def test_create_rule():
    """Test creating a new rule"""
    print("\nTesting rule creation...")
    rule_id, rule = create_simple_rule()
    
    print(f"Creating rule with ID: {rule_id}")
    response = requests.post(f"{BASE_URL}/rules", json=rule)
    print(f"Status code: {response.status_code}")
    
    if response.status_code == 201:
        print(f"Response: {response.json()}")
        return rule_id
    else:
        print(f"Error: {response.text}")
        return None

def test_get_rule(rule_id):
    """Test retrieving a rule by ID"""
    print(f"\nTesting get rule: {rule_id}")
    response = requests.get(f"{BASE_URL}/rules/{rule_id}")
    print(f"Status code: {response.status_code}")
    
    if response.status_code == 200:
        rule = response.json().get("rule", {})
        print(f"Retrieved rule: {rule['name']}")
        print(f"Number of nodes: {len(rule['nodes'])}")
        print(f"Number of edges: {len(rule['edges'])}")
        return True
    else:
        print(f"Error: {response.text}")
        return False

def test_update_rule(rule_id):
    """Test updating an existing rule"""
    print(f"\nTesting update rule: {rule_id}")
    
    # First get the current rule
    response = requests.get(f"{BASE_URL}/rules/{rule_id}")
    if response.status_code != 200:
        print(f"Error retrieving rule: {response.text}")
        return False
    
    # Modify the rule
    rule = response.json().get("rule", {})
    rule["description"] = "Updated description for testing"
    
    # Add a new node and edge (creating a more complex graph)
    rule["nodes"].append({
        "id": "transform1",
        "name": "Temperature Unit Conversion",
        "node_type": "Transform",
        "config": {
            "type": "calculate",
            "formula": "node.input1.result.temperature * 1.8 + 32"
        }
    })
    
    # Connect input to transform, and transform to condition
    rule["edges"].append({
        "from": "input1",
        "to": "transform1"
    })
    
    # Update the rule
    response = requests.put(f"{BASE_URL}/rules/{rule_id}", json=rule)
    print(f"Status code: {response.status_code}")
    
    if response.status_code == 200:
        updated_rule = response.json().get("rule", {})
        print(f"Updated rule: {updated_rule['name']}")
        print(f"New number of nodes: {len(updated_rule['nodes'])}")
        print(f"New number of edges: {len(updated_rule['edges'])}")
        return True
    else:
        print(f"Error: {response.text}")
        return False

def test_complex_rule():
    """Test creating a more complex rule with a DAG structure"""
    print("\nTesting creation of a complex rule with DAG structure...")
    rule_id = f"complex_rule_{int(time.time())}_{random.randint(1000, 9999)}"
    
    # Define a complex rule with multiple inputs, conditions, and an aggregate node
    rule = {
        "id": rule_id,
        "name": "Complex Environmental Monitoring Rule",
        "description": "Monitor multiple sensors and trigger alerts based on combined conditions",
        "enabled": True,
        "priority": 2,
        "nodes": [
            {
                "id": "temp_input",
                "name": "Temperature Sensor Input",
                "node_type": "Input",
                "config": {
                    "device_id": "temp_sensor_001",
                    "data_points": ["temperature"]
                }
            },
            {
                "id": "humid_input",
                "name": "Humidity Sensor Input",
                "node_type": "Input",
                "config": {
                    "device_id": "humid_sensor_001",
                    "data_points": ["humidity"]
                }
            },
            {
                "id": "pressure_input",
                "name": "Pressure Sensor Input",
                "node_type": "Input",
                "config": {
                    "device_id": "pressure_sensor_001",
                    "data_points": ["pressure"]
                }
            },
            {
                "id": "temp_condition",
                "name": "High Temperature Check",
                "node_type": "Condition",
                "config": {
                    "condition": "node.temp_input.result.temperature > 30"
                }
            },
            {
                "id": "humid_condition",
                "name": "High Humidity Check",
                "node_type": "Condition",
                "config": {
                    "condition": "node.humid_input.result.humidity > 80"
                }
            },
            {
                "id": "pressure_condition",
                "name": "Low Pressure Check",
                "node_type": "Condition",
                "config": {
                    "condition": "node.pressure_input.result.pressure < 980"
                }
            },
            {
                "id": "aggregate",
                "name": "Combine Conditions",
                "node_type": "Aggregate",
                "config": {
                    "type": "any",
                    "threshold": 2
                }
            },
            {
                "id": "alert_action",
                "name": "Send Alert",
                "node_type": "Action",
                "config": {
                    "type": "notify",
                    "target": "admin",
                    "message": "Environmental alert! Check conditions immediately."
                }
            }
        ],
        "edges": [
            {"from": "temp_input", "to": "temp_condition"},
            {"from": "humid_input", "to": "humid_condition"},
            {"from": "pressure_input", "to": "pressure_condition"},
            {"from": "temp_condition", "to": "aggregate"},
            {"from": "humid_condition", "to": "aggregate"},
            {"from": "pressure_condition", "to": "aggregate"},
            {"from": "aggregate", "to": "alert_action"}
        ]
    }
    
    print(f"Creating complex rule with ID: {rule_id}")
    response = requests.post(f"{BASE_URL}/rules", json=rule)
    print(f"Status code: {response.status_code}")
    
    if response.status_code == 201:
        print(f"Complex rule created successfully")
        return rule_id
    else:
        print(f"Error: {response.text}")
        return None

def test_execute_rule(rule_id):
    """Test executing a rule with context data"""
    print(f"\nTesting rule execution: {rule_id}")
    
    # Create context data for rule execution
    context = {
        "device_data": {
            "temp_sensor_001": {
                "temperature": 35,
                "humidity": 65
            },
            "humid_sensor_001": {
                "humidity": 85
            },
            "pressure_sensor_001": {
                "pressure": 975
            }
        }
    }
    
    response = requests.post(f"{BASE_URL}/rules/{rule_id}/execute", json=context)
    print(f"Status code: {response.status_code}")
    
    if response.status_code == 200:
        result = response.json()
        print(f"Execution result: {json.dumps(result, indent=2)}")
        return True
    else:
        print(f"Error: {response.text}")
        return False

def test_delete_rule(rule_id):
    """Test deleting a rule"""
    print(f"\nTesting rule deletion: {rule_id}")
    response = requests.delete(f"{BASE_URL}/rules/{rule_id}")
    print(f"Status code: {response.status_code}")
    
    if response.status_code == 200:
        print(f"Rule deleted successfully")
        
        # Verify rule is gone
        verify_response = requests.get(f"{BASE_URL}/rules/{rule_id}")
        if verify_response.status_code == 404:
            print("Verified: Rule no longer exists")
            return True
        else:
            print(f"Error: Rule still exists with status code {verify_response.status_code}")
            return False
    else:
        print(f"Error: {response.text}")
        return False

def main():
    """Main test function"""
    print("ModSrv Rules API Test Script")
    print("===========================")
    
    # Run the tests in sequence
    if not test_health():
        print("Health check failed, aborting")
        return 1
    
    test_rules_list()
    
    # Create and test a simple rule
    simple_rule_id = test_create_rule()
    if simple_rule_id:
        test_get_rule(simple_rule_id)
        test_update_rule(simple_rule_id)
    
    # Create and test a complex rule
    complex_rule_id = test_complex_rule()
    if complex_rule_id:
        test_get_rule(complex_rule_id)
        test_execute_rule(complex_rule_id)
    
    # Clean up
    if simple_rule_id:
        test_delete_rule(simple_rule_id)
    if complex_rule_id:
        test_delete_rule(complex_rule_id)
    
    # Final list to verify deletion
    test_rules_list()
    
    print("\nAll tests completed!")
    return 0

if __name__ == "__main__":
    sys.exit(main()) 