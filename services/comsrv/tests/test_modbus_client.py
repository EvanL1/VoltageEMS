#!/usr/bin/env python3
"""
Modbus测试客户端 - 用于验证Modbus服务器功能

功能：
- 测试所有Modbus功能码
- 验证数据读写
- 性能测试
- 结果报告
"""

import asyncio
import time
import argparse
from pymodbus.client import AsyncModbusTcpClient
from pymodbus.exceptions import ModbusException
import logging

# 配置日志
logging.basicConfig(
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    level=logging.INFO
)
logger = logging.getLogger(__name__)

class ModbusTestClient:
    def __init__(self, host='127.0.0.1', port=5502):
        self.host = host
        self.port = port
        self.client = None
        self.test_results = []
        
    async def connect(self):
        """连接到Modbus服务器"""
        self.client = AsyncModbusTcpClient(host=self.host, port=self.port)
        connected = await self.client.connect()
        if connected:
            logger.info(f"成功连接到 {self.host}:{self.port}")
        else:
            logger.error(f"无法连接到 {self.host}:{self.port}")
        return connected
        
    async def disconnect(self):
        """断开连接"""
        if self.client:
            self.client.close()
            logger.info("已断开连接")
            
    def log_result(self, test_name, success, details=""):
        """记录测试结果"""
        result = {
            'test': test_name,
            'success': success,
            'details': details
        }
        self.test_results.append(result)
        status = "✓ 通过" if success else "✗ 失败"
        logger.info(f"{test_name}: {status} {details}")
        
    async def test_read_coils(self, unit_id=1):
        """测试读线圈（功能码01）"""
        try:
            result = await self.client.read_coils(0, 10, slave=unit_id)
            if not result.isError():
                values = result.bits[:10]
                self.log_result("读线圈(FC01)", True, f"读取10个值: {values}")
                return True
            else:
                self.log_result("读线圈(FC01)", False, f"错误: {result}")
                return False
        except Exception as e:
            self.log_result("读线圈(FC01)", False, f"异常: {e}")
            return False
            
    async def test_read_discrete_inputs(self, unit_id=1):
        """测试读离散输入（功能码02）"""
        try:
            result = await self.client.read_discrete_inputs(0, 10, slave=unit_id)
            if not result.isError():
                values = result.bits[:10]
                self.log_result("读离散输入(FC02)", True, f"读取10个值: {values}")
                return True
            else:
                self.log_result("读离散输入(FC02)", False, f"错误: {result}")
                return False
        except Exception as e:
            self.log_result("读离散输入(FC02)", False, f"异常: {e}")
            return False
            
    async def test_read_holding_registers(self, unit_id=1):
        """测试读保持寄存器（功能码03）"""
        try:
            result = await self.client.read_holding_registers(0, 10, slave=unit_id)
            if not result.isError():
                values = result.registers
                self.log_result("读保持寄存器(FC03)", True, f"读取10个值: {values}")
                return True
            else:
                self.log_result("读保持寄存器(FC03)", False, f"错误: {result}")
                return False
        except Exception as e:
            self.log_result("读保持寄存器(FC03)", False, f"异常: {e}")
            return False
            
    async def test_read_input_registers(self, unit_id=1):
        """测试读输入寄存器（功能码04）"""
        try:
            result = await self.client.read_input_registers(0, 10, slave=unit_id)
            if not result.isError():
                values = result.registers
                self.log_result("读输入寄存器(FC04)", True, f"读取10个值: {values}")
                return True
            else:
                self.log_result("读输入寄存器(FC04)", False, f"错误: {result}")
                return False
        except Exception as e:
            self.log_result("读输入寄存器(FC04)", False, f"异常: {e}")
            return False
            
    async def test_write_single_coil(self, unit_id=1):
        """测试写单个线圈（功能码05）"""
        try:
            # 写入True
            result = await self.client.write_coil(10, True, slave=unit_id)
            if not result.isError():
                # 读回验证
                read_result = await self.client.read_coils(10, 1, slave=unit_id)
                if not read_result.isError() and read_result.bits[0]:
                    self.log_result("写单个线圈(FC05)", True, "写入并验证成功")
                    return True
            self.log_result("写单个线圈(FC05)", False, f"错误: {result}")
            return False
        except Exception as e:
            self.log_result("写单个线圈(FC05)", False, f"异常: {e}")
            return False
            
    async def test_write_single_register(self, unit_id=1):
        """测试写单个寄存器（功能码06）"""
        try:
            test_value = 12345
            result = await self.client.write_register(10, test_value, slave=unit_id)
            if not result.isError():
                # 读回验证
                read_result = await self.client.read_holding_registers(10, 1, slave=unit_id)
                if not read_result.isError() and read_result.registers[0] == test_value:
                    self.log_result("写单个寄存器(FC06)", True, f"写入值{test_value}并验证成功")
                    return True
            self.log_result("写单个寄存器(FC06)", False, f"错误: {result}")
            return False
        except Exception as e:
            self.log_result("写单个寄存器(FC06)", False, f"异常: {e}")
            return False
            
    async def test_write_multiple_coils(self, unit_id=1):
        """测试写多个线圈（功能码15）"""
        try:
            values = [True, False, True, False, True]
            result = await self.client.write_coils(20, values, slave=unit_id)
            if not result.isError():
                # 读回验证
                read_result = await self.client.read_coils(20, 5, slave=unit_id)
                if not read_result.isError():
                    read_values = read_result.bits[:5]
                    if read_values == values:
                        self.log_result("写多个线圈(FC15)", True, f"写入{len(values)}个值并验证成功")
                        return True
            self.log_result("写多个线圈(FC15)", False, f"错误: {result}")
            return False
        except Exception as e:
            self.log_result("写多个线圈(FC15)", False, f"异常: {e}")
            return False
            
    async def test_write_multiple_registers(self, unit_id=1):
        """测试写多个寄存器（功能码16）"""
        try:
            values = [100, 200, 300, 400, 500]
            result = await self.client.write_registers(20, values, slave=unit_id)
            if not result.isError():
                # 读回验证
                read_result = await self.client.read_holding_registers(20, 5, slave=unit_id)
                if not read_result.isError() and read_result.registers == values:
                    self.log_result("写多个寄存器(FC16)", True, f"写入{len(values)}个值并验证成功")
                    return True
            self.log_result("写多个寄存器(FC16)", False, f"错误: {result}")
            return False
        except Exception as e:
            self.log_result("写多个寄存器(FC16)", False, f"异常: {e}")
            return False
            
    async def test_performance(self, unit_id=1, iterations=100):
        """性能测试"""
        logger.info(f"开始性能测试，迭代次数: {iterations}")
        
        start_time = time.time()
        success_count = 0
        
        for i in range(iterations):
            try:
                result = await self.client.read_holding_registers(0, 10, slave=unit_id)
                if not result.isError():
                    success_count += 1
            except:
                pass
                
        elapsed = time.time() - start_time
        rate = iterations / elapsed
        success_rate = (success_count / iterations) * 100
        
        self.log_result(
            "性能测试", 
            success_rate > 95, 
            f"耗时: {elapsed:.2f}秒, 速率: {rate:.1f}请求/秒, 成功率: {success_rate:.1f}%"
        )
        
    async def run_all_tests(self):
        """运行所有测试"""
        print("\n" + "="*60)
        print("开始Modbus功能测试")
        print("="*60 + "\n")
        
        # 基本功能测试
        await self.test_read_coils()
        await self.test_read_discrete_inputs()
        await self.test_read_holding_registers()
        await self.test_read_input_registers()
        await self.test_write_single_coil()
        await self.test_write_single_register()
        await self.test_write_multiple_coils()
        await self.test_write_multiple_registers()
        
        # 性能测试
        await self.test_performance()
        
        # 显示测试结果摘要
        print("\n" + "="*60)
        print("测试结果摘要")
        print("="*60)
        
        total_tests = len(self.test_results)
        passed_tests = sum(1 for r in self.test_results if r['success'])
        failed_tests = total_tests - passed_tests
        
        print(f"\n总测试数: {total_tests}")
        print(f"通过: {passed_tests}")
        print(f"失败: {failed_tests}")
        print(f"成功率: {(passed_tests/total_tests)*100:.1f}%")
        
        if failed_tests > 0:
            print("\n失败的测试:")
            for result in self.test_results:
                if not result['success']:
                    print(f"  - {result['test']}: {result['details']}")
                    
        return failed_tests == 0

async def main():
    parser = argparse.ArgumentParser(description='Modbus测试客户端')
    parser.add_argument('--host', default='127.0.0.1', help='服务器地址（默认: 127.0.0.1）')
    parser.add_argument('--port', type=int, default=5502, help='服务器端口（默认: 5502）')
    parser.add_argument('--unit', type=int, default=1, help='Unit ID（默认: 1）')
    parser.add_argument('--quick', action='store_true', help='快速测试（跳过性能测试）')
    
    args = parser.parse_args()
    
    client = ModbusTestClient(host=args.host, port=args.port)
    
    if await client.connect():
        success = await client.run_all_tests()
        await client.disconnect()
        return 0 if success else 1
    else:
        return 2

if __name__ == '__main__':
    import sys
    sys.exit(asyncio.run(main()))