#!/usr/bin/env python3
"""Simple IEC 60870-5-104 server simulator for testing"""
import socket
import threading
import sys
import time
import struct
import random
import math

class IEC104Server:
    def __init__(self, host='0.0.0.0', port=2404):
        self.host = host
        self.port = port
        self.socket = None
        self.running = False
        
        # IEC104 sequence numbers
        self.ssn = 0  # Send sequence number
        self.rsn = 0  # Receive sequence number
        
        # Connection state
        self.connected = False
        self.test_mode = False
        
        # Simulated data
        self.telemetry_data = {
            1001: 220.0,   # substation_voltage
            1002: 500.0,   # line_current
            1003: 100.0,   # active_power
            1004: 30.0,    # reactive_power
            1005: 50.0,    # frequency
            1006: 15.0,    # phase_angle
        }
        
    def build_apdu(self, apci_type, apci_data=b'', asdu=b''):
        """Build Application Protocol Data Unit"""
        start_byte = 0x68
        
        if apci_type == 'I':  # Information transfer
            # Format: Start(1) + Length(1) + Control Field(4) + ASDU
            cf1 = (self.ssn << 1) & 0xFE
            cf2 = (self.ssn >> 7) & 0xFF
            cf3 = (self.rsn << 1) & 0xFE
            cf4 = (self.rsn >> 7) & 0xFF
            control_field = struct.pack('BBBB', cf1, cf2, cf3, cf4)
            self.ssn = (self.ssn + 1) & 0x7FFF
        elif apci_type == 'S':  # Supervisory
            cf1 = 0x01
            cf2 = 0x00
            cf3 = (self.rsn << 1) & 0xFE
            cf4 = (self.rsn >> 7) & 0xFF
            control_field = struct.pack('BBBB', cf1, cf2, cf3, cf4)
        elif apci_type == 'U':  # Unnumbered control
            control_field = apci_data
        else:
            return None
            
        apdu_length = len(control_field) + len(asdu)
        header = struct.pack('BB', start_byte, apdu_length)
        
        return header + control_field + asdu
        
    def build_asdu(self, type_id, cot, ca, info_objects):
        """Build Application Service Data Unit"""
        # ASDU header: TypeID(1) + SQ/NumIx(1) + COT(2) + CA(2)
        num_objects = len(info_objects)
        sq_num = 0x80 | (num_objects & 0x7F) if num_objects > 1 else num_objects
        
        asdu_header = struct.pack('<BBHH', type_id, sq_num, cot, ca)
        
        # Build information objects
        io_data = b''
        for io in info_objects:
            io_data += io
            
        return asdu_header + io_data
        
    def handle_startdt(self, client_socket):
        """Handle STARTDT (start data transfer)"""
        # Send STARTDT con
        response = self.build_apdu('U', struct.pack('BBBB', 0x0B, 0x00, 0x00, 0x00))
        client_socket.send(response)
        self.connected = True
        print("IEC104: STARTDT confirmed, data transfer enabled")
        
    def handle_testfr(self, client_socket):
        """Handle TESTFR (test frame)"""
        # Send TESTFR con
        response = self.build_apdu('U', struct.pack('BBBB', 0x83, 0x00, 0x00, 0x00))
        client_socket.send(response)
        print("IEC104: TESTFR confirmed")
        
    def handle_stopdt(self, client_socket):
        """Handle STOPDT (stop data transfer)"""
        # Send STOPDT con
        response = self.build_apdu('U', struct.pack('BBBB', 0x23, 0x00, 0x00, 0x00))
        client_socket.send(response)
        self.connected = False
        print("IEC104: STOPDT confirmed, data transfer stopped")
        
    def send_interrogation_response(self, client_socket):
        """Send response to general interrogation"""
        print("IEC104: Sending interrogation response")
        
        # Send measured values (Type 13 - Float)
        info_objects = []
        for ioa, value in self.telemetry_data.items():
            # IOA (3 bytes) + Float value (4 bytes) + QDS (1 byte)
            io = struct.pack('<I', ioa)[:3]  # 3-byte IOA
            io += struct.pack('<f', value)    # Float value
            io += b'\x00'                     # QDS (quality)
            info_objects.append(io)
            
        # Build ASDU
        asdu = self.build_asdu(
            type_id=13,     # M_ME_NC_1 (Float measurement)
            cot=20,         # Interrogated by station
            ca=1,           # Common address
            info_objects=info_objects
        )
        
        # Send I-frame
        apdu = self.build_apdu('I', asdu=asdu)
        client_socket.send(apdu)
        
        # Send interrogation end
        end_asdu = self.build_asdu(
            type_id=100,    # C_IC_NA_1 (Interrogation command)
            cot=10,         # Activation termination
            ca=1,
            info_objects=[b'\x00\x00\x00\x14']  # IOA=0, QOI=20
        )
        end_apdu = self.build_apdu('I', asdu=end_asdu)
        client_socket.send(end_apdu)
        
    def send_cyclic_data(self, client_socket):
        """Send cyclic telemetry updates"""
        if not self.connected:
            return
            
        # Update simulated values
        current_time = time.time()
        self.telemetry_data[1001] = 220.0 + 10 * math.sin(current_time * 0.1)  # Voltage
        self.telemetry_data[1002] = 500.0 + 50 * math.sin(current_time * 0.15) # Current
        self.telemetry_data[1003] = 100.0 + 20 * random.uniform(-1, 1)         # Power
        self.telemetry_data[1005] = 50.0 + 0.1 * random.uniform(-1, 1)         # Frequency
        
        # Send one random telemetry value
        ioa = random.choice(list(self.telemetry_data.keys()))
        value = self.telemetry_data[ioa]
        
        # Build information object
        io = struct.pack('<I', ioa)[:3]  # 3-byte IOA
        io += struct.pack('<f', value)    # Float value
        io += b'\x00'                     # QDS
        
        # Build ASDU
        asdu = self.build_asdu(
            type_id=13,     # Float measurement
            cot=3,          # Spontaneous
            ca=1,
            info_objects=[io]
        )
        
        # Send I-frame
        apdu = self.build_apdu('I', asdu=asdu)
        try:
            client_socket.send(apdu)
        except:
            self.connected = False
            
    def handle_i_frame(self, client_socket, data):
        """Handle Information transfer frame"""
        # Extract ASDU
        asdu = data[6:]  # Skip APCI header
        if len(asdu) < 6:
            return
            
        type_id = asdu[0]
        cot = struct.unpack('<H', asdu[2:4])[0]
        
        print(f"IEC104: Received I-frame, TypeID={type_id}, COT={cot}")
        
        if type_id == 100 and cot == 6:  # General interrogation
            self.send_interrogation_response(client_socket)
        
        # Send S-frame acknowledgment
        s_frame = self.build_apdu('S')
        client_socket.send(s_frame)
        
    def handle_client(self, client_socket, address):
        """Handle client connection"""
        print(f"IEC104: Connection from {address}")
        client_socket.settimeout(30.0)
        
        # Reset sequence numbers
        self.ssn = 0
        self.rsn = 0
        self.connected = False
        
        # Start cyclic data thread
        def cyclic_sender():
            while self.running:
                if self.connected:
                    self.send_cyclic_data(client_socket)
                time.sleep(5)  # Send data every 5 seconds
                
        cyclic_thread = threading.Thread(target=cyclic_sender)
        cyclic_thread.daemon = True
        cyclic_thread.start()
        
        try:
            while self.running:
                try:
                    data = client_socket.recv(1024)
                    if not data:
                        break
                        
                    # Check for valid IEC104 frame
                    if len(data) < 6 or data[0] != 0x68:
                        continue
                        
                    apdu_length = data[1]
                    control_field = data[2:6]
                    
                    # Check frame type
                    if control_field[0] & 0x01 == 0:  # I-frame
                        self.rsn = ((control_field[0] >> 1) | (control_field[1] << 7)) & 0x7FFF
                        self.handle_i_frame(client_socket, data)
                    elif control_field[0] & 0x03 == 0x01:  # S-frame
                        # Supervisory frame - just update RSN
                        pass
                    elif control_field[0] & 0x03 == 0x03:  # U-frame
                        # Unnumbered control frame
                        if control_field[0] == 0x07:  # STARTDT act
                            self.handle_startdt(client_socket)
                        elif control_field[0] == 0x43:  # TESTFR act
                            self.handle_testfr(client_socket)
                        elif control_field[0] == 0x13:  # STOPDT act
                            self.handle_stopdt(client_socket)
                            
                except socket.timeout:
                    # Send periodic test frames if connected
                    if self.connected and not self.test_mode:
                        self.test_mode = True
                        test_frame = self.build_apdu('U', struct.pack('BBBB', 0x43, 0x00, 0x00, 0x00))
                        try:
                            client_socket.send(test_frame)
                        except:
                            break
                except Exception as e:
                    print(f"IEC104: Error handling request: {e}")
                    break
                    
        finally:
            self.connected = False
            client_socket.close()
            print(f"IEC104: Connection closed from {address}")
            
    def start(self):
        """Start the IEC104 server"""
        self.socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.socket.bind((self.host, self.port))
        self.socket.listen(5)
        self.running = True
        
        print(f"IEC 60870-5-104 Server listening on {self.host}:{self.port}")
        
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
                
    def stop(self):
        """Stop the server"""
        self.running = False
        if self.socket:
            self.socket.close()

def main():
    host = sys.argv[1] if len(sys.argv) > 1 else "0.0.0.0"
    port = int(sys.argv[2]) if len(sys.argv) > 2 else 2404
    
    server = IEC104Server(host, port)
    try:
        server.start()
    except KeyboardInterrupt:
        print("\nShutting down server...")
        server.stop()

if __name__ == "__main__":
    main()