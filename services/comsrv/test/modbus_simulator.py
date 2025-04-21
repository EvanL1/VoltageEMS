#!/usr/bin/env python3
"""
Modbus协议模拟器
模拟Modbus TCP服务器，用于测试通信服务
"""

import logging
import argparse
import random
import time
import sys
import os
from typing import Dict, List, Any, Optional, Tuple
from threading import Thread, Lock

# 配置日志
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger("ModbusSimulator")

# 默认配置
DEFAULT_HOST = "0.0.0.0"
DEFAULT_PORT = 502  # 使用标准Modbus端口
DEFAULT_SLAVE_ID = 1

# 尝试导入pymodbus库
try:
    import pymodbus
    from pymodbus.datastore import ModbusSequentialDataBlock, ModbusSlaveContext, ModbusServerContext
    from pymodbus.device import ModbusDeviceIdentification
    from pymodbus.server import StartTcpServer
    
    logger.info(f"成功导入pymodbus库，版本: {pymodbus.__version__}")
    
except ImportError as e:
    logger.error(f"导入pymodbus库失败: {e}")
    logger.error("请安装pymodbus库: pip install pymodbus")
    print("错误: 请先安装pymodbus库: pip install pymodbus")
    sys.exit(1)

# 数据更新线程类
class DataUpdater(Thread):
    def __init__(self, simulator):
        Thread.__init__(self)
        self.simulator = simulator
        self.daemon = True
        self.running = False
        
    def run(self):
        self.running = True
        while self.running:
            if not self.simulator.auto_update:
                time.sleep(1)
                continue
                
            with self.simulator.lock:
                # 随机更新一些寄存器
                for i in range(10):
                    # 更新线圈
                    addr = random.randint(0, 9999)
                    value = random.choice([True, False])
                    self.simulator.coils.setValues(addr, [value])
                    
                    # 更新输入寄存器
                    addr = random.randint(0, 9999)
                    value = random.randint(0, 65535)
                    self.simulator.input_registers.setValues(addr, [value])
                    
                    # 更新保持寄存器
                    addr = random.randint(0, 9999)
                    value = random.randint(0, 65535)
                    self.simulator.holding_registers.setValues(addr, [value])
                    
                    # 更新离散输入
                    addr = random.randint(0, 9999)
                    value = random.choice([True, False])
                    self.simulator.discrete_inputs.setValues(addr, [value])
            
            # 等待下一次更新
            time.sleep(self.simulator.update_interval)
            
    def stop(self):
        self.running = False

# 模拟器类
class ModbusSimulator:
    """Modbus模拟器类"""
    
    def __init__(self, 
                 host: str = DEFAULT_HOST, 
                 port: int = DEFAULT_PORT,
                 slave_id: int = DEFAULT_SLAVE_ID,
                 auto_update: bool = True,
                 update_interval: float = 1.0):
        """
        初始化Modbus模拟器
        
        Args:
            host: 监听主机地址
            port: 监听端口
            slave_id: 从站ID
            auto_update: 是否自动更新寄存器值
            update_interval: 自动更新间隔（秒）
        """
        self.host = host
        self.port = port
        self.slave_id = slave_id
        self.auto_update = auto_update
        self.update_interval = update_interval
        
        # 数据存储
        self.coils = ModbusSequentialDataBlock(0, [False] * 10000)
        self.discrete_inputs = ModbusSequentialDataBlock(0, [False] * 10000)
        self.holding_registers = ModbusSequentialDataBlock(0, [0] * 10000)
        self.input_registers = ModbusSequentialDataBlock(0, [0] * 10000)
        
        # 创建上下文
        self.store = ModbusSlaveContext(
            di=self.discrete_inputs,
            co=self.coils,
            hr=self.holding_registers,
            ir=self.input_registers
        )
            
        self.context = ModbusServerContext(slaves={self.slave_id: self.store}, single=False)
        
        # 线程控制
        self.data_updater = None
        self.lock = Lock()
        
    def setup_identity(self):
        """
        配置Modbus设备标识
        
        Returns:
            ModbusDeviceIdentification: 设备标识对象
        """
        identity = ModbusDeviceIdentification()
        identity.VendorName = 'VoltageEMS'
        identity.ProductCode = 'VEMS-SIM'
        identity.VendorUrl = 'https://voltage.com'
        identity.ProductName = 'Modbus模拟器'
        identity.ModelName = 'VEMS-Modbus'
        identity.MajorMinorRevision = '1.0.0'
        return identity
        
    def start(self):
        """启动Modbus服务器"""
        logger.info(f"启动Modbus模拟器: {self.host}:{self.port}, 从站ID: {self.slave_id}")
        
        # 启动数据更新线程
        self.data_updater = DataUpdater(self)
        self.data_updater.start()
        
        try:
            # 使用同步方式启动服务器
            identity = self.setup_identity()
            logger.info(f"启动Modbus服务器: {self.host}:{self.port}")
            StartTcpServer(
                context=self.context,
                identity=identity,
                address=(self.host, self.port)
            )
        except KeyboardInterrupt:
            logger.info("收到中断信号，停止服务器")
        except Exception as e:
            logger.error(f"启动服务器失败: {e}")
            if self.data_updater:
                self.data_updater.stop()
            raise
        
        # 清理
        if self.data_updater:
            self.data_updater.stop()
        logger.info("服务器已停止")
                
    def get_register_value(self, reg_type: str, address: int) -> Any:
        """
        获取特定寄存器的值
        
        Args:
            reg_type: 寄存器类型 ('co', 'di', 'hr', 'ir')
            address: 寄存器地址
            
        Returns:
            Any: 寄存器值
        """
        with self.lock:
            if reg_type == 'co':  # 线圈
                return self.coils.getValues(address, 1)[0]
            elif reg_type == 'di':  # 离散输入
                return self.discrete_inputs.getValues(address, 1)[0]
            elif reg_type == 'hr':  # 保持寄存器
                return self.holding_registers.getValues(address, 1)[0]
            elif reg_type == 'ir':  # 输入寄存器
                return self.input_registers.getValues(address, 1)[0]
            else:
                logger.error(f"未知寄存器类型: {reg_type}")
                return None

def main():
    """主函数"""
    # 解析命令行参数
    parser = argparse.ArgumentParser(description="Modbus协议模拟器")
    parser.add_argument("--host", default=DEFAULT_HOST, help=f"监听主机地址 (默认: {DEFAULT_HOST})")
    parser.add_argument("--port", type=int, default=DEFAULT_PORT, help=f"监听端口 (默认: {DEFAULT_PORT})")
    parser.add_argument("--slave-id", type=int, default=DEFAULT_SLAVE_ID, help=f"从站ID (默认: {DEFAULT_SLAVE_ID})")
    parser.add_argument("--no-auto-update", action="store_true", help="禁用自动更新寄存器值")
    parser.add_argument("--update-interval", type=float, default=1.0, help="自动更新间隔（秒）")
    
    args = parser.parse_args()
    
    # 创建并启动模拟器
    simulator = ModbusSimulator(
        host=args.host,
        port=args.port,
        slave_id=args.slave_id,
        auto_update=not args.no_auto_update,
        update_interval=args.update_interval
    )
    
    try:
        simulator.start()
    except KeyboardInterrupt:
        logger.info("收到中断信号，停止服务器")
    except Exception as e:
        logger.error(f"服务器异常: {e}")

if __name__ == "__main__":
    main() 