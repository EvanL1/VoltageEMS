#!/usr/bin/env python3
"""
Multi-port Modbus TCP servers for comsrv pressure testing
Enhanced with large-scale point data support
"""

import time
import threading
import random
import struct
from pymodbus.server import StartTcpServer
from pymodbus.device import ModbusDeviceIdentification
from pymodbus.datastore import ModbusSequentialDataBlock, ModbusSlaveContext, ModbusServerContext

class ModbusPressureServer:
    def __init__(self, port, server_id):
        self.port = port
        self.server_id = server_id
        self.running = False
        self.server_thread = None
        self.data_update_thread = None
        
    def create_enhanced_datastore(self):
        """Create a large-scale datastore with realistic data patterns"""
        
        # Generate large-scale data matching our point table structure
        # Coils (0-999): Digital outputs
        coils = [random.choice([True, False]) for _ in range(1000)]
        
        # Discrete Inputs (0-999): Digital inputs  
        discrete_inputs = [random.choice([True, False]) for _ in range(1000)]
        
        # Input Registers (0-1999): Various sensor data
        input_registers = []
        
        # UInt16 sensors (0-499)
        for i in range(500):
            value = random.randint(0, 65535)
            input_registers.append(value)
            
        # Int16 temperature sensors (500-999) - simulating -40°C to +85°C
        for i in range(500):
            # Raw value that converts to -40 to +85°C with scale=0.1, offset=-100
            temp_celsius = random.uniform(-40, 85)
            raw_value = int((temp_celsius + 100) / 0.1)
            raw_value = max(-32768, min(32767, raw_value))  # Clamp to int16 range
            if raw_value < 0:
                raw_value = 65536 + raw_value  # Convert to unsigned representation
            input_registers.append(raw_value)
            
        # Float32 power measurements (1000-1499) - 2 registers each
        for i in range(500):
            power_kw = random.uniform(0, 1000)  # 0-1000 kW
            # Convert float32 to two 16-bit registers (big-endian)
            packed = struct.pack('>f', power_kw)
            reg1 = struct.unpack('>H', packed[0:2])[0]
            reg2 = struct.unpack('>H', packed[2:4])[0]
            input_registers.extend([reg1, reg2])
            
        # Pad to 2000 registers
        while len(input_registers) < 2000:
            input_registers.append(0)
            
        # Holding Registers (1000-2499): Control and setpoint values
        holding_registers = [0] * 1000  # Initialize with zeros
        
        # UInt16 control registers (1000-1499)
        for i in range(500):
            value = random.randint(0, 65535)
            holding_registers[i] = value
            
        # Int32 current setpoints (1500-1999) - 2 registers each
        for i in range(0, 500, 2):
            current_ma = random.uniform(4, 20)  # 4-20mA
            # Convert to raw value: current_ma / 0.01 = raw_value
            raw_value = int(current_ma / 0.01)
            # Split int32 into two registers (big-endian)
            reg1 = (raw_value >> 16) & 0xFFFF
            reg2 = raw_value & 0xFFFF
            if i < 498:
                holding_registers[500 + i] = reg1
                holding_registers[500 + i + 1] = reg2
            
        # Float32 voltage setpoints (2000-2499) - 2 registers each  
        for i in range(0, 500, 2):
            voltage_v = random.uniform(100, 500)  # 100-500V
            # Convert float32 to two 16-bit registers (big-endian)
            packed = struct.pack('>f', voltage_v)
            reg1 = struct.unpack('>H', packed[0:2])[0]
            reg2 = struct.unpack('>H', packed[2:4])[0]
            if i < 498:
                holding_registers[750 + i] = reg1
                holding_registers[750 + i + 1] = reg2
        
        # Pad holding registers to ensure we have enough
        while len(holding_registers) < 1500:
            holding_registers.append(0)
        
        store = ModbusSlaveContext(
            di=ModbusSequentialDataBlock(0, discrete_inputs),
            co=ModbusSequentialDataBlock(0, coils),
            hr=ModbusSequentialDataBlock(1000, holding_registers),  # Start at address 1000
            ir=ModbusSequentialDataBlock(0, input_registers)
        )
        
        return ModbusServerContext(slaves=store, single=True)
    
    def start(self):
        """Start the Modbus server"""
        self.context = self.create_enhanced_datastore()
        
        # Server identification
        identity = ModbusDeviceIdentification()
        identity.VendorName = f'Pressure Test Server {self.server_id}'
        identity.ProductCode = f'PTS{self.server_id}'
        identity.VendorUrl = 'http://voltageems.com'
        identity.ProductName = f'Pressure Test Modbus Server {self.server_id}'
        identity.ModelName = f'PTS-{self.server_id}'
        identity.MajorMinorRevision = '1.0'
        
        print(f"🚀 Starting enhanced Modbus TCP server {self.server_id} on port {self.port}")
        print(f"   📊 Data: 1000 coils, 1000 discrete inputs, 2000 input registers, 1500 holding registers")
        
        # Start server in background
        def run_server():
            try:
                StartTcpServer(
                    context=self.context,
                    identity=identity,
                    address=('127.0.0.1', self.port)
                )
            except Exception as e:
                print(f"❌ Server {self.server_id} on port {self.port} failed: {e}")
        
        self.server_thread = threading.Thread(target=run_server)
        self.server_thread.daemon = True
        self.server_thread.start()
        self.running = True
        
        # Start data update thread
        self.data_update_thread = threading.Thread(target=self.update_data_periodically)
        self.data_update_thread.daemon = True
        self.data_update_thread.start()
        
        # Give server time to start
        time.sleep(0.5)
        
    def update_data_periodically(self):
        """Periodically update server data to simulate real equipment"""
        update_count = 0
        while self.running:
            try:
                time.sleep(2)  # Update every 2 seconds
                if not self.running:
                    break
                
                # Update some register values to simulate changing data
                slave_context = self.context[0]  # Get slave context
                
                # Update some input registers (sensor values)
                for i in range(10):  # Update first 10 temperature sensors
                    addr = 500 + i  # Temperature sensor addresses
                    temp_celsius = random.uniform(-40, 85)
                    raw_value = int((temp_celsius + 100) / 0.1)
                    raw_value = max(-32768, min(32767, raw_value))
                    if raw_value < 0:
                        raw_value = 65536 + raw_value
                    slave_context.setValues(3, addr, [raw_value])  # Function code 3 = input registers
                
                # Update some coils (randomly toggle)
                for i in range(5):
                    addr = i
                    current_value = slave_context.getValues(1, addr, 1)[0]  # Function code 1 = coils
                    new_value = not current_value if random.random() < 0.3 else current_value
                    slave_context.setValues(1, addr, [new_value])
                
                update_count += 1
                if update_count % 10 == 0:  # Print status every 20 seconds
                    print(f"📊 Server {self.server_id} (port {self.port}) - Data updated (count: {update_count})")
                    
            except Exception as e:
                print(f"⚠️  Data update error for server {self.server_id}: {e}")
                break

def create_pressure_test_environment():
    """Create multiple enhanced Modbus servers for pressure testing"""
    
    # Define server ports (matching the pressure test config)
    server_ports = [5502, 5503, 5504, 5505, 5506, 5507, 5508, 5509, 5510, 5511]
    
    servers = []
    
    print("🏗️  Creating enhanced pressure test environment...")
    print(f"📡 Starting {len(server_ports)} Modbus TCP servers with large-scale data...")
    print("📊 Each server provides:")
    print("   • 1,000 coils (digital outputs)")
    print("   • 1,000 discrete inputs (digital inputs)")  
    print("   • 2,000 input registers (sensor data)")
    print("   • 1,500 holding registers (control data)")
    print("   = Total: 5,500 data points per server")
    
    # Start all servers
    for i, port in enumerate(server_ports, 1):
        server = ModbusPressureServer(port, i)
        server.start()
        servers.append(server)
        
        # Brief delay between server starts
        time.sleep(0.3)
    
    total_points = len(servers) * 5500
    print(f"\n✅ All {len(servers)} servers started successfully!")
    print(f"📈 Total data points available: {total_points:,}")
    print("🎯 Ready for high-scale comsrv pressure testing")
    print("-" * 70)
    print("Server Configuration:")
    for i, port in enumerate(server_ports, 1):
        print(f"  Server {i}: 127.0.0.1:{port} (5,500 data points)")
    print("-" * 70)
    
    return servers

def main():
    """Main function for enhanced pressure test servers"""
    print("🔥 COMSRV PRESSURE TEST - ENHANCED MODBUS SERVERS")
    print("=" * 70)
    
    try:
        # Create and start all servers
        servers = create_pressure_test_environment()
        
        print("\n⚡ Enhanced servers are running - Ready for large-scale pressure testing!")
        print("💡 To start comsrv pressure test, run:")
        print("   ./target/release/comsrv --config config/pressure_test_config.yaml --log-level debug")
        print("\n🛑 Press Ctrl+C to stop all servers")
        
        # Keep running until interrupted
        while True:
            time.sleep(1)
            
    except KeyboardInterrupt:
        print("\n🛑 Stopping all enhanced pressure test servers...")
        for server in servers:
            server.running = False
        print("✅ All servers stopped")
        
    except Exception as e:
        print(f"❌ Error in enhanced pressure test environment: {e}")

if __name__ == "__main__":
    main()