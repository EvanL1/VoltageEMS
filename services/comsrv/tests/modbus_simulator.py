#!/usr/bin/env python3
"""
Modbus TCP/RTU 服务器模拟器
用于comsrv集成测试
"""

import asyncio
import logging
import signal
import sys
import time
from typing import Dict, Optional

from pymodbus.datastore import (
    ModbusSequentialDataBlock,
    ModbusServerContext,
    ModbusSlaveContext,
)
from pymodbus.server.async_io import (
    ModbusTcpServer,
    ModbusSerialServer,
    StartAsyncTcpServer,
    StartAsyncSerialServer,
)
from pymodbus.device import ModbusDeviceIdentification

# 配置日志
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


class ModbusSimulator:
    """Modbus服务器模拟器"""
    
    def __init__(self, mode: str = 'tcp', address: str = '0.0.0.0', port: int = 5502):
        self.mode = mode
        self.address = address
        self.port = port
        self.server = None
        self.context = None
        self.running = False
        
        # 设置信号处理
        signal.signal(signal.SIGINT, self._signal_handler)
        signal.signal(signal.SIGTERM, self._signal_handler)
    
    def _signal_handler(self, signum, frame):
        """处理停止信号"""
        logger.info(f"收到信号 {signum}，正在停止服务器...")
        self.stop()
        sys.exit(0)
    
    def create_datastore(self, slave_id: int = 1) -> ModbusSlaveContext:
        """创建数据存储"""
        # 创建数据块
        # 线圈状态 (0x01, 0x05, 0x0F) - 10000个
        coils = ModbusSequentialDataBlock(1, [False] * 10000)
        
        # 离散输入 (0x02) - 10000个
        discrete_inputs = ModbusSequentialDataBlock(1, [False] * 10000)
        
        # 保持寄存器 (0x03, 0x06, 0x10) - 10000个
        # 初始化一些测试数据
        holding_values = [0] * 10000
        
        # 设置一些测试值
        # 电压值 (寄存器 1-3)
        holding_values[0] = 2200  # 220.0V
        holding_values[1] = 2210  # 221.0V
        holding_values[2] = 2190  # 219.0V
        
        # 电流值 (寄存器 4-6)
        holding_values[3] = 150   # 15.0A
        holding_values[4] = 155   # 15.5A
        holding_values[5] = 145   # 14.5A
        
        # 功率值 (寄存器 7-9)
        holding_values[6] = 3300  # 3300W
        holding_values[7] = 3410  # 3410W
        holding_values[8] = 3180  # 3180W
        
        # 频率 (寄存器 10)
        holding_values[9] = 5000  # 50.00Hz
        
        # 温度 (寄存器 11-13)
        holding_values[10] = 250  # 25.0°C
        holding_values[11] = 255  # 25.5°C
        holding_values[12] = 245  # 24.5°C
        
        holding_registers = ModbusSequentialDataBlock(1, holding_values)
        
        # 输入寄存器 (0x04) - 10000个
        input_values = [0] * 10000
        
        # 设置一些只读测试值
        input_values[0] = 100   # 设备状态
        input_values[1] = 200   # 固件版本
        input_values[2] = 2024  # 年份
        
        input_registers = ModbusSequentialDataBlock(1, input_values)
        
        # 创建从站上下文
        slave = ModbusSlaveContext(
            di=discrete_inputs,
            co=coils,
            hr=holding_registers,
            ir=input_registers
        )
        
        logger.info(f"创建从站 {slave_id} 的数据存储")
        return slave
    
    def setup_server_context(self) -> ModbusServerContext:
        """设置服务器上下文"""
        # 创建多个从站
        slaves = {}
        
        # 从站1 - 主要测试设备
        slaves[1] = self.create_datastore(1)
        
        # 从站2 - 备用测试设备
        slaves[2] = self.create_datastore(2)
        
        # 创建服务器上下文
        context = ModbusServerContext(slaves=slaves, single=False)
        
        logger.info("服务器上下文已创建")
        return context
    
    def create_identity(self) -> ModbusDeviceIdentification:
        """创建设备标识"""
        identity = ModbusDeviceIdentification()
        identity.VendorName = 'VoltageEMS'
        identity.ProductCode = 'VEMS-SIM'
        identity.VendorUrl = 'http://github.com/voltageems'
        identity.ProductName = 'VoltageEMS Modbus Simulator'
        identity.ModelName = 'Modbus Simulator'
        identity.MajorMinorRevision = '1.0.0'
        return identity
    
    async def update_values(self):
        """定期更新数值，模拟真实设备"""
        logger.info("启动数值更新任务")
        
        while self.running:
            try:
                # 更新从站1的数据
                slave1 = self.context[1]
                
                # 模拟电压波动 (±5V)
                for i in range(3):
                    current = slave1.getValues(3, i+1, 1)[0]
                    variation = int((time.time() * 10) % 10) - 5
                    new_value = 2200 + variation
                    slave1.setValues(3, i+1, [new_value])
                
                # 模拟电流波动 (±0.5A)
                for i in range(3):
                    variation = int((time.time() * 5) % 10) - 5
                    new_value = 150 + variation
                    slave1.setValues(3, i+4, [new_value])
                
                # 更新功率（基于电压和电流）
                for i in range(3):
                    voltage = slave1.getValues(3, i+1, 1)[0] / 10
                    current = slave1.getValues(3, i+4, 1)[0] / 10
                    power = int(voltage * current)
                    slave1.setValues(3, i+7, [power])
                
                # 模拟温度缓慢变化
                temp_base = 250 + int((time.time() / 60) % 10)
                for i in range(3):
                    slave1.setValues(3, i+11, [temp_base + i])
                
                # 更新一些线圈状态
                coil_states = []
                for i in range(10):
                    # 随机切换状态
                    state = (int(time.time()) + i) % 2 == 0
                    coil_states.append(state)
                slave1.setValues(1, 1, coil_states)
                
                logger.debug("数值已更新")
                
            except Exception as e:
                logger.error(f"更新数值时出错: {e}")
            
            # 每秒更新一次
            await asyncio.sleep(1)
    
    async def run_tcp_server(self):
        """运行TCP服务器"""
        logger.info(f"启动Modbus TCP服务器 {self.address}:{self.port}")
        
        self.context = self.setup_server_context()
        identity = self.create_identity()
        
        # 创建更新任务
        self.running = True
        update_task = asyncio.create_task(self.update_values())
        
        try:
            # 启动服务器
            await StartAsyncTcpServer(
                context=self.context,
                identity=identity,
                address=(self.address, self.port)
            )
        finally:
            self.running = False
            update_task.cancel()
            try:
                await update_task
            except asyncio.CancelledError:
                pass
    
    async def run_rtu_server(self, port: str, baudrate: int = 9600):
        """运行RTU服务器"""
        logger.info(f"启动Modbus RTU服务器 {port} @ {baudrate}bps")
        
        self.context = self.setup_server_context()
        identity = self.create_identity()
        
        # 创建更新任务
        self.running = True
        update_task = asyncio.create_task(self.update_values())
        
        try:
            # 启动服务器
            await StartAsyncSerialServer(
                context=self.context,
                identity=identity,
                port=port,
                baudrate=baudrate,
                bytesize=8,
                parity='N',
                stopbits=1,
                timeout=1
            )
        finally:
            self.running = False
            update_task.cancel()
            try:
                await update_task
            except asyncio.CancelledError:
                pass
    
    def start(self):
        """启动服务器"""
        if self.mode == 'tcp':
            asyncio.run(self.run_tcp_server())
        else:
            # RTU模式需要指定串口
            port = '/dev/ttyUSB0'  # 默认串口
            asyncio.run(self.run_rtu_server(port))
    
    def stop(self):
        """停止服务器"""
        self.running = False
        logger.info("服务器已停止")


def main():
    """主函数"""
    import argparse
    
    parser = argparse.ArgumentParser(description='Modbus服务器模拟器')
    parser.add_argument(
        '--mode', 
        choices=['tcp', 'rtu'], 
        default='tcp',
        help='服务器模式 (默认: tcp)'
    )
    parser.add_argument(
        '--address', 
        default='0.0.0.0',
        help='监听地址 (默认: 0.0.0.0)'
    )
    parser.add_argument(
        '--port', 
        type=int, 
        default=5502,
        help='监听端口 (默认: 5502)'
    )
    parser.add_argument(
        '--log-level',
        choices=['DEBUG', 'INFO', 'WARNING', 'ERROR'],
        default='INFO',
        help='日志级别 (默认: INFO)'
    )
    
    args = parser.parse_args()
    
    # 设置日志级别
    logging.getLogger().setLevel(getattr(logging, args.log_level))
    
    # 创建并启动模拟器
    simulator = ModbusSimulator(
        mode=args.mode,
        address=args.address,
        port=args.port
    )
    
    logger.info("=================================")
    logger.info("VoltageEMS Modbus 服务器模拟器")
    logger.info("=================================")
    logger.info(f"模式: {args.mode.upper()}")
    logger.info(f"地址: {args.address}:{args.port}")
    logger.info("按 Ctrl+C 停止服务器")
    
    try:
        simulator.start()
    except KeyboardInterrupt:
        logger.info("收到中断信号")
    except Exception as e:
        logger.error(f"服务器错误: {e}")
        raise
    finally:
        simulator.stop()


if __name__ == '__main__':
    main()