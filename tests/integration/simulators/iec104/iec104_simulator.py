#!/usr/bin/env python3
"""
IEC 60870-5-104 Server Simulator for Integration Testing
Simulates multiple stations with configurable data points
"""

import os
import sys
import time
import socket
import struct
import random
import logging
import threading
from typing import Dict, List, Tuple, Any
from datetime import datetime

# Configuration from environment
STATION_COUNT = int(os.getenv('STATION_COUNT', '3'))
POINTS_PER_STATION = int(os.getenv('POINTS_PER_STATION', '200'))
LOG_LEVEL = os.getenv('LOG_LEVEL', 'INFO')
ENABLE_TIME_SYNC = os.getenv('ENABLE_TIME_SYNC', 'true').lower() == 'true'
ENABLE_COMMANDS = os.getenv('ENABLE_COMMANDS', 'true').lower() == 'true'
UPDATE_RATE = int(os.getenv('UPDATE_RATE', '1'))  # Hz
PORT = int(os.getenv('PORT', '2404'))

# Setup logging
logging.basicConfig(
    level=getattr(logging, LOG_LEVEL),
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

# IEC104 Constants
STARTDT_ACT = 0x07
STARTDT_CON = 0x0B
STOPDT_ACT = 0x13
STOPDT_CON = 0x23
TESTFR_ACT = 0x43
TESTFR_CON = 0x83

# Type IDs
M_SP_NA_1 = 1    # Single-point information
M_DP_NA_1 = 3    # Double-point information
M_ME_NA_1 = 9    # Measured value, normalized
M_ME_NC_1 = 13   # Measured value, short floating point
M_SP_TB_1 = 30   # Single-point with time tag
M_ME_TF_1 = 36   # Measured value with time tag
C_SC_NA_1 = 45   # Single command
C_DC_NA_1 = 46   # Double command
C_SE_NC_1 = 50   # Set-point command, short floating point
C_IC_NA_1 = 100  # Interrogation command
C_CS_NA_1 = 103  # Clock synchronization command

class IEC104Frame:
    """IEC104 frame structure"""
    
    def __init__(self, frame_type='I', send_seq=0, recv_seq=0, asdu=None):
        self.frame_type = frame_type
        self.send_seq = send_seq
        self.recv_seq = recv_seq
        self.asdu = asdu or b''
    
    def to_bytes(self) -> bytes:
        """Convert frame to bytes"""
        if self.frame_type == 'I':
            # I-frame
            control = struct.pack('<HH', 
                                self.send_seq << 1,
                                self.recv_seq << 1)
            length = len(self.asdu) + 4
        elif self.frame_type == 'S':
            # S-frame
            control = struct.pack('<HH',
                                1,  # S-frame identifier
                                self.recv_seq << 1)
            length = 4
        else:
            # U-frame
            control = struct.pack('<I', self.asdu)
            length = 4
        
        header = struct.pack('BB', 0x68, length)
        return header + control + (self.asdu if self.frame_type == 'I' else b'')

class IEC104Station:
    """Simulates an IEC104 station"""
    
    def __init__(self, station_id: int):
        self.station_id = station_id
        self.common_address = 1 + station_id
        self.data_points = {}
        self.init_data_points()
    
    def init_data_points(self):
        """Initialize data points for this station"""
        # Single-point information (binary)
        for i in range(20):
            ioa = 1000 + i
            self.data_points[ioa] = {
                'type': M_SP_NA_1,
                'value': random.randint(0, 1),
                'quality': 0,
                'timestamp': datetime.now()
            }
        
        # Double-point information (switch positions)
        for i in range(10):
            ioa = 2000 + i
            self.data_points[ioa] = {
                'type': M_DP_NA_1,
                'value': random.randint(0, 3),  # 0-3: intermediate, off, on, fault
                'quality': 0,
                'timestamp': datetime.now()
            }
        
        # Measured values (normalized)
        for i in range(50):
            ioa = 3000 + i
            self.data_points[ioa] = {
                'type': M_ME_NA_1,
                'value': random.uniform(-1.0, 1.0),
                'quality': 0,
                'timestamp': datetime.now()
            }
        
        # Measured values (float)
        for i in range(POINTS_PER_STATION - 80):
            ioa = 4000 + i
            self.data_points[ioa] = {
                'type': M_ME_NC_1,
                'value': random.uniform(0, 1000),
                'quality': 0,
                'timestamp': datetime.now()
            }
    
    def update_values(self):
        """Update data point values"""
        now = datetime.now()
        
        for ioa, point in self.data_points.items():
            if point['type'] == M_SP_NA_1:
                # Binary - random toggle
                if random.random() < 0.05:
                    point['value'] = 1 - point['value']
            
            elif point['type'] == M_DP_NA_1:
                # Double-point - occasional change
                if random.random() < 0.02:
                    point['value'] = random.randint(0, 3)
            
            elif point['type'] == M_ME_NA_1:
                # Normalized - sine wave
                t = time.time()
                point['value'] = math.sin(t / 60 + ioa / 100) * 0.8
            
            elif point['type'] == M_ME_NC_1:
                # Float - random walk
                change = random.uniform(-10, 10)
                point['value'] = max(0, min(1000, point['value'] + change))
            
            point['timestamp'] = now
    
    def create_asdu(self, type_id: int, cause: int, ioas: List[int]) -> bytes:
        """Create ASDU (Application Service Data Unit)"""
        # ASDU header
        num_objects = len(ioas)
        sq = 0  # Structure qualifier (0 = each object has its own IOA)
        
        header = struct.pack('<BBHBB',
                           type_id,
                           (sq << 7) | (num_objects & 0x3F),
                           cause,
                           0,  # Originator address
                           self.common_address)
        
        # Information objects
        objects = b''
        for ioa in ioas:
            if ioa in self.data_points:
                point = self.data_points[ioa]
                
                # IOA (3 bytes)
                ioa_bytes = struct.pack('<I', ioa)[:3]
                
                # Value encoding based on type
                if type_id in [M_SP_NA_1, M_SP_TB_1]:
                    # Single-point
                    siq = (point['quality'] << 5) | (point['value'] & 1)
                    value_bytes = struct.pack('B', siq)
                
                elif type_id == M_DP_NA_1:
                    # Double-point
                    diq = (point['quality'] << 5) | (point['value'] & 3)
                    value_bytes = struct.pack('B', diq)
                
                elif type_id == M_ME_NA_1:
                    # Normalized value
                    nva = int(point['value'] * 32767)
                    qds = point['quality']
                    value_bytes = struct.pack('<hB', nva, qds)
                
                elif type_id in [M_ME_NC_1, M_ME_TF_1]:
                    # Float value
                    qds = point['quality']
                    value_bytes = struct.pack('<fB', point['value'], qds)
                
                else:
                    continue
                
                # Add timestamp for time-tagged types
                if type_id in [M_SP_TB_1, M_ME_TF_1]:
                    # CP56Time2a (7 bytes)
                    dt = point['timestamp']
                    ms = dt.microsecond // 1000 + dt.second * 1000
                    time_bytes = struct.pack('<HBBBBBB',
                                           ms & 0xFFFF,
                                           (ms >> 16) & 0xFF,
                                           dt.minute,
                                           dt.hour,
                                           dt.day,
                                           dt.month,
                                           dt.year % 100)
                    value_bytes += time_bytes
                
                objects += ioa_bytes + value_bytes
        
        return header + objects

class IEC104Server:
    """IEC104 server simulator"""
    
    def __init__(self):
        self.stations = []
        self.server_socket = None
        self.clients = {}
        self.running = False
        self.init_stations()
    
    def init_stations(self):
        """Initialize stations"""
        for i in range(STATION_COUNT):
            station = IEC104Station(i)
            self.stations.append(station)
        logger.info(f"Initialized {STATION_COUNT} IEC104 stations")
    
    def start_server(self):
        """Start TCP server"""
        self.server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.server_socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.server_socket.bind(('0.0.0.0', PORT))
        self.server_socket.listen(5)
        self.running = True
        
        logger.info(f"IEC104 server listening on port {PORT}")
        
        # Start accept thread
        accept_thread = threading.Thread(target=self.accept_clients, daemon=True)
        accept_thread.start()
        
        # Start update thread
        update_thread = threading.Thread(target=self.update_loop, daemon=True)
        update_thread.start()
    
    def accept_clients(self):
        """Accept client connections"""
        while self.running:
            try:
                client_socket, addr = self.server_socket.accept()
                logger.info(f"Client connected from {addr}")
                
                client_id = f"{addr[0]}:{addr[1]}"
                self.clients[client_id] = {
                    'socket': client_socket,
                    'addr': addr,
                    'send_seq': 0,
                    'recv_seq': 0,
                    'data_transfer': False
                }
                
                # Start client handler thread
                thread = threading.Thread(
                    target=self.handle_client,
                    args=(client_id,),
                    daemon=True
                )
                thread.start()
                
            except Exception as e:
                if self.running:
                    logger.error(f"Error accepting client: {e}")
    
    def handle_client(self, client_id: str):
        """Handle client communication"""
        client = self.clients[client_id]
        
        try:
            while self.running and client_id in self.clients:
                # Receive data
                data = client['socket'].recv(1024)
                if not data:
                    break
                
                # Process frames
                self.process_frame(client_id, data)
                
        except Exception as e:
            logger.error(f"Error handling client {client_id}: {e}")
        finally:
            logger.info(f"Client {client_id} disconnected")
            client['socket'].close()
            del self.clients[client_id]
    
    def process_frame(self, client_id: str, data: bytes):
        """Process received IEC104 frame"""
        if len(data) < 6:
            return
        
        client = self.clients[client_id]
        
        # Parse frame header
        start = data[0]
        length = data[1]
        
        if start != 0x68:
            return
        
        # Check frame format
        control = struct.unpack('<I', data[2:6])[0]
        
        if (control & 1) == 0:
            # I-frame
            send_seq = (control >> 1) & 0x7FFF
            recv_seq = (control >> 17) & 0x7FFF
            client['recv_seq'] = send_seq + 1
            
            # Process ASDU
            if len(data) >= 6 + length - 4:
                asdu = data[6:6+length-4]
                self.process_asdu(client_id, asdu)
            
            # Send S-frame acknowledgment
            self.send_s_frame(client_id)
            
        elif (control & 3) == 1:
            # S-frame
            recv_seq = (control >> 17) & 0x7FFF
            
        else:
            # U-frame
            if control == STARTDT_ACT:
                logger.info(f"Client {client_id}: STARTDT_ACT received")
                client['data_transfer'] = True
                self.send_u_frame(client_id, STARTDT_CON)
                
            elif control == STOPDT_ACT:
                logger.info(f"Client {client_id}: STOPDT_ACT received")
                client['data_transfer'] = False
                self.send_u_frame(client_id, STOPDT_CON)
                
            elif control == TESTFR_ACT:
                self.send_u_frame(client_id, TESTFR_CON)
    
    def process_asdu(self, client_id: str, asdu: bytes):
        """Process ASDU"""
        if len(asdu) < 6:
            return
        
        type_id = asdu[0]
        sq_num = asdu[1]
        cause = struct.unpack('<H', asdu[2:4])[0] & 0x3F
        common_addr = asdu[5]
        
        logger.debug(f"ASDU: Type={type_id}, Cause={cause}, CA={common_addr}")
        
        if type_id == C_IC_NA_1:
            # General interrogation
            logger.info(f"Client {client_id}: General interrogation for CA={common_addr}")
            self.send_interrogation_response(client_id, common_addr)
            
        elif type_id == C_CS_NA_1 and ENABLE_TIME_SYNC:
            # Clock synchronization
            logger.info(f"Client {client_id}: Clock sync for CA={common_addr}")
            self.send_clock_sync_response(client_id, common_addr)
            
        elif type_id in [C_SC_NA_1, C_DC_NA_1, C_SE_NC_1] and ENABLE_COMMANDS:
            # Control commands
            logger.info(f"Client {client_id}: Control command Type={type_id}")
            self.process_control_command(client_id, type_id, asdu)
    
    def send_interrogation_response(self, client_id: str, common_addr: int):
        """Send response to general interrogation"""
        client = self.clients[client_id]
        if not client['data_transfer']:
            return
        
        # Find station
        station = None
        for s in self.stations:
            if s.common_address == common_addr:
                station = s
                break
        
        if not station:
            return
        
        # Send data in groups by type
        type_groups = {}
        for ioa, point in station.data_points.items():
            if point['type'] not in type_groups:
                type_groups[point['type']] = []
            type_groups[point['type']].append(ioa)
        
        # Send each group
        for type_id, ioas in type_groups.items():
            # Send in chunks of 20
            for i in range(0, len(ioas), 20):
                chunk = ioas[i:i+20]
                asdu = station.create_asdu(type_id, 20, chunk)  # Cause 20 = interrogated by station
                self.send_i_frame(client_id, asdu)
                time.sleep(0.01)  # Small delay between frames
    
    def send_i_frame(self, client_id: str, asdu: bytes):
        """Send I-frame"""
        client = self.clients[client_id]
        frame = IEC104Frame('I', client['send_seq'], client['recv_seq'], asdu)
        client['socket'].send(frame.to_bytes())
        client['send_seq'] = (client['send_seq'] + 1) & 0x7FFF
    
    def send_s_frame(self, client_id: str):
        """Send S-frame"""
        client = self.clients[client_id]
        frame = IEC104Frame('S', 0, client['recv_seq'])
        client['socket'].send(frame.to_bytes())
    
    def send_u_frame(self, client_id: str, control: int):
        """Send U-frame"""
        client = self.clients[client_id]
        frame = IEC104Frame('U', asdu=control)
        client['socket'].send(frame.to_bytes())
    
    def update_loop(self):
        """Background thread to update values"""
        while self.running:
            # Update all stations
            for station in self.stations:
                station.update_values()
            
            # Send spontaneous updates to connected clients
            for client_id, client in list(self.clients.items()):
                if client['data_transfer']:
                    # Send some random updates
                    for station in self.stations:
                        # Pick random points to update
                        ioas = random.sample(list(station.data_points.keys()), 
                                           min(5, len(station.data_points)))
                        
                        for ioa in ioas:
                            point = station.data_points[ioa]
                            asdu = station.create_asdu(
                                point['type'], 
                                3,  # Cause 3 = spontaneous
                                [ioa]
                            )
                            try:
                                self.send_i_frame(client_id, asdu)
                            except:
                                pass
            
            time.sleep(1.0 / UPDATE_RATE)
    
    def run(self):
        """Main run loop"""
        self.start_server()
        
        try:
            while self.running:
                time.sleep(1)
        except KeyboardInterrupt:
            logger.info("Shutting down IEC104 server...")
        finally:
            self.running = False
            if self.server_socket:
                self.server_socket.close()

def main():
    import math  # Add missing import
    server = IEC104Server()
    server.run()

if __name__ == "__main__":
    main()