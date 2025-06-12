# YAML as the Default Configuration Format

## Update Summary

Modsrv now treats YAML as the preferred configuration format:
1. Files without an extension are parsed as YAML instead of TOML
2. New model instances are saved using YAML
3. Configuration files are loaded in the following order:
   - modsrv.yaml
   - modsrv.yml
   - modsrv.toml
   - fallback to modsrv.yaml if none exist

## New Behavior

### Loading Configuration

At startup the service looks for configuration files using the order above. When the `-c` or `--config` option is used, the file is parsed as YAML unless the extension is `.toml`.

### Templates

- Templates without an extension are treated as YAML
- New model instances created with `create_instance` or `create_instances` use YAML

### Compatibility

TOML and JSON remain supported but YAML is recommended going forward.

## Why YAML?

- More readable than JSON or TOML
- Supports complex structures
- Allows comments for clarity
- Indentation-based syntax is easy to edit

## Example YAML Configuration

```yaml
redis:
  host: "localhost"
  port: 6379
  db: 0
  key_prefix: "modsrv:"

logging:
  level: "debug"
  file: "modsrv.log"
  console: true

model:
  update_interval_ms: 5000
  config_key_pattern: "modsrv:model:config:*"
  data_key_pattern: "modsrv:data:*"
  output_key_pattern: "modsrv:model:output:*"
  templates_dir: "templates"

control:
  operation_key_pattern: "modsrv:control:operation:*"
  enabled: true
```
