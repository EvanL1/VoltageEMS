# VoltageEMS - Industrial IoT Energy Management System

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Docker](https://img.shields.io/badge/docker-ready-blue.svg)](https://www.docker.com/)

[中文版本](README-CN.md)

VoltageEMS is a high-performance industrial IoT energy management system built with Rust microservices architecture. It provides real-time data collection, processing, and monitoring capabilities for industrial energy management scenarios.

## 🚀 Features

- **High Performance**: Built with Rust for optimal performance and memory safety
- **Microservices Architecture**: Modular design with independent services
- **Multi-Protocol Support**: Modbus TCP/RTU, Virtual protocols, and extensible plugin system
- **Real-time Processing**: Low-latency data collection and processing
- **Redis-based Storage**: Fast in-memory data storage with persistence
- **RESTful APIs**: Standard HTTP/JSON interfaces for all services
- **Docker Ready**: Fully containerized deployment
- **Nginx Integration**: Unified entry point with reverse proxy

## 🏗️ Architecture

```
                ┌─────────────┐
                │   Client    │
                └──────┬──────┘
                       │
                ┌──────▼──────┐
                │ Nginx (:80) │ ← Unified entry point
                └──────┬──────┘
                       │
       ┌───────────────┴───────────────────────────┐
       │                                           │
       ▼                                           ▼
┌─────────────┐                         ┌──────────────────┐
│ API Gateway │                         │   Microservices  │
│   (:6005)   │                         │                  │
│ (Minimal)   │                         │ comsrv(:6000)    │
└─────────────┘                         │ modsrv(:6001)    │
                                        │ alarmsrv(:6002)  │
                                        │ rulesrv(:6003)   │
                                        │ hissrv(:6004)    │
                                        │ netsrv(:6006)    │
                                        └──────────────────┘
                                                 │
                                                 ▼
                                    ┌─────────────────────────┐
                                    │ Redis(:6379) & Storage  │
                                    └─────────────────────────┘
```

## 📦 Services

| Service | Port | Description |
|---------|------|-------------|
| **nginx** | 80/443 | Reverse proxy and load balancer |
| **comsrv** | 6000 | Communication service - handles industrial protocols |
| **modsrv** | 6001 | Model service - manages data models and calculations |
| **alarmsrv** | 6002 | Alarm service - monitors and manages alarms |
| **rulesrv** | 6003 | Rule engine - executes business rules |
| **hissrv** | 6004 | Historical service - stores time-series data |
| **apigateway** | 6005 | API gateway - minimal proxy service |
| **netsrv** | 6006 | Network service - handles external communications |

## 🛠️ Technology Stack

- **Language**: Rust 1.75+
- **Web Framework**: Axum
- **Database**: Redis 8+, InfluxDB 2.x
- **Container**: Docker, Docker Compose
- **Message Format**: JSON, Protocol Buffers
- **Build Tool**: Cargo

## 🚦 Quick Start

### Prerequisites

- Rust 1.75+ ([Install Rust](https://rustup.rs/))
- Docker & Docker Compose
- Redis 8+ (for development)

### Development Setup

1. Clone the repository:
```bash
git clone https://github.com/your-org/VoltageEMS.git
cd VoltageEMS
```

2. Start development environment:
```bash
./scripts/dev.sh
```

3. Run a specific service:
```bash
RUST_LOG=debug cargo run --bin comsrv
```

### Docker Deployment

1. Build all images:
```bash
./scripts/build.sh release
```

2. Start all services:
```bash
docker-compose up -d
```

3. Check service status:
```bash
docker-compose ps
```

## 📝 Configuration

Each service has its own configuration file in YAML format:

```yaml
# Example: services/comsrv/config/comsrv.yaml
service:
  name: "comsrv"
  host: "0.0.0.0"
  port: 6000

redis:
  url: "redis://localhost:6379"
  
channels:
  - id: 1
    name: "modbus_channel_1"
    protocol: "modbus"
    enabled: true
```

## 🔧 Development

### Project Structure

```
VoltageEMS/
├── services/           # Microservices
│   ├── comsrv/        # Communication service
│   ├── modsrv/        # Model service
│   ├── alarmsrv/      # Alarm service
│   ├── rulesrv/       # Rule engine
│   ├── hissrv/        # Historical service
│   └── apigateway/    # API gateway
├── libs/              # Shared libraries
├── scripts/           # Utility scripts
│   └── redis-functions/  # Redis Lua functions
├── config/            # Configuration files
└── docker/            # Docker related files
```

### Building

```bash
# Check compilation
cargo check --workspace

# Build all services
cargo build --workspace

# Run tests
cargo test --workspace

# Format code
cargo fmt --all

# Run clippy
cargo clippy --all-targets --all-features -- -D warnings
```

### Testing

```bash
# Run all tests
./scripts/test.sh

# Run specific service tests
cargo test -p comsrv

# Run with output
cargo test -- --nocapture
```

## 📊 API Documentation

All services expose RESTful APIs. Here are some common endpoints:

### Health Check
```bash
GET /health
```

### Communication Service (comsrv)
```bash
# Get all channels
GET /api/channels

# Get channel status
GET /api/channels/{id}/status

# Read data point
GET /api/channels/{id}/read/{point_id}
```

### Model Service (modsrv)
```bash
# Apply model
POST /api/models/apply
{
  "model_id": "energy_calc",
  "inputs": {...}
}
```

## 🔍 Monitoring

### Logs
```bash
# View service logs
docker logs -f voltageems-comsrv

# With debug level
RUST_LOG=debug cargo run --bin comsrv
```

### Redis Monitoring
```bash
# Monitor Redis activity
redis-cli monitor | grep comsrv

# Check data
redis-cli hgetall "comsrv:1001:T"
```

## 🤝 Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- Web framework: [Axum](https://github.com/tokio-rs/axum)
- In-memory database: [Redis](https://redis.io/)
- Time-series database: [InfluxDB](https://www.influxdata.com/)

## 📞 Contact

- Project Link: [https://github.com/your-org/VoltageEMS](https://github.com/your-org/VoltageEMS)
- Issues: [https://github.com/your-org/VoltageEMS/issues](https://github.com/your-org/VoltageEMS/issues)