#!/usr/bin/env python3
"""
简化版Modbus TCP服务器 - 适配pymodbus 3.x
"""

import asyncio
import logging
from pymodbus import __version__ as pymodbus_version
from pymodbus.datastore import ModbusSlaveContext, ModbusServerContext
from pymodbus.datastore import ModbusSequentialDataBlock
from pymodbus.server import StartAsyncTcpServer

# 配置日志
logging.basicConfig(level=logging.INFO)
log = logging.getLogger()

def setup_data_store():
    """初始化数据存储"""
    # 创建数据块
    coils = ModbusSequentialDataBlock(0, [False] * 101)
    discrete_inputs = ModbusSequentialDataBlock(0, [False] * 101)
    holding_registers = ModbusSequentialDataBlock(0, [0] * 101)
    input_registers = ModbusSequentialDataBlock(0, [0] * 101)
    
    # 初始化一些数据
    for i in range(10):
        holding_registers.setValues(i, [i * 100])
        input_registers.setValues(i, [i * 10])
        coils.setValues(i, [i % 2 == 0])
        discrete_inputs.setValues(i, [i % 3 == 0])
    
    # 创建从站上下文
    store = ModbusSlaveContext(
        di=discrete_inputs,
        co=coils,
        hr=holding_registers,
        ir=input_registers
    )
    
    # 创建服务器上下文
    context = ModbusServerContext(slaves=store, single=True)
    
    return context

async def run_server():
    """运行服务器"""
    print(f"使用pymodbus版本: {pymodbus_version}")
    print("启动Modbus TCP服务器 - 127.0.0.1:5502")
    
    context = setup_data_store()
    
    # 启动服务器
    server = await StartAsyncTcpServer(
        context=context,
        address=("127.0.0.1", 5502)
    )
    
    print("服务器已启动，按Ctrl+C停止")

if __name__ == "__main__":
    asyncio.run(run_server())