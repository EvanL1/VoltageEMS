#!/usr/bin/env python3
"""
简单的Modbus TCP服务器脚本
用于测试Modbus通信
"""

import asyncio
import logging
import argparse
import random
import time
import sys
import csv
from threading import Thread

# 配置日志
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger("ModbusServer")

# 默认配置
DEFAULT_HOST = "127.0.0.1"
DEFAULT_PORT = 5020  # 使用不需要root权限的端口
DEFAULT_SLAVE_ID = 1

# 尝试导入pymodbus库
try:
    import pymodbus
    from pymodbus.datastore import ModbusSequentialDataBlock, ModbusSlaveContext, ModbusServerContext
    from pymodbus.device import ModbusDeviceIdentification
    
    # 打印版本信息
    logger.info(f"使用pymodbus库，版本: {pymodbus.__version__}")
    
    # 导入服务器模块
    try:
        # pymodbus 3.x 导入服务器模块
        from pymodbus.server import StartTcpServer, StartAsyncTcpServer, ServerStop
        logger.info("使用pymodbus.server.StartTcpServer")
    except ImportError as e:
        logger.error(f"无法导入服务器模块: {e}")
        sys.exit(1)
        
except ImportError as e:
    logger.error(f"导入pymodbus库失败: {e}")
    logger.error("请安装pymodbus库: pip install pymodbus")
    sys.exit(1)

class DataUpdater(Thread):
    """数据更新线程，用于定期更新Modbus寄存器的值"""
    
    def __init__(self, slave_context, update_interval=1.0):
        Thread.__init__(self)
        self.slave_context = slave_context
        self.update_interval = update_interval
        self.daemon = True
        self.running = False
        
    def run(self):
        """运行数据更新线程"""
        while True:
            try:
                # 更新离散输入 (10001-10010)
                self.slave_context.setValues(2, 0, [random.choice([True, False]) for _ in range(10)])
                
                # 更新保持寄存器 (30001-30029)
                # 电压值 (220V ± 10%)
                self.slave_context.setValues(3, 0, [int(220 * (1 + random.uniform(-0.1, 0.1)) * 100) for _ in range(3)])
                # 电流值 (10A ± 5%)
                self.slave_context.setValues(3, 3, [int(10 * (1 + random.uniform(-0.05, 0.05)) * 100) for _ in range(3)])
                # 功率值 (2.2kW ± 5%)
                self.slave_context.setValues(3, 6, [int(2.2 * (1 + random.uniform(-0.05, 0.05)) * 100) for _ in range(2)])
                # 功率因数 (0.95 ± 0.02)
                self.slave_context.setValues(3, 8, [int(0.95 * (1 + random.uniform(-0.02, 0.02)) * 100)])
                # 频率 (50Hz ± 0.1Hz)
                self.slave_context.setValues(3, 9, [int(50 * (1 + random.uniform(-0.001, 0.001)) * 100)])
                # 直流电压 (400V ± 5%)
                self.slave_context.setValues(3, 10, [int(400 * (1 + random.uniform(-0.05, 0.05)) * 100)])
                # 直流电流 (10A ± 5%)
                self.slave_context.setValues(3, 11, [int(10 * (1 + random.uniform(-0.05, 0.05)) * 100)])
                # 直流功率 (4kW ± 5%)
                self.slave_context.setValues(3, 12, [int(4 * (1 + random.uniform(-0.05, 0.05)) * 100)])
                # 温度 (25°C ± 5°C)
                self.slave_context.setValues(3, 13, [int(25 * (1 + random.uniform(-0.2, 0.2)) * 10)])
                # 效率 (95% ± 2%)
                self.slave_context.setValues(3, 14, [int(95 * (1 + random.uniform(-0.02, 0.02)) * 100)])
                
                time.sleep(self.update_interval)
            except Exception as e:
                logging.error(f"更新寄存器值时出错: {e}")
                time.sleep(1)
            
    def stop(self):
        """停止线程"""
        self.running = False
        logger.info("数据更新线程已停止")

def setup_server_context(slave_id=1):
    """设置服务器上下文"""
    # 创建数据块
    coils = ModbusSequentialDataBlock(0, [False] * 10000)
    discrete_inputs = ModbusSequentialDataBlock(0, [False] * 10000)
    holding_registers = ModbusSequentialDataBlock(0, [0] * 10000)
    input_registers = ModbusSequentialDataBlock(0, [0] * 10000)
    
    # 创建从站上下文
    slave_context = ModbusSlaveContext(
        di=discrete_inputs,
        co=coils,
        hr=holding_registers,
        ir=input_registers
    )
    
    # 创建服务器上下文 (单例模式)
    context = ModbusServerContext(slaves=slave_context, single=True)
    
    return context, slave_context

def setup_device_identification():
    """设置设备标识"""
    identity = ModbusDeviceIdentification()
    identity.VendorName = 'VoltageEMS'
    identity.ProductCode = 'VEMS-SIM'
    identity.VendorUrl = 'https://voltage.com'
    identity.ProductName = 'PCS模拟器'
    identity.ModelName = 'VEMS-PCS-Sim'
    identity.MajorMinorRevision = '1.0.0'
    return identity

async def run_server(host, port, slave_id=1, update_interval=1.0):
    """运行Modbus TCP服务器"""
    # 设置服务器上下文
    context, slave_context = setup_server_context(slave_id)
    
    # 设置设备标识
    identity = setup_device_identification()
    
    # 启动数据更新线程
    updater = DataUpdater(slave_context, update_interval)
    updater.start()
    
    try:
        logger.info(f"启动Modbus TCP服务器: {host}:{port}, 从站ID: {slave_id}")
        await StartAsyncTcpServer(
            context=context,
            identity=identity,
            address=(host, port)
        )
    except KeyboardInterrupt:
        logger.info("收到中断信号，停止服务器")
    except Exception as e:
        logger.error(f"服务器异常: {e}")
    finally:
        # 停止数据更新线程
        updater.stop()
        logger.info("服务器已停止")

def main():
    """主函数"""
    # 解析命令行参数
    parser = argparse.ArgumentParser(description="PCS Modbus TCP模拟器")
    parser.add_argument("--host", default=DEFAULT_HOST, help=f"监听地址 (默认: {DEFAULT_HOST})")
    parser.add_argument("--port", type=int, default=DEFAULT_PORT, help=f"监听端口 (默认: {DEFAULT_PORT})")
    parser.add_argument("--slave-id", type=int, default=DEFAULT_SLAVE_ID, help=f"从站ID (默认: {DEFAULT_SLAVE_ID})")
    parser.add_argument("--update-interval", type=float, default=1.0, help="寄存器更新间隔(秒)")
    
    args = parser.parse_args()
    
    # 运行服务器
    try:
        # 使用asyncio.run运行异步服务器
        asyncio.run(run_server(
            host=args.host,
            port=args.port,
            slave_id=args.slave_id,
            update_interval=args.update_interval
        ))
    except KeyboardInterrupt:
        logger.info("收到中断信号，停止服务器")
    except Exception as e:
        logger.error(f"服务器异常: {e}")

if __name__ == "__main__":
    main() 