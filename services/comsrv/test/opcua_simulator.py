#!/usr/bin/env python3
"""
OPC UA协议模拟器
模拟OPC UA服务器，用于测试通信服务
"""

import logging
import argparse
import random
import time
import sys
import os
from datetime import datetime
from threading import Thread
from typing import Dict, List, Any, Optional, Tuple

# 尝试导入opcua库
try:
    from opcua import Server, ua
    from opcua.common.node import Node
    OPCUA_INSTALLED = True
except ImportError:
    OPCUA_INSTALLED = False
    print("警告: 未安装opcua库，请使用 'pip install opcua' 安装")

# 配置日志
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger("OpcUASimulator")

# 默认配置
DEFAULT_HOST = "0.0.0.0"
DEFAULT_PORT = 4840
DEFAULT_NAMESPACE = "http://voltage.com/opcua/simulator"

class OpcUaSimulator:
    """OPC UA模拟器类"""
    
    def __init__(self, 
                 host: str = DEFAULT_HOST, 
                 port: int = DEFAULT_PORT,
                 namespace: str = DEFAULT_NAMESPACE,
                 auto_update: bool = True,
                 update_interval: float = 1.0):
        """
        初始化OPC UA模拟器
        
        Args:
            host: 监听主机地址
            port: 监听端口
            namespace: 命名空间URI
            auto_update: 是否自动更新节点值
            update_interval: 自动更新间隔（秒）
        """
        if not OPCUA_INSTALLED:
            raise ImportError("请安装opcua库: pip install opcua")
            
        self.host = host
        self.port = port
        self.namespace = namespace
        self.auto_update = auto_update
        self.update_interval = update_interval
        
        # 创建服务器
        self.server = Server()
        self.server.set_endpoint(f"opc.tcp://{host}:{port}")
        
        # 设置服务器信息
        self.server.set_server_name("VoltageEMS OPC UA模拟器")
        
        # 注册命名空间
        self.idx = self.server.register_namespace(namespace)
        
        # 获取对象节点
        self.objects = self.server.get_objects_node()
        
        # 节点存储
        self.nodes = {}
        self.update_thread = None
        self.running = False
        
    def setup_nodes(self):
        """设置模拟节点"""
        logger.info("创建模拟节点...")
        
        # 创建设备文件夹
        device_folder = self.objects.add_folder(self.idx, "Devices")
        
        # 创建多个设备
        self._create_device_nodes(device_folder, "PLC1", 20)
        self._create_device_nodes(device_folder, "PLC2", 15)
        self._create_device_nodes(device_folder, "PLC3", 10)
        
        logger.info(f"已创建 {len(self.nodes)} 个节点")
        
    def _create_device_nodes(self, parent: Node, device_name: str, point_count: int):
        """
        为设备创建节点
        
        Args:
            parent: 父节点
            device_name: 设备名称
            point_count: 点位数量
        """
        # 创建设备文件夹
        device = parent.add_folder(self.idx, device_name)
        
        # 创建各种数据类型的节点
        for i in range(point_count):
            # 布尔型变量
            node_name = f"Bool_{i+1}"
            node = device.add_variable(self.idx, node_name, False)
            node.set_writable()
            self.nodes[f"{device_name}.{node_name}"] = node
            
            # 整型变量
            node_name = f"Int_{i+1}"
            node = device.add_variable(self.idx, node_name, 0)
            node.set_writable()
            self.nodes[f"{device_name}.{node_name}"] = node
            
            # 浮点型变量
            node_name = f"Float_{i+1}"
            node = device.add_variable(self.idx, node_name, 0.0)
            node.set_writable()
            self.nodes[f"{device_name}.{node_name}"] = node
            
            # 字符串变量
            node_name = f"String_{i+1}"
            node = device.add_variable(self.idx, node_name, "")
            node.set_writable()
            self.nodes[f"{device_name}.{node_name}"] = node
            
    def update_nodes(self):
        """自动更新节点值"""
        while self.running:
            if not self.auto_update:
                time.sleep(1)
                continue
                
            logger.debug("正在更新节点值...")
            
            # 为每个节点生成新的随机值
            for node_id, node in self.nodes.items():
                try:
                    current_val = node.get_value()
                    data_type = type(current_val)
                    
                    # 根据数据类型生成随机值
                    if isinstance(current_val, bool):
                        new_val = random.choice([True, False])
                    elif isinstance(current_val, int):
                        new_val = random.randint(0, 1000)
                    elif isinstance(current_val, float):
                        new_val = random.uniform(0, 100.0)
                    elif isinstance(current_val, str):
                        new_val = f"String-{random.randint(0, 1000)}"
                    else:
                        # 对于其他类型，不更新
                        continue
                        
                    # 设置新值
                    node.set_value(new_val)
                    
                except Exception as e:
                    logger.error(f"更新节点 {node_id} 失败: {str(e)}")
            
            # 等待下一次更新
            time.sleep(self.update_interval)
    
    def start(self):
        """启动OPC UA服务器"""
        if self.running:
            logger.warning("服务器已经在运行中")
            return
            
        logger.info(f"启动OPC UA模拟器: {self.host}:{self.port}")
        
        # 创建节点
        self.setup_nodes()
        
        # 启动服务器
        self.server.start()
        self.running = True
        
        # 启动更新线程
        if self.auto_update:
            self.update_thread = Thread(target=self.update_nodes)
            self.update_thread.daemon = True
            self.update_thread.start()
            
        logger.info("OPC UA模拟器已启动，按Ctrl+C停止")
        
        try:
            # 保持主线程运行
            while self.running:
                time.sleep(1)
        except KeyboardInterrupt:
            self.stop()
        
    def stop(self):
        """停止OPC UA服务器"""
        if not self.running:
            return
            
        logger.info("正在停止OPC UA模拟器...")
        self.running = False
        
        if self.update_thread and self.update_thread.is_alive():
            self.update_thread.join(timeout=2.0)
            
        try:
            self.server.stop()
        except Exception as e:
            logger.error(f"停止服务器时出错: {e}")
            
        logger.info("OPC UA模拟器已停止")
        
    def set_node_value(self, node_id: str, value: Any):
        """
        设置节点值
        
        Args:
            node_id: 节点ID
            value: 新值
        """
        if node_id in self.nodes:
            try:
                self.nodes[node_id].set_value(value)
                logger.info(f"已设置节点 {node_id} 的值为 {value}")
                return True
            except Exception as e:
                logger.error(f"设置节点 {node_id} 值失败: {e}")
                return False
        else:
            logger.error(f"节点 {node_id} 不存在")
            return False
            
    def get_node_value(self, node_id: str) -> Any:
        """
        获取节点值
        
        Args:
            node_id: 节点ID
            
        Returns:
            Any: 节点值
        """
        if node_id in self.nodes:
            try:
                value = self.nodes[node_id].get_value()
                logger.info(f"获取节点 {node_id} 的值: {value}")
                return value
            except Exception as e:
                logger.error(f"获取节点 {node_id} 值失败: {e}")
                return None
        else:
            logger.error(f"节点 {node_id} 不存在")
            return None

def main():
    """主函数"""
    if not OPCUA_INSTALLED:
        print("错误: 请先安装opcua库: pip install opcua")
        return
        
    # 解析命令行参数
    parser = argparse.ArgumentParser(description="OPC UA协议模拟器")
    parser.add_argument("--host", default=DEFAULT_HOST, help=f"监听主机地址 (默认: {DEFAULT_HOST})")
    parser.add_argument("--port", type=int, default=DEFAULT_PORT, help=f"监听端口 (默认: {DEFAULT_PORT})")
    parser.add_argument("--namespace", default=DEFAULT_NAMESPACE, help=f"命名空间URI (默认: {DEFAULT_NAMESPACE})")
    parser.add_argument("--no-auto-update", action="store_true", help="禁用自动更新节点值")
    parser.add_argument("--update-interval", type=float, default=1.0, help="自动更新间隔（秒）")
    
    args = parser.parse_args()
    
    # 创建并启动模拟器
    simulator = OpcUaSimulator(
        host=args.host,
        port=args.port,
        namespace=args.namespace,
        auto_update=not args.no_auto_update,
        update_interval=args.update_interval
    )
    
    try:
        simulator.start()
    except KeyboardInterrupt:
        logger.info("收到中断信号，停止服务器")
        simulator.stop()
    except Exception as e:
        logger.error(f"服务器异常: {e}")
        simulator.stop()

if __name__ == "__main__":
    main() 