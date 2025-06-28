#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Configuration Diagnosis Tool for comsrv
配置诊断工具

用于检查comsrv配置文件的格式和内容是否正确
"""

import yaml
import json
import sys
import os
from pathlib import Path
from typing import Dict, List, Any, Optional

def load_yaml_config(config_path: str) -> Optional[Dict[str, Any]]:
    """加载YAML配置文件"""
    try:
        with open(config_path, 'r', encoding='utf-8') as f:
            config = yaml.safe_load(f)
        print(f"✅ 成功加载配置文件: {config_path}")
        return config
    except FileNotFoundError:
        print(f"❌ 配置文件不存在: {config_path}")
        return None
    except yaml.YAMLError as e:
        print(f"❌ YAML解析错误: {e}")
        return None
    except Exception as e:
        print(f"❌ 加载配置文件时发生错误: {e}")
        return None

def validate_service_config(service_config: Dict[str, Any]) -> List[str]:
    """验证服务配置"""
    errors = []
    
    # 检查必需字段
    if 'name' not in service_config:
        errors.append("缺少service.name字段")
    
    # 检查日志配置
    if 'logging' in service_config:
        logging_config = service_config['logging']
        if 'level' not in logging_config:
            errors.append("缺少service.logging.level字段")
        elif logging_config['level'] not in ['trace', 'debug', 'info', 'warn', 'error']:
            errors.append(f"无效的日志级别: {logging_config['level']}")
    
    # 检查API配置
    if 'api' in service_config:
        api_config = service_config['api']
        if 'bind_address' in api_config:
            bind_addr = api_config['bind_address']
            if ':' not in bind_addr:
                errors.append(f"无效的API绑定地址格式: {bind_addr}")
    
    # 检查Redis配置
    if 'redis' in service_config:
        redis_config = service_config['redis']
        if 'url' in redis_config:
            url = redis_config['url']
            if not url.startswith('redis://') and not url.startswith('rediss://'):
                errors.append(f"无效的Redis URL格式: {url}")
    
    return errors

def validate_channel_config(channel: Dict[str, Any], channel_idx: int) -> List[str]:
    """验证通道配置"""
    errors = []
    
    # 检查必需字段
    required_fields = ['id', 'name', 'protocol', 'parameters']
    for field in required_fields:
        if field not in channel:
            errors.append(f"通道{channel_idx}: 缺少{field}字段")
    
    # 检查协议类型
    if 'protocol' in channel:
        protocol = channel['protocol']
        valid_protocols = ['Virtual', 'ModbusTcp', 'ModbusRtu', 'Iec104', 'Can', 'Dio', 'Iec61850']
        if protocol not in valid_protocols:
            errors.append(f"通道{channel_idx}: 无效的协议类型: {protocol}")
    
    # 检查参数配置
    if 'parameters' in channel and 'protocol' in channel:
        protocol = channel['protocol']
        params = channel['parameters']
        
        if protocol == 'ModbusTcp':
            if 'host' not in params:
                errors.append(f"通道{channel_idx}: ModbusTcp协议缺少host参数")
            if 'port' not in params:
                errors.append(f"通道{channel_idx}: ModbusTcp协议缺少port参数")
        
        elif protocol == 'ModbusRtu':
            if 'port' not in params:
                errors.append(f"通道{channel_idx}: ModbusRtu协议缺少port参数")
            if 'baud_rate' not in params:
                errors.append(f"通道{channel_idx}: ModbusRtu协议缺少baud_rate参数")
        
        elif protocol == 'Virtual':
            # Virtual协议的参数相对宽松
            pass
    
    # 检查通道日志配置
    if 'logging' in channel:
        logging_config = channel['logging']
        if 'level' in logging_config:
            level = logging_config['level']
            if level not in ['trace', 'debug', 'info', 'warn', 'error']:
                errors.append(f"通道{channel_idx}: 无效的日志级别: {level}")
    
    return errors

def validate_config(config: Dict[str, Any]) -> List[str]:
    """验证完整配置"""
    errors = []
    
    # 检查版本
    if 'version' not in config:
        errors.append("缺少version字段")
    
    # 检查服务配置
    if 'service' not in config:
        errors.append("缺少service配置")
    else:
        service_errors = validate_service_config(config['service'])
        errors.extend(service_errors)
    
    # 检查通道配置
    if 'channels' not in config:
        errors.append("缺少channels配置")
    else:
        channels = config['channels']
        if not isinstance(channels, list):
            errors.append("channels必须是数组")
        else:
            channel_ids = set()
            for idx, channel in enumerate(channels):
                channel_errors = validate_channel_config(channel, idx)
                errors.extend(channel_errors)
                
                # 检查通道ID重复
                if 'id' in channel:
                    channel_id = channel['id']
                    if channel_id in channel_ids:
                        errors.append(f"重复的通道ID: {channel_id}")
                    channel_ids.add(channel_id)
    
    return errors

def print_config_summary(config: Dict[str, Any]):
    """打印配置摘要信息"""
    print("\n" + "="*60)
    print("配置文件摘要信息")
    print("="*60)
    
    # 服务信息
    if 'service' in config:
        service = config['service']
        print(f"服务名称: {service.get('name', 'N/A')}")
        print(f"服务描述: {service.get('description', 'N/A')}")
        
        if 'logging' in service:
            logging_config = service['logging']
            print(f"日志级别: {logging_config.get('level', 'N/A')}")
            print(f"控制台输出: {logging_config.get('console', 'N/A')}")
        
        if 'api' in service:
            api_config = service['api']
            print(f"API启用: {api_config.get('enabled', 'N/A')}")
            print(f"API地址: {api_config.get('bind_address', 'N/A')}")
        
        if 'redis' in service:
            redis_config = service['redis']
            print(f"Redis启用: {redis_config.get('enabled', 'N/A')}")
            print(f"Redis URL: {redis_config.get('url', 'N/A')}")
    
    # 通道信息
    if 'channels' in config:
        channels = config['channels']
        print(f"\n通道数量: {len(channels)}")
        
        for idx, channel in enumerate(channels):
            print(f"\n通道 {idx + 1}:")
            print(f"  ID: {channel.get('id', 'N/A')}")
            print(f"  名称: {channel.get('name', 'N/A')}")
            print(f"  协议: {channel.get('protocol', 'N/A')}")
            print(f"  描述: {channel.get('description', 'N/A')}")
            
            if 'parameters' in channel:
                params = channel['parameters']
                print(f"  参数: {json.dumps(params, indent=4, ensure_ascii=False)}")

def main():
    """主函数"""
    print("ComsrvConfiguration Diagnosis Tool")
    print("comsrv配置诊断工具")
    print("="*60)
    
    # 获取配置文件路径
    if len(sys.argv) > 1:
        config_path = sys.argv[1]
    else:
        # 默认配置文件路径
        config_files = [
            "config/comsrv.yaml",
            "config/comsrv_example.yaml", 
            "config/comsrv_test_minimal.yaml"
        ]
        
        config_path = None
        for file_path in config_files:
            if os.path.exists(file_path):
                config_path = file_path
                break
        
        if not config_path:
            print("❌ 未找到配置文件，请指定配置文件路径:")
            print("   python test_config_diagnosis.py <config_file_path>")
            sys.exit(1)
    
    print(f"诊断配置文件: {config_path}")
    
    # 加载配置文件
    config = load_yaml_config(config_path)
    if config is None:
        sys.exit(1)
    
    # 验证配置
    errors = validate_config(config)
    
    # 输出验证结果
    if errors:
        print(f"\n❌ 发现 {len(errors)} 个配置错误:")
        for idx, error in enumerate(errors, 1):
            print(f"  {idx}. {error}")
    else:
        print("\n✅ 配置验证通过，未发现错误")
    
    # 打印配置摘要
    print_config_summary(config)
    
    # 输出JSON格式(用于调试)
    print(f"\n" + "="*60)
    print("JSON格式配置(用于调试)")
    print("="*60)
    try:
        json_config = json.dumps(config, indent=2, ensure_ascii=False)
        print(json_config)
    except Exception as e:
        print(f"❌ 转换为JSON时发生错误: {e}")

if __name__ == "__main__":
    main() 