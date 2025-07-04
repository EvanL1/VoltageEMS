#!/usr/bin/env python3
"""
Modbus TCP Server Simulator based on CSV configuration
This simulator reads comsrv's CSV configuration files and creates a Modbus server
that exactly matches the expected point mappings.
"""

import asyncio
import csv
import struct
import logging
import argparse
import time
import math
import random
import os
from datetime import datetime
from typing import Dict, List, Tuple, Any
from dataclasses import dataclass
from pathlib import Path

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger('ModbusCSVSimulator')

@dataclass
class TelemetryPoint:
    """Telemetry point definition"""
    point_id: str
    name: str
    description: str
    unit: str
    data_type: str
    scale: float
    offset: float

@dataclass
class SignalPoint:
    """Signal point definition"""
    point_id: str
    name: str
    description: str
    data_type: str
    reverse: int

@dataclass
class ControlPoint:
    """Control point definition"""
    point_id: str
    name: str
    description: str
    reverse: int
    data_type: str

@dataclass
class AdjustmentPoint:
    """Adjustment point definition"""
    point_id: str
    name: str
    description: str
    unit: str
    data_type: str
    scale: float
    offset: float

@dataclass
class ModbusMapping:
    """Generic Modbus mapping"""
    point_id: str
    register_address: int
    function_code: int
    slave_id: int
    data_format: str
    bit_position: int = None
    byte_order: str = None
    register_count: int = 1

class CSVConfigLoader:
    """Load configuration from CSV files"""
    
    def __init__(self, config_dir: str):
        self.config_dir = Path(config_dir)
        self.telemetry_points = {}
        self.signal_points = {}
        self.control_points = {}
        self.adjustment_points = {}
        self.telemetry_mappings = {}
        self.signal_mappings = {}
        self.control_mappings = {}
        self.adjustment_mappings = {}
        
    def load_all(self):
        """Load all CSV configuration files"""
        logger.info(f"Loading CSV configuration from {self.config_dir}")
        
        # Load point definitions
        self._load_telemetry_points()
        self._load_signal_points()
        self._load_control_points()
        self._load_adjustment_points()
        
        # Load mappings
        self._load_telemetry_mappings()
        self._load_signal_mappings()
        self._load_control_mappings()
        self._load_adjustment_mappings()
        
        logger.info(f"Loaded {len(self.telemetry_points)} telemetry points")
        logger.info(f"Loaded {len(self.signal_points)} signal points")
        logger.info(f"Loaded {len(self.control_points)} control points")
        logger.info(f"Loaded {len(self.adjustment_points)} adjustment points")
        
    def _load_telemetry_points(self):
        """Load telemetry point definitions"""
        file_path = self.config_dir / "telemetry.csv"
        if not file_path.exists():
            logger.warning(f"Telemetry file not found: {file_path}")
            return
            
        with open(file_path, 'r', encoding='utf-8') as f:
            reader = csv.DictReader(f)
            for row in reader:
                point = TelemetryPoint(
                    point_id=row['point_id'],
                    name=row['name'],
                    description=row['description'],
                    unit=row['unit'],
                    data_type=row['data_type'],
                    scale=float(row['scale']),
                    offset=float(row['offset'])
                )
                self.telemetry_points[point.point_id] = point
                
    def _load_signal_points(self):
        """Load signal point definitions"""
        file_path = self.config_dir / "signal.csv"
        if not file_path.exists():
            logger.warning(f"Signal file not found: {file_path}")
            return
            
        with open(file_path, 'r', encoding='utf-8') as f:
            reader = csv.DictReader(f)
            for row in reader:
                point = SignalPoint(
                    point_id=row['point_id'],
                    name=row['name'],
                    description=row['description'],
                    data_type=row['data_type'],
                    reverse=int(row['reverse'])
                )
                self.signal_points[point.point_id] = point
                
    def _load_control_points(self):
        """Load control point definitions"""
        file_path = self.config_dir / "control.csv"
        if not file_path.exists():
            logger.warning(f"Control file not found: {file_path}")
            return
            
        with open(file_path, 'r', encoding='utf-8') as f:
            reader = csv.DictReader(f)
            for row in reader:
                point = ControlPoint(
                    point_id=row['point_id'],
                    name=row['name'],
                    description=row['description'],
                    reverse=int(row['reverse']),
                    data_type=row['data_type']
                )
                self.control_points[point.point_id] = point
                
    def _load_adjustment_points(self):
        """Load adjustment point definitions"""
        file_path = self.config_dir / "adjustment.csv"
        if not file_path.exists():
            logger.warning(f"Adjustment file not found: {file_path}")
            return
            
        with open(file_path, 'r', encoding='utf-8') as f:
            reader = csv.DictReader(f)
            for row in reader:
                point = AdjustmentPoint(
                    point_id=row['point_id'],
                    name=row['name'],
                    description=row['description'],
                    unit=row['unit'],
                    data_type=row['data_type'],
                    scale=float(row['scale']),
                    offset=float(row['offset'])
                )
                self.adjustment_points[point.point_id] = point
                
    def _load_telemetry_mappings(self):
        """Load telemetry mappings"""
        file_path = self.config_dir / "mapping_telemetry.csv"
        if not file_path.exists():
            logger.warning(f"Telemetry mapping file not found: {file_path}")
            return
            
        with open(file_path, 'r', encoding='utf-8') as f:
            reader = csv.DictReader(f)
            for row in reader:
                mapping = ModbusMapping(
                    point_id=row['point_id'],
                    register_address=int(row['register_address']),
                    function_code=int(row['function_code']),
                    slave_id=int(row['slave_id']),
                    data_format=row['data_format'],
                    byte_order=row.get('byte_order', 'ABCD'),
                    register_count=int(row.get('register_count', 1))
                )
                self.telemetry_mappings[mapping.point_id] = mapping
                
    def _load_signal_mappings(self):
        """Load signal mappings"""
        file_path = self.config_dir / "mapping_signal.csv"
        if not file_path.exists():
            logger.warning(f"Signal mapping file not found: {file_path}")
            return
            
        with open(file_path, 'r', encoding='utf-8') as f:
            reader = csv.DictReader(f)
            for row in reader:
                mapping = ModbusMapping(
                    point_id=row['point_id'],
                    register_address=int(row['register_address']),
                    function_code=int(row['function_code']),
                    slave_id=int(row['slave_id']),
                    data_format=row['data_format'],
                    bit_position=int(row.get('bit_position', 0))
                )
                self.signal_mappings[mapping.point_id] = mapping
                
    def _load_control_mappings(self):
        """Load control mappings"""
        file_path = self.config_dir / "mapping_control.csv"
        if not file_path.exists():
            logger.warning(f"Control mapping file not found: {file_path}")
            return
            
        with open(file_path, 'r', encoding='utf-8') as f:
            reader = csv.DictReader(f)
            for row in reader:
                mapping = ModbusMapping(
                    point_id=row['point_id'],
                    register_address=int(row['register_address']),
                    function_code=int(row['function_code']),
                    slave_id=int(row['slave_id']),
                    data_format=row['data_format'],
                    bit_position=int(row.get('bit_position', 0))
                )
                self.control_mappings[mapping.point_id] = mapping
                
    def _load_adjustment_mappings(self):
        """Load adjustment mappings"""
        file_path = self.config_dir / "mapping_adjustment.csv"
        if not file_path.exists():
            logger.warning(f"Adjustment mapping file not found: {file_path}")
            return
            
        with open(file_path, 'r', encoding='utf-8') as f:
            reader = csv.DictReader(f)
            for row in reader:
                mapping = ModbusMapping(
                    point_id=row['point_id'],
                    register_address=int(row['register_address']),
                    function_code=int(row['function_code']),
                    slave_id=int(row['slave_id']),
                    data_format=row['data_format'],
                    byte_order=row.get('byte_order', 'ABCD'),
                    register_count=int(row.get('register_count', 2))
                )
                self.adjustment_mappings[mapping.point_id] = mapping

class ModbusDataStore:
    """Modbus data storage based on CSV configuration"""
    
    def __init__(self, config_loader: CSVConfigLoader):
        self.config = config_loader
        # Initialize data stores by slave ID
        self.coils = {}  # FC01, FC05, FC15
        self.discrete_inputs = {}  # FC02
        self.holding_registers = {}  # FC03, FC06, FC16
        self.input_registers = {}  # FC04
        
        # Initialize data based on mappings
        self._initialize_data()
        
    def _initialize_data(self):
        """Initialize data values based on CSV mappings"""
        
        # Initialize telemetry points
        for point_id, mapping in self.config.telemetry_mappings.items():
            point = self.config.telemetry_points.get(point_id)
            if not point:
                continue
                
            value = self._generate_initial_value(point_id, 'telemetry')
            
            if mapping.function_code == 3:  # Holding registers
                self._ensure_slave_dict(self.holding_registers, mapping.slave_id)
                if mapping.data_format == 'float32' and mapping.register_count == 2:
                    self._write_float32(self.holding_registers[mapping.slave_id], 
                                      mapping.register_address, value)
                else:
                    self.holding_registers[mapping.slave_id][mapping.register_address] = int(value)
            elif mapping.function_code == 4:  # Input registers
                self._ensure_slave_dict(self.input_registers, mapping.slave_id)
                self.input_registers[mapping.slave_id][mapping.register_address] = int(value)
                
        # Initialize signal points
        for point_id, mapping in self.config.signal_mappings.items():
            value = self._generate_initial_value(point_id, 'signal')
            
            if mapping.function_code == 1:  # Coils
                self._ensure_slave_dict(self.coils, mapping.slave_id)
                self.coils[mapping.slave_id][mapping.register_address] = bool(value)
            elif mapping.function_code == 2:  # Discrete inputs
                self._ensure_slave_dict(self.discrete_inputs, mapping.slave_id)
                self.discrete_inputs[mapping.slave_id][mapping.register_address] = bool(value)
            elif mapping.function_code == 3:  # Signal stored in holding register bits
                self._ensure_slave_dict(self.holding_registers, mapping.slave_id)
                if mapping.register_address not in self.holding_registers[mapping.slave_id]:
                    self.holding_registers[mapping.slave_id][mapping.register_address] = 0
                if value:
                    self.holding_registers[mapping.slave_id][mapping.register_address] |= (1 << mapping.bit_position)
                    
        # Initialize control points
        for point_id, mapping in self.config.control_mappings.items():
            if mapping.function_code == 5 or mapping.function_code == 15:  # Coils
                self._ensure_slave_dict(self.coils, mapping.slave_id)
                self.coils[mapping.slave_id][mapping.register_address] = False
                
        # Initialize adjustment points
        for point_id, mapping in self.config.adjustment_mappings.items():
            point = self.config.adjustment_points.get(point_id)
            if not point:
                continue
                
            value = self._generate_initial_value(point_id, 'adjustment')
            
            if mapping.function_code == 16:  # Write multiple registers
                self._ensure_slave_dict(self.holding_registers, mapping.slave_id)
                if mapping.data_format == 'float32':
                    self._write_float32(self.holding_registers[mapping.slave_id], 
                                      mapping.register_address, float(value))
                    
    def _ensure_slave_dict(self, store: dict, slave_id: int):
        """Ensure slave dictionary exists"""
        if slave_id not in store:
            store[slave_id] = {}
            
    def _generate_initial_value(self, point_id: str, point_type: str):
        """Generate realistic initial values"""
        # Telemetry values
        telemetry_values = {
            '1001': 220.0,    # 电压A相
            '1002': 15.0,     # 电流A相
            '1003': 3300.0,   # 有功功率
            '1004': 1100.0,   # 无功功率
            '1005': 50.0,     # 频率
            '1006': 25.0,     # 温度
            '1007': 65.0,     # 湿度
            '1008': 1200.0,   # 油位
        }
        
        # Adjustment values
        adjustment_values = {
            '3001': 380.0,    # 电压设定值
            '3002': 0.95,     # 功率因数设定
            '3003': 25.0,     # 温度设定值
            '3004': 60.0,     # 湿度设定值
            '3005': 5000.0,   # 负荷限制值
        }
        
        if point_type == 'telemetry':
            return telemetry_values.get(point_id, 100.0)
        elif point_type == 'signal':
            return random.choice([True, False])
        elif point_type == 'adjustment':
            return adjustment_values.get(point_id, 50.0)
        else:
            return 0
            
    def _write_float32(self, registers: dict, address: int, value: float):
        """Write float32 value to two consecutive registers"""
        bytes_data = struct.pack('>f', value)  # Big-endian float
        high_word = struct.unpack('>H', bytes_data[0:2])[0]
        low_word = struct.unpack('>H', bytes_data[2:4])[0]
        registers[address] = high_word
        registers[address + 1] = low_word
        
    def _read_float32(self, registers: dict, address: int) -> float:
        """Read float32 value from two consecutive registers"""
        high_word = registers.get(address, 0)
        low_word = registers.get(address + 1, 0)
        bytes_data = struct.pack('>HH', high_word, low_word)
        return struct.unpack('>f', bytes_data)[0]
        
    async def update_values(self):
        """Update values with sine wave patterns"""
        t = time.time()
        
        # Update telemetry values
        for point_id, mapping in self.config.telemetry_mappings.items():
            base_value = self._generate_initial_value(point_id, 'telemetry')
            variation = base_value * 0.1  # 10% variation
            value = base_value + variation * math.sin(t * 0.1 + hash(point_id) % 10)
            
            if mapping.function_code == 3:  # Holding registers
                if mapping.slave_id in self.holding_registers:
                    if mapping.data_format == 'float32':
                        self._write_float32(self.holding_registers[mapping.slave_id], 
                                          mapping.register_address, value)
                    else:
                        self.holding_registers[mapping.slave_id][mapping.register_address] = int(value)
            elif mapping.function_code == 4:  # Input registers
                if mapping.slave_id in self.input_registers:
                    self.input_registers[mapping.slave_id][mapping.register_address] = int(value)
                    
        # Randomly toggle some signals
        if random.random() < 0.05:  # 5% chance per update
            signal_ids = list(self.config.signal_mappings.keys())
            if signal_ids:
                toggle_id = random.choice(signal_ids)
                mapping = self.config.signal_mappings[toggle_id]
                
                if mapping.function_code == 1 and mapping.slave_id in self.coils:
                    current = self.coils[mapping.slave_id].get(mapping.register_address, False)
                    self.coils[mapping.slave_id][mapping.register_address] = not current
                    logger.info(f"Toggled signal {toggle_id} to {not current}")

def format_hex(data: bytes) -> str:
    """Format bytes as hex string"""
    return ' '.join(f'{b:02X}' for b in data)

class ModbusTCPProtocol(asyncio.Protocol):
    """Modbus TCP protocol handler"""
    
    def __init__(self, data_store: ModbusDataStore):
        self.data_store = data_store
        self.transport = None
        self.buffer = b''
        
    def connection_made(self, transport):
        self.transport = transport
        peer = transport.get_extra_info('peername')
        logger.info(f"Client connected from {peer}")
        
    def connection_lost(self, exc):
        peer = self.transport.get_extra_info('peername')
        logger.info(f"Client disconnected from {peer}")
        
    def data_received(self, data):
        self.buffer += data
        
        while len(self.buffer) >= 12:  # Minimum MBAP + function code
            # Parse MBAP header
            if len(self.buffer) < 7:
                break
                
            transaction_id = struct.unpack('>H', self.buffer[0:2])[0]
            protocol_id = struct.unpack('>H', self.buffer[2:4])[0]
            length = struct.unpack('>H', self.buffer[4:6])[0]
            unit_id = self.buffer[6]
            
            if protocol_id != 0:  # Must be 0 for Modbus
                self.buffer = self.buffer[1:]  # Skip bad byte
                continue
                
            if len(self.buffer) < 6 + length:
                break  # Wait for complete message
                
            # Log request in hex format (before modifying buffer)
            request_data = self.buffer[0:6+length]
            logger.info(f"RX: {format_hex(request_data)}")
            
            # Extract PDU
            pdu = self.buffer[7:6+length]
            self.buffer = self.buffer[6+length:]
            
            # Process request
            response_pdu = self._process_pdu(unit_id, pdu)
            
            # Build response
            response = self._build_response(transaction_id, unit_id, response_pdu)
            self.transport.write(response)
            
            # Log response in hex format
            logger.info(f"TX: {format_hex(response)}")
            
            # Log request details
            function_code = pdu[0] if pdu else 0
            logger.debug(f"Request: Trans={transaction_id}, Unit={unit_id}, "
                        f"FC={function_code:02X}, Response length={len(response)}")
    
    def _process_pdu(self, unit_id: int, pdu: bytes) -> bytes:
        """Process Modbus PDU and return response PDU"""
        if not pdu:
            return self._build_exception(0x01, 0x01)  # Illegal function
            
        function_code = pdu[0]
        
        try:
            if function_code == 0x01:  # Read Coils
                return self._read_coils(unit_id, pdu)
            elif function_code == 0x02:  # Read Discrete Inputs
                return self._read_discrete_inputs(unit_id, pdu)
            elif function_code == 0x03:  # Read Holding Registers
                return self._read_holding_registers(unit_id, pdu)
            elif function_code == 0x04:  # Read Input Registers
                return self._read_input_registers(unit_id, pdu)
            elif function_code == 0x05:  # Write Single Coil
                return self._write_single_coil(unit_id, pdu)
            elif function_code == 0x06:  # Write Single Register
                return self._write_single_register(unit_id, pdu)
            elif function_code == 0x0F:  # Write Multiple Coils
                return self._write_multiple_coils(unit_id, pdu)
            elif function_code == 0x10:  # Write Multiple Registers
                return self._write_multiple_registers(unit_id, pdu)
            else:
                return self._build_exception(function_code, 0x01)  # Illegal function
                
        except Exception as e:
            logger.error(f"Error processing FC {function_code:02X}: {e}")
            return self._build_exception(function_code, 0x04)  # Slave device failure
    
    def _read_coils(self, unit_id: int, pdu: bytes) -> bytes:
        """FC01 - Read Coils"""
        if len(pdu) < 5:
            return self._build_exception(0x01, 0x03)  # Illegal data value
            
        start_address = struct.unpack('>H', pdu[1:3])[0]
        quantity = struct.unpack('>H', pdu[3:5])[0]
        
        if quantity < 1 or quantity > 2000:
            return self._build_exception(0x01, 0x03)
            
        # Get coil values
        coils = self.data_store.coils.get(unit_id, {})
        byte_count = (quantity + 7) // 8
        coil_bytes = bytearray(byte_count)
        
        for i in range(quantity):
            addr = start_address + i
            if addr in coils and coils[addr]:
                byte_idx = i // 8
                bit_idx = i % 8
                coil_bytes[byte_idx] |= (1 << bit_idx)
        
        return bytes([0x01, byte_count]) + bytes(coil_bytes)
    
    def _read_discrete_inputs(self, unit_id: int, pdu: bytes) -> bytes:
        """FC02 - Read Discrete Inputs"""
        if len(pdu) < 5:
            return self._build_exception(0x02, 0x03)
            
        start_address = struct.unpack('>H', pdu[1:3])[0]
        quantity = struct.unpack('>H', pdu[3:5])[0]
        
        if quantity < 1 or quantity > 2000:
            return self._build_exception(0x02, 0x03)
            
        # Get discrete input values
        inputs = self.data_store.discrete_inputs.get(unit_id, {})
        byte_count = (quantity + 7) // 8
        input_bytes = bytearray(byte_count)
        
        for i in range(quantity):
            addr = start_address + i
            if addr in inputs and inputs[addr]:
                byte_idx = i // 8
                bit_idx = i % 8
                input_bytes[byte_idx] |= (1 << bit_idx)
        
        return bytes([0x02, byte_count]) + bytes(input_bytes)
    
    def _read_holding_registers(self, unit_id: int, pdu: bytes) -> bytes:
        """FC03 - Read Holding Registers"""
        if len(pdu) < 5:
            return self._build_exception(0x03, 0x03)
            
        start_address = struct.unpack('>H', pdu[1:3])[0]
        quantity = struct.unpack('>H', pdu[3:5])[0]
        
        logger.debug(f"Read holding registers: slave={unit_id}, addr={start_address}, count={quantity}")
        
        if quantity < 1 or quantity > 125:
            return self._build_exception(0x03, 0x03)
            
        # Get register values
        registers = self.data_store.holding_registers.get(unit_id, {})
        byte_count = quantity * 2
        register_bytes = bytearray()
        
        for i in range(quantity):
            addr = start_address + i
            value = registers.get(addr, 0)
            register_bytes.extend(struct.pack('>H', value))
            logger.debug(f"  Register {addr}: {value} (0x{value:04X})")
        
        return bytes([0x03, byte_count]) + bytes(register_bytes)
    
    def _read_input_registers(self, unit_id: int, pdu: bytes) -> bytes:
        """FC04 - Read Input Registers"""
        if len(pdu) < 5:
            return self._build_exception(0x04, 0x03)
            
        start_address = struct.unpack('>H', pdu[1:3])[0]
        quantity = struct.unpack('>H', pdu[3:5])[0]
        
        logger.debug(f"Read input registers: slave={unit_id}, addr={start_address}, count={quantity}")
        
        if quantity < 1 or quantity > 125:
            return self._build_exception(0x04, 0x03)
            
        # Get register values
        registers = self.data_store.input_registers.get(unit_id, {})
        byte_count = quantity * 2
        register_bytes = bytearray()
        
        for i in range(quantity):
            addr = start_address + i
            value = registers.get(addr, 0)
            register_bytes.extend(struct.pack('>H', value))
            logger.debug(f"  Register {addr}: {value} (0x{value:04X})")
        
        return bytes([0x04, byte_count]) + bytes(register_bytes)
    
    def _write_single_coil(self, unit_id: int, pdu: bytes) -> bytes:
        """FC05 - Write Single Coil"""
        if len(pdu) < 5:
            return self._build_exception(0x05, 0x03)
            
        address = struct.unpack('>H', pdu[1:3])[0]
        value = struct.unpack('>H', pdu[3:5])[0]
        
        if value not in [0x0000, 0xFF00]:
            return self._build_exception(0x05, 0x03)
            
        # Write coil
        if unit_id not in self.data_store.coils:
            self.data_store.coils[unit_id] = {}
        self.data_store.coils[unit_id][address] = (value == 0xFF00)
        
        logger.info(f"Write coil: Unit={unit_id}, Addr={address}, Value={value == 0xFF00}")
        
        return pdu  # Echo request
    
    def _write_single_register(self, unit_id: int, pdu: bytes) -> bytes:
        """FC06 - Write Single Register"""
        if len(pdu) < 5:
            return self._build_exception(0x06, 0x03)
            
        address = struct.unpack('>H', pdu[1:3])[0]
        value = struct.unpack('>H', pdu[3:5])[0]
        
        # Write register
        if unit_id not in self.data_store.holding_registers:
            self.data_store.holding_registers[unit_id] = {}
        self.data_store.holding_registers[unit_id][address] = value
        
        logger.info(f"Write register: Unit={unit_id}, Addr={address}, Value={value}")
        
        return pdu  # Echo request
    
    def _write_multiple_coils(self, unit_id: int, pdu: bytes) -> bytes:
        """FC15 - Write Multiple Coils"""
        if len(pdu) < 6:
            return self._build_exception(0x0F, 0x03)
            
        start_address = struct.unpack('>H', pdu[1:3])[0]
        quantity = struct.unpack('>H', pdu[3:5])[0]
        byte_count = pdu[5]
        
        expected_bytes = (quantity + 7) // 8
        if byte_count != expected_bytes or len(pdu) < 6 + byte_count:
            return self._build_exception(0x0F, 0x03)
            
        # Write coils
        if unit_id not in self.data_store.coils:
            self.data_store.coils[unit_id] = {}
            
        coil_bytes = pdu[6:6+byte_count]
        for i in range(quantity):
            byte_idx = i // 8
            bit_idx = i % 8
            value = bool(coil_bytes[byte_idx] & (1 << bit_idx))
            self.data_store.coils[unit_id][start_address + i] = value
        
        logger.info(f"Write multiple coils: Unit={unit_id}, Start={start_address}, Count={quantity}")
        
        return pdu[:5]  # Echo function code, address and quantity
    
    def _write_multiple_registers(self, unit_id: int, pdu: bytes) -> bytes:
        """FC16 - Write Multiple Registers"""
        if len(pdu) < 6:
            return self._build_exception(0x10, 0x03)
            
        start_address = struct.unpack('>H', pdu[1:3])[0]
        quantity = struct.unpack('>H', pdu[3:5])[0]
        byte_count = pdu[5]
        
        if byte_count != quantity * 2 or len(pdu) < 6 + byte_count:
            return self._build_exception(0x10, 0x03)
            
        # Write registers
        if unit_id not in self.data_store.holding_registers:
            self.data_store.holding_registers[unit_id] = {}
            
        for i in range(quantity):
            offset = 6 + i * 2
            value = struct.unpack('>H', pdu[offset:offset+2])[0]
            self.data_store.holding_registers[unit_id][start_address + i] = value
        
        logger.info(f"Write multiple registers: Unit={unit_id}, Start={start_address}, Count={quantity}")
        
        # Special handling for float32 values (adjustment points)
        if quantity == 2:
            # Check if this is an adjustment point
            for point_id, mapping in self.data_store.config.adjustment_mappings.items():
                if (mapping.slave_id == unit_id and 
                    mapping.register_address == start_address and
                    mapping.data_format == 'float32'):
                    float_value = self.data_store._read_float32(
                        self.data_store.holding_registers[unit_id], start_address)
                    logger.info(f"  Adjustment point {point_id}: {float_value}")
                    break
        
        return pdu[:5]  # Echo function code, address and quantity
    
    def _build_exception(self, function_code: int, exception_code: int) -> bytes:
        """Build exception response"""
        return bytes([function_code | 0x80, exception_code])
    
    def _build_response(self, transaction_id: int, unit_id: int, pdu: bytes) -> bytes:
        """Build complete Modbus TCP response"""
        mbap = struct.pack('>HHHB', 
                          transaction_id,  # Transaction ID
                          0,               # Protocol ID (always 0)
                          len(pdu) + 1,    # Length
                          unit_id)         # Unit ID
        return mbap + pdu

class ModbusTCPServer:
    """Modbus TCP Server based on CSV configuration"""
    
    def __init__(self, host='0.0.0.0', port=5020, config_dir='config/test_points/ModbusTCP_Demo'):
        self.host = host
        self.port = port
        self.config_loader = CSVConfigLoader(config_dir)
        self.data_store = None
        self.server = None
        self.update_task = None
        
    async def start(self):
        """Start the server"""
        # Load configuration
        self.config_loader.load_all()
        self.data_store = ModbusDataStore(self.config_loader)
        
        # Start data update task
        self.update_task = asyncio.create_task(self._update_loop())
        
        # Create server
        loop = asyncio.get_event_loop()
        self.server = await loop.create_server(
            lambda: ModbusTCPProtocol(self.data_store),
            self.host,
            self.port
        )
        
        addr = self.server.sockets[0].getsockname()
        logger.info(f"Modbus TCP server listening on {addr[0]}:{addr[1]}")
        logger.info("Configuration loaded from CSV files:")
        logger.info(f"  - Telemetry: {len(self.config_loader.telemetry_points)} points")
        logger.info(f"  - Signal: {len(self.config_loader.signal_points)} points")
        logger.info(f"  - Control: {len(self.config_loader.control_points)} points")
        logger.info(f"  - Adjustment: {len(self.config_loader.adjustment_points)} points")
        
    async def stop(self):
        """Stop the server"""
        if self.update_task:
            self.update_task.cancel()
            try:
                await self.update_task
            except asyncio.CancelledError:
                pass
                
        if self.server:
            self.server.close()
            await self.server.wait_closed()
            
    async def _update_loop(self):
        """Background task to update values"""
        while True:
            try:
                await asyncio.sleep(1)  # Update every second
                await self.data_store.update_values()
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error in update loop: {e}")
                
    def print_status(self):
        """Print current data values"""
        print("\n=== Current Data Values ===")
        
        # Print telemetry values
        print("\n遥测 (Telemetry) Points:")
        for point_id, mapping in self.config_loader.telemetry_mappings.items():
            point = self.config_loader.telemetry_points.get(point_id)
            if not point:
                continue
                
            if mapping.function_code == 3:  # Holding registers
                registers = self.data_store.holding_registers.get(mapping.slave_id, {})
                if mapping.data_format == 'float32':
                    value = self.data_store._read_float32(registers, mapping.register_address)
                else:
                    value = registers.get(mapping.register_address, 0)
            elif mapping.function_code == 4:  # Input registers
                registers = self.data_store.input_registers.get(mapping.slave_id, {})
                value = registers.get(mapping.register_address, 0)
            else:
                value = 0
                
            print(f"  {point_id} ({point.name}): {value} {point.unit}")
            
        # Print signal values
        print("\n遥信 (Signal) Points:")
        for point_id, mapping in self.config_loader.signal_mappings.items():
            point = self.config_loader.signal_points.get(point_id)
            if not point:
                continue
                
            if mapping.function_code == 1:  # Coils
                coils = self.data_store.coils.get(mapping.slave_id, {})
                value = coils.get(mapping.register_address, False)
            elif mapping.function_code == 2:  # Discrete inputs
                inputs = self.data_store.discrete_inputs.get(mapping.slave_id, {})
                value = inputs.get(mapping.register_address, False)
            elif mapping.function_code == 3:  # Holding register bits
                registers = self.data_store.holding_registers.get(mapping.slave_id, {})
                reg_value = registers.get(mapping.register_address, 0)
                value = bool(reg_value & (1 << mapping.bit_position))
            else:
                value = False
                
            print(f"  {point_id} ({point.name}): {value}")

async def main():
    """Main function"""
    parser = argparse.ArgumentParser(description='Modbus TCP Server Simulator based on CSV configuration')
    parser.add_argument('--host', default='0.0.0.0', help='Host to bind to')
    parser.add_argument('--port', type=int, default=5020, help='Port to listen on')
    parser.add_argument('--config-dir', default='config/test_points/ModbusTCP_Demo', 
                       help='Directory containing CSV configuration files')
    parser.add_argument('--debug', action='store_true', help='Enable debug logging')
    
    args = parser.parse_args()
    
    if args.debug:
        logger.setLevel(logging.DEBUG)
    
    server = ModbusTCPServer(args.host, args.port, args.config_dir)
    
    try:
        await server.start()
        
        # Print status periodically
        while True:
            await asyncio.sleep(10)
            server.print_status()
            
    except KeyboardInterrupt:
        logger.info("Shutting down...")
    finally:
        await server.stop()

if __name__ == '__main__':
    asyncio.run(main())