#!/usr/bin/env python3
"""
Generate point table configurations for comsrv pressure testing
Creates 5000 points per channel for comprehensive pressure testing
"""

import csv
import os
from pathlib import Path

def generate_modbus_points(channel_id, base_address=0, count=5000):
    """Generate modbus point mappings for a channel"""
    points = []
    
    # Generate different types of points for comprehensive testing
    for i in range(count):
        point_id = f"CH{channel_id:02d}_PT{i:04d}"
        address = base_address + i
        
        # Distribute different register types and data types
        if i < 1000:
            # Coils (0-999)
            point = {
                'name': f"{point_id}_COIL",
                'display_name': f"Channel {channel_id} Coil {i}",
                'register_type': 'coil',
                'address': address,
                'data_type': 'bool',
                'scale': 1.0,
                'offset': 0.0,
                'unit': '',
                'description': f"Coil point {i} for channel {channel_id}",
                'access_mode': 'read_write',
                'group': 'digital_outputs',
                'byte_order': 'big_endian'
            }
        elif i < 2000:
            # Discrete Inputs (1000-1999)
            point = {
                'name': f"{point_id}_DI",
                'display_name': f"Channel {channel_id} Discrete Input {i-1000}",
                'register_type': 'discrete_input',
                'address': address - 1000,
                'data_type': 'bool',
                'scale': 1.0,
                'offset': 0.0,
                'unit': '',
                'description': f"Discrete input {i-1000} for channel {channel_id}",
                'access_mode': 'read',
                'group': 'digital_inputs',
                'byte_order': 'big_endian'
            }
        elif i < 3500:
            # Input Registers (2000-3499) - Various data types
            reg_offset = i - 2000
            if reg_offset < 500:
                # UInt16 registers
                point = {
                    'name': f"{point_id}_IR_U16",
                    'display_name': f"Channel {channel_id} Input Register UInt16 {reg_offset}",
                    'register_type': 'input_register',
                    'address': reg_offset,
                    'data_type': 'uint16',
                    'scale': 1.0,
                    'offset': 0.0,
                    'unit': 'count',
                    'description': f"UInt16 input register {reg_offset} for channel {channel_id}",
                    'access_mode': 'read',
                    'group': 'analog_inputs',
                    'byte_order': 'big_endian'
                }
            elif reg_offset < 1000:
                # Int16 registers with scaling
                point = {
                    'name': f"{point_id}_IR_I16",
                    'display_name': f"Channel {channel_id} Input Register Int16 {reg_offset-500}",
                    'register_type': 'input_register',
                    'address': reg_offset,
                    'data_type': 'int16',
                    'scale': 0.1,
                    'offset': -100.0,
                    'unit': '°C',
                    'description': f"Temperature sensor {reg_offset-500} for channel {channel_id}",
                    'access_mode': 'read',
                    'group': 'temperature',
                    'byte_order': 'big_endian'
                }
            else:
                # Float32 registers (2 registers each)
                point = {
                    'name': f"{point_id}_IR_F32",
                    'display_name': f"Channel {channel_id} Input Register Float32 {reg_offset-1000}",
                    'register_type': 'input_register',
                    'address': reg_offset,
                    'data_type': 'float32',
                    'scale': 1.0,
                    'offset': 0.0,
                    'unit': 'kW',
                    'description': f"Power measurement {reg_offset-1000} for channel {channel_id}",
                    'access_mode': 'read',
                    'group': 'power',
                    'byte_order': 'big_endian'
                }
        else:
            # Holding Registers (3500-4999) - Various data types
            reg_offset = i - 3500
            if reg_offset < 500:
                # UInt16 holding registers
                point = {
                    'name': f"{point_id}_HR_U16",
                    'display_name': f"Channel {channel_id} Holding Register UInt16 {reg_offset}",
                    'register_type': 'holding_register',
                    'address': reg_offset + 1000,  # Start at address 1000
                    'data_type': 'uint16',
                    'scale': 1.0,
                    'offset': 0.0,
                    'unit': 'count',
                    'description': f"UInt16 holding register {reg_offset} for channel {channel_id}",
                    'access_mode': 'read_write',
                    'group': 'control_registers',
                    'byte_order': 'big_endian'
                }
            elif reg_offset < 1000:
                # Int32 holding registers
                point = {
                    'name': f"{point_id}_HR_I32",
                    'display_name': f"Channel {channel_id} Holding Register Int32 {reg_offset-500}",
                    'register_type': 'holding_register',
                    'address': reg_offset + 1000,
                    'data_type': 'int32',
                    'scale': 0.01,
                    'offset': 0.0,
                    'unit': 'A',
                    'description': f"Current setpoint {reg_offset-500} for channel {channel_id}",
                    'access_mode': 'read_write',
                    'group': 'current_control',
                    'byte_order': 'big_endian'
                }
            else:
                # Float32 holding registers
                point = {
                    'name': f"{point_id}_HR_F32",
                    'display_name': f"Channel {channel_id} Holding Register Float32 {reg_offset-1000}",
                    'register_type': 'holding_register',
                    'address': reg_offset + 1000,
                    'data_type': 'float32',
                    'scale': 1.0,
                    'offset': 0.0,
                    'unit': 'V',
                    'description': f"Voltage setpoint {reg_offset-1000} for channel {channel_id}",
                    'access_mode': 'read_write',
                    'group': 'voltage_control',
                    'byte_order': 'big_endian'
                }
        
        points.append(point)
    
    return points

def write_points_to_csv(points, filename):
    """Write points to CSV file"""
    fieldnames = [
        'name', 'display_name', 'register_type', 'address', 'data_type',
        'scale', 'offset', 'unit', 'description', 'access_mode', 'group', 'byte_order'
    ]
    
    with open(filename, 'w', newline='', encoding='utf-8') as csvfile:
        writer = csv.DictWriter(csvfile, fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(points)

def main():
    """Generate point tables for all pressure test channels"""
    base_path = Path("../config/points")
    base_path.mkdir(exist_ok=True)
    
    # Generate for 12 channels (matching pressure test config)
    total_points = 0
    
    for channel_id in range(1, 13):  # Channels 1-12
        points = generate_modbus_points(channel_id, base_address=0, count=5000)
        filename = base_path / f"pressure_test_channel_{channel_id:02d}.csv"
        write_points_to_csv(points, filename)
        total_points += len(points)
        print(f"✅ Generated {len(points)} points for channel {channel_id}: {filename}")
    
    print(f"\n🎯 Total points generated: {total_points:,}")
    print(f"📊 Average points per channel: {total_points // 12:,}")
    print("\n📁 Point table files created in: ../config/points/")

if __name__ == "__main__":
    main() 