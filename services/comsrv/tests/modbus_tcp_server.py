#!/usr/bin/env python3
"""
Modbus TCP服务器 - 适配pymodbus 3.x API
"""

import asyncio
import logging
import time
import math
from pymodbus.datastore import ModbusSlaveContext, ModbusServerContext
from pymodbus.datastore import ModbusSequentialDataBlock
from pymodbus.server import ModbusTcpServer, StartAsyncTcpServer

# 配置日志
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

class ModbusSimulator:
    def __init__(self, host='127.0.0.1', port=5502):
        self.host = host
        self.port = port
        self.server = None
        self.context = None
        self.start_time = time.time()
        
    def setup_datastore(self):
        """初始化Modbus数据存储"""
        # 创建数据块
        coils = ModbusSequentialDataBlock(0, [False] * 101)
        discrete_inputs = ModbusSequentialDataBlock(0, [False] * 101)
        holding_registers = ModbusSequentialDataBlock(0, [0] * 101)
        input_registers = ModbusSequentialDataBlock(0, [0] * 101)
        
        # 初始化数据
        for i in range(101):
            # 线圈：偶数为True
            coils.setValues(i, [i % 2 == 0])
            # 离散输入：能被3整除为True
            discrete_inputs.setValues(i, [i % 3 == 0])
            # 保持寄存器：值为i*10
            holding_registers.setValues(i, [i * 10])
            # 输入寄存器：值为i*5
            input_registers.setValues(i, [i * 5])
        
        # 创建从站上下文
        slave_context = ModbusSlaveContext(
            di=discrete_inputs,
            co=coils,
            hr=holding_registers,
            ir=input_registers
        )
        
        # 创建服务器上下文
        self.context = ModbusServerContext(slaves={
            1: slave_context,  # Unit ID 1
            2: slave_context,  # Unit ID 2
        }, single=False)
        
        logger.info("数据存储初始化完成")
        
    async def update_data(self):
        """定期更新数据"""
        while True:
            try:
                elapsed = time.time() - self.start_time
                
                # 更新所有从站的数据
                for unit_id in [1, 2]:
                    slave = self.context[unit_id]
                    
                    # 更新输入寄存器（模拟正弦波）
                    for addr in range(10):
                        value = int(100 * (1 + math.sin(elapsed * (0.1 + addr * 0.05))))
                        slave.setValues(4, addr, [value])
                    
                    # 更新离散输入
                    for addr in range(10):
                        state = (int(elapsed / (2 + addr)) % 2) == 0
                        slave.setValues(2, addr, [state])
                
                await asyncio.sleep(1)
                
            except Exception as e:
                logger.error(f"更新数据时出错: {e}")
                await asyncio.sleep(5)
                
    async def run_server(self):
        """运行服务器"""
        # 设置数据存储
        self.setup_datastore()
        
        logger.info(f"启动Modbus TCP服务器 - {self.host}:{self.port}")
        
        # 启动更新任务
        update_task = asyncio.create_task(self.update_data())
        
        # 启动服务器
        await StartAsyncTcpServer(
            context=self.context,
            address=(self.host, self.port)
        )

async def main():
    import argparse
    
    parser = argparse.ArgumentParser(description='Modbus TCP服务器')
    parser.add_argument('--host', default='127.0.0.1', help='监听地址')
    parser.add_argument('--port', type=int, default=5502, help='监听端口')
    
    args = parser.parse_args()
    
    print(f"""
╔══════════════════════════════════════════════════════════╗
║            Modbus TCP 服务器                             ║
╠══════════════════════════════════════════════════════════╣
║ 地址: {args.host:18s} 端口: {args.port:<5d}              ║
║ Unit IDs: 1, 2                                           ║
║                                                          ║
║ 数据分布:                                                ║
║ - 线圈: 0-100 (偶数为True)                              ║
║ - 离散输入: 0-100 (3的倍数为True)                       ║
║ - 保持寄存器: 0-100 (值=地址*10)                        ║
║ - 输入寄存器: 0-100 (值=地址*5, 前10个为动态数据)       ║
║                                                          ║
║ 按 Ctrl+C 停止服务器                                     ║
╚══════════════════════════════════════════════════════════╝
    """)
    
    simulator = ModbusSimulator(host=args.host, port=args.port)
    
    try:
        await simulator.run_server()
    except KeyboardInterrupt:
        print("\n服务器已停止")

if __name__ == '__main__':
    asyncio.run(main())