#!/usr/bin/env python3
"""
Modbus客户端测试脚本
用于测试与Modbus模拟器的连接
"""

import logging
import argparse
import time
import sys

# 配置日志
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger("ModbusClient")

# 尝试导入pymodbus库
try:
    import pymodbus
    from pymodbus.client import ModbusTcpClient
    logger.info(f"成功导入pymodbus库，版本: {pymodbus.__version__}")
except ImportError as e:
    logger.error(f"导入pymodbus库失败: {e}")
    logger.error("请安装pymodbus库: pip install pymodbus")
    print("错误: 请先安装pymodbus库: pip install pymodbus")
    sys.exit(1)

# 默认配置
DEFAULT_HOST = "127.0.0.1"
DEFAULT_PORT = 5020  # 使用新的端口
DEFAULT_CYCLES = 3

class ModbusTestClient:
    """Modbus测试客户端类"""
    
    def __init__(self, host="127.0.0.1", port=502, unit=1):
        """
        初始化Modbus测试客户端
        
        Args:
            host: 服务器主机地址
            port: 服务器端口
            unit: 单元ID (从站ID)
        """
        self.host = host
        self.port = port
        self.unit = unit
        self.client = None
        
    def connect(self):
        """连接到Modbus服务器"""
        try:
            logger.info(f"正在连接到Modbus服务器 {self.host}:{self.port}...")
            # pymodbus 3.8.6 版本的初始化参数变化
            self.client = ModbusTcpClient(
                host=self.host, 
                port=self.port
            )
            connected = self.client.connect()
            
            if connected:
                logger.info("成功连接到Modbus服务器")
                return True
            else:
                logger.error("连接到Modbus服务器失败")
                return False
        except Exception as e:
            logger.error(f"连接过程中发生错误: {e}")
            return False
            
    def disconnect(self):
        """断开与Modbus服务器的连接"""
        if self.client:
            self.client.close()
            logger.info("已断开与Modbus服务器的连接")
            
    def read_coils(self, address, count):
        """读取线圈"""
        try:
            logger.info(f"读取线圈: 地址={address}, 数量={count}")
            # pymodbus 3.8.6 版本的API
            response = self.client.read_coils(address=address, count=count, slave=self.unit)
            
            if hasattr(response, 'isError') and response.isError():
                logger.error(f"读取线圈错误: {response}")
                return None
                
            values = response.bits[:count]
            logger.info(f"线圈值: {values}")
            return values
        except Exception as e:
            logger.error(f"读取线圈时发生错误: {e}")
            return None
            
    def read_discrete_inputs(self, address, count):
        """读取离散输入"""
        try:
            logger.info(f"读取离散输入: 地址={address}, 数量={count}")
            # pymodbus 3.8.6 版本的API
            response = self.client.read_discrete_inputs(address=address, count=count, slave=self.unit)
            
            if hasattr(response, 'isError') and response.isError():
                logger.error(f"读取离散输入错误: {response}")
                return None
                
            values = response.bits[:count]
            logger.info(f"离散输入值: {values}")
            return values
        except Exception as e:
            logger.error(f"读取离散输入时发生错误: {e}")
            return None
            
    def read_holding_registers(self, address, count):
        """读取保持寄存器"""
        try:
            logger.info(f"读取保持寄存器: 地址={address}, 数量={count}")
            # pymodbus 3.8.6 版本的API
            response = self.client.read_holding_registers(address=address, count=count, slave=self.unit)
            
            if hasattr(response, 'isError') and response.isError():
                logger.error(f"读取保持寄存器错误: {response}")
                return None
                
            values = response.registers
            logger.info(f"保持寄存器值: {values}")
            return values
        except Exception as e:
            logger.error(f"读取保持寄存器时发生错误: {e}")
            return None
            
    def read_input_registers(self, address, count):
        """读取输入寄存器"""
        try:
            logger.info(f"读取输入寄存器: 地址={address}, 数量={count}")
            # pymodbus 3.8.6 版本的API
            response = self.client.read_input_registers(address=address, count=count, slave=self.unit)
            
            if hasattr(response, 'isError') and response.isError():
                logger.error(f"读取输入寄存器错误: {response}")
                return None
                
            values = response.registers
            logger.info(f"输入寄存器值: {values}")
            return values
        except Exception as e:
            logger.error(f"读取输入寄存器时发生错误: {e}")
            return None
    
    def write_coil(self, address, value):
        """写入单个线圈"""
        try:
            logger.info(f"写入线圈: 地址={address}, 值={value}")
            # pymodbus 3.8.6 版本的API
            response = self.client.write_coil(address=address, value=value, slave=self.unit)
            
            if hasattr(response, 'isError') and response.isError():
                logger.error(f"写入线圈错误: {response}")
                return False
                
            logger.info(f"线圈写入成功: 地址={address}, 值={value}")
            return True
        except Exception as e:
            logger.error(f"写入线圈时发生错误: {e}")
            return False
            
    def write_register(self, address, value):
        """写入单个寄存器"""
        try:
            logger.info(f"写入寄存器: 地址={address}, 值={value}")
            # pymodbus 3.8.6 版本的API
            response = self.client.write_register(address=address, value=value, slave=self.unit)
            
            if hasattr(response, 'isError') and response.isError():
                logger.error(f"写入寄存器错误: {response}")
                return False
                
            logger.info(f"寄存器写入成功: 地址={address}, 值={value}")
            return True
        except Exception as e:
            logger.error(f"写入寄存器时发生错误: {e}")
            return False
            
    def read_write_test(self):
        """执行完整的读写测试"""
        logger.info("开始执行Modbus读写测试...")
        
        # 测试读取各种类型的寄存器
        coils = self.read_coils(0, 10)
        discrete_inputs = self.read_discrete_inputs(0, 10)
        holding_registers = self.read_holding_registers(0, 10)
        input_registers = self.read_input_registers(0, 10)
        
        # 测试写入线圈
        if coils is not None:
            # 写入与当前状态相反的值
            self.write_coil(0, not coils[0])
            # 检查写入是否成功
            new_coils = self.read_coils(0, 10)
            if new_coils and new_coils[0] != coils[0]:
                logger.info("线圈写入测试成功")
            else:
                logger.error("线圈写入测试失败")
        
        # 测试写入寄存器
        if holding_registers is not None:
            # 写入递增值
            new_value = (holding_registers[0] + 1) % 65536
            self.write_register(0, new_value)
            # 检查写入是否成功
            new_registers = self.read_holding_registers(0, 10)
            if new_registers and new_registers[0] == new_value:
                logger.info("寄存器写入测试成功")
            else:
                logger.error("寄存器写入测试失败")
            
        logger.info("Modbus读写测试完成")

def main():
    """主函数"""
    parser = argparse.ArgumentParser(description="Modbus客户端测试工具")
    parser.add_argument("--host", default="127.0.0.1", help="Modbus服务器主机地址")
    parser.add_argument("--port", type=int, default=502, help="Modbus服务器端口")
    parser.add_argument("--unit", type=int, default=1, help="Modbus单元ID（从站ID）")
    parser.add_argument("--cycles", type=int, default=3, help="测试循环次数")
    parser.add_argument("--interval", type=float, default=1.0, help="测试间隔时间（秒）")
    
    args = parser.parse_args()
    
    client = ModbusTestClient(host=args.host, port=args.port, unit=args.unit)
    
    try:
        # 连接到服务器
        if not client.connect():
            logger.error("无法连接到Modbus服务器，测试终止")
            return
        
        # 执行指定次数的测试
        for cycle in range(args.cycles):
            logger.info(f"===== 开始测试循环 {cycle+1}/{args.cycles} =====")
            client.read_write_test()
            
            if cycle < args.cycles - 1:
                logger.info(f"等待 {args.interval} 秒后开始下一次测试...")
                time.sleep(args.interval)
    
    except KeyboardInterrupt:
        logger.info("收到中断信号，测试终止")
    except Exception as e:
        logger.error(f"测试过程中发生错误: {e}")
    finally:
        # 断开连接
        client.disconnect()

if __name__ == "__main__":
    main() 