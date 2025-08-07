# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

VoltageEMS is a high-performance industrial IoT energy management system built with Rust microservices architecture. It supports multiple industrial protocols (Modbus TCP/RTU, Virtual, gRPC) and real-time data processing through a hybrid architecture combining Rust services with Redis Lua Functions for optimal performance.

## Workspace Structure

```
VoltageEMS/
├── Cargo.toml              # Workspace root
├── libs/                   # Shared libraries (voltage_libs crate)
├── services/               # Microservices
│   ├── comsrv/            # Communication service (protocols)
│   ├── modsrv/            # Model service (lightweight)
│   ├── alarmsrv/          # Alarm service (lightweight)
│   ├── rulesrv/           # Rule engine (lightweight)
│   ├── hissrv/            # Historical data (lightweight)
│   ├── apigateway/        # API gateway (minimal proxy)
│   └── netsrv/            # Network service
├── scripts/
│   ├── redis-functions/   # Lua functions for Redis
│   ├── quick-check.sh     # Run format, clippy, and compile checks
│   ├── dev.sh            # Development environment setup
│   └── validate-comsrv-config.sh  # Validate CSV configurations
├── apps/                   # Frontend applications
└── docker-compose.yml      # Container orchestration
```

[... rest of the existing content remains unchanged ...]

## Development Principles

- **改代码时不要考虑兼容性，直接改。** (When modifying code, do not consider compatibility, change directly.)