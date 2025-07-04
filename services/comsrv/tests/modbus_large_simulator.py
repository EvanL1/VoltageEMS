#!/usr/bin/env python3
"""
Enhanced Modbus TCP Server Simulator for Large-Scale Testing
Supports thousands of points with realistic data simulation
"""

import asyncio
import struct
import random
import math
import time
import csv
import os
import argparse
from collections import defaultdict
import logging

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger('ModbusLargeSimulator')

class ModbusLargeSimulator:
    def __init__(self, csv_dir, host='0.0.0.0', port=5030):
        self.csv_dir = csv_dir
        self.host = host
        self.port = port
        self.server = None
        
        # Data storage by slave ID
        self.holding_registers = defaultdict(lambda: [0] * 65536)
        self.input_registers = defaultdict(lambda: [0] * 65536)
        self.coils = defaultdict(lambda: [False] * 65536)
        self.discrete_inputs = defaultdict(lambda: [False] * 65536)
        
        # Point mappings
        self.telemetry_mappings = {}
        self.signal_mappings = {}
        self.control_mappings = {}
        self.adjustment_mappings = {}
        
        # Performance counters
        self.request_count = 0
        self.start_time = time.time()
        self.slave_request_count = defaultdict(int)
        
        # Load configuration
        self.load_csv_config()
        
        # Initialize data
        self.initialize_data()

    def load_csv_config(self):
        """Load point mappings from CSV files"""
        # Load telemetry mappings
        telemetry_file = os.path.join(self.csv_dir, 'mapping_telemetry.csv')
        if os.path.exists(telemetry_file):
            with open(telemetry_file, 'r') as f:
                reader = csv.DictReader(f)
                for row in reader:
                    point_id = int(row['point_id'])
                    # Parse address: slave_id:function_code:register_address
                    addr_parts = row['address'].split(':')
                    slave_id = int(addr_parts[0])
                    register = int(addr_parts[2])
                    
                    self.telemetry_mappings[point_id] = {
                        'slave_id': slave_id,
                        'register': register,
                        'data_type': row['data_type'],
                        'scale': float(row.get('scale', 1)),
                        'name': row['signal_name']
                    }
            logger.info(f"Loaded {len(self.telemetry_mappings)} telemetry mappings")
        
        # Load signal mappings
        signal_file = os.path.join(self.csv_dir, 'mapping_signal.csv')
        if os.path.exists(signal_file):
            with open(signal_file, 'r') as f:
                reader = csv.DictReader(f)
                for row in reader:
                    point_id = int(row['point_id'])
                    addr_parts = row['address'].split(':')
                    slave_id = int(addr_parts[0])
                    coil = int(addr_parts[2])
                    
                    self.signal_mappings[point_id] = {
                        'slave_id': slave_id,
                        'coil': coil,
                        'name': row['signal_name']
                    }
            logger.info(f"Loaded {len(self.signal_mappings)} signal mappings")

    def initialize_data(self):
        """Initialize data with realistic values"""
        current_time = time.time()
        
        # Initialize telemetry data
        for point_id, mapping in self.telemetry_mappings.items():
            slave_id = mapping['slave_id']
            register = mapping['register']
            data_type = mapping['data_type']
            
            # Generate realistic values based on point type
            if 'Voltage' in mapping['name']:
                # Voltage: 220V ± 10V with sine wave variation
                base_value = 220.0
                variation = 10.0 * math.sin(2 * math.pi * current_time / 60)
                value = base_value + variation + random.uniform(-1, 1)
            elif 'Current' in mapping['name']:
                # Current: 10A ± 2A with load variation
                base_value = 10.0
                variation = 2.0 * math.sin(2 * math.pi * current_time / 30)
                value = base_value + variation + random.uniform(-0.5, 0.5)
            elif 'Power' in mapping['name']:
                # Power: 2.2kW ± 0.5kW
                base_value = 2200.0
                variation = 500.0 * math.sin(2 * math.pi * current_time / 45)
                value = base_value + variation + random.uniform(-50, 50)
            else:
                # Generic sensor data
                value = 100.0 + 20.0 * math.sin(2 * math.pi * current_time / 120) + random.uniform(-5, 5)
            
            # Apply scale
            scaled_value = int(value / mapping['scale'])
            
            # Store based on data type
            if data_type == 'uint16':
                self.holding_registers[slave_id][register - 40001] = scaled_value & 0xFFFF
            elif data_type == 'int16':
                if scaled_value < 0:
                    scaled_value = 0x10000 + scaled_value
                self.holding_registers[slave_id][register - 40001] = scaled_value & 0xFFFF
            elif data_type == 'float32':
                # Store as two registers (big-endian)
                float_bytes = struct.pack('>f', value)
                reg_values = struct.unpack('>HH', float_bytes)
                self.holding_registers[slave_id][register - 40001] = reg_values[0]
                self.holding_registers[slave_id][register - 40000] = reg_values[1]
        
        # Initialize signal data
        for point_id, mapping in self.signal_mappings.items():
            slave_id = mapping['slave_id']
            coil = mapping['coil']
            
            # Random status with some patterns
            if point_id % 10 == 0:
                # Every 10th signal is always ON
                value = True
            elif point_id % 7 == 0:
                # Every 7th signal is always OFF
                value = False
            else:
                # Random status
                value = random.choice([True, False])
            
            self.coils[slave_id][coil - 10001] = value
        
        logger.info("Data initialization complete")

    async def update_data(self):
        """Continuously update data to simulate real device behavior"""
        while True:
            await asyncio.sleep(1)  # Update every second
            current_time = time.time()
            
            # Update subset of telemetry points
            update_count = min(100, len(self.telemetry_mappings))
            points_to_update = random.sample(list(self.telemetry_mappings.items()), update_count)
            
            for point_id, mapping in points_to_update:
                slave_id = mapping['slave_id']
                register = mapping['register']
                
                # Get current value and add small variation
                current_val = self.holding_registers[slave_id][register - 40001]
                
                # Add realistic variation
                if 'Voltage' in mapping['name']:
                    variation = random.uniform(-2, 2)
                elif 'Current' in mapping['name']:
                    variation = random.uniform(-0.5, 0.5)
                else:
                    variation = random.uniform(-1, 1)
                
                new_val = current_val + int(variation / mapping['scale'])
                
                # Keep within reasonable bounds
                if mapping['data_type'] == 'uint16':
                    new_val = max(0, min(65535, new_val))
                else:
                    new_val = new_val & 0xFFFF
                
                self.holding_registers[slave_id][register - 40001] = new_val
            
            # Update some signals
            signal_update_count = min(50, len(self.signal_mappings))
            signals_to_update = random.sample(list(self.signal_mappings.items()), signal_update_count)
            
            for point_id, mapping in signals_to_update:
                slave_id = mapping['slave_id']
                coil = mapping['coil']
                
                # Toggle with low probability
                if random.random() < 0.1:
                    self.coils[slave_id][coil - 10001] = not self.coils[slave_id][coil - 10001]

    def build_exception_response(self, slave_id, function_code, exception_code):
        """Build Modbus exception response"""
        return struct.pack('>BBB', slave_id, function_code | 0x80, exception_code)

    def handle_read_coils(self, slave_id, data):
        """Handle FC01 - Read Coils"""
        if len(data) < 4:
            return self.build_exception_response(slave_id, 0x01, 0x03)
        
        start_addr, count = struct.unpack('>HH', data[:4])
        
        if count < 1 or count > 2000:
            return self.build_exception_response(slave_id, 0x01, 0x03)
        
        # Map to coils array
        coil_start = start_addr - 10001 if start_addr >= 10001 else start_addr
        
        if coil_start + count > 65536:
            return self.build_exception_response(slave_id, 0x01, 0x02)
        
        # Pack coils into bytes
        byte_count = (count + 7) // 8
        coil_bytes = bytearray(byte_count)
        
        for i in range(count):
            if self.coils[slave_id][coil_start + i]:
                byte_idx = i // 8
                bit_idx = i % 8
                coil_bytes[byte_idx] |= (1 << bit_idx)
        
        return struct.pack('>BBB', slave_id, 0x01, byte_count) + coil_bytes

    def handle_read_holding_registers(self, slave_id, data):
        """Handle FC03 - Read Holding Registers"""
        if len(data) < 4:
            return self.build_exception_response(slave_id, 0x03, 0x03)
        
        start_addr, count = struct.unpack('>HH', data[:4])
        
        if count < 1 or count > 125:
            return self.build_exception_response(slave_id, 0x03, 0x03)
        
        # Map to holding registers array
        reg_start = start_addr - 40001 if start_addr >= 40001 else start_addr
        
        if reg_start + count > 65536:
            return self.build_exception_response(slave_id, 0x03, 0x02)
        
        # Build response
        byte_count = count * 2
        response = struct.pack('>BBB', slave_id, 0x03, byte_count)
        
        for i in range(count):
            reg_value = self.holding_registers[slave_id][reg_start + i]
            response += struct.pack('>H', reg_value)
        
        return response

    def handle_write_single_coil(self, slave_id, data):
        """Handle FC05 - Write Single Coil"""
        if len(data) < 4:
            return self.build_exception_response(slave_id, 0x05, 0x03)
        
        addr, value = struct.unpack('>HH', data[:4])
        
        # Map to coils array
        coil_addr = addr - 10001 if addr >= 10001 else addr
        
        if coil_addr >= 65536:
            return self.build_exception_response(slave_id, 0x05, 0x02)
        
        # Set coil value
        self.coils[slave_id][coil_addr] = (value == 0xFF00)
        
        # Echo request
        return struct.pack('>BBHH', slave_id, 0x05, addr, value)

    def handle_write_single_register(self, slave_id, data):
        """Handle FC06 - Write Single Register"""
        if len(data) < 4:
            return self.build_exception_response(slave_id, 0x06, 0x03)
        
        addr, value = struct.unpack('>HH', data[:4])
        
        # Map to holding registers array
        reg_addr = addr - 40001 if addr >= 40001 else addr
        
        if reg_addr >= 65536:
            return self.build_exception_response(slave_id, 0x06, 0x02)
        
        # Set register value
        self.holding_registers[slave_id][reg_addr] = value
        
        # Echo request
        return struct.pack('>BBHH', slave_id, 0x06, addr, value)

    async def handle_client(self, reader, writer):
        """Handle client connection"""
        client_addr = writer.get_extra_info('peername')
        logger.info(f"Client connected from {client_addr}")
        
        try:
            while True:
                # Read MBAP header (7 bytes)
                header = await reader.read(7)
                if not header or len(header) < 7:
                    break
                
                trans_id, proto_id, length, unit_id = struct.unpack('>HHHB', header)
                
                # Read PDU
                pdu_data = await reader.read(length - 1)
                if not pdu_data:
                    break
                
                self.request_count += 1
                self.slave_request_count[unit_id] += 1
                
                # Process request
                function_code = pdu_data[0]
                request_data = pdu_data[1:]
                
                # Route to appropriate handler
                if function_code == 0x01:
                    response_pdu = self.handle_read_coils(unit_id, request_data)
                elif function_code == 0x03:
                    response_pdu = self.handle_read_holding_registers(unit_id, request_data)
                elif function_code == 0x05:
                    response_pdu = self.handle_write_single_coil(unit_id, request_data)
                elif function_code == 0x06:
                    response_pdu = self.handle_write_single_register(unit_id, request_data)
                else:
                    response_pdu = self.build_exception_response(unit_id, function_code, 0x01)
                
                # Build MBAP response
                response_length = len(response_pdu)
                response_header = struct.pack('>HHHB', trans_id, proto_id, response_length, unit_id)
                response = response_header + response_pdu[1:]  # Skip unit_id in PDU
                
                # Send response
                writer.write(response)
                await writer.drain()
                
                # Log every 1000th request
                if self.request_count % 1000 == 0:
                    elapsed = time.time() - self.start_time
                    rps = self.request_count / elapsed
                    logger.info(f"Processed {self.request_count} requests ({rps:.0f} req/s)")
                    
        except asyncio.CancelledError:
            pass
        except Exception as e:
            logger.error(f"Error handling client: {e}")
        finally:
            writer.close()
            await writer.wait_closed()
            logger.info(f"Client {client_addr} disconnected")

    async def run(self):
        """Run the Modbus server"""
        self.server = await asyncio.start_server(
            self.handle_client, self.host, self.port
        )
        
        addr = self.server.sockets[0].getsockname()
        logger.info(f"Modbus simulator listening on {addr[0]}:{addr[1]}")
        logger.info(f"Total points: {len(self.telemetry_mappings)} telemetry, {len(self.signal_mappings)} signals")
        
        # Start data update task
        update_task = asyncio.create_task(self.update_data())
        
        # Start statistics task
        stats_task = asyncio.create_task(self.print_statistics())
        
        try:
            await self.server.serve_forever()
        except KeyboardInterrupt:
            logger.info("Shutting down...")
        finally:
            update_task.cancel()
            stats_task.cancel()
            self.server.close()
            await self.server.wait_closed()

    async def print_statistics(self):
        """Print performance statistics periodically"""
        while True:
            await asyncio.sleep(10)  # Print every 10 seconds
            
            elapsed = time.time() - self.start_time
            if elapsed > 0:
                rps = self.request_count / elapsed
                logger.info(f"\nStatistics:")
                logger.info(f"  Total requests: {self.request_count}")
                logger.info(f"  Requests/second: {rps:.0f}")
                logger.info(f"  Active slaves: {len(self.slave_request_count)}")
                
                # Top slaves by request count
                if self.slave_request_count:
                    top_slaves = sorted(self.slave_request_count.items(), 
                                      key=lambda x: x[1], reverse=True)[:5]
                    logger.info("  Top slaves by requests:")
                    for slave_id, count in top_slaves:
                        logger.info(f"    Slave {slave_id}: {count} requests")

def main():
    parser = argparse.ArgumentParser(description='Large-scale Modbus TCP simulator')
    parser.add_argument('--csv-dir', required=True, help='Directory containing CSV configuration files')
    parser.add_argument('--host', default='0.0.0.0', help='Host to bind to')
    parser.add_argument('--port', type=int, default=5030, help='Port to listen on')
    parser.add_argument('--log-level', default='INFO', help='Logging level')
    
    args = parser.parse_args()
    
    # Configure logging
    logging.getLogger().setLevel(getattr(logging, args.log_level.upper()))
    
    # Create and run simulator
    simulator = ModbusLargeSimulator(args.csv_dir, args.host, args.port)
    
    try:
        asyncio.run(simulator.run())
    except KeyboardInterrupt:
        logger.info("Simulator stopped by user")

if __name__ == '__main__':
    main()