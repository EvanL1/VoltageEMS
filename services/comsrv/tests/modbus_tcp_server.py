#!/usr/bin/env python3
"""Enhanced Modbus TCP server simulator with real responses"""
import socket
import threading
import sys
import time
import struct
import random
import math
from datetime import datetime

class ModbusServer:
    def __init__(self, host='0.0.0.0', port=5502, slave_id=1):
        self.host = host
        self.port = port
        self.slave_id = slave_id
        self.socket = None
        self.running = False
        
        # Initialize registers for different Modbus areas
        self.holding_registers = [0] * 10000  # 40001-50000
        self.input_registers = [0] * 10000   # 30001-40000
        self.coils = [False] * 10000         # 00001-10000
        self.discrete_inputs = [False] * 10000  # 10001-20000
        
        # Initialize data
        self._init_data()
        
    def _init_data(self):
        """Initialize register data based on slave ID"""
        if self.slave_id == 1:
            # Power meter data
            self._init_power_meter_data()
        elif self.slave_id == 2:
            # Battery system data
            self._init_battery_data()
            
    def _init_power_meter_data(self):
        """Initialize power meter simulation data"""
        # Set initial values
        self.base_voltage = 220.0
        self.base_current = 50.0
        self.base_power = 10000.0
        self.base_frequency = 50.0
        self.base_temperature = 25.0
        
        # Initialize discrete inputs (signals)
        self.discrete_inputs[0] = True   # breaker_status
        self.discrete_inputs[5] = True   # device_online
        
    def _init_battery_data(self):
        """Initialize battery system simulation data"""
        self.battery_voltage = 48.0
        self.battery_current = 20.0
        self.battery_soc = 85.0
        self.battery_soh = 95.0
        self.battery_temp = 25.0
        
        # Initialize discrete inputs
        self.discrete_inputs[0] = True   # charging_status
        self.discrete_inputs[6] = True   # communication_ok
        
    def _update_simulation_data(self):
        """Update simulation data with realistic variations"""
        current_time = time.time()
        
        if self.slave_id == 1:
            # Update power meter data
            # Voltage with small sine wave variation
            va = self.base_voltage + 5 * math.sin(current_time * 0.1)
            vb = self.base_voltage + 5 * math.sin(current_time * 0.1 + 2.094)
            vc = self.base_voltage + 5 * math.sin(current_time * 0.1 + 4.189)
            
            # Current with random variations
            ia = self.base_current + random.uniform(-5, 5)
            ib = self.base_current + random.uniform(-5, 5)
            ic = self.base_current + random.uniform(-5, 5)
            
            # Power calculations
            power_active = (va * ia + vb * ib + vc * ic) / 1000  # kW
            power_reactive = power_active * 0.3  # kVar
            power_factor = 0.92 + random.uniform(-0.02, 0.02)
            
            # Frequency with small variations
            frequency = self.base_frequency + random.uniform(-0.1, 0.1)
            
            # Temperature with slow changes
            self.base_temperature += random.uniform(-0.1, 0.1)
            self.base_temperature = max(20, min(40, self.base_temperature))
            
            # Update holding registers (Function code 3)
            self._write_float32(0, va)      # voltage_a
            self._write_float32(2, vb)      # voltage_b
            self._write_float32(4, vc)      # voltage_c
            self._write_float32(6, ia)      # current_a
            self._write_float32(8, ib)      # current_b
            self._write_float32(10, ic)     # current_c
            self._write_int32(12, int(power_active * 1000))    # power_active
            self._write_int32(14, int(power_reactive * 1000))  # power_reactive
            self._write_int16(16, int(power_factor * 1000))    # power_factor
            self._write_uint16(17, int(frequency * 100))       # frequency
            self._write_uint32(18, int(time.time() % 100000))  # energy_active
            self._write_int16(20, int(self.base_temperature * 10))  # temperature
            
        elif self.slave_id == 2:
            # Update battery system data
            self.battery_voltage += random.uniform(-0.1, 0.1)
            self.battery_current += random.uniform(-0.5, 0.5)
            
            # SOC changes slowly
            if self.discrete_inputs[0]:  # charging
                self.battery_soc = min(100, self.battery_soc + 0.01)
            else:
                self.battery_soc = max(0, self.battery_soc - 0.01)
                
            self.battery_temp += random.uniform(-0.05, 0.05)
            self.battery_temp = max(15, min(45, self.battery_temp))
            
            # Update registers
            self._write_float32(0, self.battery_voltage)    # dc_voltage
            self._write_float32(2, self.battery_current)    # dc_current
            self._write_uint16(4, int(self.battery_soc * 10))     # battery_soc
            self._write_uint16(5, int(self.battery_soh * 10))     # battery_soh
            self._write_float32(6, self.battery_voltage)    # battery_voltage
            self._write_float32(8, self.battery_current)    # battery_current
            self._write_int16(10, int(self.battery_temp * 10))    # battery_temperature
            
    def _write_float32(self, addr, value):
        """Write float32 to holding registers (big-endian)"""
        data = struct.pack('>f', value)
        self.holding_registers[addr] = struct.unpack('>H', data[0:2])[0]
        self.holding_registers[addr + 1] = struct.unpack('>H', data[2:4])[0]
        
    def _write_int32(self, addr, value):
        """Write int32 to holding registers (big-endian)"""
        high = (value >> 16) & 0xFFFF
        low = value & 0xFFFF
        self.holding_registers[addr] = high
        self.holding_registers[addr + 1] = low
        
    def _write_uint32(self, addr, value):
        """Write uint32 to holding registers (big-endian)"""
        self._write_int32(addr, value)
        
    def _write_int16(self, addr, value):
        """Write int16 to holding registers"""
        self.holding_registers[addr] = value & 0xFFFF
        
    def _write_uint16(self, addr, value):
        """Write uint16 to holding registers"""
        self.holding_registers[addr] = value & 0xFFFF
        
    def handle_modbus_request(self, data):
        """Handle Modbus TCP request and return response"""
        if len(data) < 12:
            return None
            
        # Parse MBAP header
        transaction_id = struct.unpack('>H', data[0:2])[0]
        protocol_id = struct.unpack('>H', data[2:4])[0]
        length = struct.unpack('>H', data[4:6])[0]
        unit_id = data[6]
        function_code = data[7]
        
        # Check protocol ID (must be 0 for Modbus)
        if protocol_id != 0:
            return None
            
        # Handle different function codes
        response_pdu = None
        
        if function_code == 1:  # Read Coils
            response_pdu = self._read_coils(data[8:])
        elif function_code == 2:  # Read Discrete Inputs
            response_pdu = self._read_discrete_inputs(data[8:])
        elif function_code == 3:  # Read Holding Registers
            response_pdu = self._read_holding_registers(data[8:])
        elif function_code == 4:  # Read Input Registers
            response_pdu = self._read_input_registers(data[8:])
        elif function_code == 5:  # Write Single Coil
            response_pdu = self._write_single_coil(data[8:])
        elif function_code == 6:  # Write Single Register
            response_pdu = self._write_single_register(data[8:])
        else:
            # Unsupported function code
            response_pdu = struct.pack('BB', function_code | 0x80, 0x01)
            
        if response_pdu:
            # Build response with MBAP header
            response_length = len(response_pdu) + 1
            response = struct.pack('>HHHB', transaction_id, protocol_id, 
                                 response_length, unit_id) + response_pdu
            return response
        return None
        
    def _read_holding_registers(self, data):
        """Handle Read Holding Registers (FC 03)"""
        start_addr = struct.unpack('>H', data[0:2])[0]
        quantity = struct.unpack('>H', data[2:4])[0]
        
        if quantity < 1 or quantity > 125:
            return struct.pack('BB', 0x83, 0x03)  # Illegal data value
            
        if start_addr + quantity > len(self.holding_registers):
            return struct.pack('BB', 0x83, 0x02)  # Illegal data address
            
        # Build response
        byte_count = quantity * 2
        response = struct.pack('BB', 0x03, byte_count)
        
        for i in range(quantity):
            value = self.holding_registers[start_addr + i]
            response += struct.pack('>H', value)
            
        return response
        
    def _read_coils(self, data):
        """Handle Read Coils (FC 01)"""
        start_addr = struct.unpack('>H', data[0:2])[0]
        quantity = struct.unpack('>H', data[2:4])[0]
        
        if quantity < 1 or quantity > 2000:
            return struct.pack('BB', 0x81, 0x03)  # Illegal data value
            
        if start_addr + quantity > len(self.coils):
            return struct.pack('BB', 0x81, 0x02)  # Illegal data address
            
        # Build response
        byte_count = (quantity + 7) // 8
        response = struct.pack('BB', 0x01, byte_count)
        
        # Pack bits into bytes
        for byte_idx in range(byte_count):
            byte_val = 0
            for bit_idx in range(8):
                addr = start_addr + byte_idx * 8 + bit_idx
                if addr < start_addr + quantity and addr < len(self.coils) and self.coils[addr]:
                    byte_val |= (1 << bit_idx)
            response += struct.pack('B', byte_val)
            
        return response
        
    def _read_discrete_inputs(self, data):
        """Handle Read Discrete Inputs (FC 02)"""
        start_addr = struct.unpack('>H', data[0:2])[0]
        quantity = struct.unpack('>H', data[2:4])[0]
        
        if quantity < 1 or quantity > 2000:
            return struct.pack('BB', 0x82, 0x03)
            
        # Build response
        byte_count = (quantity + 7) // 8
        response = struct.pack('BB', 0x02, byte_count)
        
        # Pack bits into bytes
        for byte_idx in range(byte_count):
            byte_val = 0
            for bit_idx in range(8):
                addr = start_addr + byte_idx * 8 + bit_idx
                if addr < len(self.discrete_inputs) and self.discrete_inputs[addr]:
                    byte_val |= (1 << bit_idx)
            response += struct.pack('B', byte_val)
            
        return response
        
    def _read_input_registers(self, data):
        """Handle Read Input Registers (FC 04)"""
        start_addr = struct.unpack('>H', data[0:2])[0]
        quantity = struct.unpack('>H', data[2:4])[0]
        
        if quantity < 1 or quantity > 125:
            return struct.pack('BB', 0x84, 0x03)  # Illegal data value
            
        if start_addr + quantity > len(self.input_registers):
            return struct.pack('BB', 0x84, 0x02)  # Illegal data address
            
        # Build response
        byte_count = quantity * 2
        response = struct.pack('BB', 0x04, byte_count)
        
        for i in range(quantity):
            value = self.input_registers[start_addr + i]
            response += struct.pack('>H', value)
            
        return response
        
    def _write_single_coil(self, data):
        """Handle Write Single Coil (FC 05)"""
        addr = struct.unpack('>H', data[0:2])[0]
        value = struct.unpack('>H', data[2:4])[0]
        
        if addr >= len(self.coils):
            return struct.pack('BB', 0x85, 0x02)
            
        # Set coil value (0xFF00 = ON, 0x0000 = OFF)
        self.coils[addr] = (value == 0xFF00)
        
        # Echo request as response
        return struct.pack('BHH', 0x05, addr, value)
        
    def _write_single_register(self, data):
        """Handle Write Single Register (FC 06)"""
        addr = struct.unpack('>H', data[0:2])[0]
        value = struct.unpack('>H', data[2:4])[0]
        
        if addr >= len(self.holding_registers):
            return struct.pack('BB', 0x86, 0x02)
            
        self.holding_registers[addr] = value
        
        # Echo request as response
        return struct.pack('BHH', 0x06, addr, value)
        
    def handle_client(self, client_socket, address):
        """Handle client connection"""
        print(f"[Slave {self.slave_id}] Connection from {address}")
        client_socket.settimeout(30.0)
        
        try:
            while self.running:
                try:
                    data = client_socket.recv(1024)
                    if not data:
                        break
                        
                    print(f"[Slave {self.slave_id}] Received {len(data)} bytes: {data.hex()}")
                    
                    # Process Modbus request
                    response = self.handle_modbus_request(data)
                    if response:
                        client_socket.send(response)
                        print(f"[Slave {self.slave_id}] Sent response: {response.hex()}")
                        
                except socket.timeout:
                    continue
                except Exception as e:
                    print(f"[Slave {self.slave_id}] Error handling request: {e}")
                    break
                    
        finally:
            client_socket.close()
            print(f"[Slave {self.slave_id}] Connection closed from {address}")
            
    def start(self):
        """Start the Modbus server"""
        self.socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.socket.bind((self.host, self.port))
        self.socket.listen(5)
        self.running = True
        
        print(f"Modbus TCP Server (Slave {self.slave_id}) listening on {self.host}:{self.port}")
        
        # Start data update thread
        update_thread = threading.Thread(target=self._update_loop)
        update_thread.daemon = True
        update_thread.start()
        
        # Accept connections
        while self.running:
            try:
                client_socket, address = self.socket.accept()
                client_thread = threading.Thread(
                    target=self.handle_client, 
                    args=(client_socket, address)
                )
                client_thread.daemon = True
                client_thread.start()
            except KeyboardInterrupt:
                break
                
    def _update_loop(self):
        """Update simulation data periodically"""
        while self.running:
            self._update_simulation_data()
            time.sleep(0.1)
            
    def stop(self):
        """Stop the server"""
        self.running = False
        if self.socket:
            self.socket.close()

def main():
    host = sys.argv[1] if len(sys.argv) > 1 else "0.0.0.0"
    port = int(sys.argv[2]) if len(sys.argv) > 2 else 5502
    slave_id = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    
    server = ModbusServer(host, port, slave_id)
    try:
        server.start()
    except KeyboardInterrupt:
        print("\nShutting down server...")
        server.stop()

if __name__ == "__main__":
    main()