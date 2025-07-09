#!/usr/bin/env python3
"""
IEC 60870-5-104 Server Simulator for Testing
Simulates IEC104 slave station with various data patterns
"""

import asyncio
import logging
import argparse
import struct
import time
import math
import random
from typing import Dict, List, Tuple, Optional
from dataclasses import dataclass
from enum import Enum

logging.basicConfig()
log = logging.getLogger()

# IEC104 Constants
class TypeID(Enum):
    """IEC104 Type Identifications"""
    M_SP_NA_1 = 1    # Single-point information
    M_DP_NA_1 = 3    # Double-point information
    M_ME_NA_1 = 9    # Measured value, normalized
    M_ME_NB_1 = 11   # Measured value, scaled
    M_ME_NC_1 = 13   # Measured value, short floating point
    M_IT_NA_1 = 15   # Integrated totals
    C_SC_NA_1 = 45   # Single command
    C_DC_NA_1 = 46   # Double command
    C_SE_NA_1 = 48   # Set-point command, normalized
    C_SE_NB_1 = 49   # Set-point command, scaled
    C_SE_NC_1 = 50   # Set-point command, short floating point
    C_IC_NA_1 = 100  # Interrogation command
    C_CS_NA_1 = 103  # Clock synchronization command

class Cause(Enum):
    """Cause of Transmission"""
    PERIODIC = 1
    BACKGROUND = 2
    SPONTANEOUS = 3
    INITIALIZED = 4
    REQUEST = 5
    ACTIVATION = 6
    ACTIVATION_CON = 7
    DEACTIVATION = 8
    DEACTIVATION_CON = 9
    ACTIVATION_TERM = 10
    REMOTE_CMD = 11
    LOCAL_CMD = 12
    FILE_TRANSFER = 13
    INTERROGATION = 20

@dataclass
class IECPoint:
    """IEC104 data point definition"""
    ioa: int  # Information Object Address
    type_id: TypeID
    name: str
    pattern: str
    min_value: float
    max_value: float
    period: float
    quality: int = 0

class DataPattern:
    """Data pattern generator for simulating different value changes"""
    
    @staticmethod
    def sine_wave(t: float, min_val: float, max_val: float, period: float) -> float:
        """Generate sine wave pattern"""
        amplitude = (max_val - min_val) / 2
        offset = (max_val + min_val) / 2
        return offset + amplitude * math.sin(2 * math.pi * t / period)
    
    @staticmethod
    def square_wave(t: float, min_val: float, max_val: float, period: float) -> float:
        """Generate square wave pattern"""
        phase = (t % period) / period
        return max_val if phase < 0.5 else min_val
    
    @staticmethod
    def sawtooth(t: float, min_val: float, max_val: float, period: float) -> float:
        """Generate sawtooth pattern"""
        phase = (t % period) / period
        return min_val + (max_val - min_val) * phase
    
    @staticmethod
    def random_walk(current: float, min_val: float, max_val: float, step: float) -> float:
        """Generate random walk pattern"""
        change = random.uniform(-step, step)
        new_val = current + change
        return max(min_val, min(max_val, new_val))

class IEC104Connection:
    """IEC104 connection handler"""
    
    def __init__(self, reader, writer, simulator):
        self.reader = reader
        self.writer = writer
        self.simulator = simulator
        self.connected = True
        self.test_frame_enabled = False
        self.ssn = 0  # Send sequence number
        self.rsn = 0  # Receive sequence number
        self.k = 12   # Max unacknowledged I-frames
        self.w = 8    # Ack after w I-frames
        self.t1 = 15  # Timeout for ack (seconds)
        self.t2 = 10  # Timeout for S-frame (seconds)
        self.t3 = 20  # Timeout for test frames (seconds)
        self.unack_count = 0
        
    async def handle_connection(self):
        """Handle IEC104 connection"""
        log.info(f"New IEC104 connection from {self.writer.get_extra_info('peername')}")
        
        # Start background tasks
        asyncio.create_task(self.send_test_frames())
        asyncio.create_task(self.send_periodic_data())
        
        try:
            while self.connected:
                # Read APDU header (6 bytes)
                header = await self.reader.read(6)
                if len(header) < 6:
                    break
                
                start_byte = header[0]
                apdu_len = header[1]
                
                if start_byte != 0x68:
                    log.error("Invalid start byte")
                    break
                
                # Read control fields
                cf1 = header[2]
                cf2 = header[3]
                cf3 = header[4]
                cf4 = header[5]
                
                # Determine frame type
                if cf1 & 0x01 == 0:  # I-frame
                    await self.handle_i_frame(apdu_len, cf1, cf2, cf3, cf4)
                elif cf1 & 0x03 == 0x01:  # S-frame
                    await self.handle_s_frame(cf1, cf2, cf3, cf4)
                elif cf1 & 0x03 == 0x03:  # U-frame
                    await self.handle_u_frame(cf1)
                
        except asyncio.CancelledError:
            pass
        except Exception as e:
            log.error(f"Connection error: {e}")
        finally:
            self.connected = False
            self.writer.close()
            await self.writer.wait_closed()
            log.info("IEC104 connection closed")
    
    async def handle_i_frame(self, apdu_len, cf1, cf2, cf3, cf4):
        """Handle Information transfer frame"""
        # Extract sequence numbers
        send_num = ((cf2 & 0xFF) << 1) | ((cf1 & 0xFE) >> 1)
        recv_num = ((cf4 & 0xFF) << 1) | ((cf3 & 0xFE) >> 1)
        
        # Update receive sequence number
        self.rsn = (send_num + 1) & 0x7FFF
        
        # Read ASDU
        asdu_len = apdu_len - 4  # Subtract control field length
        if asdu_len > 0:
            asdu = await self.reader.read(asdu_len)
            await self.handle_asdu(asdu)
        
        # Send S-frame acknowledgment if needed
        self.unack_count += 1
        if self.unack_count >= self.w:
            await self.send_s_frame()
    
    async def handle_s_frame(self, cf1, cf2, cf3, cf4):
        """Handle Supervisory frame"""
        recv_num = ((cf4 & 0xFF) << 1) | ((cf3 & 0xFE) >> 1)
        log.debug(f"Received S-frame, ACK up to {recv_num}")
    
    async def handle_u_frame(self, cf1):
        """Handle Unnumbered control frame"""
        if cf1 & 0xFC == 0x04:  # STARTDT act
            log.info("Received STARTDT act")
            await self.send_u_frame(0x08)  # STARTDT con
            self.test_frame_enabled = True
        elif cf1 & 0xFC == 0x10:  # STOPDT act
            log.info("Received STOPDT act")
            await self.send_u_frame(0x20)  # STOPDT con
            self.test_frame_enabled = False
        elif cf1 & 0xFC == 0x40:  # TESTFR act
            log.debug("Received TESTFR act")
            await self.send_u_frame(0x80)  # TESTFR con
    
    async def handle_asdu(self, asdu):
        """Handle Application Service Data Unit"""
        if len(asdu) < 6:
            return
        
        type_id = asdu[0]
        vsq = asdu[1]
        num_objects = vsq & 0x7F
        sq = (vsq & 0x80) != 0
        cot = asdu[2] & 0x3F
        test = (asdu[2] & 0x80) != 0
        pn = (asdu[2] & 0x40) != 0
        originator = asdu[3]
        common_addr = struct.unpack('<H', asdu[4:6])[0]
        
        log.info(f"ASDU: TypeID={type_id}, NumObj={num_objects}, COT={cot}, CA={common_addr}")
        
        # Handle specific type IDs
        if type_id == TypeID.C_IC_NA_1.value:  # General Interrogation
            await self.handle_interrogation(common_addr, originator)
        elif type_id == TypeID.C_CS_NA_1.value:  # Clock sync
            await self.handle_clock_sync(asdu[6:])
        elif type_id in [TypeID.C_SC_NA_1.value, TypeID.C_DC_NA_1.value]:  # Commands
            await self.handle_command(type_id, asdu[6:], num_objects, sq)
    
    async def handle_interrogation(self, common_addr, originator):
        """Handle general interrogation command"""
        log.info("Handling general interrogation")
        
        # Send activation confirmation
        await self.send_activation_confirmation(TypeID.C_IC_NA_1, common_addr, originator)
        
        # Send all data points
        for ioa, point in self.simulator.iec_points.items():
            value = self.simulator.get_current_value(ioa)
            await self.send_measurement(point, value, Cause.INTERROGATION, common_addr)
        
        # Send activation termination
        await self.send_activation_termination(TypeID.C_IC_NA_1, common_addr, originator)
    
    async def handle_clock_sync(self, data):
        """Handle clock synchronization"""
        if len(data) >= 7:
            # CP56Time2a format
            ms = struct.unpack('<H', data[0:2])[0]
            minute = data[2] & 0x3F
            hour = data[3] & 0x1F
            day = data[4] & 0x1F
            month = data[5] & 0x0F
            year = data[6] & 0x7F
            
            log.info(f"Clock sync: {year+2000:04d}-{month:02d}-{day:02d} {hour:02d}:{minute:02d}:{ms/1000:.3f}")
    
    async def handle_command(self, type_id, data, num_objects, sq):
        """Handle control commands"""
        # Parse IOA and command value
        if len(data) >= 4:
            ioa = struct.unpack('<I', data[0:3] + b'\x00')[0]
            
            if type_id == TypeID.C_SC_NA_1.value:  # Single command
                sco = data[3]
                state = sco & 0x01
                select = (sco & 0x80) != 0
                log.info(f"Single command: IOA={ioa}, State={state}, Select={select}")
            elif type_id == TypeID.C_DC_NA_1.value:  # Double command
                dco = data[3]
                state = dco & 0x03
                select = (dco & 0x80) != 0
                log.info(f"Double command: IOA={ioa}, State={state}, Select={select}")
    
    async def send_i_frame(self, asdu):
        """Send Information transfer frame"""
        # Build control fields
        cf1 = (self.ssn << 1) & 0xFE
        cf2 = (self.ssn >> 7) & 0xFF
        cf3 = (self.rsn << 1) & 0xFE
        cf4 = (self.rsn >> 7) & 0xFF
        
        # Build APDU
        apdu = struct.pack('BBBBBB', 0x68, len(asdu) + 4, cf1, cf2, cf3, cf4) + asdu
        
        # Send frame
        self.writer.write(apdu)
        await self.writer.drain()
        
        # Update sequence number
        self.ssn = (self.ssn + 1) & 0x7FFF
    
    async def send_s_frame(self):
        """Send Supervisory frame"""
        cf1 = 0x01  # S-frame
        cf2 = 0x00
        cf3 = (self.rsn << 1) & 0xFE
        cf4 = (self.rsn >> 7) & 0xFF
        
        frame = struct.pack('BBBBBB', 0x68, 4, cf1, cf2, cf3, cf4)
        self.writer.write(frame)
        await self.writer.drain()
        
        self.unack_count = 0
        log.debug(f"Sent S-frame, ACK up to {self.rsn}")
    
    async def send_u_frame(self, function):
        """Send Unnumbered control frame"""
        frame = struct.pack('BBBBBB', 0x68, 4, function | 0x03, 0, 0, 0)
        self.writer.write(frame)
        await self.writer.drain()
    
    async def send_test_frames(self):
        """Send periodic test frames"""
        while self.connected:
            await asyncio.sleep(self.t3)
            if self.test_frame_enabled:
                await self.send_u_frame(0x40)  # TESTFR act
                log.debug("Sent TESTFR act")
    
    async def send_periodic_data(self):
        """Send periodic measurements"""
        while self.connected:
            await asyncio.sleep(5)  # Send data every 5 seconds
            
            if self.test_frame_enabled:
                # Send spontaneous changes
                for ioa, point in self.simulator.iec_points.items():
                    if random.random() < 0.2:  # 20% chance
                        value = self.simulator.get_current_value(ioa)
                        await self.send_measurement(point, value, Cause.SPONTANEOUS, 1)
    
    async def send_measurement(self, point: IECPoint, value: float, cause: Cause, common_addr: int):
        """Send measurement value"""
        # Build ASDU header
        asdu = struct.pack('BBBBH', 
            point.type_id.value,  # Type ID
            0x01,  # VSQ: 1 object, no sequence
            cause.value,  # COT
            0x00,  # Originator address
            common_addr  # Common address
        )
        
        # Add Information Object
        ioa_bytes = struct.pack('<I', point.ioa)[:3]  # 3-byte IOA
        
        if point.type_id == TypeID.M_SP_NA_1:  # Single point
            spi = int(value) & 0x01
            quality = point.quality & 0xF0
            asdu += ioa_bytes + struct.pack('B', spi | quality)
            
        elif point.type_id == TypeID.M_ME_NC_1:  # Float value
            asdu += ioa_bytes + struct.pack('<fB', value, point.quality)
            
        elif point.type_id == TypeID.M_ME_NB_1:  # Scaled value
            scaled = int(value)
            asdu += ioa_bytes + struct.pack('<hB', scaled, point.quality)
        
        await self.send_i_frame(asdu)
    
    async def send_activation_confirmation(self, type_id: TypeID, common_addr: int, originator: int):
        """Send activation confirmation"""
        asdu = struct.pack('BBBBH', 
            type_id.value,
            0x01,
            Cause.ACTIVATION_CON.value,
            originator,
            common_addr
        )
        asdu += b'\x00\x00\x00\x14'  # IOA=0, QOI=20 (station interrogation)
        
        await self.send_i_frame(asdu)
    
    async def send_activation_termination(self, type_id: TypeID, common_addr: int, originator: int):
        """Send activation termination"""
        asdu = struct.pack('BBBBH', 
            type_id.value,
            0x01,
            Cause.ACTIVATION_TERM.value,
            originator,
            common_addr
        )
        asdu += b'\x00\x00\x00\x14'  # IOA=0, QOI=20
        
        await self.send_i_frame(asdu)

class IEC104Simulator:
    """IEC104 server simulator"""
    
    def __init__(self, host: str, port: int):
        self.host = host
        self.port = port
        self.server = None
        self.connections = []
        self.start_time = time.time()
        self.iec_points: Dict[int, IECPoint] = {}
        self.current_values = {}
        
        # Initialize test points
        self.init_test_points()
    
    def init_test_points(self):
        """Initialize IEC104 test data points"""
        # Single point information (YX - 遥信)
        self.add_point(IECPoint(
            ioa=1001,
            type_id=TypeID.M_SP_NA_1,
            name="Breaker_1_Status",
            pattern="square",
            min_value=0,
            max_value=1,
            period=120.0
        ))
        
        self.add_point(IECPoint(
            ioa=1002,
            type_id=TypeID.M_SP_NA_1,
            name="Breaker_2_Status",
            pattern="square",
            min_value=0,
            max_value=1,
            period=180.0
        ))
        
        # Measured values - float (YC - 遥测)
        self.add_point(IECPoint(
            ioa=2001,
            type_id=TypeID.M_ME_NC_1,
            name="Voltage_A",
            pattern="sine",
            min_value=210.0,
            max_value=230.0,
            period=60.0
        ))
        
        self.add_point(IECPoint(
            ioa=2002,
            type_id=TypeID.M_ME_NC_1,
            name="Voltage_B",
            pattern="sine",
            min_value=210.0,
            max_value=230.0,
            period=60.0
        ))
        
        self.add_point(IECPoint(
            ioa=2003,
            type_id=TypeID.M_ME_NC_1,
            name="Voltage_C",
            pattern="sine",
            min_value=210.0,
            max_value=230.0,
            period=60.0
        ))
        
        self.add_point(IECPoint(
            ioa=2011,
            type_id=TypeID.M_ME_NC_1,
            name="Current_A",
            pattern="random_walk",
            min_value=0.0,
            max_value=100.0,
            period=1.0
        ))
        
        self.add_point(IECPoint(
            ioa=2021,
            type_id=TypeID.M_ME_NC_1,
            name="Active_Power",
            pattern="sine",
            min_value=0.0,
            max_value=5000.0,
            period=120.0
        ))
        
        self.add_point(IECPoint(
            ioa=2031,
            type_id=TypeID.M_ME_NC_1,
            name="Frequency",
            pattern="sine",
            min_value=49.8,
            max_value=50.2,
            period=30.0
        ))
        
        # Scaled values (YC - 遥测)
        self.add_point(IECPoint(
            ioa=3001,
            type_id=TypeID.M_ME_NB_1,
            name="Temperature",
            pattern="sine",
            min_value=200,  # 20.0°C * 10
            max_value=350,  # 35.0°C * 10
            period=300.0
        ))
    
    def add_point(self, point: IECPoint):
        """Add an IEC104 data point"""
        self.iec_points[point.ioa] = point
        self.current_values[point.ioa] = (point.min_value + point.max_value) / 2
    
    def get_current_value(self, ioa: int) -> float:
        """Get current value for a point"""
        if ioa not in self.iec_points:
            return 0.0
        
        point = self.iec_points[ioa]
        current_time = time.time() - self.start_time
        
        if point.pattern == "sine":
            value = DataPattern.sine_wave(current_time, point.min_value, point.max_value, point.period)
        elif point.pattern == "square":
            value = DataPattern.square_wave(current_time, point.min_value, point.max_value, point.period)
        elif point.pattern == "sawtooth":
            value = DataPattern.sawtooth(current_time, point.min_value, point.max_value, point.period)
        elif point.pattern == "random_walk":
            current = self.current_values[ioa]
            step = (point.max_value - point.min_value) * 0.05
            value = DataPattern.random_walk(current, point.min_value, point.max_value, step)
        elif point.pattern == "constant":
            value = point.min_value
        else:
            value = point.min_value
        
        self.current_values[ioa] = value
        return value
    
    async def handle_client(self, reader, writer):
        """Handle a client connection"""
        connection = IEC104Connection(reader, writer, self)
        self.connections.append(connection)
        
        try:
            await connection.handle_connection()
        finally:
            self.connections.remove(connection)
    
    async def start(self):
        """Start the IEC104 server"""
        self.server = await asyncio.start_server(
            self.handle_client, self.host, self.port
        )
        
        log.info(f"IEC104 server listening on {self.host}:{self.port}")
        
        async with self.server:
            await self.server.serve_forever()

async def main():
    parser = argparse.ArgumentParser(description='IEC 60870-5-104 Server Simulator')
    parser.add_argument('--host', default='0.0.0.0', help='Server host address')
    parser.add_argument('--port', type=int, default=2404, help='Server port')
    parser.add_argument('--debug', action='store_true', help='Enable debug logging')
    
    args = parser.parse_args()
    
    # Configure logging
    if args.debug:
        log.setLevel(logging.DEBUG)
    else:
        log.setLevel(logging.INFO)
    
    # Create and start simulator
    simulator = IEC104Simulator(args.host, args.port)
    
    try:
        await simulator.start()
    except KeyboardInterrupt:
        log.info("Shutting down IEC104 server...")

if __name__ == '__main__':
    asyncio.run(main())