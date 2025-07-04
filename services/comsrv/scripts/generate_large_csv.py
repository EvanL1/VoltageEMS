#!/usr/bin/env python3
"""
Generate large CSV configuration files for stress testing
"""

import csv
import os
import sys

def generate_telemetry_csv(num_points, output_dir):
    """Generate telemetry.csv with specified number of points"""
    filename = os.path.join(output_dir, 'telemetry.csv')
    
    with open(filename, 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(['point_id', 'name', 'description', 'unit', 'data_type', 'scale', 'offset'])
        
        for i in range(1, num_points + 1):
            # Simulate different types of telemetry points
            if i % 10 == 0:
                # Power measurements
                writer.writerow([
                    f'{i}', 
                    f'Power_{i}', 
                    f'Power meter {i}', 
                    'kW', 
                    'float', 
                    '0.001', 
                    '0'
                ])
            elif i % 5 == 0:
                # Current measurements
                writer.writerow([
                    f'{i}', 
                    f'Current_{i}', 
                    f'Current sensor {i}', 
                    'A', 
                    'float', 
                    '0.01', 
                    '0'
                ])
            else:
                # Voltage measurements
                writer.writerow([
                    f'{i}', 
                    f'Voltage_{i}', 
                    f'Voltage sensor {i}', 
                    'V', 
                    'float', 
                    '0.1', 
                    '0'
                ])
    
    print(f"Generated {filename} with {num_points} telemetry points")

def generate_signal_csv(num_points, output_dir):
    """Generate signal.csv with specified number of points"""
    filename = os.path.join(output_dir, 'signal.csv')
    
    with open(filename, 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(['point_id', 'name', 'description', 'unit', 'data_type', 'scale', 'offset'])
        
        for i in range(1, num_points + 1):
            writer.writerow([
                f'{2000 + i}', 
                f'Status_{i}', 
                f'Device status {i}', 
                '', 
                'bool', 
                '1', 
                '0'
            ])
    
    print(f"Generated {filename} with {num_points} signal points")

def generate_mapping_telemetry_csv(num_points, output_dir):
    """Generate mapping_telemetry.csv with Modbus addresses"""
    filename = os.path.join(output_dir, 'mapping_telemetry.csv')
    
    with open(filename, 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(['point_id', 'signal_name', 'address', 'data_type', 'data_format', 'number_of_bytes', 'scale', 'offset'])
        
        base_address = 40001
        for i in range(1, num_points + 1):
            # Distribute across multiple slaves for realistic scenario
            slave_id = ((i - 1) // 200) + 1  # 200 points per slave
            register_offset = ((i - 1) % 200) * 2  # 2 registers per point
            
            # Format: slave_id:function_code:register_address
            address = f"{slave_id}:3:{base_address + register_offset}"
            
            if i % 10 == 0:
                # Power - float32 (4 bytes)
                writer.writerow([
                    f'{i}',
                    f'Power_{i}',
                    address,
                    'float32',
                    'ABCD',
                    '4',
                    '0.001',
                    '0'
                ])
            else:
                # Voltage/Current - uint16 (2 bytes)
                writer.writerow([
                    f'{i}',
                    f'Voltage_{i}' if i % 5 != 0 else f'Current_{i}',
                    address,
                    'uint16',
                    'AB',
                    '2',
                    '0.1' if i % 5 != 0 else '0.01',
                    '0'
                ])
    
    print(f"Generated {filename} with {num_points} telemetry mappings")

def generate_mapping_signal_csv(num_points, output_dir):
    """Generate mapping_signal.csv with Modbus addresses"""
    filename = os.path.join(output_dir, 'mapping_signal.csv')
    
    with open(filename, 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(['point_id', 'signal_name', 'address', 'data_type', 'data_format', 'number_of_bytes', 'scale', 'offset'])
        
        base_address = 10001
        for i in range(1, num_points + 1):
            # Distribute across slaves
            slave_id = ((i - 1) // 200) + 1
            coil_offset = (i - 1) % 200
            
            # Format: slave_id:function_code:coil_address
            address = f"{slave_id}:1:{base_address + coil_offset}"
            
            writer.writerow([
                f'{2000 + i}',
                f'Status_{i}',
                address,
                'bool',
                '',
                '1',
                '1',
                '0'
            ])
    
    print(f"Generated {filename} with {num_points} signal mappings")

def main():
    if len(sys.argv) < 2:
        print("Usage: python generate_large_csv.py <output_directory> [num_telemetry_points] [num_signal_points]")
        sys.exit(1)
    
    output_dir = sys.argv[1]
    num_telemetry = int(sys.argv[2]) if len(sys.argv) > 2 else 1000
    num_signal = int(sys.argv[3]) if len(sys.argv) > 3 else 500
    
    # Create output directory if it doesn't exist
    os.makedirs(output_dir, exist_ok=True)
    
    print(f"\nGenerating CSV files for stress testing:")
    print(f"  Output directory: {output_dir}")
    print(f"  Telemetry points: {num_telemetry}")
    print(f"  Signal points: {num_signal}")
    print()
    
    # Generate all CSV files
    generate_telemetry_csv(num_telemetry, output_dir)
    generate_signal_csv(num_signal, output_dir)
    generate_mapping_telemetry_csv(num_telemetry, output_dir)
    generate_mapping_signal_csv(num_signal, output_dir)
    
    # Generate empty control and adjustment files
    for filename in ['control.csv', 'adjustment.csv', 'mapping_control.csv', 'mapping_adjustment.csv']:
        filepath = os.path.join(output_dir, filename)
        with open(filepath, 'w', newline='') as f:
            writer = csv.writer(f)
            if filename.startswith('mapping_'):
                writer.writerow(['point_id', 'signal_name', 'address', 'data_type', 'data_format', 'number_of_bytes', 'scale', 'offset'])
            else:
                writer.writerow(['point_id', 'name', 'description', 'unit', 'data_type', 'scale', 'offset'])
        print(f"Generated empty {filepath}")
    
    print(f"\n✅ Successfully generated CSV configuration for {num_telemetry + num_signal} total points!")
    print(f"\nConfiguration details:")
    print(f"  - Slaves: {(num_telemetry - 1) // 200 + 1}")
    print(f"  - Points per slave: up to 200")
    print(f"  - Telemetry register range: 40001-{40001 + num_telemetry * 2}")
    print(f"  - Signal coil range: 10001-{10001 + num_signal}")

if __name__ == '__main__':
    main()