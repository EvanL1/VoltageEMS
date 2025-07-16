# VoltageEMS Configuration CLI

A command-line tool for managing VoltageEMS service configurations using the unified `voltage-config` framework.

## Installation

```bash
cd services/config-cli
cargo install --path .
```

## Usage

```bash
voltage-config [OPTIONS] <COMMAND>
```

### Commands

#### Validate Configuration

Validate a configuration file for a specific service:

```bash
# Validate alarmsrv configuration
voltage-config validate --service alarmsrv --file config/alarmsrv.yml

# Show warnings in addition to errors
voltage-config validate --service hissrv --file config/hissrv.yml --warnings
```

#### Generate Configuration

Generate a default configuration file for a service:

```bash
# Generate YAML configuration
voltage-config generate --service comsrv --output config/comsrv.yml

# Generate JSON configuration
voltage-config generate --service modsrv --output config/modsrv.json --format json

# Generate with comments
voltage-config generate --service netsrv --output config/netsrv.yml --comments
```

#### Migrate Configuration

Migrate configuration from old format to new format:

```bash
# Migrate configuration
voltage-config migrate --from old-config.yml --to config/new.yml --service alarmsrv

# Dry run (don't write files)
voltage-config migrate --from old.yml --to new.yml --service hissrv --dry-run

# Create backup before migration
voltage-config migrate --from old.yml --to new.yml --service comsrv --backup
```

#### Show Configuration

Display configuration in a readable format:

```bash
# Show configuration as YAML
voltage-config show --service alarmsrv --file config/alarmsrv.yml

# Show as JSON
voltage-config show --service hissrv --file config/hissrv.yml --format json

# Show specific section
voltage-config show --service comsrv --file config/comsrv.yml --section redis
```

#### Compare Configurations

Compare two configuration files:

```bash
# Basic diff
voltage-config diff config/dev.yml config/prod.yml

# Side-by-side comparison
voltage-config diff config/old.yml config/new.yml --format side-by-side

# Ignore whitespace
voltage-config diff file1.yml file2.yml --ignore-whitespace
```

#### Export as Environment Variables

Export configuration as environment variables:

```bash
# Export to .env file
voltage-config export --service alarmsrv --file config/alarmsrv.yml --env-file .env

# Custom prefix
voltage-config export --service hissrv --env-file .env --prefix MYAPP

# Export default configuration
voltage-config export --service comsrv --env-file .env
```

### Global Options

- `-v, --verbose`: Enable verbose output for debugging
- `--help`: Display help information
- `--version`: Display version information

## Supported Services

- `alarmsrv` - Alarm Management Service
- `hissrv` - Historical Data Service
- `comsrv` - Communication Service
- `modsrv` - Model Calculation Service
- `netsrv` - Network Forwarding Service

## Configuration Formats

- `yaml` / `yml` - YAML format (default)
- `json` - JSON format
- `toml` - TOML format

## Examples

### Validation Workflow

```bash
# 1. Generate default configuration
voltage-config generate --service alarmsrv --output config/alarmsrv.yml --comments

# 2. Edit the configuration
vim config/alarmsrv.yml

# 3. Validate the configuration
voltage-config validate --service alarmsrv --file config/alarmsrv.yml --warnings
```

### Migration Workflow

```bash
# 1. Check current configuration
voltage-config show --service hissrv --file old-config.yml

# 2. Dry run migration
voltage-config migrate --from old-config.yml --to new-config.yml --service hissrv --dry-run

# 3. Perform migration with backup
voltage-config migrate --from old-config.yml --to new-config.yml --service hissrv --backup

# 4. Validate new configuration
voltage-config validate --service hissrv --file new-config.yml
```

### Environment Variable Export

```bash
# 1. Generate configuration
voltage-config generate --service netsrv --output config/netsrv.yml

# 2. Export as environment variables
voltage-config export --service netsrv --file config/netsrv.yml --env-file .env

# 3. Use in shell
source .env
echo $NET_REDIS_URL
```

## Error Handling

The CLI provides clear error messages and suggestions:

- Configuration validation errors show specific fields and constraints
- File not found errors suggest checking the path
- Format errors provide examples of valid formats
- Service errors list valid service names

## Contributing

When adding new features to the CLI:

1. Update the command enum in `src/main.rs`
2. Implement the command in `src/commands/`
3. Add tests in `tests/`
4. Update this README with examples

## License

MIT OR Apache-2.0