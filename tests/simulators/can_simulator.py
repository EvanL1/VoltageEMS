#!/usr/bin/env python3
"""
CAN Bus Simulator for Testing
Simulates CAN messages with various data patterns
"""

import can
import time
import argparse
import logging
import math
import random
import struct
import asyncio
from typing import Dict, List, Tuple
from dataclasses import dataclass

logging.basicConfig()
log = logging.getLogger()

@dataclass
class CANPoint:
    """CAN data point definition"""
    can_id: int
    name: str
    data_type: str
    byte_offset: int
    byte_length: int
    pattern: str
    min_value: float
    max_value: float
    period: float

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

class CANSimulator:
    def __init__(self, interface: str, bitrate: int = 500000):
        self.interface = interface
        self.bitrate = bitrate
        self.bus = None
        self.start_time = time.time()
        self.can_points: Dict[int, List[CANPoint]] = {}
        self.current_values = {}
        self.running = False
        
        # Initialize test data points
        self.init_test_points()
    
    def init_test_points(self):
        """Initialize CAN test data points"""
        # Battery Management System (BMS) messages
        self.add_point(CANPoint(
            can_id=0x100,
            name="battery_voltage",
            data_type="uint16",
            byte_offset=0,
            byte_length=2,
            pattern="sine",
            min_value=320.0,  # 32.0V * 10
            max_value=420.0,  # 42.0V * 10
            period=60.0
        ))
        
        self.add_point(CANPoint(
            can_id=0x100,
            name="battery_current",
            data_type="int16",
            byte_offset=2,
            byte_length=2,
            pattern="random_walk",
            min_value=-1000,  # -100.0A * 10
            max_value=1000,   # 100.0A * 10
            period=1.0
        ))
        
        self.add_point(CANPoint(
            can_id=0x100,
            name="battery_soc",
            data_type="uint8",
            byte_offset=4,
            byte_length=1,
            pattern="sawtooth",
            min_value=0,      # 0%
            max_value=100,    # 100%
            period=300.0
        ))
        
        self.add_point(CANPoint(
            can_id=0x100,
            name="battery_temp",
            data_type="int8",
            byte_offset=5,
            byte_length=1,
            pattern="sine",
            min_value=15,     # 15°C
            max_value=35,     # 35°C
            period=180.0
        ))
        
        # Motor Controller messages
        self.add_point(CANPoint(
            can_id=0x200,
            name="motor_speed",
            data_type="uint16",
            byte_offset=0,
            byte_length=2,
            pattern="sine",
            min_value=0,      # 0 RPM
            max_value=6000,   # 6000 RPM
            period=30.0
        ))
        
        self.add_point(CANPoint(
            can_id=0x200,
            name="motor_torque",
            data_type="int16",
            byte_offset=2,
            byte_length=2,
            pattern="square",
            min_value=-200,   # -200 Nm
            max_value=200,    # 200 Nm
            period=20.0
        ))
        
        self.add_point(CANPoint(
            can_id=0x200,
            name="motor_temp",
            data_type="uint8",
            byte_offset=4,
            byte_length=1,
            pattern="sine",
            min_value=30,     # 30°C
            max_value=80,     # 80°C
            period=120.0
        ))
        
        # Digital I/O status
        self.add_point(CANPoint(
            can_id=0x300,
            name="digital_inputs",
            data_type="uint8",
            byte_offset=0,
            byte_length=1,
            pattern="random",
            min_value=0,
            max_value=255,
            period=5.0
        ))
        
        self.add_point(CANPoint(
            can_id=0x300,
            name="digital_outputs",
            data_type="uint8",
            byte_offset=1,
            byte_length=1,
            pattern="square",
            min_value=0,
            max_value=255,
            period=10.0
        ))
        
        # System status message
        self.add_point(CANPoint(
            can_id=0x400,
            name="system_status",
            data_type="uint32",
            byte_offset=0,
            byte_length=4,
            pattern="constant",
            min_value=0x01020304,
            max_value=0x01020304,
            period=1.0
        ))
    
    def add_point(self, point: CANPoint):
        """Add a CAN data point"""
        if point.can_id not in self.can_points:
            self.can_points[point.can_id] = []
        self.can_points[point.can_id].append(point)
        
        # Initialize current value
        key = f"{point.can_id}_{point.name}"
        self.current_values[key] = (point.min_value + point.max_value) / 2
    
    def connect(self):
        """Connect to CAN bus"""
        try:
            if self.interface == "virtual":
                # Use virtual CAN interface for testing
                self.bus = can.interface.Bus(
                    interface='socketcan',
                    channel='vcan0',
                    bitrate=self.bitrate
                )
            else:
                # Use real CAN interface
                self.bus = can.interface.Bus(
                    interface='socketcan',
                    channel=self.interface,
                    bitrate=self.bitrate
                )
            log.info(f"Connected to CAN bus: {self.interface}")
            return True
        except Exception as e:
            log.error(f"Failed to connect to CAN bus: {e}")
            return False
    
    def disconnect(self):
        """Disconnect from CAN bus"""
        if self.bus:
            self.bus.shutdown()
            self.bus = None
            log.info("Disconnected from CAN bus")
    
    def generate_value(self, point: CANPoint, current_time: float) -> float:
        """Generate value based on pattern"""
        key = f"{point.can_id}_{point.name}"
        current_value = self.current_values.get(key, point.min_value)
        
        if point.pattern == "sine":
            value = DataPattern.sine_wave(current_time, point.min_value, point.max_value, point.period)
        elif point.pattern == "square":
            value = DataPattern.square_wave(current_time, point.min_value, point.max_value, point.period)
        elif point.pattern == "sawtooth":
            value = DataPattern.sawtooth(current_time, point.min_value, point.max_value, point.period)
        elif point.pattern == "random_walk":
            step = (point.max_value - point.min_value) * 0.1
            value = DataPattern.random_walk(current_value, point.min_value, point.max_value, step)
        elif point.pattern == "random":
            value = random.uniform(point.min_value, point.max_value)
        elif point.pattern == "constant":
            value = point.min_value
        else:
            value = point.min_value
        
        self.current_values[key] = value
        return value
    
    def pack_value(self, value: float, data_type: str) -> bytes:
        """Pack value into bytes based on data type"""
        int_value = int(value)
        
        if data_type == "uint8":
            return struct.pack("B", int_value & 0xFF)
        elif data_type == "int8":
            return struct.pack("b", max(-128, min(127, int_value)))
        elif data_type == "uint16":
            return struct.pack(">H", int_value & 0xFFFF)  # Big-endian
        elif data_type == "int16":
            return struct.pack(">h", max(-32768, min(32767, int_value)))
        elif data_type == "uint32":
            return struct.pack(">I", int_value & 0xFFFFFFFF)
        elif data_type == "int32":
            return struct.pack(">i", int_value)
        elif data_type == "float":
            return struct.pack(">f", value)
        else:
            return b'\x00'
    
    def create_can_message(self, can_id: int, current_time: float) -> can.Message:
        """Create CAN message with data"""
        if can_id not in self.can_points:
            return None
        
        # Create 8-byte data buffer
        data = bytearray(8)
        
        # Fill data for each point in this CAN ID
        for point in self.can_points[can_id]:
            value = self.generate_value(point, current_time)
            packed = self.pack_value(value, point.data_type)
            
            # Copy packed bytes to correct position
            for i in range(min(len(packed), point.byte_length)):
                if point.byte_offset + i < 8:
                    data[point.byte_offset + i] = packed[i]
        
        # Create CAN message
        msg = can.Message(
            arbitration_id=can_id,
            data=data,
            is_extended_id=False
        )
        
        return msg
    
    async def run_async(self, update_interval: float = 0.1):
        """Run simulator asynchronously"""
        self.running = True
        
        while self.running:
            current_time = time.time() - self.start_time
            
            # Send messages for all configured CAN IDs
            for can_id in self.can_points.keys():
                msg = self.create_can_message(can_id, current_time)
                if msg and self.bus:
                    try:
                        self.bus.send(msg)
                        
                        # Log occasionally
                        if random.random() < 0.05:  # 5% chance
                            log.info(f"Sent CAN ID 0x{can_id:03X}: {msg.data.hex()}")
                    except Exception as e:
                        log.error(f"Error sending CAN message: {e}")
            
            await asyncio.sleep(update_interval)
    
    def stop(self):
        """Stop the simulator"""
        self.running = False

async def main():
    parser = argparse.ArgumentParser(description='CAN Bus Simulator')
    parser.add_argument('--interface', default='vcan0', help='CAN interface (e.g., vcan0, can0)')
    parser.add_argument('--bitrate', type=int, default=500000, help='CAN bitrate')
    parser.add_argument('--update-interval', type=float, default=0.1, help='Message update interval (seconds)')
    parser.add_argument('--debug', action='store_true', help='Enable debug logging')
    
    args = parser.parse_args()
    
    # Configure logging
    if args.debug:
        log.setLevel(logging.DEBUG)
    else:
        log.setLevel(logging.INFO)
    
    # Create and start simulator
    simulator = CANSimulator(args.interface, args.bitrate)
    
    if not simulator.connect():
        log.error("Failed to start CAN simulator")
        return
    
    try:
        log.info(f"CAN simulator running on {args.interface} @ {args.bitrate} bps")
        log.info("Press Ctrl+C to stop")
        await simulator.run_async(args.update_interval)
    except KeyboardInterrupt:
        log.info("Stopping simulator...")
    finally:
        simulator.stop()
        simulator.disconnect()

if __name__ == '__main__':
    asyncio.run(main())