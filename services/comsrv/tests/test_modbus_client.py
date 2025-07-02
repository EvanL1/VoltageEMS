#!/usr/bin/env python3
"""
Test client for Modbus server simulator

This script tests all four telemetry types against the simulator.
"""

import asyncio
import struct
import socket
import sys
from datetime import datetime


class ModbusTestClient:
    """Simple Modbus TCP client for testing"""
    
    def __init__(self, host='127.0.0.1', port=5020):
        self.host = host
        self.port = port
        self.transaction_id = 0
        
    def _next_transaction_id(self):
        """Get next transaction ID"""
        self.transaction_id = (self.transaction_id + 1) % 65536
        return self.transaction_id
    
    def _build_request(self, unit_id, function_code, data):
        """Build Modbus TCP request"""
        transaction_id = self._next_transaction_id()
        pdu = bytes([function_code]) + data
        mbap = struct.pack('>HHHB',
                          transaction_id,  # Transaction ID
                          0,               # Protocol ID
                          len(pdu) + 1,    # Length
                          unit_id)         # Unit ID
        return mbap + pdu, transaction_id
    
    def _parse_response(self, response, expected_transaction_id):
        """Parse Modbus TCP response"""
        if len(response) < 8:
            raise ValueError(f"Response too short: {len(response)} bytes")
            
        transaction_id = struct.unpack('>H', response[0:2])[0]
        protocol_id = struct.unpack('>H', response[2:4])[0]
        length = struct.unpack('>H', response[4:6])[0]
        unit_id = response[6]
        
        if transaction_id != expected_transaction_id:
            raise ValueError(f"Transaction ID mismatch: {transaction_id} != {expected_transaction_id}")
            
        pdu = response[7:]
        return unit_id, pdu
    
    def test_all_functions(self):
        """Test all Modbus functions"""
        print(f"\n🔌 Connecting to Modbus server at {self.host}:{self.port}")
        
        try:
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
                sock.connect((self.host, self.port))
                sock.settimeout(5.0)
                print("✅ Connected successfully\n")
                
                # Test 遥测 (Telemetry) - Read Holding Registers
                print("=== Testing 遥测 (YC) - Telemetry ===")
                self._test_read_holding_registers(sock)
                
                # Test 遥信 (Signaling) - Read Coils and Discrete Inputs
                print("\n=== Testing 遥信 (YX) - Signaling ===")
                self._test_read_coils(sock)
                self._test_read_discrete_inputs(sock)
                
                # Test 遥控 (Control) - Write Coils
                print("\n=== Testing 遥控 (YK) - Control ===")
                self._test_write_coils(sock)
                
                # Test 遥调 (Adjustment) - Write Holding Registers
                print("\n=== Testing 遥调 (YT) - Adjustment ===")
                self._test_write_holding_registers(sock)
                
                print("\n✅ All tests passed!")
                
        except Exception as e:
            print(f"\n❌ Error: {e}")
            return False
            
        return True
    
    def _test_read_holding_registers(self, sock):
        """Test reading holding registers (遥测)"""
        print("📊 Reading holding registers (FC03):")
        
        # Read from slave 1, addresses 1001-1009
        request, trans_id = self._build_request(1, 0x03, struct.pack('>HH', 1001, 5))
        sock.send(request)
        
        response = sock.recv(1024)
        unit_id, pdu = self._parse_response(response, trans_id)
        
        if pdu[0] == 0x03:
            byte_count = pdu[1]
            values = []
            for i in range(0, byte_count, 2):
                value = struct.unpack('>H', pdu[2+i:4+i])[0]
                values.append(value)
            
            print(f"  Slave {unit_id} registers 1001-1009:")
            addrs = [1001, 1003, 1005, 1007, 1009]
            names = ["Voltage", "Current", "Power", "Temperature", "Frequency"]
            scales = [1, 0.1, 1, 0.1, 0.1]
            units = ["V", "A", "W", "°C", "Hz"]
            
            for i, (addr, name, scale, unit) in enumerate(zip(addrs, names, scales, units)):
                if i < len(values):
                    real_value = values[i] * scale
                    print(f"    {addr}: {name} = {real_value:.1f} {unit} (raw: {values[i]})")
        else:
            print(f"  ❌ Error response: {pdu.hex()}")
    
    def _test_read_coils(self, sock):
        """Test reading coils (遥信)"""
        print("🔌 Reading coils (FC01):")
        
        # Read from slave 1, coils 1-3
        request, trans_id = self._build_request(1, 0x01, struct.pack('>HH', 1, 3))
        sock.send(request)
        
        response = sock.recv(1024)
        unit_id, pdu = self._parse_response(response, trans_id)
        
        if pdu[0] == 0x01:
            byte_count = pdu[1]
            coil_bytes = pdu[2:2+byte_count]
            
            print(f"  Slave {unit_id} coils 1-3:")
            for i in range(3):
                byte_idx = i // 8
                bit_idx = i % 8
                value = bool(coil_bytes[byte_idx] & (1 << bit_idx))
                print(f"    Coil {i+1}: {'ON' if value else 'OFF'}")
        else:
            print(f"  ❌ Error response: {pdu.hex()}")
    
    def _test_read_discrete_inputs(self, sock):
        """Test reading discrete inputs (遥信)"""
        print("📥 Reading discrete inputs (FC02):")
        
        # Read from slave 1, discrete inputs 4-5
        request, trans_id = self._build_request(1, 0x02, struct.pack('>HH', 4, 2))
        sock.send(request)
        
        response = sock.recv(1024)
        unit_id, pdu = self._parse_response(response, trans_id)
        
        if pdu[0] == 0x02:
            byte_count = pdu[1]
            input_bytes = pdu[2:2+byte_count]
            
            print(f"  Slave {unit_id} discrete inputs 4-5:")
            for i in range(2):
                byte_idx = i // 8
                bit_idx = i % 8
                value = bool(input_bytes[byte_idx] & (1 << bit_idx))
                print(f"    Input {i+4}: {'ON' if value else 'OFF'}")
        else:
            print(f"  ❌ Error response: {pdu.hex()}")
    
    def _test_write_coils(self, sock):
        """Test writing coils (遥控)"""
        print("🎛️ Writing single coil (FC05):")
        
        # Write to slave 1, coil 1001
        value = 0xFF00  # ON
        request, trans_id = self._build_request(1, 0x05, struct.pack('>HH', 1001, value))
        sock.send(request)
        
        response = sock.recv(1024)
        unit_id, pdu = self._parse_response(response, trans_id)
        
        if pdu[0] == 0x05:
            address = struct.unpack('>H', pdu[1:3])[0]
            written_value = struct.unpack('>H', pdu[3:5])[0]
            print(f"  ✅ Wrote coil {address} = {'ON' if written_value == 0xFF00 else 'OFF'}")
        else:
            print(f"  ❌ Error response: {pdu.hex()}")
            
        # Test multiple coils
        print("\n🎛️ Writing multiple coils (FC15):")
        
        # Write to slave 2, coils 2001-2003
        coil_values = [True, False, True]  # ON, OFF, ON
        byte_count = 1
        coil_bytes = bytearray(1)
        for i, value in enumerate(coil_values):
            if value:
                coil_bytes[0] |= (1 << i)
                
        data = struct.pack('>HHB', 2001, 3, byte_count) + bytes(coil_bytes)
        request, trans_id = self._build_request(2, 0x0F, data)
        sock.send(request)
        
        response = sock.recv(1024)
        unit_id, pdu = self._parse_response(response, trans_id)
        
        if pdu[0] == 0x0F:
            address = struct.unpack('>H', pdu[1:3])[0]
            quantity = struct.unpack('>H', pdu[3:5])[0]
            print(f"  ✅ Wrote {quantity} coils starting at {address}")
        else:
            print(f"  ❌ Error response: {pdu.hex()}")
    
    def _test_write_holding_registers(self, sock):
        """Test writing holding registers (遥调)"""
        print("📈 Writing single register (FC06):")
        
        # Write to slave 1, register 1000
        value = 1234
        request, trans_id = self._build_request(1, 0x06, struct.pack('>HH', 1000, value))
        sock.send(request)
        
        response = sock.recv(1024)
        unit_id, pdu = self._parse_response(response, trans_id)
        
        if pdu[0] == 0x06:
            address = struct.unpack('>H', pdu[1:3])[0]
            written_value = struct.unpack('>H', pdu[3:5])[0]
            print(f"  ✅ Wrote register {address} = {written_value}")
        else:
            print(f"  ❌ Error response: {pdu.hex()}")
            
        # Test float32 write
        print("\n📈 Writing float32 value (FC16):")
        
        # Write to slave 1, registers 2001-2002 (float32)
        float_value = 50.5
        float_bytes = struct.pack('>f', float_value)
        registers = [struct.unpack('>H', float_bytes[0:2])[0],
                    struct.unpack('>H', float_bytes[2:4])[0]]
        
        byte_count = 4
        data = struct.pack('>HHB', 2001, 2, byte_count)
        for reg in registers:
            data += struct.pack('>H', reg)
            
        request, trans_id = self._build_request(1, 0x10, data)
        sock.send(request)
        
        response = sock.recv(1024)
        unit_id, pdu = self._parse_response(response, trans_id)
        
        if pdu[0] == 0x10:
            address = struct.unpack('>H', pdu[1:3])[0]
            quantity = struct.unpack('>H', pdu[3:5])[0]
            print(f"  ✅ Wrote float32 {float_value} to registers {address}-{address+1}")
        else:
            print(f"  ❌ Error response: {pdu.hex()}")


def main():
    """Main function"""
    import argparse
    
    parser = argparse.ArgumentParser(description='Test Modbus TCP server')
    parser.add_argument('--host', default='127.0.0.1', help='Server host')
    parser.add_argument('--port', type=int, default=5020, help='Server port')
    
    args = parser.parse_args()
    
    print("=== Modbus TCP Client Test ===")
    print(f"Testing server at {args.host}:{args.port}")
    print(f"Time: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    
    client = ModbusTestClient(args.host, args.port)
    success = client.test_all_functions()
    
    if success:
        print("\n✅ All tests completed successfully!")
        return 0
    else:
        print("\n❌ Some tests failed!")
        return 1


if __name__ == '__main__':
    sys.exit(main())