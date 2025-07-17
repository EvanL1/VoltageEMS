# VoltageEMS Desktop Application

A Tauri-based desktop application for real-time monitoring of VoltageEMS data.

## Features

- Real-time data monitoring via WebSocket connection
- Channel status overview with online/offline indicators
- Point data display with quality indicators
- Real-time charts with ECharts
- Control and adjustment commands for YK/YT points
- System tray integration

## Architecture

The application communicates exclusively with the API Gateway service:

- REST API for fetching channel and point data
- WebSocket for real-time data updates
- No direct connection to other services

## Development

### Prerequisites

- Node.js 16+
- Rust 1.70+
- Tauri CLI

### Setup

```bash
# Install dependencies
npm install

# Install Tauri CLI
npm install -g @tauri-apps/cli

# Run in development mode
npm run tauri:dev

# Build for production
npm run tauri:build
```

### API Gateway Configuration

Ensure the API Gateway is running at `http://localhost:8080` with:

- REST endpoints at `/api/v1/realtime/*`
- WebSocket endpoint at `/ws/realtime`

## Technology Stack

- **Frontend**: Vue 3, TypeScript, Element Plus, ECharts
- **State Management**: Pinia
- **Desktop Framework**: Tauri
- **Backend Communication**: Axios (REST), Native WebSocket

## Project Structure

```
src/
├── api/              # API clients and WebSocket manager
├── components/       # Vue components
├── stores/          # Pinia stores
├── types/           # TypeScript type definitions
├── router/          # Vue Router configuration
└── main.ts          # Application entry point

src-tauri/
├── src/
│   └── main.rs      # Tauri backend
├── Cargo.toml       # Rust dependencies
└── tauri.conf.json  # Tauri configuration
```