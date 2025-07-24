#!/usr/bin/env python3
"""
本地 Modbus TCP 模拟器
用于 comsrv 本地测试
"""

import asyncio
import logging
import struct
from pymodbus.server import StartAsyncTcpServer
from pymodbus.device import ModbusDeviceIdentification
from pymodbus.datastore import ModbusSequentialDataBlock
from pymodbus.datastore import ModbusSlaveContext, ModbusServerContext

# 配置日志
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


def create_datastore():
    """创建 Modbus 数据存储"""

    # 创建数据块
    # 线圈（0x）: 地址 0-99
    coils = ModbusSequentialDataBlock(0, [False] * 100)

    # 离散输入（1x）: 地址 10000-10099
    discrete_inputs = ModbusSequentialDataBlock(10000, [False] * 100)

    # 保持寄存器（4x）: 地址 40000-40199
    # 初始化一些测试数据
    holding_values = [0] * 200

    # 设置遥测数据（浮点数需要2个寄存器）
    # voltage_a = 220.5V at 40001-40002
    voltage_a = struct.unpack(">HH", struct.pack(">f", 220.5))
    holding_values[1] = voltage_a[0]
    holding_values[2] = voltage_a[1]

    # voltage_b = 221.3V at 40003-40004
    voltage_b = struct.unpack(">HH", struct.pack(">f", 221.3))
    holding_values[3] = voltage_b[0]
    holding_values[4] = voltage_b[1]

    # voltage_c = 219.8V at 40005-40006
    voltage_c = struct.unpack(">HH", struct.pack(">f", 219.8))
    holding_values[5] = voltage_c[0]
    holding_values[6] = voltage_c[1]

    # current_a = 45.2A at 40007-40008
    current_a = struct.unpack(">HH", struct.pack(">f", 45.2))
    holding_values[7] = current_a[0]
    holding_values[8] = current_a[1]

    # current_b = 44.8A at 40009-40010
    current_b = struct.unpack(">HH", struct.pack(">f", 44.8))
    holding_values[9] = current_b[0]
    holding_values[10] = current_b[1]

    # current_c = 45.5A at 40011-40012
    current_c = struct.unpack(">HH", struct.pack(">f", 45.5))
    holding_values[11] = current_c[0]
    holding_values[12] = current_c[1]

    # active_power = 28.5kW at 40013-40014
    active_power = struct.unpack(">HH", struct.pack(">f", 28.5))
    holding_values[13] = active_power[0]
    holding_values[14] = active_power[1]

    # reactive_power = 12.3kVar at 40015-40016
    reactive_power = struct.unpack(">HH", struct.pack(">f", 12.3))
    holding_values[15] = reactive_power[0]
    holding_values[16] = reactive_power[1]

    # power_factor = 0.92 (存储为920) at 40017
    holding_values[17] = 920

    holding_registers = ModbusSequentialDataBlock(40000, holding_values)

    # 输入寄存器（3x）: 地址 30000-30099
    input_registers = ModbusSequentialDataBlock(30000, [0] * 100)

    # 创建从站上下文
    slave_context = ModbusSlaveContext(
        di=discrete_inputs, co=coils, hr=holding_registers, ir=input_registers
    )

    # 创建服务器上下文
    server_context = ModbusServerContext(slaves=slave_context, single=True)

    return server_context


async def update_data(context):
    """定期更新数据以模拟真实设备"""
    import random

    while True:
        await asyncio.sleep(2)

        # 获取从站上下文
        slave_id = 0x00  # 单一从站模式

        # 更新电压值（轻微波动）
        for i, base_value in enumerate([220.5, 221.3, 219.8]):
            value = base_value + random.uniform(-1, 1)
            registers = struct.unpack(">HH", struct.pack(">f", value))
            context[slave_id].setValues(3, 40001 + i * 2, registers)

        # 更新电流值（轻微波动）
        for i, base_value in enumerate([45.2, 44.8, 45.5]):
            value = base_value + random.uniform(-0.5, 0.5)
            registers = struct.unpack(">HH", struct.pack(">f", value))
            context[slave_id].setValues(3, 40007 + i * 2, registers)

        # 更新功率值
        active_power = 28.5 + random.uniform(-2, 2)
        registers = struct.unpack(">HH", struct.pack(">f", active_power))
        context[slave_id].setValues(3, 40013, registers)

        reactive_power = 12.3 + random.uniform(-1, 1)
        registers = struct.unpack(">HH", struct.pack(">f", reactive_power))
        context[slave_id].setValues(3, 40015, registers)

        # 更新功率因数
        pf = int(920 + random.uniform(-10, 10))
        context[slave_id].setValues(3, 40017, [pf])

        logger.debug(
            f"Updated values - Voltage A: {value:.1f}V, Active Power: {active_power:.1f}kW"
        )


async def main():
    """主函数"""
    # 创建数据存储
    context = create_datastore()

    # 设备标识信息
    identity = ModbusDeviceIdentification()
    identity.VendorName = "VoltageEMS"
    identity.ProductCode = "COMSRV-TEST"
    identity.VendorUrl = "https://github.com/voltage-ems"
    identity.ProductName = "COMSRV Modbus Simulator"
    identity.ModelName = "Test Device"
    identity.MajorMinorRevision = "1.0.0"

    # 启动更新任务
    asyncio.create_task(update_data(context))

    # 启动服务器
    logger.info("Starting Modbus TCP server on localhost:5502")
    await StartAsyncTcpServer(
        context=context, identity=identity, address=("localhost", 5502)
    )


if __name__ == "__main__":
    logger.info("Modbus TCP Simulator for COMSRV Testing")
    logger.info("Press Ctrl+C to stop")

    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        logger.info("Simulator stopped by user")
