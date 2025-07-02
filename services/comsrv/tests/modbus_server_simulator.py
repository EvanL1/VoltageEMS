#!/usr/bin/env python3
"""
Modbus TCP Server Simulator for comsrv testing

This script simulates a Modbus TCP server that matches the comsrv configuration.
It supports all four telemetry types: YC (遥测), YX (遥信), YK (遥控), YT (遥调)

Usage:
    python modbus_server_simulator.py [--port PORT] [--host HOST]
"""

import asyncio
import struct
import logging
import argparse
import time
import math
import random
from datetime import datetime
from typing import Dict, List, Tuple, Any

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger('ModbusSimulator')

class ModbusDataStore:
    """Modbus data storage matching comsrv configuration"""
    
    def __init__(self):
        # Initialize data stores for different slave IDs
        self.coils = {1: {}, 2: {}}           # FC01, FC05, FC15 - 遥信/遥控
        self.discrete_inputs = {1: {}, 2: {}} # FC02 - 遥信输入
        self.holding_registers = {1: {}, 2: {}}  # FC03, FC06, FC16 - 遥测/遥调
        self.input_registers = {1: {}, 2: {}}    # FC04 - 遥测输入
        
        # Initialize data based on comsrv mapping configuration
        self._initialize_data()
        
        # Start value update task
        self.update_task = None
        
    def _initialize_data(self):
        """Initialize data values based on comsrv mapping files"""
        
        # 遥测点 (Telemetry) - Holding/Input Registers
        # Slave 1: addresses 1001, 1003, 1005, 1007, 1009
        for addr in [1001, 1003, 1005, 1007, 1009]:
            self.holding_registers[1][addr] = self._generate_telemetry_value(addr)
            
        # Slave 2: addresses 2001, 2002, 2003
        for addr in [2001, 2002, 2003]:
            self.input_registers[2][addr] = self._generate_telemetry_value(addr)
        
        # 遥信点 (Signaling) - Coils/Discrete Inputs
        # Slave 1: coils 1-3, discrete inputs 4-5
        for addr in range(1, 4):
            self.coils[1][addr] = random.choice([True, False])
        for addr in range(4, 6):
            self.discrete_inputs[1][addr] = random.choice([True, False])
            
        # Slave 2: holding registers 1001-1003 as bit signals
        for addr in [1001, 1002, 1003]:
            self.holding_registers[2][addr] = random.randint(0, 7)  # 3 bits
        
        # 遥控点 (Control) - Coils (writable)
        # Slave 1: addresses 1001-1005
        for addr in range(1001, 1006):
            self.coils[1][addr] = False  # Initialize as off
            
        # Slave 2: addresses 2001-2003
        for addr in range(2001, 2004):
            self.coils[2][addr] = False
        
        # 遥调点 (Adjustment) - Holding Registers (float32)
        # Slave 1: addresses 2001, 2003, 2005, 2007, 2009 (2 registers each)
        for base_addr in [2001, 2003, 2005, 2007, 2009]:
            value = self._generate_adjustment_value(base_addr)
            self._write_float32(1, base_addr, value)
    
    def _generate_telemetry_value(self, address: int) -> int:
        """Generate realistic telemetry values based on address"""
        base_values = {
            1001: 220,   # Voltage ~220V
            1003: 150,   # Current ~15A (scaled by 0.1)
            1005: 3300,  # Power ~3300W
            1007: 250,   # Temperature ~25°C (scaled by 0.1)
            1009: 500,   # Frequency ~50Hz (scaled by 0.1)
            2001: 380,   # Voltage ~380V
            2002: 100,   # Current ~10A (scaled by 0.1)
            2003: 5000,  # Power ~5000W
        }
        base = base_values.get(address, 100)
        # Add some variation
        return int(base + random.uniform(-base * 0.05, base * 0.05))
    
    def _generate_adjustment_value(self, address: int) -> float:
        """Generate adjustment values"""
        base_values = {
            2001: 50.0,   # Frequency setpoint
            2003: 380.0,  # Voltage setpoint
            2005: 100.0,  # Power setpoint
            2007: 1.0,    # Power factor setpoint
            2009: 25.0,   # Temperature setpoint
        }
        return base_values.get(address, 0.0)
    
    def _write_float32(self, slave_id: int, address: int, value: float):
        """Write float32 value to two consecutive registers (big-endian)"""
        bytes_data = struct.pack('>f', value)  # Big-endian float
        high_word = struct.unpack('>H', bytes_data[0:2])[0]
        low_word = struct.unpack('>H', bytes_data[2:4])[0]
        self.holding_registers[slave_id][address] = high_word
        self.holding_registers[slave_id][address + 1] = low_word
    
    def _read_float32(self, slave_id: int, address: int) -> float:
        """Read float32 value from two consecutive registers"""
        high_word = self.holding_registers[slave_id].get(address, 0)
        low_word = self.holding_registers[slave_id].get(address + 1, 0)
        bytes_data = struct.pack('>HH', high_word, low_word)
        return struct.unpack('>f', bytes_data)[0]
    
    async def start_updates(self):
        """Start background task to update values"""
        self.update_task = asyncio.create_task(self._update_values())
    
    async def stop_updates(self):
        """Stop background updates"""
        if self.update_task:
            self.update_task.cancel()
            try:
                await self.update_task
            except asyncio.CancelledError:
                pass
    
    async def _update_values(self):
        """Periodically update telemetry values to simulate real device"""
        while True:
            try:
                await asyncio.sleep(1)  # Update every second
                
                # Update telemetry values with sine wave patterns
                t = time.time()
                
                # Slave 1 holding registers
                self.holding_registers[1][1001] = int(220 + 10 * math.sin(t * 0.1))  # Voltage
                self.holding_registers[1][1003] = int(150 + 20 * math.sin(t * 0.2))  # Current
                self.holding_registers[1][1005] = int(3300 + 200 * math.sin(t * 0.15))  # Power
                self.holding_registers[1][1007] = int(250 + 30 * math.sin(t * 0.05))  # Temperature
                self.holding_registers[1][1009] = int(500 + 5 * math.sin(t * 0.3))  # Frequency
                
                # Slave 2 input registers
                self.input_registers[2][2001] = int(380 + 15 * math.sin(t * 0.1))
                self.input_registers[2][2002] = int(100 + 10 * math.sin(t * 0.2))
                self.input_registers[2][2003] = int(5000 + 500 * math.sin(t * 0.15))
                
                # Randomly toggle some signals
                if random.random() < 0.1:  # 10% chance per second
                    addr = random.choice([1, 2, 3])
                    self.coils[1][addr] = not self.coils[1].get(addr, False)
                    
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Error updating values: {e}")


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
                
            # Extract PDU
            pdu = self.buffer[7:6+length]
            self.buffer = self.buffer[6+length:]
            
            # Process request
            response_pdu = self._process_pdu(unit_id, pdu)
            
            # Build response
            response = self._build_response(transaction_id, unit_id, response_pdu)
            self.transport.write(response)
            
            # Log request/response
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
        
        return bytes([0x03, byte_count]) + bytes(register_bytes)
    
    def _read_input_registers(self, unit_id: int, pdu: bytes) -> bytes:
        """FC04 - Read Input Registers"""
        if len(pdu) < 5:
            return self._build_exception(0x04, 0x03)
            
        start_address = struct.unpack('>H', pdu[1:3])[0]
        quantity = struct.unpack('>H', pdu[3:5])[0]
        
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
        if start_address >= 2000 and quantity == 2:
            float_value = self.data_store._read_float32(unit_id, start_address)
            logger.info(f"  Float32 value: {float_value}")
        
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
    """Modbus TCP Server"""
    
    def __init__(self, host='0.0.0.0', port=5020):
        self.host = host
        self.port = port
        self.data_store = ModbusDataStore()
        self.server = None
        
    async def start(self):
        """Start the server"""
        # Start data updates
        await self.data_store.start_updates()
        
        # Create server
        loop = asyncio.get_event_loop()
        self.server = await loop.create_server(
            lambda: ModbusTCPProtocol(self.data_store),
            self.host,
            self.port
        )
        
        addr = self.server.sockets[0].getsockname()
        logger.info(f"Modbus TCP server listening on {addr[0]}:{addr[1]}")
        logger.info("Simulating data for:")
        logger.info("  - 遥测 (YC): Holding/Input registers")
        logger.info("  - 遥信 (YX): Coils/Discrete inputs")
        logger.info("  - 遥控 (YK): Writable coils")
        logger.info("  - 遥调 (YT): Writable holding registers (float32)")
        
    async def stop(self):
        """Stop the server"""
        if self.server:
            self.server.close()
            await self.server.wait_closed()
        await self.data_store.stop_updates()
        
    def print_status(self):
        """Print current data values"""
        print("\n=== Current Data Values ===")
        
        print("\n遥测 (Telemetry) - Slave 1 Holding Registers:")
        for addr in sorted([1001, 1003, 1005, 1007, 1009]):
            value = self.data_store.holding_registers[1].get(addr, 0)
            print(f"  Address {addr}: {value}")
            
        print("\n遥测 (Telemetry) - Slave 2 Input Registers:")
        for addr in sorted([2001, 2002, 2003]):
            value = self.data_store.input_registers[2].get(addr, 0)
            print(f"  Address {addr}: {value}")
            
        print("\n遥信 (Signaling) - Slave 1 Coils/Discrete Inputs:")
        for addr in range(1, 6):
            if addr <= 3:
                value = self.data_store.coils[1].get(addr, False)
                print(f"  Coil {addr}: {value}")
            else:
                value = self.data_store.discrete_inputs[1].get(addr, False)
                print(f"  Discrete Input {addr}: {value}")
                
        print("\n遥调 (Adjustment) - Slave 1 Float32 Values:")
        for addr in [2001, 2003, 2005, 2007, 2009]:
            value = self.data_store._read_float32(1, addr)
            print(f"  Address {addr}-{addr+1}: {value:.2f}")


async def main():
    """Main function"""
    parser = argparse.ArgumentParser(description='Modbus TCP Server Simulator for comsrv')
    parser.add_argument('--host', default='0.0.0.0', help='Host to bind to')
    parser.add_argument('--port', type=int, default=5020, help='Port to listen on')
    parser.add_argument('--debug', action='store_true', help='Enable debug logging')
    
    args = parser.parse_args()
    
    if args.debug:
        logger.setLevel(logging.DEBUG)
    
    server = ModbusTCPServer(args.host, args.port)
    
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