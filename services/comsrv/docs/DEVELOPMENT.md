# Comsrv Development Guide

## Overview

Comsrv is a high performance communication service written in Rust. It manages multiple industrial protocols using an asynchronous architecture and supports high concurrency.

### Core Features
- Multi-protocol support: Modbus TCP/RTU, IEC 60870-5-104 and more
- High performance with the Tokio runtime
- Flexible YAML configuration with hot reload
- Built-in metrics collection and monitoring
- RESTful API for management
- Connection pooling and data caching in Redis

## Directory Structure
```
services/comsrv/
├── src/
│   ├── main.rs                 # application entry
│   ├── api/                    # HTTP APIs
│   ├── core/                   # core modules
│   └── utils/                  # helper utilities
├── config/                     # configuration files
├── docs/                       # documentation
└── Cargo.toml                  # dependencies
```

## Protocol Factory
The `ProtocolFactory` module registers protocol implementations and creates clients on demand. Custom protocols can be added by implementing the `ProtocolClientFactory` trait.

## Testing
Unit tests cover frame encoding/decoding, CRC computation and configuration validation. Integration tests verify end-to-end communication and error handling.

## Contribution
Please follow the coding guidelines in AGENT.md and provide unit tests for new features.
