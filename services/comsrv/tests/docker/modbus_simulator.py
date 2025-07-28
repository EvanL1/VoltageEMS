#!/usr/bin/env python3
"""
Modbus TCP模拟器
用于测试COMSRV的Modbus TCP通信功能
"""

import os
import sys
import time
import logging
import asyncio
from pymodbus.server import StartAsyncTcpServer
from pymodbus.datastore import ModbusSlaveContext, ModbusServerContext
from pymodbus.datastore import ModbusSequentialDataBlock

# 配置日志
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[
        logging.FileHandler("/app/logs/modbus_simulator.log"),
        logging.StreamHandler(sys.stdout),
    ],
)
logger = logging.getLogger(__name__)


class ModbusSimulator:
    def __init__(self):
        self.port = int(os.getenv("MODBUS_PORT", 502))
        self.slave_id = int(os.getenv("SLAVE_ID", 1))
        self.register_count = int(os.getenv("REGISTER_COUNT", 100))
        self.running = False

    def create_datastore(self):
        """创建Modbus数据存储"""
        # 创建保持寄存器 (Holding Registers) - 功能码3,6,16
        # 注意：起始地址设为0，与CSV配置匹配
        holding_registers = ModbusSequentialDataBlock(0, [0] * self.register_count)

        # 创建输入寄存器 (Input Registers) - 功能码4
        input_registers = ModbusSequentialDataBlock(0, [0] * self.register_count)

        # 创建线圈 (Coils) - 功能码1,5,15
        coils = ModbusSequentialDataBlock(0, [False] * self.register_count)

        # 创建离散输入 (Discrete Inputs) - 功能码2
        discrete_inputs = ModbusSequentialDataBlock(0, [False] * self.register_count)

        # 创建从站上下文
        slave_context = ModbusSlaveContext(
            di=discrete_inputs,  # 离散输入
            co=coils,  # 线圈
            hr=holding_registers,  # 保持寄存器
            ir=input_registers,  # 输入寄存器
        )

        # 创建服务器上下文
        context = ModbusServerContext(
            slaves={self.slave_id: slave_context}, single=False
        )

        logger.info(
            f"创建Modbus数据存储 - 从站ID: {self.slave_id}, 寄存器数量: {self.register_count}"
        )
        return context

    def update_registers(self, context):
        """模拟数据更新"""
        try:
            slave_context = context[self.slave_id]

            # 根据CSV配置，更新对应地址的寄存器
            # 地址0-1: uint16类型的电压值 (对应point_id 10001-10002)
            voltage_a = int(220 + (time.time() % 10))  # 220-230V
            current_a = int(50 + (time.time() % 5))  # 50-55A
            slave_context.setValues(3, 0, [voltage_a])
            slave_context.setValues(3, 2, [current_a])

            # 地址4-5: float32类型的功率值 (对应point_id 10003)
            # Float32需要2个寄存器，按ABCD字节序
            power_value = 1500.5 + (time.time() % 100)  # 1500.5-1600.5 kW
            import struct

            power_bytes = struct.pack(">f", power_value)  # 大端序
            power_regs = struct.unpack(">HH", power_bytes)  # 转为2个uint16
            slave_context.setValues(3, 4, list(power_regs))

            # 地址6-7: float32类型的无功功率 (对应point_id 10004)
            reactive_power = 800.25 + (time.time() % 50)
            reactive_bytes = struct.pack(">f", reactive_power)
            reactive_regs = struct.unpack(">HH", reactive_bytes)
            slave_context.setValues(3, 6, list(reactive_regs))

            # 地址8-9: int32类型的能耗值 (对应point_id 10005)
            energy_value = int(10000 + time.time() % 1000)
            energy_bytes = struct.pack(">i", energy_value)  # 大端序有符号整数
            energy_regs = struct.unpack(">HH", energy_bytes)
            slave_context.setValues(3, 8, list(energy_regs))

            # 离散输入 (功能码2)
            # 地址0: 断路器状态 (对应point_id 20001)
            breaker_status = bool(int(time.time()) % 2)
            slave_context.setValues(2, 0, [breaker_status])

            # 地址1: 故障报警 (对应point_id 20002) - 目前没有在CSV中
            alarm_status = False
            slave_context.setValues(2, 1, [alarm_status])

            # 线圈 (功能码1)
            # 地址0: 通信状态 (对应point_id 20003)
            comm_status = True  # 始终在线
            slave_context.setValues(1, 0, [comm_status])

        except Exception as e:
            logger.error(f"更新寄存器数据失败: {e}")

    async def data_updater(self, context):
        """数据更新协程"""
        logger.info("启动数据更新协程")
        while self.running:
            try:
                self.update_registers(context)
                await asyncio.sleep(1)  # 每秒更新一次数据
            except Exception as e:
                logger.error(f"数据更新协程错误: {e}")
                await asyncio.sleep(5)

    async def start_server(self):
        """启动Modbus TCP服务器"""
        logger.info("启动Modbus TCP模拟器...")
        logger.info(f"端口: {self.port}")
        logger.info(f"从站ID: {self.slave_id}")
        logger.info(f"寄存器数量: {self.register_count}")

        # 创建数据存储
        context = self.create_datastore()

        # 设置运行标志
        self.running = True

        # 启动数据更新任务
        update_task = asyncio.create_task(self.data_updater(context))

        try:
            # 启动Modbus TCP服务器
            await StartAsyncTcpServer(
                context=context,
                address=("0.0.0.0", self.port),
            )
        except Exception as e:
            logger.error(f"启动Modbus服务器失败: {e}")
            self.running = False
            update_task.cancel()
            raise
        finally:
            self.running = False
            if not update_task.done():
                update_task.cancel()


async def main():
    """主函数"""
    simulator = ModbusSimulator()

    try:
        logger.info("=== Modbus TCP模拟器启动 ===")
        await simulator.start_server()
    except KeyboardInterrupt:
        logger.info("接收到停止信号，正在关闭模拟器...")
    except Exception as e:
        logger.error(f"模拟器运行错误: {e}")
        sys.exit(1)
    finally:
        logger.info("Modbus TCP模拟器已停止")


if __name__ == "__main__":
    # 创建日志目录
    os.makedirs("/app/logs", exist_ok=True)

    # 运行模拟器
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
