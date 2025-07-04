# ModSrv Fix Log

## 2025-01-04 - Configuration Center Support

### Changes Made:

1. **Refactored Configuration Management**
   - Modified `config.rs` to support multiple configuration sources:
     - Local configuration files (YAML/JSON)
     - Configuration center service (HTTP)
     - Environment variables (override)
   - Added `ConfigLoader` struct for flexible configuration loading
   - Added `ServiceInfo` struct for service metadata
   - Updated all config structs to use new naming conventions

2. **Updated Main Function**
   - Changed `main()` to `#[tokio::main] async fn main()` for async support
   - Updated configuration loading to use new `ConfigLoader`
   - Removed hardcoded configuration file paths
   - Added support for `CONFIG_CENTER_URL` environment variable

3. **Added Dependencies**
   - Added `reqwest` for HTTP client support
   - Added `dotenv` for .env file loading

4. **Configuration File Structure**
   - Created new configuration file examples in `/config` directory
   - Added comprehensive documentation in configuration files
   - Supported environment variable overrides with `MODSRV_` prefix

5. **Backward Compatibility**
   - Maintained legacy fields for compatibility
   - Support for old Redis configuration format (host/port)
   - Automatic migration of legacy configurations

### Configuration Loading Priority:
1. Command line `--config` flag
2. Environment variable `MODSRV_CONFIG_FILE`
3. Default location: `config/modsrv.yaml`
4. Configuration center (if `CONFIG_CENTER_URL` is set)
5. Environment variable overrides

### Environment Variables:
- `MODSRV_CONFIG_FILE`: Path to configuration file
- `CONFIG_CENTER_URL`: URL of configuration center
- `MODSRV_REDIS_URL`: Override Redis URL
- `MODSRV_API_HOST`: Override API host
- `MODSRV_API_PORT`: Override API port
- `MODSRV_LOG_LEVEL`: Override log level
- `MODSRV_MODEL_UPDATE_INTERVAL_MS`: Override model update interval

### Breaking Changes:
- None (backward compatible)

### Migration Guide:
1. No immediate action required - old configurations will still work
2. To use configuration center:
   ```bash
   export CONFIG_CENTER_URL=http://config-center:8080
   cargo run --bin modsrv
   ```
3. To use environment overrides:
   ```bash
   export MODSRV_REDIS_URL=redis://production:6379
   export MODSRV_LOG_LEVEL=debug
   cargo run --bin modsrv
   ```