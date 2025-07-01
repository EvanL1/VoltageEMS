#!/usr/bin/env python3
"""
Modbus TCP Test Client for VoltageEMS Communication Service

This script tests the Modbus communication functionality of comsrv.
"""

from pymodbus.client import ModbusTcpClient
from pymodbus.constants import Endian
from pymodbus.payload import BinaryPayloadDecoder, BinaryPayloadBuilder
import time
import logging
import argparse
import random

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


class ModbusTestClient:
    """Test client for Modbus TCP communication"""
    
    def __init__(self, host='127.0.0.1', port=502, unit_id=1):
        self.host = host
        self.port = port
        self.unit_id = unit_id
        self.client = None
        
    def connect(self):
        """Connect to Modbus TCP server"""
        try:
            self.client = ModbusTcpClient(host=self.host, port=self.port)
            if self.client.connect():
                logger.info(f"Successfully connected to {self.host}:{self.port}")
                return True
            else:
                logger.error(f"Failed to connect to {self.host}:{self.port}")
                return False
        except Exception as e:
            logger.error(f"Connection error: {e}")
            return False
            
    def disconnect(self):
        """Disconnect from Modbus TCP server"""
        if self.client:
            self.client.close()
            logger.info("Disconnected from server")
            
    def test_read_holding_registers(self):
        """Test reading holding registers (Function Code 03)"""
        logger.info("\n=== Testing Read Holding Registers ===")
        
        # Test reading telemetry points from CSV configuration
        test_addresses = [
            (0, 2, "Voltage (Float32)"),
            (2, 2, "Current (Float32)"),
            (4, 1, "Status (UInt16)"),
            (10, 2, "Power (Float32)"),
        ]
        
        for address, count, description in test_addresses:
            try:
                result = self.client.read_holding_registers(
                    address=address,
                    count=count,
                    slave=self.unit_id
                )
                
                if not result.isError():
                    logger.info(f"Read {description} at address {address}: {result.registers}")
                    
                    # Decode float values if applicable
                    if count == 2 and "Float32" in description:
                        decoder = BinaryPayloadDecoder.fromRegisters(
                            result.registers,
                            byteorder=Endian.BIG,
                            wordorder=Endian.BIG
                        )
                        value = decoder.decode_32bit_float()
                        logger.info(f"  Decoded value: {value:.2f}")
                else:
                    logger.error(f"Error reading {description}: {result}")
                    
            except Exception as e:
                logger.error(f"Exception reading {description}: {e}")
                
            time.sleep(0.5)
            
    def test_read_input_registers(self):
        """Test reading input registers (Function Code 04)"""
        logger.info("\n=== Testing Read Input Registers ===")
        
        test_addresses = [(0, 2), (10, 1), (20, 4)]
        
        for address, count in test_addresses:
            try:
                result = self.client.read_input_registers(
                    address=address,
                    count=count,
                    slave=self.unit_id
                )
                
                if not result.isError():
                    logger.info(f"Read input registers at address {address}: {result.registers}")
                else:
                    logger.error(f"Error reading input registers at {address}: {result}")
                    
            except Exception as e:
                logger.error(f"Exception reading input registers: {e}")
                
            time.sleep(0.5)
            
    def test_read_coils(self):
        """Test reading coils (Function Code 01)"""
        logger.info("\n=== Testing Read Coils ===")
        
        # Test signal points from CSV configuration
        test_addresses = [
            (0, 8, "Digital Inputs 0-7"),
            (10, 1, "Alarm Status"),
            (20, 4, "Control States"),
        ]
        
        for address, count, description in test_addresses:
            try:
                result = self.client.read_coils(
                    address=address,
                    count=count,
                    slave=self.unit_id
                )
                
                if not result.isError():
                    logger.info(f"Read {description} at address {address}: {result.bits[:count]}")
                else:
                    logger.error(f"Error reading {description}: {result}")
                    
            except Exception as e:
                logger.error(f"Exception reading coils: {e}")
                
            time.sleep(0.5)
            
    def test_write_single_register(self):
        """Test writing single register (Function Code 06)"""
        logger.info("\n=== Testing Write Single Register ===")
        
        # Test adjustment points
        test_values = [
            (100, 1234, "Setpoint 1"),
            (101, 5678, "Setpoint 2"),
            (102, 9999, "Control Value"),
        ]
        
        for address, value, description in test_values:
            try:
                result = self.client.write_register(
                    address=address,
                    value=value,
                    slave=self.unit_id
                )
                
                if not result.isError():
                    logger.info(f"Wrote {description} value {value} to address {address}")
                    
                    # Read back to verify
                    read_result = self.client.read_holding_registers(
                        address=address,
                        count=1,
                        slave=self.unit_id
                    )
                    if not read_result.isError():
                        logger.info(f"  Read back value: {read_result.registers[0]}")
                else:
                    logger.error(f"Error writing {description}: {result}")
                    
            except Exception as e:
                logger.error(f"Exception writing register: {e}")
                
            time.sleep(0.5)
            
    def test_write_single_coil(self):
        """Test writing single coil (Function Code 05)"""
        logger.info("\n=== Testing Write Single Coil ===")
        
        # Test control points
        test_values = [
            (50, True, "Control Switch 1"),
            (51, False, "Control Switch 2"),
            (52, True, "Enable Signal"),
        ]
        
        for address, value, description in test_values:
            try:
                result = self.client.write_coil(
                    address=address,
                    value=value,
                    slave=self.unit_id
                )
                
                if not result.isError():
                    logger.info(f"Wrote {description} value {value} to coil {address}")
                    
                    # Read back to verify
                    read_result = self.client.read_coils(
                        address=address,
                        count=1,
                        slave=self.unit_id
                    )
                    if not read_result.isError():
                        logger.info(f"  Read back value: {read_result.bits[0]}")
                else:
                    logger.error(f"Error writing {description}: {result}")
                    
            except Exception as e:
                logger.error(f"Exception writing coil: {e}")
                
            time.sleep(0.5)
            
    def test_write_multiple_registers(self):
        """Test writing multiple registers (Function Code 16)"""
        logger.info("\n=== Testing Write Multiple Registers ===")
        
        # Write float32 values
        address = 200
        
        # Create a float32 value
        builder = BinaryPayloadBuilder(byteorder=Endian.BIG, wordorder=Endian.BIG)
        builder.add_32bit_float(123.456)
        payload = builder.to_registers()
        
        try:
            result = self.client.write_registers(
                address=address,
                values=payload,
                slave=self.unit_id
            )
            
            if not result.isError():
                logger.info(f"Wrote float32 value 123.456 to address {address}")
                
                # Read back and decode
                read_result = self.client.read_holding_registers(
                    address=address,
                    count=2,
                    slave=self.unit_id
                )
                if not read_result.isError():
                    decoder = BinaryPayloadDecoder.fromRegisters(
                        read_result.registers,
                        byteorder=Endian.BIG,
                        wordorder=Endian.BIG
                    )
                    value = decoder.decode_32bit_float()
                    logger.info(f"  Read back float32 value: {value:.3f}")
            else:
                logger.error(f"Error writing multiple registers: {result}")
                
        except Exception as e:
            logger.error(f"Exception writing multiple registers: {e}")
            
    def run_continuous_test(self, duration=60):
        """Run continuous read/write test for specified duration"""
        logger.info(f"\n=== Running Continuous Test for {duration} seconds ===")
        
        start_time = time.time()
        cycle = 0
        
        while time.time() - start_time < duration:
            cycle += 1
            logger.info(f"\n--- Test Cycle {cycle} ---")
            
            # Read some values
            try:
                # Read voltage
                result = self.client.read_holding_registers(0, 2, slave=self.unit_id)
                if not result.isError():
                    decoder = BinaryPayloadDecoder.fromRegisters(
                        result.registers, byteorder=Endian.BIG, wordorder=Endian.BIG
                    )
                    voltage = decoder.decode_32bit_float()
                    logger.info(f"Voltage: {voltage:.2f} V")
                    
                # Read status
                result = self.client.read_holding_registers(4, 1, slave=self.unit_id)
                if not result.isError():
                    status = result.registers[0]
                    logger.info(f"Status: {status}")
                    
                # Write a random setpoint
                setpoint = random.uniform(100, 200)
                builder = BinaryPayloadBuilder(byteorder=Endian.BIG, wordorder=Endian.BIG)
                builder.add_32bit_float(setpoint)
                result = self.client.write_registers(100, builder.to_registers(), slave=self.unit_id)
                if not result.isError():
                    logger.info(f"Wrote setpoint: {setpoint:.2f}")
                    
            except Exception as e:
                logger.error(f"Error in continuous test: {e}")
                
            time.sleep(5)
            
        logger.info(f"\nContinuous test completed. Total cycles: {cycle}")


def main():
    parser = argparse.ArgumentParser(description='Modbus TCP Test Client')
    parser.add_argument('--host', default='127.0.0.1', help='Modbus server host')
    parser.add_argument('--port', type=int, default=502, help='Modbus server port')
    parser.add_argument('--unit', type=int, default=1, help='Modbus unit ID')
    parser.add_argument('--continuous', type=int, default=0, help='Run continuous test for N seconds')
    
    args = parser.parse_args()
    
    # Create test client
    client = ModbusTestClient(host=args.host, port=args.port, unit_id=args.unit)
    
    # Connect to server
    if not client.connect():
        logger.error("Failed to connect to Modbus server")
        return
        
    try:
        if args.continuous > 0:
            # Run continuous test
            client.run_continuous_test(duration=args.continuous)
        else:
            # Run all tests
            client.test_read_holding_registers()
            client.test_read_input_registers()
            client.test_read_coils()
            client.test_write_single_register()
            client.test_write_single_coil()
            client.test_write_multiple_registers()
            
    except KeyboardInterrupt:
        logger.info("\nTest interrupted by user")
    except Exception as e:
        logger.error(f"Test error: {e}")
    finally:
        client.disconnect()
        
    logger.info("\nTest completed")


if __name__ == '__main__':
    main()