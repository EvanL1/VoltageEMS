# Voltage EMS Desktop Application

This is the Electron-based desktop bundle of Voltage EMS with all backend services included.

## Features

- Ships with modsrv, comsrv, hissrv and netsrv
- Unified service management UI
- Cross-platform (Windows, macOS, Linux)
- Automatic updates
- Offline operation

## System Requirements

- Windows 10/11, macOS 10.13+, or Ubuntu 18.04+
- 4GB RAM or more
- 500MB free disk space

## Installation

### Windows
1. Download the latest `VoltageEMS-Setup-x.x.x.exe` file
2. Run the installer and follow the prompts
3. Start the application from the Start Menu or desktop shortcut

### macOS
1. Download `VoltageEMS-x.x.x.dmg`
2. Mount the image and drag the app to Applications
3. Launch from Launchpad or the Applications folder

### Linux
1. Download `VoltageEMS-x.x.x.AppImage` or `.deb`
2. For AppImage: make it executable (`chmod +x ...`) and run
3. For deb: install with `sudo dpkg -i VoltageEMS-x.x.x.deb`

## Development

### Setup
```bash
# clone
git clone https://github.com/voltage/ems.git
cd ems
npm install

# frontend dependencies
cd frontend
npm install
cd ..
```

### Development Mode
```bash
npm run dev
```

### Build
```bash
npm run build:all   # build frontend, backend and electron
npm run build       # build only the electron app
```

### Project Layout
```
voltage-ems/
├── electron/       # Electron main process
├── frontend/       # Vue.js frontend code
├── services/       # Backend services
├── build/          # Build scripts
└── config/         # Configuration files
```

## Service Management

The desktop app provides a control panel where you can:
1. Start/stop/restart individual services
2. Start/stop all services
3. View logs and service status
4. Configure service parameters

## Troubleshooting

### Common Issues
1. **Application fails to start** – check the logs under `%APPDATA%/voltage-ems/logs` or `~/.config/voltage-ems/logs`
2. **Service startup failure** – review service logs and verify configuration
3. **UI not responding** – restart the application and check system resources

### Log Locations
- Windows: `%APPDATA%/voltage-ems/logs`
- macOS: `~/Library/Logs/voltage-ems`
- Linux: `~/.config/voltage-ems/logs`

## License

Copyright © 2025 Voltage, LLC. All rights reserved.
