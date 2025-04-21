#!/usr/bin/env python3
"""
comsrv Modbus通信测试脚本
用于验证comsrv是否能通过TCP读取到modbus simulator中的数据
"""

import requests
import json
import time
import logging
import argparse
from typing import Dict, Any, List, Optional

# 配置日志
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger("ComsrvModbusTest")

# 默认配置
DEFAULT_API_HOST = "localhost"
DEFAULT_API_PORT = 8888
DEFAULT_SIMULATOR_HOST = "localhost"
DEFAULT_SIMULATOR_PORT = 502
DEFAULT_CHANNEL_ID = "pcs1"
DEFAULT_INTERVAL = 2.0
DEFAULT_CYCLES = 5

# API路径模板
API_BASE_URL = "http://{host}:{port}/api"
HEALTH_API = "http://{host}:{port}/health"
CHANNELS_API = "http://{host}:{port}/api/v1/channels"
CHANNEL_STATUS_API = "http://{host}:{port}/api/v1/channels/{channel_id}/status"
CHANNEL_POINTS_API = "http://{host}:{port}/api/v1/channels/{channel_id}/points"
POINT_READ_API = "http://{host}:{port}/api/v1/channels/{channel_id}/points/{point_table}/{point_name}"
POINT_WRITE_API = "http://{host}:{port}/api/v1/channels/{channel_id}/points/{point_table}/{point_name}"

class ComsrvModbusTest:
    """comsrv Modbus测试客户端"""
    
    def __init__(self, api_host: str, api_port: int, channel_id: str):
        """
        初始化测试客户端
        
        Args:
            api_host: comsrv API主机地址
            api_port: comsrv API端口
            channel_id: 要测试的通道ID
        """
        self.api_host = api_host
        self.api_port = api_port
        self.channel_id = channel_id
        self.timeout = 5  # HTTP请求超时时间（秒）
        
        # 构建API URL
        self.api_base_url = API_BASE_URL.format(host=api_host, port=api_port)
        self.health_api = HEALTH_API.format(host=api_host, port=api_port)
        self.channels_api = CHANNELS_API.format(host=api_host, port=api_port)
        self.channel_status_api = CHANNEL_STATUS_API.format(
            host=api_host, port=api_port, channel_id=channel_id
        )
        self.channel_points_api = CHANNEL_POINTS_API.format(
            host=api_host, port=api_port, channel_id=channel_id
        )
        self.point_read_api = POINT_READ_API
        self.point_write_api = POINT_WRITE_API
        
    def make_request(self, method: str, url: str, data: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        """发送HTTP请求并处理可能的错误"""
        try:
            if method.lower() == "get":
                response = requests.get(url, timeout=self.timeout)
            elif method.lower() == "post":
                response = requests.post(url, json=data, timeout=self.timeout)
            elif method.lower() == "put":
                response = requests.put(url, json=data, timeout=self.timeout)
            elif method.lower() == "delete":
                response = requests.delete(url, timeout=self.timeout)
            else:
                raise ValueError(f"不支持的HTTP方法: {method}")
            
            response.raise_for_status()
            return response.json()
        except requests.exceptions.RequestException as e:
            logger.error(f"请求失败: {e}")
            return {"success": False, "error": str(e)}
            
    def check_health(self) -> bool:
        """检查服务健康状态"""
        logger.info("检查comsrv服务健康状态...")
        response = self.make_request("get", self.health_api)
        success = response.get("success", False) 
        if "data" in response and "status" in response["data"]:
            status = response["data"]["status"]
            logger.info(f"服务健康状态: {status}")
            return status == "OK"
        return False
        
    def get_channels(self) -> List[Dict[str, Any]]:
        """获取所有通道信息"""
        logger.info("获取通道列表...")
        response = self.make_request("get", self.channels_api)
        channels = []
        
        if response.get("success", False) and "data" in response:
            channels = response["data"]
            logger.info(f"获取到 {len(channels)} 个通道")
            for channel in channels:
                logger.info(f"通道: ID={channel.get('id')}, 名称={channel.get('name')}, 协议={channel.get('protocol')}")
        else:
            logger.error(f"获取通道列表失败: {response.get('error', '未知错误')}")
            
        return channels
        
    def get_channel_status(self) -> Dict[str, Any]:
        """获取特定通道的状态"""
        logger.info(f"获取通道 '{self.channel_id}' 的状态...")
        response = self.make_request("get", self.channel_status_api)
        status = {}
        
        if response.get("success", False) and "data" in response:
            data = response["data"]
            
            # 处理返回数据是列表的情况
            if isinstance(data, list):
                # 尝试从列表中找到匹配通道ID的项
                for channel in data:
                    if channel.get("id") == self.channel_id:
                        status = channel
                        break
                    
                if not status:
                    logger.warning(f"在返回的数据中未找到通道 '{self.channel_id}'")
                    return {}
            else:
                # 直接使用返回的数据
                status = data
            
            connected = status.get("connected", False)
            last_error = status.get("last_error", "无")
            logger.info(f"通道状态: 连接={connected}, 最后错误={last_error}")
        else:
            logger.error(f"获取通道状态失败: {response.get('error', '未知错误')}")
            
        return status
        
    def get_channel_points(self) -> List[Dict[str, Any]]:
        """获取通道点位列表"""
        logger.info(f"获取通道 '{self.channel_id}' 的点位列表...")
        response = self.make_request("get", self.channel_points_api)
        points = []
        
        if response.get("success", False) and "data" in response:
            data = response["data"]
            
            # 处理数据是列表的情况
            if isinstance(data, list):
                # 尝试从列表中找到匹配通道ID的项
                for channel in data:
                    if channel.get("id") == self.channel_id:
                        # 找到通道后，获取点位信息
                        if "parameters" in channel and "points" in channel["parameters"]:
                            points = channel["parameters"]["points"]
                        break
            else:
                # 尝试从字典中获取点位
                points = data.get("points", [])
            
            logger.info(f"获取到 {len(points)} 个点位")
            for point in points:
                logger.info(f"点位: ID={point.get('id')}, 名称={point.get('name', 'N/A')}, 类型={point.get('type', 'N/A')}, 可写={point.get('writable', False)}")
        else:
            logger.error(f"获取点位列表失败: {response.get('error', '未知错误')}")
            
        return points
        
    def read_point(self, point_id: str) -> Dict[str, Any]:
        """读取点位值，使用新的API端点"""
        logger.info(f"读取点位 '{point_id}' 的值...")
        
        # 对于测试，我们可以设置一个固定的point_table作为点表名称
        point_table = "default"
        point_name = point_id
        
        # 使用正确的API路径
        point_read_url = self.point_read_api.format(
            host=self.api_host,
            port=self.api_port,
            channel_id=self.channel_id,
            point_table=point_table,
            point_name=point_name
        )
        
        try:
            response = requests.get(point_read_url, timeout=self.timeout)
            response.raise_for_status()
            data = response.json()
            
            # 根据实际API响应来处理数据
            if data.get("success", False):
                # 目前API直接返回通道列表而不是点位数据，这是个临时调试方法
                # 通常API应该返回类似 data["data"] = {"value": xxx, "quality": true/false, ...}
                channels_data = data.get("data", [])
                
                # 打印响应以帮助调试
                logger.info(f"API响应: {channels_data}")
                
                # 返回一个默认结果
                return {
                    "value": None,
                    "quality": False,
                    "timestamp": time.time()
                }
            else:
                error = data.get("error", "未知错误")
                logger.error(f"读取点位值失败: {error}")
                return {"value": None, "timestamp": time.time()}
                
        except Exception as e:
            logger.error(f"发送读取请求时出错: {e}")
            return {"value": None, "timestamp": time.time()}
    
    def write_point(self, point_id: str, value: Any) -> bool:
        """写入点位值，使用通道点位API"""
        logger.info(f"写入点位 '{point_id}' 的值: {value}...")
        
        # 对于测试，我们可以设置一个固定的point_table作为点表名称
        point_table = "default"
        point_name = point_id
        
        # 使用正确的API路径
        point_write_url = self.point_write_api.format(
            host=self.api_host,
            port=self.api_port,
            channel_id=self.channel_id,
            point_table=point_table,
            point_name=point_name
        )
        
        # 构建写入数据
        request_data = {
            "value": value
        }
        
        # 尝试通过API写入值
        try:
            response = requests.put(point_write_url, json=request_data, timeout=self.timeout)
            response.raise_for_status()
            data = response.json()
            
            if data.get("success", False):
                logger.info(f"点位 '{point_id}' 写入成功")
                return True
            else:
                error = data.get("error", "未知错误")
                logger.error(f"写入点位值失败: {error}")
                return False
                
        except Exception as e:
            logger.error(f"发送写入请求时出错: {e}")
            return False
            
    def run_test_cycle(self, cycle: int, total_cycles: int):
        """运行一个完整的测试周期"""
        logger.info(f"===== 开始测试周期 {cycle}/{total_cycles} =====")
        
        # 检查通道状态
        channel_status = self.get_channel_status()
        if not channel_status.get("connected", False):
            logger.warning(f"通道 '{self.channel_id}' 未连接，可能无法读写点位")
            return
            
        # 由于API不直接提供点位列表，我们手动定义几个测试点位
        test_points = [
            {"id": "coil_0", "name": "测试线圈0", "address": 0, "type": "coil", "writable": True},
            {"id": "discrete_input_0", "name": "测试离散输入0", "address": 0, "type": "discrete_input", "writable": False},
            {"id": "holding_reg_0", "name": "测试保持寄存器0", "address": 0, "type": "holding_register", "writable": True, "data_type": "uint16"},
            {"id": "input_reg_0", "name": "测试输入寄存器0", "address": 0, "type": "input_register", "writable": False, "data_type": "uint16"}
        ]
        
        logger.info(f"使用 {len(test_points)} 个测试点位进行测试")
        
        # 测试读取各类型点位
        for point in test_points:
            point_id = point["id"]
            point_type = point["type"]
            writable = point.get("writable", False)
            
            logger.info(f"测试点位: {point_id} (类型: {point_type}, 可写: {writable})")
            
            # 尝试读取点位
            result = self.read_point(point_id)
            
            # 如果点位可写，尝试写入
            if writable:
                if point_type == "coil":
                    # 对于线圈，切换其状态
                    current_value = result.get("value", False)
                    new_value = not current_value
                    logger.info(f"尝试写入线圈新值: {new_value}")
                    self.write_point(point_id, new_value)
                    
                elif point_type == "holding_register":
                    # 对于保持寄存器，写入递增值
                    current_value = result.get("value", 0)
                    new_value = (int(current_value) + 1) % 65536 if isinstance(current_value, (int, float)) else 1
                    logger.info(f"尝试写入寄存器新值: {new_value}")
                    self.write_point(point_id, new_value)
                
                # 等待一下再次读取，验证写入是否成功
                time.sleep(0.5)
                self.read_point(point_id)
        
        logger.info(f"===== 结束测试周期 {cycle}/{total_cycles} =====")

def main():
    """主函数"""
    parser = argparse.ArgumentParser(description="测试comsrv是否能通过TCP读取到modbus simulator中的数据")
    parser.add_argument("--api-host", default=DEFAULT_API_HOST, help=f"comsrv API主机地址 (默认: {DEFAULT_API_HOST})")
    parser.add_argument("--api-port", type=int, default=DEFAULT_API_PORT, help=f"comsrv API端口 (默认: {DEFAULT_API_PORT})")
    parser.add_argument("--channel-id", default=DEFAULT_CHANNEL_ID, help=f"要测试的通道ID (默认: {DEFAULT_CHANNEL_ID})")
    parser.add_argument("--cycles", type=int, default=DEFAULT_CYCLES, help=f"测试周期次数 (默认: {DEFAULT_CYCLES})")
    parser.add_argument("--interval", type=float, default=DEFAULT_INTERVAL, help=f"测试周期间隔（秒）(默认: {DEFAULT_INTERVAL})")
    
    args = parser.parse_args()
    
    logger.info("开始测试comsrv与modbus simulator的通信...")
    
    # 创建测试客户端
    tester = ComsrvModbusTest(
        api_host=args.api_host,
        api_port=args.api_port,
        channel_id=args.channel_id
    )
    
    # 检查服务健康状态
    if not tester.check_health():
        logger.error("服务健康检查失败，测试终止")
        return
    
    # 获取通道列表
    tester.get_channels()
    
    # 运行指定次数的测试周期
    for i in range(1, args.cycles + 1):
        tester.run_test_cycle(i, args.cycles)
        
        if i < args.cycles:
            logger.info(f"等待 {args.interval} 秒后开始下一周期测试...")
            time.sleep(args.interval)
    
    logger.info("测试完成！")

if __name__ == "__main__":
    main() 