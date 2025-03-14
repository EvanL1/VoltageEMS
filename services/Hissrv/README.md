# HisSrv - Redis to InfluxDB Data Transfer Service

A high-performance service for transferring time-series data from Redis to InfluxDB with configurable storage policies.

## Features

- **Multiple Connection Methods**: Supports Redis TCP and Unix Socket connections
- **Flexible Data Point Storage Policies**: Specify which data points need to be stored via a configurable file
- **Dynamic Configuration Updates**: Update configuration without restarting the service
- **Data Retention Policies**: Automatically manage data retention time in InfluxDB
- **Multiple Redis Data Type Support**: Supports strings, hashes, lists, sets, and sorted sets
- **Automatic Data Type Conversion**: Attempts to convert string values to numeric values for better analysis in InfluxDB

## Installation

### Dependencies

- C++17 compatible compiler
- CMake 3.10+
- hiredis
- influxdb-cxx

### Build

```sh
# Clone repository
git clone https://github.com/yourusername/hissrv.git
cd hissrv

# Create build directory
mkdir build && cd build

# Configure and build
cmake ..
make

# Install (optional)
sudo make install
```

## Configuration

HisSrv reads configurations from `hissrv.conf`. By default, it looks for the `hissrv.conf` file in the current directory, but you can specify the path to the configuration file via command-line arguments.

### Example Configuration File

```ini
# HisSrv Configuration File

# Redis Configuration
redis_host = 127.0.0.1
redis_port = 6379
redis_password = 123456
redis_key_pattern = sensor:*
# redis_socket = /var/run/redis/redis.sock  # Uncomment to use Unix socket instead of TCP

# InfluxDB Configuration
influxdb_url = http://localhost:8086
influxdb_db = iot_data
influxdb_user = admin
influxdb_password = admin

# Program Configuration
interval_seconds = 30
verbose = true
enable_influxdb = true
retention_days = 90

# Point Storage Configuration
point_storage = point_pattern=true/false
# Examples:
point_storage = sensor:temp=true
point_storage = sensor:humidity=true
point_storage = sensor:pressure=false
point_storage = sensor:status=false
# Default for points not matching any pattern (true/false)
default_point_storage = true
```

## Usage

```sh
# Use default configuration file
./hissrv

# Specify configuration file
./hissrv --config /path/to/custom/config.conf

# View help
./hissrv --help
```

## Architecture

HisSrv plays a key role in the following architecture:

```
[ Device Sensor Data ] ---> [ COM SRV ]
                                 |
                           [ Redis Cache ]
                                 |
                           [ HisSrv: Write to InfluxDB ]
                                 |
                  [ Alarm Module: Anomaly Detection ]
                                 |
                           [ InfluxDB ]
                                 |
                  [ New Real-time Data Storage ]
                                 |
[ PostgreSQL: Long-term Data Archive + Configuration Management ]
```

## Contributing

Contributions, issue reports, and improvement suggestions are welcome. Please follow these steps:

1. Fork the repository
2. Create your feature branch: `git checkout -b feature/amazing-feature`
3. Commit your changes: `git commit -m 'Add some amazing feature'`
4. Push to the branch: `git push origin feature/amazing-feature`
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Changelog

### [1.0.0] - 2023-07-15

#### Added
- Initial release
- Basic transfer functionality from Redis to InfluxDB
- Support for Redis TCP and Unix Socket connections
- Support for specifying data point storage policies via configuration file
- Support for dynamic configuration updates
- Support for InfluxDB data retention policy management

