# Channel Logger System

## Overview

The Channel Logger system provides independent, configurable logging functionality for the communication service. Each communication channel can have its own Logger instance, supporting different log levels and independent log file management.

## Core Features

### 🔧 Independent Configuration

- Each channel owns an independent Logger instance
- Configurable log levels (TRACE, DEBUG, INFO, WARN, ERROR)
- Independent log files and directory structure

### 📁 File Management

- Automatic daily log file creation
- Directory organization by channel ID
- Support for log file rotation

### 🎯 High Performance

- Thread-safe Logger manager
- Asynchronous log writing
- Memory-efficient implementation

### 🔄 Unified Integration

- Integration with tracing system
- Simultaneous output to files and console
- Standardized log format

## Architecture Design

```
Logger System
├── ChannelLogger          # Individual channel Logger
├── ChannelLoggerManager   # Logger manager
├── LogLevel              # Log level enumeration
└── Utility Functions
    ├── init_logger()     # Initialize service Logger
    ├── init_channel_logger()  # Initialize channel Logger
    └── log_message()     # Log message packets
```

## Usage Guide

### Basic Usage

```rust
use comsrv::utils::logger::{init_channel_logger, LogLevel};

// Create channel Logger
let logger = init_channel_logger("./logs", "my_service", "modbus_01", "debug")?;

// Log messages at different levels
logger.error("Connection failed");
logger.warn("Device timeout");
logger.info("Data received");
logger.debug("Processing packet");
logger.trace("Internal state");
```

### Using Logger Manager

```rust
use comsrv::utils::logger::{ChannelLoggerManager, LogLevel};

// Create manager
let manager = ChannelLoggerManager::new("./logs");

// Get or create Logger
let logger = manager.get_logger("channel_1", LogLevel::Info)?;

// List all active Loggers
let active_loggers = manager.list_loggers()?;

// Remove Logger
manager.remove_logger("channel_1")?;
```

### Direct Logger Creation

```rust
use comsrv::utils::logger::{ChannelLogger, LogLevel};

// Create Logger instance directly
let mut logger = ChannelLogger::new("./logs", "my_channel", LogLevel::Debug)?;

// Modify log level
logger.set_level(LogLevel::Info);

// Get Logger properties
println!("Channel: {}", logger.channel_id());
println!("Level: {:?}", logger.level());
```

## Log Levels

| Level | Value | Description                       | Use Cases                       |
| ----- | ----- | --------------------------------- | ------------------------------- |
| TRACE | 0     | Most detailed tracing information | Internal state tracking         |
| DEBUG | 1     | Debug information                 | Development and troubleshooting |
| INFO  | 2     | General information               | Normal operation recording      |
| WARN  | 3     | Warning information               | Potential issue alerts          |
| ERROR | 4     | Error information                 | Errors and exceptions           |

### Level Filtering Rules

Log levels use numerical comparison, only logs **equal to or higher than** the current set level will be recorded:

```rust
// Logger set to INFO level
let logger = ChannelLogger::new("./logs", "test", LogLevel::Info)?;

logger.error("❌ This will be logged");    // ERROR (4) >= INFO (2)
logger.warn("⚠️  This will be logged");    // WARN (3) >= INFO (2)  
logger.info("ℹ️  This will be logged");    // INFO (2) >= INFO (2)
logger.debug("🔍 This will NOT be logged");  // DEBUG (1) < INFO (2)
logger.trace("🔬 This will NOT be logged");  // TRACE (0) < INFO (2)
```

## Directory Structure

The Logger system automatically creates the following directory structure:

```
logs/
├── channels/                    # Channel log directory
│   ├── modbus_01/              # Channel ID directory
│   │   ├── 2025-01-01.log     # Date-named log files
│   │   └── 2025-01-02.log
│   ├── iec104_01/
│   │   └── 2025-01-01.log
│   └── mqtt_pub/
│       └── 2025-01-01.log
└── messages/                    # Message packet log directory
    ├── modbus_01/
    │   └── 2025-01-01.msg
    └── iec104_01/
        └── 2025-01-01.msg
```

## Log Format

### Channel Log Format

```
[2025-01-01 10:30:45.123][modbus_01][INFO] Successfully connected to device
[2025-01-01 10:30:46.456][modbus_01][ERROR] Failed to read register 1001
```

Format Description:

- `[timestamp]` - Timestamp accurate to milliseconds
- `[channel_id]` - Channel identifier
- `[level]` - Log level
- `message` - Log message content

### Message Packet Log Format

```
[2025-01-01 10:30:45.123][send] 01 03 00 00 00 0A C5 CD
[2025-01-01 10:30:45.234][receive] 01 03 14 00 01 00 02 00 03 00 04 00 05
```

## API Reference

### ChannelLogger

#### Constructor

```rust
pub fn new(log_dir: impl AsRef<Path>, channel_id: &str, level: LogLevel) -> Result<Self>
```

Create a new channel Logger instance.

**Parameters:**

- `log_dir` - Log directory path
- `channel_id` - Channel identifier
- `level` - Initial log level

**Returns:** `Result<ChannelLogger>`

#### Logging Methods

```rust
pub fn trace(&self, message: &str)
pub fn debug(&self, message: &str)
pub fn info(&self, message: &str)
pub fn warn(&self, message: &str)
pub fn error(&self, message: &str)
```

Record log messages at different levels.

#### Property Methods

```rust
pub fn channel_id(&self) -> &str
pub fn level(&self) -> LogLevel
pub fn set_level(&mut self, level: LogLevel)
```

Get and set Logger properties.

### ChannelLoggerManager

#### Constructor

```rust
pub fn new(log_dir: impl AsRef<Path>) -> Self
```

Create a Logger manager instance.

#### Management Methods

```rust
pub fn get_logger(&self, channel_id: &str, level: LogLevel) -> Result<ChannelLogger>
pub fn remove_logger(&self, channel_id: &str) -> Result<()>
pub fn list_loggers(&self) -> Result<Vec<String>>
```

Manage multiple Logger instances.

### Utility Functions

```rust
pub fn init_logger(log_dir: impl AsRef<Path>, service_name: &str, level: &str, console: bool) -> Result<()>
```

Initialize the main service Logger.

```rust
pub fn init_channel_logger(log_dir: impl AsRef<Path>, service_name: &str, channel_id: &str, level: &str) -> Result<ChannelLogger>
```

Initialize a channel-specific Logger.

```rust
pub fn log_message(log_dir: impl AsRef<Path>, channel_id: &str, direction: &str, message: &[u8]) -> Result<()>
```

Log binary message packets.

## Best Practices

### 1. Log Level Selection

```rust
// Production environment - use INFO or WARN level
let prod_logger = init_channel_logger("./logs", "prod", "modbus_01", "info")?;

// Development environment - use DEBUG level
let dev_logger = init_channel_logger("./logs", "dev", "modbus_01", "debug")?;

// Troubleshooting - use TRACE level
let debug_logger = init_channel_logger("./logs", "debug", "modbus_01", "trace")?;
```

### 2. Unified Logger Management

```rust
// Use manager to centrally manage all channel Loggers
pub struct ServiceLogger {
    manager: ChannelLoggerManager,
}

impl ServiceLogger {
    pub fn new(log_dir: &str) -> Self {
        Self {
            manager: ChannelLoggerManager::new(log_dir),
        }
    }
  
    pub fn get_channel_logger(&self, channel_id: &str) -> Result<ChannelLogger> {
        self.manager.get_logger(channel_id, LogLevel::Info)
    }
}
```

### 3. Error Handling Integration

```rust
// Integration with error handling system
impl From<ComSrvError> for LogMessage {
    fn from(error: ComSrvError) -> Self {
        match error {
            ComSrvError::ConnectionError(msg) => {
                logger.error(&format!("Connection error: {}", msg));
                LogMessage::Error(msg)
            },
            ComSrvError::TimeoutError(msg) => {
                logger.warn(&format!("Timeout: {}", msg));
                LogMessage::Warning(msg)
            },
            _ => {
                logger.info(&format!("General error: {}", error));
                LogMessage::Info(error.to_string())
            }
        }
    }
}
```

### 4. Performance Optimization

```rust
// Avoid frequent string allocations
logger.info(&format!("Device {} status: {}", device_id, status)); // ❌

// Use formatting macros
logger.info(&format!("Device {device_id} status: {status}")); // ✅

// Conditional logging
if logger.level() <= LogLevel::Debug {
    let debug_info = expensive_debug_calculation();
    logger.debug(&format!("Debug info: {:?}", debug_info));
}
```

## Example Programs

For complete usage examples, please refer to: `examples/channel_logger_demo.rs`

Run the example:

```bash
cargo run --example channel_logger_demo
```

## Troubleshooting

### Common Issues

1. **Permission Error**

   ```
   Error: Permission denied: /var/log/comsrv/
   ```

   **Solution:** Ensure the application has write permissions to the log directory, or use an application data directory.
2. **Insufficient Disk Space**

   ```
   Error: No space left on device
   ```

   **Solution:** Implement log rotation strategy and regularly clean up old log files.
3. **Logger Initialization Failure**

   ```
   Error: Failed to initialize logger
   ```

   **Solution:** Check if the log directory exists and is writable, ensure no duplicate global Logger initialization.

### Debugging Techniques

1. **Enable Verbose Logging**

   ```rust
   // Temporarily increase log level for debugging
   logger.set_level(LogLevel::Trace);
   ```
2. **Check Log Files**

   ```bash
   # View latest logs
   tail -f logs/channels/modbus_01/$(date +%Y-%m-%d).log

   # Search for specific errors
   grep "ERROR" logs/channels/*/$(date +%Y-%m-%d).log
   ```
3. **Monitor Log Directory**

   ```bash
   # Monitor directory changes
   watch -n 1 "find logs/ -name '*.log' -exec wc -l {} + | sort -n"
   ```

## Summary

The Channel Logger system now provides complete, production-ready logging functionality:

✅ **Feature Complete** - Supports independent channel logs, multi-level filtering, file management
✅ **Performance Optimized** - Async writing, memory efficient, thread-safe
✅ **Easy to Use** - Clean API, comprehensive documentation, example programs
✅ **Production Ready** - Error handling, resource management, best practices

This implementation addresses all limitations of the previous simplified version, providing a reliable logging infrastructure foundation for the entire communication service system.
