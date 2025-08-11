#!/usr/bin/env python3
"""
Enhanced Modbus TCP Server - Supports multiple data types and byte orders
"""

import struct
import socket
import threading
import logging
from typing import Dict, List, Tuple, Any

logging.basicConfig(
    level=logging.DEBUG, format="%(asctime)s - %(levelname)s - %(message)s"
)


class EnhancedModbusServer:
    def __init__(self, host="0.0.0.0", port=502):
        self.host = host
        self.port = port
        self.running = False

        # Store different types of data for multiple slaves
        self.slaves_data = {
            1: self._init_slave1_data(),  # Channel 1001 & 1002
            2: self._init_slave2_data(),  # Channel 1003
        }

        # Coils for control operations (FC5, FC15)
        self.coils = {
            1: [False] * 100,
            2: [False] * 100,
        }

    def _init_slave1_data(self) -> Dict[int, Any]:
        """Initialize data for slave 1 - mixed data types"""
        data = {}

        # Channel 1001 data (float32) - registers 0-49 (25 points) + 250-259 (5 points)
        # First 25 points at registers 0-49
        float_values = [
            100.0,
            200.0,
            300.0,
            400.0,
            500.0,  # Points 1-5
            600.0,
            700.0,
            800.0,
            900.0,
            1000.0,  # Points 6-10
            5000.0,
            10000.0,
            15000.0,
            20000.0,
            25000.0,  # Points 11-15
            30000.0,
            35000.0,
            40000.0,
            45000.0,
            50000.0,  # Points 16-20
            1234.5,
            2345.6,
            3456.7,
            4567.8,
            5678.9,  # Points 21-25
        ]
        for i, val in enumerate(float_values):
            self._write_float32(data, i * 2, val)

        # Points 26-30 at registers 250-259
        extra_values = [
            273.15,  # Point 26: 露点温度原始值
            150.0,  # Point 27: CO浓度
            450.0,  # Point 28: CO2浓度
            2095.0,  # Point 29: O2浓度
            850.0,  # Point 30: 烟气温度
        ]
        for i, val in enumerate(extra_values):
            self._write_float32(data, 250 + i * 2, val)

        # Channel 1002 data starts at register 50 to avoid overlap with Channel 1001
        # Register 50: uint16 (temperature) - raw value 1500 (byte order AB)
        data[50] = 1500

        # Register 51: int16 (temperature) - raw value -200 (byte order BA - swap bytes)
        self._write_int16_ba(data, 51, -200)

        # Registers 52-53: uint32 (pressure) - raw value 3500000 (byte order DCBA)
        self._write_uint32_dcba(data, 52, 3500000)

        # Registers 54-55: int32 (flow) - raw value -15000 (byte order CDAB)
        self._write_int32_cdab(data, 54, -15000)

        # Registers 56-57: float32 (power) - 1234.56 kW (byte order ABCD)
        self._write_float32(data, 56, 1234.56)

        # Registers 58-61: float64 (energy) - 987654.321 MWh (byte order ABCD)
        self._write_float64(data, 58, 987654.321)

        # Register 62: uint16 (speed) - 30000 (byte order AB)
        data[62] = 30000

        # Register 63: int16 (torque) - 15000 (byte order BA)
        self._write_int16_ba(data, 63, 15000)

        # Registers 64-65: uint32 (accumulated) - 456789 (byte order BADC)
        self._write_uint32_badc(data, 64, 456789)

        # Registers 66-67: int32 (position) - -123456 (byte order DCBA)
        self._write_int32_dcba(data, 66, -123456)

        # Signal data - registers 100-103 (bits)
        data[100] = 0b0000000000001011  # bits 0,1,3 are 1
        data[101] = 0b0000000000000101  # bits 0,2 are 1

        # Control/write registers 200-210 (for FC6, FC16)
        for i in range(200, 211):
            data[i] = 0

        # Signal registers 200-203 for channel 1001
        data[200] = 0b0000000000011111  # First 5 bits
        data[201] = 0b0000000000001110  # bits 1,2,3
        data[202] = 0b0000000000001000  # bit 3
        data[203] = 0b0000000000000001  # bit 0

        return data

    def _init_slave2_data(self) -> Dict[int, Any]:
        """Initialize data for slave 2 - complex byte orders"""
        data = {}

        # Registers 0-3: float64 ABCD (temperature) - 298.15 K
        self._write_float64(data, 0, 298.15)

        # Registers 4-7: float64 DCBA (pressure) - 101325.0 Pa
        self._write_float64_dcba(data, 4, 101325.0)

        # Registers 8-9: uint32 ABCD (counter) - 4294967295 (max uint32)
        self._write_uint32(data, 8, 4294967295)

        # Registers 10-11: int32 BADC (position) - -2147483648 (min int32)
        self._write_int32_badc(data, 10, -2147483648)

        # Registers 12-13: float32 BADC - 123.456
        self._write_float32_badc(data, 12, 123.456)

        # Registers 14-15: float32 DCBA - -456.789
        self._write_float32_dcba(data, 14, -456.789)

        # Registers 16-17: uint32 CDAB - 0x12345678 (305419896)
        self._write_uint32_cdab(data, 16, 0x12345678)

        # Registers 18-19: int32 DCBA - -0x12345678 (-305419896)
        self._write_int32_dcba(data, 18, -0x12345678)

        return data

    def _to_signed16(self, value: int) -> int:
        """Convert to signed 16-bit representation"""
        if value < 0:
            return (value + 65536) & 0xFFFF
        return value & 0xFFFF

    def _write_uint16(self, data: Dict, addr: int, value: int):
        """Write uint16 value"""
        data[addr] = value & 0xFFFF

    def _write_int16(self, data: Dict, addr: int, value: int):
        """Write int16 value"""
        data[addr] = self._to_signed16(value)

    def _write_int16_ba(self, data: Dict, addr: int, value: int):
        """Write int16 with BA byte order (swap bytes)"""
        if value < 0:
            value = (value + 65536) & 0xFFFF
        # Swap bytes: BA means low byte first, high byte second
        data[addr] = ((value & 0xFF) << 8) | ((value >> 8) & 0xFF)

    def _write_uint32(self, data: Dict, addr: int, value: int):
        """Write uint32 as two registers (ABCD - big endian)"""
        data[addr] = (value >> 16) & 0xFFFF
        data[addr + 1] = value & 0xFFFF

    def _write_uint32_dcba(self, data: Dict, addr: int, value: int):
        """Write uint32 with DCBA byte order (fully reversed)"""
        bytes_val = struct.pack(">I", value)
        # DCBA: D=bytes[3], C=bytes[2], B=bytes[1], A=bytes[0]
        data[addr] = (bytes_val[3] << 8) | bytes_val[2]  # DC
        data[addr + 1] = (bytes_val[1] << 8) | bytes_val[0]  # BA

    def _write_uint32_badc(self, data: Dict, addr: int, value: int):
        """Write uint32 with BADC byte order"""
        bytes_val = struct.pack(">I", value)
        # BADC: B=bytes[1], A=bytes[0], D=bytes[3], C=bytes[2]
        data[addr] = (bytes_val[1] << 8) | bytes_val[0]  # BA
        data[addr + 1] = (bytes_val[3] << 8) | bytes_val[2]  # DC

    def _write_uint32_cdab(self, data: Dict, addr: int, value: int):
        """Write uint32 with CDAB byte order"""
        bytes_val = struct.pack(">I", value)
        # CDAB: C=bytes[2], D=bytes[3], A=bytes[0], B=bytes[1]
        data[addr] = (bytes_val[2] << 8) | bytes_val[3]  # CD
        data[addr + 1] = (bytes_val[0] << 8) | bytes_val[1]  # AB

    def _write_int32(self, data: Dict, addr: int, value: int):
        """Write int32 as two registers (ABCD - big endian)"""
        if value < 0:
            value = (value + 0x100000000) & 0xFFFFFFFF
        data[addr] = (value >> 16) & 0xFFFF
        data[addr + 1] = value & 0xFFFF

    def _write_int32_cdab(self, data: Dict, addr: int, value: int):
        """Write int32 with CDAB byte order"""
        if value < 0:
            value = (value + 0x100000000) & 0xFFFFFFFF
        bytes_val = struct.pack(">I", value)
        # CDAB: C=bytes[2], D=bytes[3], A=bytes[0], B=bytes[1]
        data[addr] = (bytes_val[2] << 8) | bytes_val[3]  # CD
        data[addr + 1] = (bytes_val[0] << 8) | bytes_val[1]  # AB

    def _write_int32_dcba(self, data: Dict, addr: int, value: int):
        """Write int32 with DCBA byte order (fully reversed)"""
        if value < 0:
            value = (value + 0x100000000) & 0xFFFFFFFF
        bytes_val = struct.pack(">I", value)
        # DCBA: D=bytes[3], C=bytes[2], B=bytes[1], A=bytes[0]
        data[addr] = (bytes_val[3] << 8) | bytes_val[2]  # DC
        data[addr + 1] = (bytes_val[1] << 8) | bytes_val[0]  # BA

    def _write_int32_badc(self, data: Dict, addr: int, value: int):
        """Write int32 with BADC byte order"""
        if value < 0:
            value = (value + 0x100000000) & 0xFFFFFFFF
        bytes_val = struct.pack(">I", value)
        # BADC: B=bytes[1], A=bytes[0], D=bytes[3], C=bytes[2]
        data[addr] = (bytes_val[1] << 8) | bytes_val[0]  # BA
        data[addr + 1] = (bytes_val[3] << 8) | bytes_val[2]  # DC

    def _write_float32(self, data: Dict, addr: int, value: float):
        """Write float32 as two registers (IEEE 754, ABCD - big endian)"""
        bytes_val = struct.pack(">f", value)
        data[addr] = struct.unpack(">H", bytes_val[0:2])[0]
        data[addr + 1] = struct.unpack(">H", bytes_val[2:4])[0]

    def _write_float32_badc(self, data: Dict, addr: int, value: float):
        """Write float32 with BADC byte order"""
        bytes_val = struct.pack(">f", value)
        # BADC: swap bytes within each 16-bit register
        data[addr] = (bytes_val[1] << 8) | bytes_val[0]  # BA
        data[addr + 1] = (bytes_val[3] << 8) | bytes_val[2]  # DC

    def _write_float32_dcba(self, data: Dict, addr: int, value: float):
        """Write float32 with DCBA byte order (fully reversed)"""
        bytes_val = struct.pack(">f", value)
        # DCBA: reverse all bytes
        data[addr] = (bytes_val[3] << 8) | bytes_val[2]  # DC
        data[addr + 1] = (bytes_val[1] << 8) | bytes_val[0]  # BA

    def _write_float64(self, data: Dict, addr: int, value: float):
        """Write float64 as four registers (IEEE 754, ABCD - big endian)"""
        bytes_val = struct.pack(">d", value)
        for i in range(4):
            data[addr + i] = struct.unpack(">H", bytes_val[i * 2 : (i + 1) * 2])[0]

    def _write_float64_dcba(self, data: Dict, addr: int, value: float):
        """Write float64 with DCBA byte order (fully reversed HGFEDCBA)"""
        bytes_val = struct.pack(">d", value)
        # DCBA for float64: ABCDEFGH -> HGFEDCBA (reverse all 8 bytes)
        # bytes_val[0-7] = ABCDEFGH = [40, F8, BC, D0, 00, 00, 00, 00]
        # We want HGFEDCBA = [00, 00, 00, 00, D0, BC, F8, 40]
        # As registers: [0000, 0000, D0BC, F840]
        reversed_bytes = bytes(reversed(bytes_val))
        # Now convert to registers
        for i in range(4):
            data[addr + i] = struct.unpack(">H", reversed_bytes[i * 2 : (i + 1) * 2])[0]

    def handle_read_holding_registers(
        self, slave_id: int, start_addr: int, count: int
    ) -> bytes:
        """Handle FC3 - Read Holding Registers"""
        if slave_id not in self.slaves_data:
            return self.build_exception(3, 0x02)  # Illegal data address

        slave_data = self.slaves_data[slave_id]

        # Check address range
        if start_addr + count > 65536:
            return self.build_exception(3, 0x02)

        # Build response
        byte_count = count * 2
        response = bytes([3, byte_count])

        for addr in range(start_addr, start_addr + count):
            value = slave_data.get(addr, 0)
            response += struct.pack(">H", value)

        logging.debug(
            f"FC3: Slave {slave_id}, Start {start_addr}, Count {count}, Values: {[slave_data.get(addr, 0) for addr in range(start_addr, start_addr + count)]}"
        )
        return response

    def handle_read_coils(self, slave_id: int, start_addr: int, count: int) -> bytes:
        """Handle FC1 - Read Coils"""
        if slave_id not in self.coils:
            return self.build_exception(1, 0x02)

        coil_data = self.coils[slave_id]

        # Build response
        byte_count = (count + 7) // 8
        response = bytes([1, byte_count])

        bytes_data = []
        for byte_idx in range(byte_count):
            byte_val = 0
            for bit_idx in range(8):
                coil_idx = start_addr + byte_idx * 8 + bit_idx
                if coil_idx < start_addr + count and coil_idx < len(coil_data):
                    if coil_data[coil_idx]:
                        byte_val |= 1 << bit_idx
            bytes_data.append(byte_val)

        response += bytes(bytes_data)
        logging.debug(
            f"FC1: Slave {slave_id}, Start {start_addr}, Count {count}, Coils: {coil_data[start_addr : start_addr + count]}"
        )
        return response

    def handle_write_single_coil(self, slave_id: int, addr: int, value: int) -> bytes:
        """Handle FC5 - Write Single Coil"""
        if slave_id not in self.coils:
            return self.build_exception(5, 0x02)

        # Value should be 0xFF00 for ON, 0x0000 for OFF
        coil_value = value == 0xFF00
        self.coils[slave_id][addr] = coil_value

        logging.info(f"FC5: Slave {slave_id}, Addr {addr}, Value {coil_value}")

        # Echo back the request
        return struct.pack(">BHH", 5, addr, value)

    def handle_write_single_register(
        self, slave_id: int, addr: int, value: int
    ) -> bytes:
        """Handle FC6 - Write Single Register"""
        if slave_id not in self.slaves_data:
            return self.build_exception(6, 0x02)

        self.slaves_data[slave_id][addr] = value

        logging.info(f"FC6: Slave {slave_id}, Addr {addr}, Value {value}")

        # Echo back the request
        return struct.pack(">BHH", 6, addr, value)

    def handle_write_multiple_registers(
        self, slave_id: int, start_addr: int, values: List[int]
    ) -> bytes:
        """Handle FC16 - Write Multiple Registers"""
        if slave_id not in self.slaves_data:
            return self.build_exception(16, 0x02)

        for i, value in enumerate(values):
            self.slaves_data[slave_id][start_addr + i] = value

        logging.info(
            f"FC16: Slave {slave_id}, Start {start_addr}, Count {len(values)}, Values {values}"
        )

        # Response: function code, start address, quantity
        return struct.pack(">BHH", 16, start_addr, len(values))

    def build_exception(self, func_code: int, exception_code: int) -> bytes:
        """Build Modbus exception response"""
        return bytes([func_code | 0x80, exception_code])

    def process_request(self, request: bytes) -> Tuple[int, bytes]:
        """Process Modbus request and return response"""
        if len(request) < 8:
            return 0, b""

        # Parse MBAP header
        transaction_id = struct.unpack(">H", request[0:2])[0]
        protocol_id = struct.unpack(">H", request[2:4])[0]
        length = struct.unpack(">H", request[4:6])[0]
        unit_id = request[6]

        # PDU starts at byte 7
        pdu = request[7:]

        if len(pdu) < 1:
            return transaction_id, b""

        function_code = pdu[0]

        logging.debug(
            f"Request: Transaction {transaction_id}, Unit {unit_id}, FC {function_code}"
        )

        # Process based on function code
        if function_code == 3:  # Read Holding Registers
            start_addr = struct.unpack(">H", pdu[1:3])[0]
            count = struct.unpack(">H", pdu[3:5])[0]
            response_pdu = self.handle_read_holding_registers(
                unit_id, start_addr, count
            )

        elif function_code == 1:  # Read Coils
            start_addr = struct.unpack(">H", pdu[1:3])[0]
            count = struct.unpack(">H", pdu[3:5])[0]
            response_pdu = self.handle_read_coils(unit_id, start_addr, count)

        elif function_code == 5:  # Write Single Coil
            addr = struct.unpack(">H", pdu[1:3])[0]
            value = struct.unpack(">H", pdu[3:5])[0]
            response_pdu = self.handle_write_single_coil(unit_id, addr, value)

        elif function_code == 6:  # Write Single Register
            addr = struct.unpack(">H", pdu[1:3])[0]
            value = struct.unpack(">H", pdu[3:5])[0]
            response_pdu = self.handle_write_single_register(unit_id, addr, value)

        elif function_code == 16:  # Write Multiple Registers
            start_addr = struct.unpack(">H", pdu[1:3])[0]
            count = struct.unpack(">H", pdu[3:5])[0]
            byte_count = pdu[5]
            values = []
            for i in range(count):
                idx = 6 + i * 2
                values.append(struct.unpack(">H", pdu[idx : idx + 2])[0])
            response_pdu = self.handle_write_multiple_registers(
                unit_id, start_addr, values
            )

        else:
            # Unsupported function code
            response_pdu = self.build_exception(function_code, 0x01)

        return transaction_id, response_pdu

    def handle_client(self, conn, addr):
        """Handle client connection"""
        logging.info(f"Connection from {addr}")

        try:
            while self.running:
                # Receive request
                request = conn.recv(1024)
                if not request:
                    break

                # Process request
                transaction_id, response_pdu = self.process_request(request)

                if response_pdu:
                    # Build MBAP header for response
                    unit_id = request[6] if len(request) > 6 else 1
                    response_length = len(response_pdu) + 1  # PDU + unit_id

                    response = struct.pack(
                        ">HHHB",
                        transaction_id,
                        0,  # Protocol ID
                        response_length,
                        unit_id,
                    )
                    response += response_pdu

                    conn.send(response)

        except Exception as e:
            logging.error(f"Error handling client {addr}: {e}")
        finally:
            conn.close()
            logging.info(f"Connection closed: {addr}")

    def start(self):
        """Start the Modbus server"""
        self.running = True
        server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        server_socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)

        try:
            server_socket.bind((self.host, self.port))
            server_socket.listen(5)
            logging.info(f"Enhanced Modbus server listening on {self.host}:{self.port}")

            while self.running:
                conn, addr = server_socket.accept()
                client_thread = threading.Thread(
                    target=self.handle_client, args=(conn, addr)
                )
                client_thread.start()

        except KeyboardInterrupt:
            logging.info("Server stopped by user")
        except Exception as e:
            logging.error(f"Server error: {e}")
        finally:
            self.running = False
            server_socket.close()


if __name__ == "__main__":
    server = EnhancedModbusServer()
    server.start()
