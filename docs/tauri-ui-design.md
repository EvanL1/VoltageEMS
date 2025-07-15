# VoltageEMS Desktop Application UI Design

## Overview

The VoltageEMS Desktop Application is a Tauri-based real-time monitoring system that provides comprehensive visibility into energy management infrastructure. The application communicates exclusively with the API Gateway service to ensure security and maintainability.

## Design Principles

1. **Real-time First**: All data updates are pushed via WebSocket for immediate visibility
2. **Clean & Professional**: Industrial-grade UI suitable for control room environments
3. **High Information Density**: Display maximum useful information without clutter
4. **Dark Theme Ready**: Optimized for extended viewing in control room settings
5. **Responsive Layout**: Adaptable to different screen sizes and resolutions

## Main Features

### 1. Dashboard Overview

The main dashboard provides at-a-glance system health:

- **Statistics Cards**: 
  - Total Channels (with icon)
  - Online Channels (green indicator)
  - Offline Channels (yellow indicator)
  - Total Points monitored
  
- **Connection Status Bar**: Real-time WebSocket connection indicator with auto-reconnect

- **Channel List Table**:
  - Channel ID and Name
  - Status with color coding (online=green, offline=yellow, error=red)
  - Point count and error count
  - Last update timestamp
  - Click to view details

### 2. Channel Details View

Modal dialog showing detailed point data:

- **Tabbed Interface**:
  - Measurements (YC) - Voltage, Current, Power readings
  - Signals (YX) - Status indicators, alarms
  - Controls (YK) - Circuit breaker controls
  - Adjustments (YT) - Setpoints and targets
  - Real-time Chart - Live trending

- **Point Table Features**:
  - Point ID and Description
  - Real-time value with formatting
  - Quality indicator (Good/Fair/Poor)
  - Timestamp showing last update
  - Control buttons for YK/YT points

### 3. Real-time Charts

ECharts-based visualization:

- **Multi-point Trending**: Select up to 5 points for simultaneous display
- **Time Range Selection**: 1 min, 5 min, 10 min views
- **Auto-scaling Y-axis**: Adapts to data range
- **Smooth Line Rendering**: No gaps in data display
- **Data Zoom**: Interactive zoom and pan

### 4. Control Features

- **YK (Control) Points**:
  - ON/OFF toggle buttons
  - Confirmation dialog before sending
  - Visual feedback on command status

- **YT (Adjustment) Points**:
  - Numeric input with validation
  - Step controls for fine adjustment
  - Current value display

## Technical Implementation

### Component Architecture

```
App.vue
├── DataMonitor.vue (Main Component)
│   ├── Statistics Cards
│   ├── Connection Status
│   └── Channel Table
└── ChannelDetails.vue (Modal)
    ├── PointTable.vue
    └── RealtimeChart.vue
```

### State Management (Pinia)

```typescript
realtimeStore
├── channels: ChannelStatus[]
├── channelPoints: Map<channelId, PointData[]>
├── statistics: Statistics
├── loading: boolean
└── error: string | null
```

### WebSocket Manager

- Automatic reconnection with exponential backoff
- Heartbeat mechanism (30s interval)
- Channel subscription management
- Real-time data update callbacks

### Data Flow

1. **Initial Load**: REST API fetches channel list and statistics
2. **WebSocket Connect**: Establishes real-time connection
3. **Subscribe**: Subscribe to relevant data channels
4. **Real-time Updates**: Push updates to Pinia store
5. **UI Reactivity**: Vue components auto-update

## Color Scheme

- **Background**: #f5f7fa (Light gray)
- **Header**: #1e1e1e (Dark)
- **Success**: #67C23A (Green)
- **Warning**: #E6A23C (Yellow)
- **Error**: #F56C6C (Red)
- **Primary**: #409EFF (Blue)

## Responsive Breakpoints

- **Desktop**: 1600x1000 (default)
- **Minimum**: 1200x700
- **Table Adjustments**: Hide description on <1400px
- **Card Layout**: 2x2 grid on <1200px

## Performance Optimizations

1. **Virtual Scrolling**: For large point lists
2. **Debounced Updates**: Batch UI updates every 100ms
3. **Chart Data Limiting**: Keep only visible time range
4. **Lazy Loading**: Load point details on demand
5. **WebWorker Ready**: Heavy calculations off main thread

## Accessibility

- **Keyboard Navigation**: Full tab support
- **Screen Reader**: ARIA labels on all controls
- **High Contrast**: Sufficient color contrast ratios
- **Focus Indicators**: Clear visual focus states

## Future Enhancements

1. **Multi-language Support**: i18n ready architecture
2. **Theme Switching**: Light/Dark mode toggle
3. **Data Export**: CSV/Excel export functionality
4. **Alarm Management**: Dedicated alarm view
5. **Historical Playback**: Time-series data replay
6. **Custom Dashboards**: User-configurable layouts

## System Tray Integration

- **Minimize to Tray**: Keep monitoring in background
- **Notification Support**: Critical alarm notifications
- **Quick Actions**: Start/Stop monitoring from tray
- **Status Indicator**: Connection status in tray icon