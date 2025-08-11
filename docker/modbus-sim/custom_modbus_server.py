#!/usr/bin/env python3
"""
Custom Modbus TCP server with proper 0-based addressing
"""

import logging
import struct
from pymodbus.server import StartTcpServer
from pymodbus.device import ModbusDeviceIdentification
from pymodbus.datastore import (
    ModbusSequentialDataBlock,
    ModbusSlaveContext,
    ModbusServerContext,
)

# Configure logging
logging.basicConfig(
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s", level=logging.DEBUG
)
logger = logging.getLogger(__name__)


def create_datablock_with_float_values():
    """Create a datablock with predefined float32 values"""
    # Initialize with zeros (we need extra space for proper addressing)
    values = [0] * 300

    # Define float32 values to store
    float_values = [
        100.0,  # Point 1 at registers 0-1
        200.0,  # Point 2 at registers 2-3
        300.0,  # Point 3 at registers 4-5
        400.0,  # Point 4 at registers 6-7
        500.0,  # Point 5 at registers 8-9
        600.0,  # Point 6 at registers 10-11
        700.0,  # Point 7 at registers 12-13
        800.0,  # Point 8 at registers 14-15
        900.0,  # Point 9 at registers 16-17
        1000.0,  # Point 10 at registers 18-19
    ]

    # Convert floats to register values (big-endian, ABCD byte order)
    for i, float_val in enumerate(float_values):
        # Pack as big-endian float
        bytes_data = struct.pack(">f", float_val)
        # Unpack as two 16-bit registers
        reg_high, reg_low = struct.unpack(">HH", bytes_data)
        # Store at the correct position (0-based)
        values[i * 2] = reg_high
        values[i * 2 + 1] = reg_low
        logger.info(
            f"Float {float_val} stored at registers {i * 2}-{i * 2 + 1}: [{reg_high:04X}, {reg_low:04X}]"
        )

    # Add some discrete values for testing
    values[200] = 1
    values[201] = 0
    values[202] = 1
    values[203] = 1

    return values


def run_server():
    """Run the Modbus TCP server"""
    logger.info("Starting custom Modbus TCP server with 0-based addressing...")

    # Create the datablock with our test values
    values = create_datablock_with_float_values()

    # Create datablock with 0-based addressing
    # address=0 means the block starts at address 0
    # This ensures that read_holding_registers(0, count) reads from actual address 0
    datablock = ModbusSequentialDataBlock(0, values)

    # Log the initial values for verification
    logger.info("Initial register values (0-based):")
    for i in range(20):
        val = datablock.getValues(i, 1)[0]
        if val != 0:
            logger.info(f"  Register {i}: {val} (0x{val:04X})")

    # Create the slave context
    store = ModbusSlaveContext(
        di=ModbusSequentialDataBlock(0, [0] * 100),
        co=ModbusSequentialDataBlock(0, [0] * 100),
        hr=datablock,  # Our holding registers with float values
        ir=ModbusSequentialDataBlock(0, [0] * 100),
        zero_mode=True,  # Enable true 0-based addressing
    )

    # Create server context
    context = ModbusServerContext(slaves=store, single=True)

    # Setup the server identification
    identity = ModbusDeviceIdentification()
    identity.VendorName = "VoltageEMS"
    identity.ProductCode = "VEMS"
    identity.VendorUrl = "http://github.com/voltageems"
    identity.ProductName = "VoltageEMS Modbus Server"
    identity.ModelName = "Custom Modbus Server"
    identity.MajorMinorRevision = "1.0.0"

    # Start the server
    logger.info("Server listening on 0.0.0.0:502")
    StartTcpServer(context=context, identity=identity, address=("0.0.0.0", 502))


if __name__ == "__main__":
    try:
        run_server()
    except KeyboardInterrupt:
        logger.info("Server stopped by user")
    except Exception as e:
        logger.error(f"Server error: {e}", exc_info=True)
