#!/usr/bin/env python3
import asyncio
from pymodbus.server.async_io import StartTcpServer
from pymodbus.datastore import ModbusSlaveContext, ModbusServerContext, ModbusSequentialDataBlock

async def run_server():
    # 创建数据存储
    store = ModbusSlaveContext(
        di=ModbusSequentialDataBlock(0, [17]*100),  # 离散输入
        co=ModbusSequentialDataBlock(0, [17]*100),  # 线圈
        hr=ModbusSequentialDataBlock(0, [17]*100),  # 保持寄存器
        ir=ModbusSequentialDataBlock(0, [17]*100)   # 输入寄存器
    )
    context = ModbusServerContext(slaves=store, single=True)
    
    print('Starting simple Modbus server on port 5502...')
    await StartTcpServer(context=context, address=('127.0.0.1', 5502))

if __name__ == "__main__":
    asyncio.run(run_server()) 