#!/usr/bin/env python3
"""
CAN Bus Simulator for Integration Testing
Simulates multiple CAN nodes with configurable message patterns
"""

import os
import sys
import time
import random
import logging
import threading
import struct
from typing import Dict, List, Any
import can

# Configuration from environment
CAN_INTERFACE = os.getenv('CAN_INTERFACE', 'vcan0')
NODE_COUNT = int(os.getenv('NODE_COUNT', '8'))
MESSAGE_RATE = int(os.getenv('MESSAGE_RATE', '100'))  # Messages per second
POINTS_PER_NODE = int(os.getenv('POINTS_PER_NODE', '50'))
USE_EXTENDED_ID = os.getenv('USE_EXTENDED_ID', 'false').lower() == 'true'
BATCH_MODE = os.getenv('BATCH_MODE', 'false').lower() == 'true'
LOG_LEVEL = os.getenv('LOG_LEVEL', 'INFO')

# Setup logging
logging.basicConfig(
    level=getattr(logging, LOG_LEVEL),
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

class CANNode:
    """Simulates a CAN node with multiple data points"""
    
    def __init__(self, node_id: int):
        self.node_id = node_id
        self.base_can_id = 0x100 + (node_id * 0x10)
        self.data_points = {}
        self.message_counter = 0
        self.init_data_points()
    
    def init_data_points(self):
        """Initialize data points for this node"""
        for i in range(POINTS_PER_NODE):
            point_id = f"node_{self.node_id}_point_{i}"
            
            # Different data types based on point index
            if i < 10:  # Analog values
                self.data_points[point_id] = {
                    'type': 'analog',
                    'value': random.uniform(0, 100),
                    'min': 0,
                    'max': 100,
                    'can_id': self.base_can_id + i
                }
            elif i < 20:  # Digital values
                self.data_points[point_id] = {
                    'type': 'digital',
                    'value': random.randint(0, 1),
                    'can_id': self.base_can_id + i
                }
            elif i < 30:  # Counter values
                self.data_points[point_id] = {
                    'type': 'counter',
                    'value': 0,
                    'max': 65535,
                    'can_id': self.base_can_id + i
                }
            else:  # Status values
                self.data_points[point_id] = {
                    'type': 'status',
                    'value': 0,
                    'can_id': self.base_can_id + i
                }
    
    def update_values(self):
        """Update data point values"""
        for point_id, point in self.data_points.items():
            if point['type'] == 'analog':
                # Random walk
                change = random.uniform(-5, 5)
                new_value = point['value'] + change
                point['value'] = max(point['min'], min(point['max'], new_value))
            
            elif point['type'] == 'digital':
                # Random toggle with low probability
                if random.random() < 0.05:
                    point['value'] = 1 - point['value']
            
            elif point['type'] == 'counter':
                # Increment counter
                point['value'] = (point['value'] + 1) % point['max']
            
            elif point['type'] == 'status':
                # Status bits
                point['value'] = random.randint(0, 255)
    
    def get_messages(self) -> List[can.Message]:
        """Generate CAN messages for current values"""
        messages = []
        
        # Select subset of points to send (simulate real-world bandwidth limits)
        points_to_send = random.sample(
            list(self.data_points.items()),
            min(10, len(self.data_points))
        )
        
        for point_id, point in points_to_send:
            can_id = point['can_id']
            
            if USE_EXTENDED_ID:
                can_id |= 0x80000000  # Set extended ID bit
            
            # Pack data based on type
            if point['type'] == 'analog':
                # Pack as float32
                data = struct.pack('<f', point['value'])
            elif point['type'] == 'digital':
                # Pack as single byte
                data = struct.pack('B', point['value'])
            elif point['type'] == 'counter':
                # Pack as uint16
                data = struct.pack('<H', point['value'])
            elif point['type'] == 'status':
                # Pack as byte
                data = struct.pack('B', point['value'])
            
            # Pad data to 8 bytes (CAN standard)
            data = data[:8].ljust(8, b'\x00')
            
            message = can.Message(
                arbitration_id=can_id,
                data=data,
                is_extended_id=USE_EXTENDED_ID
            )
            messages.append(message)
        
        return messages

class CANSimulator:
    """Main CAN bus simulator"""
    
    def __init__(self):
        self.nodes = []
        self.bus = None
        self.running = False
        self.stats = {
            'messages_sent': 0,
            'errors': 0,
            'start_time': time.time()
        }
    
    def setup_can_interface(self):
        """Setup CAN interface"""
        try:
            # Try to create virtual CAN interface if it doesn't exist
            os.system(f"ip link add dev {CAN_INTERFACE} type vcan 2>/dev/null")
            os.system(f"ip link set up {CAN_INTERFACE}")
            
            # Create CAN bus instance
            self.bus = can.interface.Bus(
                channel=CAN_INTERFACE,
                bustype='socketcan'
            )
            logger.info(f"CAN interface {CAN_INTERFACE} initialized successfully")
            return True
        except Exception as e:
            logger.error(f"Failed to setup CAN interface: {e}")
            return False
    
    def init_nodes(self):
        """Initialize CAN nodes"""
        for i in range(NODE_COUNT):
            node = CANNode(i)
            self.nodes.append(node)
        logger.info(f"Initialized {NODE_COUNT} CAN nodes")
    
    def send_messages(self):
        """Send messages from all nodes"""
        all_messages = []
        
        # Collect messages from all nodes
        for node in self.nodes:
            node.update_values()
            messages = node.get_messages()
            all_messages.extend(messages)
        
        # Send messages with rate limiting
        if BATCH_MODE:
            # Send in batches
            for message in all_messages:
                try:
                    self.bus.send(message)
                    self.stats['messages_sent'] += 1
                except Exception as e:
                    logger.error(f"Failed to send message: {e}")
                    self.stats['errors'] += 1
        else:
            # Send with timing to match MESSAGE_RATE
            interval = 1.0 / MESSAGE_RATE if MESSAGE_RATE > 0 else 0.01
            
            for message in all_messages:
                try:
                    self.bus.send(message)
                    self.stats['messages_sent'] += 1
                    time.sleep(interval)
                except Exception as e:
                    logger.error(f"Failed to send message: {e}")
                    self.stats['errors'] += 1
    
    def print_stats(self):
        """Print statistics"""
        runtime = time.time() - self.stats['start_time']
        rate = self.stats['messages_sent'] / runtime if runtime > 0 else 0
        
        logger.info(
            f"Stats - Messages: {self.stats['messages_sent']}, "
            f"Errors: {self.stats['errors']}, "
            f"Rate: {rate:.2f} msg/s"
        )
    
    def run(self):
        """Main run loop"""
        if not self.setup_can_interface():
            logger.error("Failed to setup CAN interface, exiting")
            return
        
        self.init_nodes()
        self.running = True
        
        logger.info(f"Starting CAN simulation with {NODE_COUNT} nodes at {MESSAGE_RATE} msg/s")
        
        stats_thread = threading.Thread(target=self.stats_loop, daemon=True)
        stats_thread.start()
        
        try:
            while self.running:
                self.send_messages()
                
                # Small delay between cycles
                if not BATCH_MODE:
                    time.sleep(0.1)
        
        except KeyboardInterrupt:
            logger.info("Shutting down CAN simulator...")
        except Exception as e:
            logger.error(f"Error in main loop: {e}")
        finally:
            self.running = False
            if self.bus:
                self.bus.shutdown()
    
    def stats_loop(self):
        """Background thread to print statistics"""
        while self.running:
            time.sleep(10)  # Print stats every 10 seconds
            self.print_stats()

def main():
    simulator = CANSimulator()
    simulator.run()

if __name__ == "__main__":
    main()