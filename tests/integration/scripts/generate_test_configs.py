#!/usr/bin/env python3
"""
Generate test configurations for different phases of integration testing
Creates channel configs and CSV point tables
"""

import os
import sys
import yaml
import csv
import random
from typing import Dict, List, Any
from pathlib import Path

# Test phase configurations
PHASE_CONFIGS = {
    'phase1': {
        'channels': {
            'modbus_tcp': 3,
            'modbus_rtu': 2,
            'can': 3,
            'iec104': 2
        },
        'points_per_channel': 50,
        'description': 'Functional testing - 10 channels, 500 points'
    },
    'phase2': {
        'channels': {
            'modbus_tcp': 10,
            'modbus_rtu': 5,
            'can': 8,
            'iec104': 7
        },
        'points_per_channel': 100,
        'description': 'Scale testing - 30 channels, 3000 points'
    },
    'phase3': {
        'channels': {
            'modbus_tcp': 15,
            'modbus_rtu': 10,
            'can': 15,
            'iec104': 10
        },
        'points_per_channel': 200,
        'description': 'Stress testing - 50 channels, 10000 points'
    }
}

# Industrial data patterns
DATA_PATTERNS = {
    'voltage': {'min': 210, 'max': 230, 'unit': 'V', 'type': 'float32'},
    'current': {'min': 0, 'max': 100, 'unit': 'A', 'type': 'float32'},
    'power': {'min': 0, 'max': 5000, 'unit': 'kW', 'type': 'float32'},
    'power_factor': {'min': 0.85, 'max': 0.99, 'unit': '', 'type': 'float32'},
    'frequency': {'min': 49.5, 'max': 50.5, 'unit': 'Hz', 'type': 'float32'},
    'temperature': {'min': 20, 'max': 80, 'unit': '°C', 'type': 'float32'},
    'pressure': {'min': 0, 'max': 10, 'unit': 'bar', 'type': 'float32'},
    'flow_rate': {'min': 0, 'max': 1000, 'unit': 'm³/h', 'type': 'float32'},
    'level': {'min': 0, 'max': 100, 'unit': '%', 'type': 'float32'},
    'speed': {'min': 0, 'max': 3000, 'unit': 'rpm', 'type': 'uint16'},
    'status': {'min': 0, 'max': 1, 'unit': '', 'type': 'bool'},
    'alarm': {'min': 0, 'max': 1, 'unit': '', 'type': 'bool'},
    'command': {'min': 0, 'max': 1, 'unit': '', 'type': 'bool'},
    'setpoint': {'min': 0, 'max': 1000, 'unit': '', 'type': 'uint16'}
}

class TestConfigGenerator:
    def __init__(self, phase: str, output_dir: str):
        self.phase = phase
        self.config = PHASE_CONFIGS[phase]
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)
        
    def generate_all(self):
        """Generate all configurations for the phase"""
        print(f"\nGenerating {self.phase} configurations:")
        print(f"  {self.config['description']}")
        
        # Generate main config
        main_config = self.generate_main_config()
        config_file = self.output_dir / 'config.yaml'
        with open(config_file, 'w') as f:
            yaml.dump(main_config, f, default_flow_style=False)
        print(f"  Created: {config_file}")
        
        # Generate channel configs
        channel_id = 1
        for protocol, count in self.config['channels'].items():
            for i in range(count):
                self.generate_channel_config(protocol, channel_id, i)
                channel_id += 1
        
        print(f"  Total channels created: {channel_id - 1}")
        print(f"  Total points created: {(channel_id - 1) * self.config['points_per_channel']}")
    
    def generate_main_config(self) -> Dict[str, Any]:
        """Generate main comsrv configuration"""
        return {
            'version': '1.0',  # 添加 version 字段
            'service': {
                'name': f'comsrv_{self.phase}',
                'api': {
                    'enabled': True,
                    'bind_address': '0.0.0.0:3000'
                },
                'redis': {
                    'url': 'redis://host.docker.internal:6379',
                    'prefix': f'{self.phase}:'
                },
                'logging': {
                    'level': 'info',
                    'file': f'/app/logs/service/comsrv_{self.phase}.log',
                    'max_size': 10485760,  # 10MB
                    'max_files': 5,
                    'console': True
                }
            },
            'channels': self._generate_channel_list()
        }
    
    def _generate_channel_list(self) -> List[Dict[str, Any]]:
        """Generate list of channel configurations"""
        channels = []
        channel_id = 1
        
        for protocol, count in self.config['channels'].items():
            for i in range(count):
                channel = {
                    'id': channel_id,  # 使用数字 ID
                    'name': f'{protocol.upper()} Channel {i+1}',
                    'protocol': protocol,  # 直接使用协议名，不替换
                    'enabled': True,
                    'parameters': self._get_protocol_params(protocol, i),
                    'polling': {
                        'enabled': True,
                        'interval_ms': 1000,
                        'batch_enabled': True,
                        'max_batch_size': 125
                    },
                    'table_config': {
                        'four_telemetry_route': f'Channel_{channel_id}',
                        'four_telemetry_files': {
                            'telemetry_file': 'telemetry.csv',
                            'signal_file': 'signal.csv',
                            'adjustment_file': 'adjustment.csv',
                            'control_file': 'control.csv'
                        },
                        'protocol_mapping_route': f'Channel_{channel_id}',
                        'protocol_mapping_files': {
                            'telemetry_mapping': 'mapping_telemetry.csv',
                            'signal_mapping': 'mapping_signal.csv',
                            'adjustment_mapping': 'mapping_adjustment.csv',
                            'control_mapping': 'mapping_control.csv'
                        }
                    },
                    'logging': {
                        'enabled': True,
                        'level': 'info',
                        'log_dir': f'/app/logs/channels/channel_{channel_id}',
                        'max_file_size': 5242880,  # 5MB
                        'max_files': 3,
                        'retention_days': 7,
                        'console_output': True,
                        'log_messages': True
                    }
                }
                channels.append(channel)
                channel_id += 1
        
        return channels
    
    def _get_protocol_params(self, protocol: str, index: int) -> Dict[str, Any]:
        """Get protocol-specific parameters"""
        if protocol == 'modbus_tcp':
            return {
                'transport': 'tcp',  # 添加 transport 类型
                'host': 'modbus_tcp_simulator',
                'port': 502,
                'timeout': 5000,
                'retry_count': 3,
                'retry_delay': 1000
            }
        elif protocol == 'modbus_rtu':
            return {
                'transport': 'serial',  # 添加 transport 类型
                'port_name': '/dev/ttyUSB0',
                'baud_rate': 9600,
                'data_bits': 8,
                'parity': 'N',
                'stop_bits': 1,
                'timeout': 3000,
                'retry_count': 3,
                'retry_delay': 500
            }
        elif protocol == 'can':
            return {
                'interface': 'vcan0',
                'bitrate': 250000,
                'timeout': 1000
            }
        elif protocol == 'iec104':
            return {
                'transport': 'tcp',  # 添加 transport 类型
                'host': 'iec104_simulator', 
                'port': 2404,
                'station_address': index + 1,
                'common_addr': index + 1,  # 添加 common_addr
                'timeout': 15000,
                'k': 12,
                'w': 8,
                't1': 15,
                't2': 10,
                't3': 20
            }
        return {}
    
    def generate_channel_config(self, protocol: str, channel_id: int, index: int):
        """Generate individual channel configuration"""
        channel_dir = self.output_dir / 'channels'
        channel_dir.mkdir(exist_ok=True)
        
        config = {
            'channel': {
                'id': channel_id,  # 使用数字 ID
                'name': f'{protocol.upper()} Channel {index+1}',
                'protocol': protocol,
                'description': f'Test channel for {protocol} protocol'
            }
        }
        
        # Protocol-specific parameters
        if protocol == 'modbus_tcp':
            config['connection'] = {
                'host': 'modbus_tcp_simulator',
                'port': 5020 + index if self.phase == 'phase3' else 502,
                'slave_id': index + 1,
                'timeout': 3000,
                'retry_count': 3
            }
        elif protocol == 'modbus_rtu':
            config['connection'] = {
                'port': '/dev/ttyV1',  # Host side of virtual serial
                'baudrate': 9600 if self.phase == 'phase1' else 19200,
                'slave_id': index + 1,
                'timeout': 3000,
                'retry_count': 3,
                'parity': 'N',
                'stopbits': 1,
                'bytesize': 8
            }
        elif protocol == 'can':
            config['connection'] = {
                'interface': 'vcan0',
                'node_id': index + 1,
                'bitrate': 250000,
                'timeout': 1000
            }
        elif protocol == 'iec104':
            config['connection'] = {
                'host': 'iec104_simulator',
                'port': 2404,
                'common_address': index + 1,
                'timeout': 5000,
                'k': 12,
                'w': 8,
                't1': 15,
                't2': 10,
                't3': 20
            }
        
        # CSV table references
        csv_dir = f'csv/channel_{channel_id}'
        config['point_tables'] = {
            'telemetry': f'{csv_dir}/telemetry.csv',
            'control': f'{csv_dir}/control.csv',
            'adjustment': f'{csv_dir}/adjustment.csv',
            'signal': f'{csv_dir}/signal.csv'
        }
        
        # Add channel-level logging for phase2 and phase3
        if self.phase in ['phase2', 'phase3']:
            config['logging'] = {
                'enabled': True,
                'level': 'debug' if self.phase == 'phase3' else 'info',
                'log_dir': f'/logs/channels/channel_{channel_id}',
                'max_file_size': 5242880,  # 5MB
                'max_files': 3,
                'retention_days': 7,
                'log_messages': True  # Log protocol messages
            }
        
        # Save channel config
        config_file = channel_dir / f'channel_{channel_id}.yaml'
        with open(config_file, 'w') as f:
            yaml.dump(config, f, default_flow_style=False)
        
        # Generate CSV tables
        self.generate_csv_tables(channel_id, protocol)
    
    def generate_csv_tables(self, channel_id: int, protocol: str):
        """Generate CSV point tables for a channel"""
        csv_dir = self.output_dir / 'csv' / f'channel_{channel_id}'
        csv_dir.mkdir(parents=True, exist_ok=True)
        
        points_per_type = self.config['points_per_channel'] // 4
        base_point_id = (channel_id - 1) * self.config['points_per_channel']
        
        # Telemetry points (YC)
        self._generate_telemetry_csv(
            csv_dir / 'telemetry.csv',
            base_point_id,
            points_per_type,
            protocol
        )
        
        # Control points (YK)
        self._generate_control_csv(
            csv_dir / 'control.csv',
            base_point_id + points_per_type,
            points_per_type,
            protocol
        )
        
        # Adjustment points (YT)
        self._generate_adjustment_csv(
            csv_dir / 'adjustment.csv',
            base_point_id + points_per_type * 2,
            points_per_type,
            protocol
        )
        
        # Signal points (YX)
        self._generate_signal_csv(
            csv_dir / 'signal.csv',
            base_point_id + points_per_type * 3,
            points_per_type,
            protocol
        )
    
    def _generate_telemetry_csv(self, file_path: Path, start_id: int, count: int, protocol: str):
        """Generate telemetry points CSV"""
        patterns = ['voltage', 'current', 'power', 'power_factor', 'frequency', 
                   'temperature', 'pressure', 'flow_rate', 'level']
        
        with open(file_path, 'w', newline='') as f:
            writer = csv.writer(f)
            writer.writerow(['point_id', 'name', 'address', 'data_type', 'scale', 'offset', 'unit'])
            
            for i in range(count):
                point_id = start_id + i
                pattern = patterns[i % len(patterns)]
                config = DATA_PATTERNS[pattern]
                
                # Generate address based on protocol
                if protocol in ['modbus_tcp', 'modbus_rtu']:
                    address = f"3{i*2+1:04d}"  # Input registers
                elif protocol == 'can':
                    address = f"0x{0x100 + i:03X}"  # CAN ID
                elif protocol == 'iec104':
                    address = f"{3000 + i}"  # IOA
                
                name = f"{pattern}_{i+1}"
                scale = 0.1 if pattern in ['voltage', 'current'] else 1.0
                
                writer.writerow([
                    point_id,
                    name,
                    address,
                    config['type'],
                    scale,
                    0,
                    config['unit']
                ])
    
    def _generate_control_csv(self, file_path: Path, start_id: int, count: int, protocol: str):
        """Generate control points CSV"""
        with open(file_path, 'w', newline='') as f:
            writer = csv.writer(f)
            writer.writerow(['point_id', 'name', 'address', 'data_type'])
            
            for i in range(count):
                point_id = start_id + i
                
                # Generate address based on protocol
                if protocol in ['modbus_tcp', 'modbus_rtu']:
                    address = f"0{i+1:04d}"  # Coils
                elif protocol == 'can':
                    address = f"0x{0x200 + i:03X}"
                elif protocol == 'iec104':
                    address = f"{1000 + i}"
                
                name = f"control_{i+1}"
                
                writer.writerow([
                    point_id,
                    name,
                    address,
                    'bool'
                ])
    
    def _generate_adjustment_csv(self, file_path: Path, start_id: int, count: int, protocol: str):
        """Generate adjustment points CSV"""
        with open(file_path, 'w', newline='') as f:
            writer = csv.writer(f)
            writer.writerow(['point_id', 'name', 'address', 'data_type', 'min_value', 'max_value'])
            
            for i in range(count):
                point_id = start_id + i
                
                # Generate address based on protocol
                if protocol in ['modbus_tcp', 'modbus_rtu']:
                    address = f"4{i+1:04d}"  # Holding registers
                elif protocol == 'can':
                    address = f"0x{0x300 + i:03X}"
                elif protocol == 'iec104':
                    address = f"{5000 + i}"
                
                name = f"setpoint_{i+1}"
                
                writer.writerow([
                    point_id,
                    name,
                    address,
                    'uint16',
                    0,
                    1000
                ])
    
    def _generate_signal_csv(self, file_path: Path, start_id: int, count: int, protocol: str):
        """Generate signal points CSV"""
        signal_types = ['breaker_status', 'alarm_status', 'fault_status', 'mode_status']
        
        with open(file_path, 'w', newline='') as f:
            writer = csv.writer(f)
            writer.writerow(['point_id', 'name', 'address', 'data_type'])
            
            for i in range(count):
                point_id = start_id + i
                signal_type = signal_types[i % len(signal_types)]
                
                # Generate address based on protocol
                if protocol in ['modbus_tcp', 'modbus_rtu']:
                    address = f"1{i+1:04d}"  # Discrete inputs
                elif protocol == 'can':
                    address = f"0x{0x400 + i:03X}"
                elif protocol == 'iec104':
                    address = f"{2000 + i}"
                
                name = f"{signal_type}_{i+1}"
                
                writer.writerow([
                    point_id,
                    name,
                    address,
                    'bool'
                ])

def main():
    """Main entry point"""
    if len(sys.argv) < 3:
        print("Usage: generate_test_configs.py <phase> <output_dir>")
        print("Phases: phase1, phase2, phase3")
        sys.exit(1)
    
    phase = sys.argv[1]
    output_dir = sys.argv[2]
    
    if phase not in PHASE_CONFIGS:
        print(f"Invalid phase: {phase}")
        print(f"Valid phases: {', '.join(PHASE_CONFIGS.keys())}")
        sys.exit(1)
    
    generator = TestConfigGenerator(phase, output_dir)
    generator.generate_all()
    
    print(f"\n✓ Configuration generation complete for {phase}")

if __name__ == "__main__":
    main()