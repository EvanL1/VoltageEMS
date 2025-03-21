#!/usr/bin/env python3
"""
清理测试创建的规则
"""
import requests
import sys

BASE_URL = "http://localhost:8000/api"

def delete_rule(rule_id):
    """删除一条规则"""
    print(f"删除规则: {rule_id}")
    response = requests.delete(f"{BASE_URL}/rules/{rule_id}")
    print(f"状态码: {response.status_code}")
    return response.status_code == 200

def list_rules():
    """列出所有规则"""
    print("列出当前规则...")
    response = requests.get(f"{BASE_URL}/rules")
    if response.status_code == 200:
        rules = response.json().get("rules", [])
        print(f"找到 {len(rules)} 条规则:")
        for rule in rules:
            print(f"  - {rule['id']}: {rule['name']}")
        return rules
    else:
        print(f"列出规则失败: {response.text}")
        return []

def main():
    """主函数"""
    # 首先列出当前规则
    rules = list_rules()
    
    # 定义要删除的规则ID列表
    rule_ids_to_delete = [
        'api_test_rule_1742538377_8367', 
        'dag_test_rule_1742538377_9834'
    ]
    
    # 也可以选择删除所有以api_test_或dag_test_开头的规则
    for rule in rules:
        rule_id = rule['id']
        if rule_id.startswith('api_test_') or rule_id.startswith('dag_test_'):
            if rule_id not in rule_ids_to_delete:
                rule_ids_to_delete.append(rule_id)
    
    # 删除规则
    print(f"\n即将删除 {len(rule_ids_to_delete)} 条规则...")
    deleted_count = 0
    for rule_id in rule_ids_to_delete:
        if delete_rule(rule_id):
            deleted_count += 1
    
    # 最终验证
    print("\n清理完成，验证结果:")
    list_rules()
    
    print(f"\n成功删除 {deleted_count}/{len(rule_ids_to_delete)} 条规则")
    return 0

if __name__ == "__main__":
    sys.exit(main()) 