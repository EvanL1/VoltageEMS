#!/usr/bin/env python3
"""
Modbus TCP Server Simulator for VoltageEMS Testing

This script simulates a Modbus TCP server with data points matching
the VoltageEMS configuration.
"""

from pymodbus.server import StartTcpServer
from pymodbus.device import ModbusDeviceIdentification
from pymodbus.datastore import ModbusSequentialDataBlock, ModbusSlaveContext, ModbusServerContext
from pymodbus.version import version
import logging
import threading
import time
import random
import math
import argparse

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


class ModbusSimulator:
    """Modbus TCP server simulator with dynamic data"""
    
    def __init__(self, host='0.0.0.0', port=502):
        self.host = host
        self.port = port
        self.context = None
        self.server_thread = None
        self.running = False
        self.update_thread = None
        
    def setup_datastore(self):
        """Setup the Modbus datastore with initial values"""
        logger.info("Setting up Modbus datastore...")
        
        # Create data blocks
        # Coils (0x): Digital outputs
        coils = ModbusSequentialDataBlock(0, [False] * 100)
        
        # Discrete Inputs (1x): Digital inputs
        discrete_inputs = ModbusSequentialDataBlock(0, [False] * 100)
        
        # Input Registers (3x): Read-only analog inputs
        input_registers = ModbusSequentialDataBlock(0, [0] * 100)
        
        # Holding Registers (4x): Read/write analog values
        holding_registers = ModbusSequentialDataBlock(0, [0] * 500)
        
        # Create slave context
        slave_context = ModbusSlaveContext(
            di=discrete_inputs,
            co=coils,
            hr=holding_registers,
            ir=input_registers
        )
        
        # Create server context
        self.context = ModbusServerContext(slaves=slave_context, single=True)
        
        # Initialize some data
        self._initialize_data()
        
    def _initialize_data(self):
        """Initialize data points with realistic values"""
        logger.info("Initializing data points...")
        
        # Get the slave context
        slave = self.context[0]
        
        # Initialize holding registers with test data
        # Telemetry points (matching telemetry.csv)
        # Address 0-1: Voltage (Float32)
        self._write_float32(slave, 3, 0, 220.5)
        
        # Address 2-3: Current (Float32)
        self._write_float32(slave, 3, 2, 15.3)
        
        # Address 4: Status (UInt16)
        slave.setValues(3, 4, [1])  # Status = 1 (Running)
        
        # Address 10-11: Power (Float32)
        self._write_float32(slave, 3, 10, 3366.15)
        
        # Address 20-21: Power Factor (Float32)
        self._write_float32(slave, 3, 20, 0.95)
        
        # Address 30-31: Frequency (Float32)
        self._write_float32(slave, 3, 30, 50.01)
        
        # Address 40-41: Energy (Float32)
        self._write_float32(slave, 3, 40, 12345.67)
        
        # Signal points (coils)
        # Address 0-7: Digital inputs
        slave.setValues(1, 0, [True, False, True, True, False, False, True, False])
        
        # Address 10: Alarm status
        slave.setValues(1, 10, [False])
        
        # Address 20-23: Control states
        slave.setValues(1, 20, [True, True, False, True])
        
        # Adjustment points (holding registers 100+)
        # Address 100-101: Voltage Setpoint (Float32)
        self._write_float32(slave, 3, 100, 220.0)
        
        # Address 102-103: Current Limit (Float32)
        self._write_float32(slave, 3, 102, 20.0)
        
        # Address 104: Control Mode (UInt16)
        slave.setValues(3, 104, [1])  # 1 = Auto mode
        
        logger.info("Data initialization complete")
        
    def _write_float32(self, slave, function_code, address, value):
        """Write a float32 value to two consecutive registers"""
        import struct
        
        # Convert float to two 16-bit registers (big-endian)
        bytes_data = struct.pack('>f', value)
        reg1 = struct.unpack('>H', bytes_data[0:2])[0]
        reg2 = struct.unpack('>H', bytes_data[2:4])[0]
        
        slave.setValues(function_code, address, [reg1, reg2])
        
    def _read_float32(self, slave, function_code, address):
        """Read a float32 value from two consecutive registers"""
        import struct
        
        registers = slave.getValues(function_code, address, 2)
        bytes_data = struct.pack('>HH', registers[0], registers[1])
        return struct.unpack('>f', bytes_data)[0]
        
    def update_data(self):
        """Update data values to simulate real device behavior"""
        slave = self.context[0]
        start_time = time.time()
        
        while self.running:
            try:
                elapsed = time.time() - start_time
                
                # Update voltage with small variations
                base_voltage = 220.0
                voltage = base_voltage + 5 * math.sin(elapsed / 10) + random.uniform(-0.5, 0.5)
                self._write_float32(slave, 3, 0, voltage)
                
                # Update current with variations
                base_current = 15.0
                current = base_current + 2 * math.sin(elapsed / 8) + random.uniform(-0.2, 0.2)
                self._write_float32(slave, 3, 2, current)
                
                # Calculate and update power
                power = voltage * current
                self._write_float32(slave, 3, 10, power)
                
                # Update frequency with small variations
                frequency = 50.0 + 0.1 * math.sin(elapsed / 5) + random.uniform(-0.02, 0.02)
                self._write_float32(slave, 3, 30, frequency)
                
                # Update energy (accumulating)
                current_energy = self._read_float32(slave, 3, 40)
                new_energy = current_energy + (power / 3600)  # Add power in kWh
                self._write_float32(slave, 3, 40, new_energy)
                
                # Toggle some digital inputs randomly
                if random.random() < 0.1:  # 10% chance
                    current_state = slave.getValues(1, 0, 8)
                    bit_to_toggle = random.randint(0, 7)
                    current_state[bit_to_toggle] = not current_state[bit_to_toggle]
                    slave.setValues(1, 0, current_state)
                
                # Simulate alarm condition
                if voltage > 230 or voltage < 210:
                    slave.setValues(1, 10, [True])  # Set alarm
                else:
                    slave.setValues(1, 10, [False])  # Clear alarm
                    
                # Log current values periodically
                if int(elapsed) % 10 == 0 and elapsed - int(elapsed) < 0.1:
                    logger.info(f"Current values - Voltage: {voltage:.2f}V, "
                               f"Current: {current:.2f}A, Power: {power:.2f}W, "
                               f"Frequency: {frequency:.2f}Hz")
                    
            except Exception as e:
                logger.error(f"Error updating data: {e}")
                
            time.sleep(1)
            
    def start(self):
        """Start the Modbus server"""
        self.setup_datastore()
        
        # Start data update thread
        self.running = True
        self.update_thread = threading.Thread(target=self.update_data)
        self.update_thread.daemon = True
        self.update_thread.start()
        
        # Setup server identification
        identity = ModbusDeviceIdentification()
        identity.VendorName = 'VoltageEMS'
        identity.ProductCode = 'VEMS'
        identity.VendorUrl = 'https://github.com/pplmx/VoltageEMS'
        identity.ProductName = 'VoltageEMS Modbus Simulator'
        identity.ModelName = 'Modbus Simulator'
        identity.MajorMinorRevision = version.short()
        
        logger.info(f"Starting Modbus TCP server on {self.host}:{self.port}")
        
        # Start the server
        StartTcpServer(
            context=self.context,
            identity=identity,
            address=(self.host, self.port),
            allow_reuse_address=True
        )
        
    def stop(self):
        """Stop the Modbus server"""
        self.running = False
        if self.update_thread:
            self.update_thread.join()
        logger.info("Modbus server stopped")


def main():
    parser = argparse.ArgumentParser(description='Modbus TCP Server Simulator')
    parser.add_argument('--host', default='0.0.0.0', help='Server host address')
    parser.add_argument('--port', type=int, default=502, help='Server port')
    
    args = parser.parse_args()
    
    # Create and start simulator
    simulator = ModbusSimulator(host=args.host, port=args.port)
    
    try:
        logger.info("=== VoltageEMS Modbus TCP Server Simulator ===")
        logger.info(f"Server will listen on {args.host}:{args.port}")
        logger.info("Press Ctrl+C to stop the server")
        logger.info("")
        
        simulator.start()
        
    except KeyboardInterrupt:
        logger.info("\nShutting down server...")
        simulator.stop()
    except Exception as e:
        logger.error(f"Server error: {e}")
        

if __name__ == '__main__':
    main()