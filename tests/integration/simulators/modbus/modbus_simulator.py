#!/usr/bin/env python3
"""
Multi-device Modbus TCP/RTU Simulator for Integration Testing
Supports configurable number of devices and points per device
"""

import os
import sys
import time
import math
import random
import logging
import threading
from typing import Dict, List, Any
from pymodbus.server import StartTcpServer, StartSerialServer
from pymodbus.device import ModbusDeviceIdentification
from pymodbus.datastore import ModbusSequentialDataBlock, ModbusSlaveContext, ModbusServerContext
from pymodbus.transaction import ModbusRtuFramer, ModbusBinaryFramer, ModbusSocketFramer

# Configuration from environment
SIMULATOR_MODE = os.getenv('SIMULATOR_MODE', 'tcp')
DEVICE_COUNT = int(os.getenv('DEVICE_COUNT', '10'))
POINTS_PER_DEVICE = int(os.getenv('POINTS_PER_DEVICE', '100'))
START_PORT = int(os.getenv('START_PORT', '502'))
SERIAL_PORT = os.getenv('SERIAL_PORT', '/dev/ttyUSB0')
BAUD_RATE = int(os.getenv('BAUD_RATE', '9600'))
LOG_LEVEL = os.getenv('LOG_LEVEL', 'INFO')
UPDATE_INTERVAL = float(os.getenv('UPDATE_INTERVAL', '1.0'))
MULTI_PORT = os.getenv('MULTI_PORT', 'false').lower() == 'true'

# Setup logging
logging.basicConfig(
    level=getattr(logging, LOG_LEVEL),
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

class DataPatternGenerator:
    """Generate different data patterns for testing"""
    
    @staticmethod
    def sine_wave(t: float, min_val: float, max_val: float, period: float) -> float:
        """Generate sine wave pattern"""
        amplitude = (max_val - min_val) / 2
        offset = (max_val + min_val) / 2
        return amplitude * math.sin(2 * math.pi * t / period) + offset
    
    @staticmethod
    def square_wave(t: float, min_val: float, max_val: float, period: float) -> float:
        """Generate square wave pattern"""
        return max_val if (t % period) < (period / 2) else min_val
    
    @staticmethod
    def random_walk(current: float, min_val: float, max_val: float, step: float = 0.1) -> float:
        """Generate random walk pattern"""
        change = random.uniform(-step, step) * (max_val - min_val)
        new_val = current + change
        return max(min_val, min(max_val, new_val))
    
    @staticmethod
    def sawtooth(t: float, min_val: float, max_val: float, period: float) -> float:
        """Generate sawtooth pattern"""
        return min_val + (max_val - min_val) * ((t % period) / period)

class ModbusSimulator:
    def __init__(self, device_id: int, slave_id: int):
        self.device_id = device_id
        self.slave_id = slave_id
        self.time_offset = random.random() * 10  # Random phase offset
        self.init_data_store()
        
    def init_data_store(self):
        """Initialize Modbus data store with realistic values"""
        # Coils (0x) - 开关状态
        coils = ModbusSequentialDataBlock(0, [0] * POINTS_PER_DEVICE)
        
        # Discrete Inputs (1x) - 状态信号
        discrete_inputs = ModbusSequentialDataBlock(0, [0] * POINTS_PER_DEVICE)
        
        # Holding Registers (4x) - 可写寄存器
        holding_registers = ModbusSequentialDataBlock(0, [0] * (POINTS_PER_DEVICE * 2))
        
        # Input Registers (3x) - 只读测量值
        input_registers = ModbusSequentialDataBlock(0, [0] * (POINTS_PER_DEVICE * 2))
        
        self.store = ModbusSlaveContext(
            di=discrete_inputs,
            co=coils,
            hr=holding_registers,
            ir=input_registers
        )
        
        # Initialize with some default values
        self.update_values(0)
    
    def update_values(self, timestamp: float):
        """Update simulated values based on different patterns"""
        t = timestamp + self.time_offset
        
        # Update Coils (bool values)
        for i in range(min(10, POINTS_PER_DEVICE)):
            # 断路器状态模拟
            if i < 5:
                value = int(DataPatternGenerator.square_wave(t, 0, 1, 120 + i * 10))
            else:
                value = random.randint(0, 1) if random.random() < 0.01 else self.store.getValues(1, i, 1)[0]
            self.store.setValues(1, i, [value])
        
        # Update Discrete Inputs (bool values)
        for i in range(min(20, POINTS_PER_DEVICE)):
            # 告警状态模拟
            if i < 10:
                value = 1 if random.random() < 0.05 else 0
            else:
                value = int(DataPatternGenerator.square_wave(t, 0, 1, 300 + i * 20))
            self.store.setValues(2, i, [value])
        
        # Update Input Registers (float32 values as 2 registers)
        for i in range(0, min(POINTS_PER_DEVICE * 2, len(self.store.getValues(4, 0, POINTS_PER_DEVICE * 2))), 2):
            point_idx = i // 2
            
            if point_idx < 10:  # 电压
                value = DataPatternGenerator.sine_wave(t, 215, 225, 60 + point_idx)
            elif point_idx < 20:  # 电流
                value = DataPatternGenerator.random_walk(
                    self.get_float32_value(4, i), 0, 100, 0.1
                )
            elif point_idx < 30:  # 功率
                value = DataPatternGenerator.sine_wave(t, 0, 5000, 120)
            elif point_idx < 40:  # 功率因数
                value = DataPatternGenerator.sine_wave(t, 0.85, 0.99, 180)
            else:  # 其他测量值
                value = DataPatternGenerator.sawtooth(t, 0, 1000, 240)
            
            # Convert float to two uint16 registers
            self.set_float32_value(4, i, value)
        
        # Update Holding Registers (setpoints)
        for i in range(0, min(20, POINTS_PER_DEVICE * 2), 2):
            if i == 0:  # 保持一些固定设定值
                self.set_float32_value(3, i, 1000.0)
            elif i == 2:
                self.set_float32_value(3, i, 500.0)
    
    def get_float32_value(self, fx: int, address: int) -> float:
        """Get float32 value from two registers"""
        try:
            values = self.store.getValues(fx, address, 2)
            # Combine two 16-bit registers into float32
            combined = (values[0] << 16) | values[1]
            # Simple conversion (you might need proper IEEE 754 conversion)
            return combined / 1000.0
        except:
            return 0.0
    
    def set_float32_value(self, fx: int, address: int, value: float):
        """Set float32 value to two registers"""
        try:
            # Simple conversion (you might need proper IEEE 754 conversion)
            int_value = int(value * 1000)
            high = (int_value >> 16) & 0xFFFF
            low = int_value & 0xFFFF
            self.store.setValues(fx, address, [high, low])
        except:
            pass

def run_tcp_server(port: int, slave_contexts: Dict[int, ModbusSlaveContext]):
    """Run Modbus TCP server"""
    context = ModbusServerContext(slaves=slave_contexts, single=False)
    
    identity = ModbusDeviceIdentification()
    identity.VendorName = 'VoltageEMS'
    identity.ProductCode = 'VEMS-SIM'
    identity.VendorUrl = 'http://github.com/voltage-ems'
    identity.ProductName = 'VoltageEMS Modbus Simulator'
    identity.ModelName = f'TCP-Device-Port-{port}'
    identity.MajorMinorRevision = '1.0'
    
    logger.info(f"Starting Modbus TCP server on port {port} with {len(slave_contexts)} devices")
    
    StartTcpServer(
        context=context,
        identity=identity,
        address=("0.0.0.0", port),
        allow_reuse_address=True
    )

def run_rtu_server(port: str, slave_contexts: Dict[int, ModbusSlaveContext]):
    """Run Modbus RTU server"""
    context = ModbusServerContext(slaves=slave_contexts, single=False)
    
    identity = ModbusDeviceIdentification()
    identity.VendorName = 'VoltageEMS'
    identity.ProductCode = 'VEMS-SIM'
    identity.VendorUrl = 'http://github.com/voltage-ems'
    identity.ProductName = 'VoltageEMS Modbus RTU Simulator'
    identity.ModelName = 'RTU-Device'
    identity.MajorMinorRevision = '1.0'
    
    logger.info(f"Starting Modbus RTU server on {port} at {BAUD_RATE} baud with {len(slave_contexts)} devices")
    
    StartSerialServer(
        context=context,
        identity=identity,
        port=port,
        framer=ModbusRtuFramer,
        baudrate=BAUD_RATE,
        stopbits=1,
        bytesize=8,
        parity='N'
    )

def update_loop(simulators: List[ModbusSimulator]):
    """Background thread to update simulator values"""
    logger.info(f"Starting update loop for {len(simulators)} simulators")
    start_time = time.time()
    
    while True:
        current_time = time.time() - start_time
        
        for sim in simulators:
            sim.update_values(current_time)
        
        time.sleep(UPDATE_INTERVAL)

def main():
    logger.info(f"Starting Modbus {SIMULATOR_MODE.upper()} Simulator")
    logger.info(f"Device Count: {DEVICE_COUNT}, Points per Device: {POINTS_PER_DEVICE}")
    
    # Create simulators
    simulators = []
    slave_contexts = {}
    
    for i in range(DEVICE_COUNT):
        slave_id = i + 1
        simulator = ModbusSimulator(i, slave_id)
        simulators.append(simulator)
        slave_contexts[slave_id] = simulator.store
    
    # Start update thread
    update_thread = threading.Thread(target=update_loop, args=(simulators,), daemon=True)
    update_thread.start()
    
    # Start server(s)
    if SIMULATOR_MODE == 'tcp':
        if MULTI_PORT and DEVICE_COUNT > 1:
            # Run multiple servers on different ports
            threads = []
            for i in range(DEVICE_COUNT):
                slave_id = i + 1
                port = START_PORT + i
                context = {slave_id: simulators[i].store}
                thread = threading.Thread(
                    target=run_tcp_server,
                    args=(port, context),
                    daemon=True
                )
                thread.start()
                threads.append(thread)
                time.sleep(0.1)  # Small delay between starting servers
            
            # Keep main thread alive
            try:
                while True:
                    time.sleep(1)
            except KeyboardInterrupt:
                logger.info("Shutting down...")
        else:
            # Single server with all devices
            run_tcp_server(START_PORT, slave_contexts)
    else:
        # RTU mode
        run_rtu_server(SERIAL_PORT, slave_contexts)

if __name__ == "__main__":
    main()