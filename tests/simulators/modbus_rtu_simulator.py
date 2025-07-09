#!/usr/bin/env python3
"""
Modbus RTU Server Simulator for Testing
Supports all four remote types (YC/YX/YK/YT) with configurable data patterns
"""

import asyncio
import logging
import argparse
import math
import time
import random
import struct
import serial
from typing import Dict, List, Tuple
from pymodbus.server import StartAsyncSerialServer
from pymodbus.device import ModbusDeviceIdentification
from pymodbus.datastore import ModbusSequentialDataBlock, ModbusSlaveContext, ModbusServerContext
from pymodbus.version import version

logging.basicConfig()
log = logging.getLogger()

class DataPattern:
    """Data pattern generator for simulating different value changes"""
    
    @staticmethod
    def sine_wave(t: float, min_val: float, max_val: float, period: float) -> float:
        """Generate sine wave pattern"""
        amplitude = (max_val - min_val) / 2
        offset = (max_val + min_val) / 2
        return offset + amplitude * math.sin(2 * math.pi * t / period)
    
    @staticmethod
    def square_wave(t: float, min_val: float, max_val: float, period: float) -> float:
        """Generate square wave pattern"""
        phase = (t % period) / period
        return max_val if phase < 0.5 else min_val
    
    @staticmethod
    def sawtooth(t: float, min_val: float, max_val: float, period: float) -> float:
        """Generate sawtooth pattern"""
        phase = (t % period) / period
        return min_val + (max_val - min_val) * phase
    
    @staticmethod
    def random_walk(current: float, min_val: float, max_val: float, step: float) -> float:
        """Generate random walk pattern"""
        change = random.uniform(-step, step)
        new_val = current + change
        return max(min_val, min(max_val, new_val))

class ModbusRTUSimulator:
    def __init__(self, slave_id: int = 1):
        self.slave_id = slave_id
        self.start_time = time.time()
        self.current_values = {}
        
        # Initialize data blocks
        # Coils (0x) - Digital Outputs (YK - 遥控)
        self.coils = ModbusSequentialDataBlock(1, [0] * 1000)
        
        # Discrete Inputs (1x) - Digital Inputs (YX - 遥信)
        self.discrete_inputs = ModbusSequentialDataBlock(1, [0] * 1000)
        
        # Input Registers (3x) - Analog Inputs (YC - 遥测)
        self.input_registers = ModbusSequentialDataBlock(1, [0] * 10000)
        
        # Holding Registers (4x) - Analog Outputs (YT - 遥调)
        self.holding_registers = ModbusSequentialDataBlock(1, [0] * 10000)
        
        # Create slave context
        self.store = ModbusSlaveContext(
            di=self.discrete_inputs,
            co=self.coils,
            hr=self.holding_registers,
            ir=self.input_registers
        )
        
        # Initialize test data points
        self.init_test_points()
    
    def init_test_points(self):
        """Initialize test data points with different patterns"""
        # YC (遥测) - Input Registers (30001-39999)
        # Voltage measurements (3-phase)
        self.register_point('ir', 30001, 'voltage_a', 'sine', 210.0, 230.0, 60.0)
        self.register_point('ir', 30003, 'voltage_b', 'sine', 210.0, 230.0, 60.0, phase=120)
        self.register_point('ir', 30005, 'voltage_c', 'sine', 210.0, 230.0, 60.0, phase=240)
        
        # Current measurements (3-phase)
        self.register_point('ir', 30007, 'current_a', 'random_walk', 0.0, 100.0, 5.0)
        self.register_point('ir', 30009, 'current_b', 'random_walk', 0.0, 100.0, 5.0)
        self.register_point('ir', 30011, 'current_c', 'random_walk', 0.0, 100.0, 5.0)
        
        # Power measurements
        self.register_point('ir', 30013, 'active_power', 'sine', 0.0, 5000.0, 120.0)
        self.register_point('ir', 30015, 'reactive_power', 'sine', -1000.0, 1000.0, 90.0)
        self.register_point('ir', 30017, 'apparent_power', 'constant', 5000.0, 5000.0)
        
        # Power factor
        self.register_point('ir', 30019, 'power_factor', 'sine', 0.85, 0.99, 180.0)
        
        # Frequency
        self.register_point('ir', 30021, 'frequency', 'sine', 49.8, 50.2, 30.0)
        
        # Temperature
        self.register_point('ir', 30023, 'temperature', 'sine', 20.0, 35.0, 300.0)
        
        # YX (遥信) - Discrete Inputs (10001-19999)
        self.register_point('di', 1, 'breaker_status', 'square', 0, 1, 120.0)
        self.register_point('di', 2, 'alarm_status', 'constant', 0, 0)
        self.register_point('di', 3, 'fault_status', 'random', 0, 1, 0.01)  # 1% chance
        self.register_point('di', 4, 'door_open', 'square', 0, 1, 300.0)
        
        # YK (遥控) - Coils (00001-09999)
        # These are writable by the client
        self.coils.setValues(1, [0] * 10)  # Initialize control coils
        
        # YT (遥调) - Holding Registers (40001-49999)
        # These are writable by the client
        self.holding_registers.setValues(40001, [1000] * 100)  # Initialize setpoints
    
    def register_point(self, block_type: str, address: int, name: str, 
                      pattern: str, min_val: float, max_val: float, 
                      period: float = 60.0, phase: float = 0.0, **kwargs):
        """Register a data point with pattern configuration"""
        self.current_values[f"{block_type}_{address}"] = {
            'block': block_type,
            'address': address,
            'name': name,
            'pattern': pattern,
            'min': min_val,
            'max': max_val,
            'period': period,
            'phase': phase,
            'current': (min_val + max_val) / 2,
            'kwargs': kwargs
        }
    
    def float32_to_registers(self, value: float) -> Tuple[int, int]:
        """Convert float32 to two 16-bit registers (big-endian)"""
        bytes_val = struct.pack('>f', value)
        high = struct.unpack('>H', bytes_val[0:2])[0]
        low = struct.unpack('>H', bytes_val[2:4])[0]
        return high, low
    
    async def update_values(self):
        """Update all registered values based on their patterns"""
        current_time = time.time() - self.start_time
        
        for key, config in self.current_values.items():
            t = current_time + config['phase']
            
            if config['pattern'] == 'sine':
                value = DataPattern.sine_wave(t, config['min'], config['max'], config['period'])
            elif config['pattern'] == 'square':
                value = DataPattern.square_wave(t, config['min'], config['max'], config['period'])
            elif config['pattern'] == 'sawtooth':
                value = DataPattern.sawtooth(t, config['min'], config['max'], config['period'])
            elif config['pattern'] == 'random_walk':
                step = config['max'] if 'step' not in config['kwargs'] else config['kwargs']['step']
                value = DataPattern.random_walk(config['current'], config['min'], config['max'], step)
                config['current'] = value
            elif config['pattern'] == 'random':
                threshold = config['kwargs'].get('threshold', 0.5)
                value = config['max'] if random.random() < threshold else config['min']
            elif config['pattern'] == 'constant':
                value = config['min']
            else:
                value = config['min']
            
            # Update the appropriate data block
            if config['block'] in ['ir', 'hr']:  # Registers (16-bit or float32)
                if config['name'].endswith('_float32') or 'float' in config['name']:
                    # Store as float32 (2 registers)
                    high, low = self.float32_to_registers(value)
                    if config['block'] == 'ir':
                        self.input_registers.setValues(config['address'], [high, low])
                    else:
                        self.holding_registers.setValues(config['address'], [high, low])
                else:
                    # Store as single 16-bit value
                    int_value = int(value * 10) if value < 6553.5 else int(value)
                    if config['block'] == 'ir':
                        self.input_registers.setValues(config['address'], [int_value])
                    else:
                        self.holding_registers.setValues(config['address'], [int_value])
            else:  # Coils/Discrete inputs (boolean)
                bool_value = int(value) > 0
                if config['block'] == 'di':
                    self.discrete_inputs.setValues(config['address'], [bool_value])
                else:
                    self.coils.setValues(config['address'], [bool_value])
    
    async def print_status(self):
        """Print current status of some key values"""
        voltage_a = self.input_registers.getValues(30001, 2)
        current_a = self.input_registers.getValues(30007, 2)
        breaker_status = self.discrete_inputs.getValues(1, 1)[0]
        
        # Convert registers to float if needed
        if len(voltage_a) == 2:
            voltage_bytes = struct.pack('>HH', voltage_a[0], voltage_a[1])
            voltage_val = struct.unpack('>f', voltage_bytes)[0]
        else:
            voltage_val = voltage_a[0] / 10.0
        
        log.info(f"RTU Status - Voltage A: {voltage_val:.1f}V, Breaker: {'ON' if breaker_status else 'OFF'}")

async def run_update_loop(simulator: ModbusRTUSimulator, interval: float):
    """Run the update loop for simulator values"""
    while True:
        try:
            await simulator.update_values()
            if random.random() < 0.1:  # Print status 10% of the time
                await simulator.print_status()
        except Exception as e:
            log.error(f"Error updating values: {e}")
        
        await asyncio.sleep(interval)

async def run_server(port: str, baudrate: int, slave_id: int, update_interval: float, **kwargs):
    """Run the Modbus RTU server"""
    # Create simulator
    simulator = ModbusRTUSimulator(slave_id)
    
    # Create server context
    context = ModbusServerContext(slaves={slave_id: simulator.store}, single=False)
    
    # Server identification
    identity = ModbusDeviceIdentification()
    identity.VendorName = 'VoltageEMS'
    identity.ProductCode = 'TEST'
    identity.VendorUrl = 'http://github.com/voltageems'
    identity.ProductName = 'VoltageEMS RTU Test Simulator'
    identity.ModelName = 'Modbus RTU Simulator'
    identity.MajorMinorRevision = version.short()
    
    # Start update loop
    asyncio.create_task(run_update_loop(simulator, update_interval))
    
    # Configure serial parameters
    serial_params = {
        'port': port,
        'baudrate': baudrate,
        'bytesize': kwargs.get('bytesize', 8),
        'parity': kwargs.get('parity', 'N'),
        'stopbits': kwargs.get('stopbits', 1),
        'timeout': kwargs.get('timeout', 1)
    }
    
    # Start server
    log.info(f"Starting Modbus RTU server on {port} @ {baudrate} baud, slave ID {slave_id}")
    log.info(f"Serial params: {serial_params}")
    
    await StartAsyncSerialServer(
        context=context,
        identity=identity,
        **serial_params
    )

def main():
    parser = argparse.ArgumentParser(description='Modbus RTU Server Simulator')
    parser.add_argument('--port', default='/dev/ttyUSB0', help='Serial port (e.g., /dev/ttyUSB0, COM1)')
    parser.add_argument('--baudrate', type=int, default=9600, help='Baud rate')
    parser.add_argument('--bytesize', type=int, default=8, choices=[5, 6, 7, 8], help='Data bits')
    parser.add_argument('--parity', default='N', choices=['N', 'E', 'O', 'M', 'S'], help='Parity')
    parser.add_argument('--stopbits', type=float, default=1, choices=[1, 1.5, 2], help='Stop bits')
    parser.add_argument('--slave-id', type=int, default=1, help='Modbus slave ID')
    parser.add_argument('--update-interval', type=float, default=0.1, help='Value update interval (seconds)')
    parser.add_argument('--debug', action='store_true', help='Enable debug logging')
    parser.add_argument('--create-virtual', action='store_true', help='Create virtual serial port using socat')
    
    args = parser.parse_args()
    
    # Configure logging
    if args.debug:
        log.setLevel(logging.DEBUG)
    else:
        log.setLevel(logging.INFO)
    
    # Create virtual serial port if requested
    if args.create_virtual:
        import subprocess
        import os
        
        # Create virtual serial port pair using socat
        log.info("Creating virtual serial port pair...")
        
        # Use pty for macOS/Linux
        socat_cmd = [
            'socat',
            '-d', '-d',
            'pty,raw,echo=0,link=/tmp/modbus_rtu_master',
            'pty,raw,echo=0,link=/tmp/modbus_rtu_slave'
        ]
        
        try:
            # Start socat in background
            socat_process = subprocess.Popen(socat_cmd)
            log.info("Virtual serial ports created:")
            log.info("  Master: /tmp/modbus_rtu_master")
            log.info("  Slave: /tmp/modbus_rtu_slave")
            
            # Wait a bit for socat to initialize
            time.sleep(1)
            
            # Use the slave port for the server
            args.port = '/tmp/modbus_rtu_slave'
            
            # Run server with cleanup on exit
            try:
                asyncio.run(run_server(
                    args.port, 
                    args.baudrate, 
                    args.slave_id, 
                    args.update_interval,
                    bytesize=args.bytesize,
                    parity=args.parity,
                    stopbits=args.stopbits
                ))
            finally:
                socat_process.terminate()
                socat_process.wait()
        except Exception as e:
            log.error(f"Failed to create virtual serial ports: {e}")
            log.info("Please install socat: brew install socat (macOS) or apt-get install socat (Linux)")
            return
    else:
        # Run server with real serial port
        asyncio.run(run_server(
            args.port, 
            args.baudrate, 
            args.slave_id, 
            args.update_interval,
            bytesize=args.bytesize,
            parity=args.parity,
            stopbits=args.stopbits
        ))

if __name__ == '__main__':
    main()