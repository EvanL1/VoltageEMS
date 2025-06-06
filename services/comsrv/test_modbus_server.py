#!/usr/bin/env python3
"""
Simple Modbus TCP server for testing comsrv logging functionality
"""

import time
import threading
from pymodbus.server.sync import StartTcpServer
from pymodbus.device import ModbusDeviceIdentification
from pymodbus.datastore import ModbusSequentialDataBlock, ModbusSlaveContext, ModbusServerContext
from pymodbus.transaction import ModbusRtuFramer, ModbusAsciiFramer

def create_modbus_server(port=5502, slave_id=1):
    """Create a simple Modbus TCP server"""
    
    # Initialize data store
    store = ModbusSlaveContext(
        di=ModbusSequentialDataBlock(0, [17]*100),  # Discrete inputs
        co=ModbusSequentialDataBlock(0, [17]*100),  # Coils
        hr=ModbusSequentialDataBlock(0, [17]*100),  # Holding registers
        ir=ModbusSequentialDataBlock(0, [17]*100)   # Input registers
    )
    
    context = ModbusServerContext(slaves=store, single=True)
    
    # Server identification
    identity = ModbusDeviceIdentification()
    identity.VendorName = 'Test Server'
    identity.ProductCode = 'TS'
    identity.VendorUrl = 'http://test.com'
    identity.ProductName = 'Test Modbus Server'
    identity.ModelName = 'Test Model'
    identity.MajorMinorRevision = '1.0'
    
    print(f"Starting Modbus TCP server on port {port} for slave {slave_id}")
    
    # Start server in a separate thread
    server_thread = threading.Thread(
        target=StartTcpServer,
        kwargs={
            'context': context,
            'identity': identity,
            'address': ('127.0.0.1', port),
            'defer_reactor_run': True
        }
    )
    server_thread.daemon = True
    server_thread.start()
    
    return server_thread

def main():
    """Main function to start test servers"""
    print("Starting test Modbus servers for comsrv logging test...")
    
    # Start two servers on different ports
    server1 = create_modbus_server(port=5502, slave_id=1)
    server2 = create_modbus_server(port=5503, slave_id=2)
    
    print("Servers started. Press Ctrl+C to stop.")
    
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("\nStopping servers...")

if __name__ == "__main__":
    main() 