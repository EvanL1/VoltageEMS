# Comsrv - Highly Configurable Communication Service

Comsrv is a highly configurable communication service for connecting and managing various industrial devices and protocols. It provides a unified interface for handling different communication protocols such as Modbus RTU, Modbus TCP, etc.

## Features

- Support for multiple industrial communication protocols
  - Modbus RTU master/slave
  - Modbus TCP master/slave
  - Extensible for more protocols
- Configuration-based device management
- Flexible data polling and processing
- Data export to Redis and MQTT
- Real-time data processing and monitoring
- Thread-safe design
- High performance and low latency
- Prometheus metrics integration

## Architecture

Comsrv adopts a modular architecture design, including the following components:

1. **Core Framework**: Provides infrastructure such as configuration management, thread pool, logging, etc.
2. **Communication Interface**: Defines basic interfaces and abstract classes for communication protocols.
3. **Protocol Implementation**: Concrete implementations of various communication protocols.
4. **Data Processing**: Components for processing and transforming data.
5. **Data Export**: Components for exporting data to external systems like Redis, MQTT.
6. **Metrics**: Prometheus metrics for monitoring system performance and status.

### Architecture Diagram

```
+------------------+     +------------------+
|  Config Manager  |     |  Metrics Manager |
+------------------+     +------------------+
        |                       |
+------------------+           |
| Protocol Factory |           |
+------------------+           |
        |                     |
        v                     v
+------------------+     +------------------+
|  ComBase Class   | --> |  Metrics Export  |
+------------------+     +------------------+
        |                       |
        v                       v
+------------------+     +------------------+
| Data Processing  | --> |   Prometheus    |
+------------------+     +------------------+
```

## Configuration

Comsrv uses YAML format configuration files to define communication devices and parameters. Here's an example:

```yaml
version: "1.0"
service:
  name: "comsrv"
  description: "Communication Service"
  metrics:
    enabled: true
    bind_address: "0.0.0.0:9100"
  logging:
    level: "info"
    file: "/var/log/comsrv/comsrv.log"
    max_size: 10485760  # 10MB
    max_files: 5
    console: true

channels:
  - id: "pcs1"
    name: "PCS Controller 1"
    description: "Power Conversion System"
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.10"
      port: 502
      timeout: 1000
      max_retries: 3
      point_tables:
        di: "points/pcs_di.csv"
        ai: "points/pcs_ai.csv"
        do: "points/pcs_do.csv"
        ao: "points/pcs_ao.csv"
      poll_rate: 1000
```

## Building and Installation

### Dependencies

- C++17 compatible compiler
- CMake 3.10 or higher
- yaml-cpp library (automatically downloaded)
- prometheus-cpp library (automatically downloaded)
- spdlog library (automatically downloaded)

### Build Steps

```bash
mkdir build
cd build
cmake ..
make
```

### Installation

```bash
sudo make install
```

## Usage

### Running the Service

```bash
comsrv /path/to/config.yaml
```

### Using Docker

```bash
docker build -t comsrv .
docker run -v /path/to/config.yaml:/etc/comsrv/comsrv.yaml -d comsrv
```

### Monitoring

Comsrv exposes Prometheus metrics at `http://<host>:9100/metrics`. Available metrics include:

- Communication metrics:
  - `comsrv_bytes_total`: Total number of bytes sent/received
  - `comsrv_packets_total`: Total number of packets sent/received
  - `comsrv_packet_errors_total`: Total number of packet errors by type
  - `comsrv_packet_processing_duration_seconds`: Packet processing duration

- Channel metrics:
  - `comsrv_channel_status`: Channel connection status
  - `comsrv_channel_response_time_seconds`: Channel response time
  - `comsrv_channel_errors_total`: Channel errors by type

- Protocol metrics:
  - `comsrv_protocol_status`: Protocol status
  - `comsrv_protocol_errors_total`: Protocol errors by type

- Service metrics:
  - `comsrv_service_status`: Service status
  - `comsrv_service_uptime_seconds`: Service uptime
  - `comsrv_service_errors_total`: Service errors by type

### Logging

Comsrv provides comprehensive logging at both service and channel levels:

- Service log (`/var/log/ems/comsrv.log`):
  - Service startup/shutdown events
  - Channel configuration and status changes
  - System-wide events and errors

- Channel logs (`/var/log/ems/channels/<channel_id>.log`):
  - Channel connection status
  - Raw communication data (INFO level)
  - Data parsing details (DEBUG level)
  - Channel-specific errors and warnings

## Development

### Adding a New Protocol

1. Create a new protocol class inheriting from ComBase
2. Implement all required virtual functions
3. Register the new protocol type in ProtocolFactory

Example:

```cpp
class NewProtocol : public ComBase {
public:
    NewProtocol(const std::string& name);
    virtual ~NewProtocol();
    
    bool start() override;
    bool stop() override;
    
    // Protocol specific methods...
};

// Register in ProtocolFactory
factory.registerProtocol("new_protocol", 
    [](const std::map<std::string, ConfigManager::ConfigValue>& config) -> std::unique_ptr<ComBase> {
        return std::make_unique<NewProtocol>(config.at("name").get<std::string>());
    });
```

### Adding New Metrics

1. Define new metric in the Metrics class
2. Initialize the metric in the constructor
3. Add methods to update the metric
4. Use the metric in your protocol implementation

Example:

```cpp
// In metrics.h
prometheus::Counter& my_new_metric_;

// In metrics.cpp
my_new_metric_(prometheus::BuildCounter()
    .Name("comsrv_my_new_metric_total")
    .Help("Description of my new metric")
    .Labels({{"service", "comsrv"}})
    .Register(*registry_))

// Usage
Metrics::instance().incrementMyNewMetric();
```

## Contributing

Pull requests and issues are welcome.

## License

MIT License 