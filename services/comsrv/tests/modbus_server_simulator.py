#!/usr/bin/env python3
"""
Modbus TCP服务器模拟器 - 用于测试comsrv的Modbus客户端功能

功能特性：
- 支持所有4种数据类型（线圈、离散输入、保持寄存器、输入寄存器）
- 支持Modbus功能码：01/02/03/04/05/06/15/16
- 实时数据更新（正弦波模拟）
- 与comsrv配置匹配的地址映射
- 调试信息输出
"""

import asyncio
import time
import math
import logging
from pymodbus.server import StartAsyncTcpServer
from pymodbus.device import ModbusDeviceIdentification
from pymodbus.datastore import ModbusSequentialDataBlock, ModbusSlaveContext, ModbusServerContext

# 配置日志
logging.basicConfig(
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    level=logging.INFO
)
logger = logging.getLogger(__name__)

class ModbusSimulator:
    def __init__(self, host='127.0.0.1', port=5502):
        self.host = host
        self.port = port
        self.server = None
        self.context = None
        self.update_task = None
        self.start_time = time.time()
        
    def setup_datastore(self):
        """初始化Modbus数据存储"""
        # 创建数据块
        # 线圈（Coils）: 地址0-100
        coils = ModbusSequentialDataBlock(0, [False] * 101)
        # 初始化：偶数地址为True，奇数为False
        for i in range(101):
            coils.setValues(i, [i % 2 == 0])
            
        # 离散输入（Discrete Inputs）: 地址0-100 (映射到10001-10100)
        discrete_inputs = ModbusSequentialDataBlock(0, [False] * 101)
        # 初始化：能被3整除的为True
        for i in range(101):
            discrete_inputs.setValues(i, [i % 3 == 0])
            
        # 保持寄存器（Holding Registers）: 地址0-100 (映射到40001-40100)
        holding_registers = ModbusSequentialDataBlock(0, [0] * 101)
        # 初始化：值为地址*10
        for i in range(101):
            holding_registers.setValues(i, [i * 10])
            
        # 输入寄存器（Input Registers）: 地址0-100 (映射到30001-30100)
        input_registers = ModbusSequentialDataBlock(0, [0] * 101)
        # 初始化：值为地址*5
        for i in range(101):
            input_registers.setValues(i, [i * 5])
            
        # 创建从站上下文
        slave_context = ModbusSlaveContext(
            di=discrete_inputs,
            co=coils,
            hr=holding_registers,
            ir=input_registers
        )
        
        # 创建服务器上下文，支持多个从站
        self.context = ModbusServerContext(slaves={
            1: slave_context,  # Unit ID 1
            2: slave_context,  # Unit ID 2（备用）
        }, single=False)
        
        logger.info("数据存储初始化完成")
        
    async def update_data(self):
        """定期更新数据以模拟真实设备"""
        while True:
            try:
                elapsed = time.time() - self.start_time
                
                # 更新输入寄存器（模拟传感器数据）
                for unit_id in [1, 2]:
                    slave = self.context[unit_id]
                    
                    # 更新输入寄存器：正弦波数据
                    for addr in range(10):
                        # 不同频率的正弦波
                        value = int(100 * (1 + math.sin(elapsed * (0.1 + addr * 0.05))))
                        slave.setValues(4, addr, [value])  # 功能码4对应输入寄存器
                    
                    # 更新一些离散输入：模拟开关状态
                    for addr in range(10):
                        # 周期性切换
                        state = (int(elapsed / (2 + addr)) % 2) == 0
                        slave.setValues(2, addr, [state])  # 功能码2对应离散输入
                    
                    # 记录部分数据用于调试
                    if int(elapsed) % 10 == 0 and addr == 0:
                        ir_values = slave.getValues(4, 0, 5)
                        di_values = slave.getValues(2, 0, 5)
                        logger.debug(f"Unit {unit_id} - 输入寄存器[0-4]: {ir_values}")
                        logger.debug(f"Unit {unit_id} - 离散输入[0-4]: {di_values}")
                
                await asyncio.sleep(1)  # 每秒更新一次
                
            except Exception as e:
                logger.error(f"更新数据时出错: {e}")
                await asyncio.sleep(5)
                
    def handle_request(self, unit_id, function_code, address, count=1, values=None):
        """处理Modbus请求的回调（用于调试）"""
        fc_names = {
            1: "读线圈", 2: "读离散输入", 3: "读保持寄存器", 4: "读输入寄存器",
            5: "写单个线圈", 6: "写单个寄存器", 15: "写多个线圈", 16: "写多个寄存器"
        }
        
        fc_name = fc_names.get(function_code, f"未知功能码({function_code})")
        
        if values is None:
            logger.info(f"请求: Unit={unit_id}, 功能={fc_name}, 地址={address}, 数量={count}")
        else:
            logger.info(f"请求: Unit={unit_id}, 功能={fc_name}, 地址={address}, 值={values}")
            
    async def run_server(self):
        """运行Modbus服务器"""
        # 设置设备标识（可选）
        identity = ModbusDeviceIdentification()
        identity.VendorName = 'VoltageEMS'
        identity.ProductCode = 'VS'
        identity.VendorUrl = 'http://github.com/pymodbus'
        identity.ProductName = 'Modbus Server Simulator'
        identity.ModelName = 'Modbus Simulator'
        identity.MajorMinorRevision = '1.0.0'
        
        # 启动服务器
        logger.info(f"启动Modbus TCP服务器 - {self.host}:{self.port}")
        
        await StartAsyncTcpServer(
            context=self.context,
            identity=identity,
            address=(self.host, self.port)
        )
        
    async def start(self):
        """启动模拟器"""
        # 设置数据存储
        self.setup_datastore()
        
        # 创建并发任务
        server_task = asyncio.create_task(self.run_server())
        self.update_task = asyncio.create_task(self.update_data())
        
        # 等待任务完成
        await asyncio.gather(server_task, self.update_task)

def main():
    """主函数"""
    import argparse
    
    parser = argparse.ArgumentParser(description='Modbus TCP服务器模拟器')
    parser.add_argument('--host', default='127.0.0.1', help='监听地址（默认: 127.0.0.1）')
    parser.add_argument('--port', type=int, default=5502, help='监听端口（默认: 5502）')
    parser.add_argument('--debug', action='store_true', help='启用调试日志')
    
    args = parser.parse_args()
    
    # 配置日志级别
    if args.debug:
        logging.getLogger().setLevel(logging.DEBUG)
        logging.getLogger('pymodbus').setLevel(logging.DEBUG)
    else:
        logging.getLogger('pymodbus').setLevel(logging.WARNING)
    
    # 创建并运行模拟器
    simulator = ModbusSimulator(host=args.host, port=args.port)
    
    print(f"""
╔══════════════════════════════════════════════════════════╗
║            Modbus TCP 服务器模拟器                       ║
╠══════════════════════════════════════════════════════════╣
║ 地址: {args.host:18s} 端口: {args.port:<5d}              ║
║ Unit IDs: 1, 2                                           ║
║                                                          ║
║ 数据类型:                                                ║
║ - 线圈 (FC 01/05/15): 地址 0-100                        ║
║ - 离散输入 (FC 02): 地址 0-100 (10001-10100)           ║
║ - 保持寄存器 (FC 03/06/16): 地址 0-100 (40001-40100)   ║
║ - 输入寄存器 (FC 04): 地址 0-100 (30001-30100)         ║
║                                                          ║
║ 按 Ctrl+C 停止服务器                                     ║
╚══════════════════════════════════════════════════════════╝
    """)
    
    try:
        asyncio.run(simulator.start())
    except KeyboardInterrupt:
        print("\n服务器已停止")

if __name__ == '__main__':
    main()