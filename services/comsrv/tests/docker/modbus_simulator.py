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
        holding_registers = ModbusSequentialDataBlock(40001, [0] * self.register_count)

        # 创建输入寄存器 (Input Registers) - 功能码4
        input_registers = ModbusSequentialDataBlock(30001, [0] * self.register_count)

        # 创建线圈 (Coils) - 功能码1,5,15
        coils = ModbusSequentialDataBlock(1, [False] * self.register_count)

        # 创建离散输入 (Discrete Inputs) - 功能码2
        discrete_inputs = ModbusSequentialDataBlock(
            10001, [False] * self.register_count
        )

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

            # 模拟温度数据 (40001-40010)
            for i in range(10):
                # 温度范围: 20-30度，带小数
                temp_value = int(
                    (20 + (i * 0.5) + (time.time() % 10)) * 100
                )  # 扩大100倍存储
                slave_context.setValues(3, 40001 + i, [temp_value])

            # 模拟电压数据 (40011-40020)
            for i in range(10):
                # 电压范围: 220-240V
                voltage_value = int(220 + (i * 2) + (time.time() % 5))
                slave_context.setValues(3, 40011 + i, [voltage_value])

            # 模拟状态数据 (线圈1-10)
            for i in range(10):
                # 随机开关状态
                status = bool((int(time.time()) + i) % 3)
                slave_context.setValues(1, 1 + i, [status])

            # 模拟计数器 (40021-40030)
            counter_base = int(time.time()) % 65536
            for i in range(10):
                counter_value = (counter_base + i * 100) % 65536
                slave_context.setValues(3, 40021 + i, [counter_value])

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
