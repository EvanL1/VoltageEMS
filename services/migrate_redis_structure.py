#!/usr/bin/env python3
"""
Migration script for Redis data structure optimization.
Migrates from old full JSON structure to new separated config/realtime structure.
"""

import redis
import json
import time
from datetime import datetime
import argparse
import sys

class RedisStructureMigrator:
    def __init__(self, redis_host='localhost', redis_port=6379, dry_run=False):
        self.redis_client = redis.Redis(host=redis_host, port=redis_port, decode_responses=True)
        self.dry_run = dry_run
        self.stats = {
            'channels_processed': 0,
            'points_migrated': 0,
            'errors': 0,
            'start_time': time.time()
        }
    
    def check_connection(self):
        """Check Redis connection"""
        try:
            self.redis_client.ping()
            print("✓ Redis connection successful")
            return True
        except Exception as e:
            print(f"✗ Redis connection failed: {e}")
            return False
    
    def get_old_channels(self):
        """Get all channels using old structure"""
        pattern = "comsrv:realtime:channel:*"
        channels = []
        
        for key in self.redis_client.scan_iter(match=pattern):
            # Check if it's using old structure
            sample_data = self.redis_client.hgetall(key)
            if sample_data:
                # Check first value to determine structure
                first_value = list(sample_data.values())[0]
                try:
                    data = json.loads(first_value)
                    # Old structure has many fields
                    if 'name' in data and 'unit' in data and 'description' in data:
                        channel_id = key.split(':')[-1]
                        channels.append(int(channel_id))
                except:
                    pass
        
        return sorted(channels)
    
    def migrate_channel(self, channel_id):
        """Migrate a single channel from old to new structure"""
        old_key = f"comsrv:realtime:channel:{channel_id}"
        config_key = f"comsrv:config:channel:{channel_id}:points"
        
        print(f"\nMigrating channel {channel_id}...")
        
        # Get old data
        old_data = self.redis_client.hgetall(old_key)
        if not old_data:
            print(f"  No data found for channel {channel_id}")
            return
        
        # Prepare new structures
        config_data = {}
        realtime_data = {}
        
        for point_id, json_str in old_data.items():
            try:
                data = json.loads(json_str)
                
                # Extract static configuration
                config = {
                    "name": data.get("name", ""),
                    "unit": data.get("unit", ""),
                    "telemetry_type": data.get("telemetry_type", "Measurement"),
                    "description": data.get("description", ""),
                    "scale": 1.0,  # Default scale
                    "offset": 0.0,  # Default offset
                    "address": ""  # Will need to be filled from CSV config
                }
                
                # Extract realtime values
                realtime = {
                    "raw": float(data.get("value", 0)),  # Assuming no raw value in old structure
                    "value": float(data.get("value", 0)),
                    "ts": int(datetime.fromisoformat(data.get("timestamp", datetime.now().isoformat()).replace('Z', '+00:00')).timestamp() * 1000)
                }
                
                config_data[point_id] = json.dumps(config)
                realtime_data[point_id] = json.dumps(realtime)
                
                self.stats['points_migrated'] += 1
                
            except Exception as e:
                print(f"  Error migrating point {point_id}: {e}")
                self.stats['errors'] += 1
        
        if not self.dry_run:
            # Store configuration
            if config_data:
                self.redis_client.hset(config_key, mapping=config_data)
                self.redis_client.expire(config_key, 86400)  # 24 hour TTL
                print(f"  Stored {len(config_data)} point configs")
            
            # Update realtime data
            if realtime_data:
                # Create backup of old data
                backup_key = f"{old_key}:backup"
                self.redis_client.rename(old_key, backup_key)
                self.redis_client.expire(backup_key, 3600)  # 1 hour backup
                
                # Store new format
                self.redis_client.hset(old_key, mapping=realtime_data)
                print(f"  Updated {len(realtime_data)} realtime values")
        else:
            print(f"  [DRY RUN] Would store {len(config_data)} configs and {len(realtime_data)} realtime values")
        
        self.stats['channels_processed'] += 1
    
    def migrate_all(self):
        """Migrate all channels"""
        channels = self.get_old_channels()
        
        if not channels:
            print("\nNo channels found using old structure")
            return
        
        print(f"\nFound {len(channels)} channels to migrate")
        
        if self.dry_run:
            print("Running in DRY RUN mode - no changes will be made")
        
        confirm = input("\nProceed with migration? (y/n): ")
        if confirm.lower() != 'y':
            print("Migration cancelled")
            return
        
        for channel_id in channels:
            self.migrate_channel(channel_id)
        
        self.print_stats()
    
    def rollback_channel(self, channel_id):
        """Rollback a channel to old structure"""
        backup_key = f"comsrv:realtime:channel:{channel_id}:backup"
        original_key = f"comsrv:realtime:channel:{channel_id}"
        
        if self.redis_client.exists(backup_key):
            self.redis_client.delete(original_key)
            self.redis_client.rename(backup_key, original_key)
            print(f"Rolled back channel {channel_id}")
        else:
            print(f"No backup found for channel {channel_id}")
    
    def print_stats(self):
        """Print migration statistics"""
        elapsed = time.time() - self.stats['start_time']
        
        print("\n" + "="*50)
        print("Migration Statistics")
        print("="*50)
        print(f"Channels processed: {self.stats['channels_processed']}")
        print(f"Points migrated: {self.stats['points_migrated']}")
        print(f"Errors: {self.stats['errors']}")
        print(f"Time elapsed: {elapsed:.2f} seconds")
        
        if self.stats['points_migrated'] > 0:
            print(f"Average: {self.stats['points_migrated']/elapsed:.1f} points/second")

def main():
    parser = argparse.ArgumentParser(description='Migrate Redis structure for VoltageEMS')
    parser.add_argument('--host', default='localhost', help='Redis host')
    parser.add_argument('--port', type=int, default=6379, help='Redis port')
    parser.add_argument('--dry-run', action='store_true', help='Perform dry run without changes')
    parser.add_argument('--rollback', type=int, help='Rollback specific channel ID')
    
    args = parser.parse_args()
    
    migrator = RedisStructureMigrator(
        redis_host=args.host,
        redis_port=args.port,
        dry_run=args.dry_run
    )
    
    if not migrator.check_connection():
        sys.exit(1)
    
    if args.rollback:
        migrator.rollback_channel(args.rollback)
    else:
        migrator.migrate_all()

if __name__ == "__main__":
    main()