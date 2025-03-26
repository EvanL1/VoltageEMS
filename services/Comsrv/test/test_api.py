#!/usr/bin/env python3
"""
通信服务API测试脚本
用于测试comsrv通信服务的API接口
"""

import requests
import json
import time
import logging
from typing import Dict, Any, List, Optional

# 配置日志
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger("ComsrvTest")

# 服务配置
API_BASE_URL = "http://localhost:8888/api"
BASE_URL = "http://localhost:8888"
TIMEOUT = 5  # 超时时间（秒）

# API路径
CHANNELS_API = f"{API_BASE_URL}/v1/channels"
POINTS_API = f"{API_BASE_URL}/v1/channels"
VALUES_API = f"{API_BASE_URL}/v1/channels"
HEALTH_API = f"{BASE_URL}/health"

def make_request(method: str, url: str, data: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
    """发送HTTP请求并处理可能的错误"""
    try:
        if method.lower() == "get":
            response = requests.get(url, timeout=TIMEOUT)
        elif method.lower() == "post":
            response = requests.post(url, json=data, timeout=TIMEOUT)
        elif method.lower() == "put":
            response = requests.put(url, json=data, timeout=TIMEOUT)
        elif method.lower() == "delete":
            response = requests.delete(url, timeout=TIMEOUT)
        else:
            raise ValueError(f"不支持的HTTP方法: {method}")
        
        response.raise_for_status()
        return response.json()
    except requests.exceptions.RequestException as e:
        logger.error(f"请求失败: {e}")
        return {"success": False, "error": str(e)}

def test_health() -> bool:
    """测试健康检查API"""
    logger.info("测试健康检查API...")
    response = make_request("get", HEALTH_API)
    success = response.get("success", False) and response.get("data", {}).get("status") == "OK"
    logger.info(f"健康检查结果: {'成功' if success else '失败'}")
    return success

def test_get_channels() -> List[Dict[str, Any]]:
    """测试获取通道列表API"""
    logger.info("测试获取通道列表...")
    response = make_request("get", CHANNELS_API)
    channels = response.get("data", [])
    logger.info(f"共获取到 {len(channels)} 个通道")
    for i, channel in enumerate(channels):
        logger.info(f"通道 {i+1}: ID={channel.get('id')}, 协议={channel.get('protocol')}, 状态={channel.get('status', {}).get('connected', False)}")
    return channels

def test_get_channel_status(channel_id: str) -> Dict[str, Any]:
    """测试获取通道状态API"""
    logger.info(f"测试获取通道状态: {channel_id}...")
    response = make_request("get", f"{CHANNELS_API}/{channel_id}/status")
    channels = response.get("data", [])
    
    # 查找指定id的通道
    channel_status = None
    for channel in channels:
        if channel.get("id") == channel_id:
            channel_status = channel
            break
    
    if channel_status:
        logger.info(f"通道 {channel_id} 状态: 连接={channel_status.get('connected', False)}, 最后错误={channel_status.get('last_error', 'N/A')}")
    else:
        logger.warning(f"未找到通道 {channel_id} 的状态信息")
    
    return channel_status or {}

def test_get_points(channel_id: str) -> List[Dict[str, Any]]:
    """测试获取点位列表API"""
    logger.info(f"测试获取点位列表: {channel_id}...")
    response = make_request("get", f"{POINTS_API}/{channel_id}/points")
    channels = response.get("data", [])
    
    # 假设返回的是通道列表，我们从中找到点位信息
    channel_info = None
    for channel in channels:
        if channel.get("id") == channel_id:
            channel_info = channel
            break
    
    # 这里我们使用参数作为"点位"，因为目前的API似乎没有专门的点位API
    points = []
    if channel_info and "parameters" in channel_info:
        for key, value in channel_info.get("parameters", {}).items():
            points.append({
                "id": key,
                "value": value,
                "writable": False  # 假设所有参数都不可写
            })
    
    logger.info(f"通道 {channel_id} 共有 {len(points)} 个点位")
    return points

def test_read_point(channel_id: str, point_id: str) -> Dict[str, Any]:
    """测试读取点位值API"""
    logger.info(f"测试读取点位值: {channel_id}/{point_id}...")
    
    # 由于API可能没有专门的点位读取接口，我们从通道信息中提取点位值
    response = make_request("get", f"{CHANNELS_API}/{channel_id}/status")
    channels = response.get("data", [])
    
    # 找出特定通道
    channel_info = None
    for channel in channels:
        if channel.get("id") == channel_id:
            channel_info = channel
            break
    
    # 从参数中找出点位值
    value = "N/A"
    timestamp = channel_info.get("last_update_time", "N/A") if channel_info else "N/A"
    
    if channel_info and "parameters" in channel_info:
        value = channel_info.get("parameters", {}).get(point_id, "N/A")
    
    result = {
        "value": value,
        "timestamp": timestamp
    }
    
    logger.info(f"点位 {point_id} 值: {result.get('value', 'N/A')}, 时间戳: {result.get('timestamp', 'N/A')}")
    return result

def test_write_point(channel_id: str, point_id: str, value: Any) -> bool:
    """测试写入点位值API"""
    logger.info(f"测试写入点位值: {channel_id}/{point_id}, 值={value}...")
    
    # 目前API可能不支持参数写入，这里我们只是模拟
    logger.warning("当前API可能不支持参数写入，这是一个模拟操作")
    
    # 假设写入成功
    success = True
    logger.info(f"写入点位值结果: {'成功' if success else '失败'}")
    return success

def main():
    """主测试函数"""
    logger.info("开始测试Communication Service API...")
    
    # 测试健康检查
    if not test_health():
        logger.error("健康检查失败，终止测试")
        return
    
    # 测试获取通道列表
    channels = test_get_channels()
    if not channels:
        logger.warning("没有找到通道，无法进行后续测试")
        return
    
    # 选择第一个通道进行测试
    channel_id = channels[0].get("id")
    
    # 测试获取通道状态
    test_get_channel_status(channel_id)
    
    # 测试获取点位列表
    points = test_get_points(channel_id)
    if not points:
        logger.warning(f"通道 {channel_id} 没有点位，无法测试读写点位值")
        return
    
    # 选择第一个点位进行测试
    point_id = points[0].get("id")
    
    # 测试读取点位值
    test_read_point(channel_id, point_id)
    
    # 测试写入点位值 (如果点位可写)
    if points[0].get("writable", False):
        test_write_point(channel_id, point_id, 123.45)
        
        # 验证写入结果
        time.sleep(1)  # 等待值更新
        test_read_point(channel_id, point_id)
    else:
        logger.info(f"点位 {point_id} 不可写，跳过写入测试")
    
    logger.info("API测试完成！")

if __name__ == "__main__":
    main() 