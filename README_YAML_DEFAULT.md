# YAML 作为默认配置格式

## 更新内容

Modsrv 服务已更新，现在 YAML 成为默认的配置文件格式。这意味着：

1. 所有不带扩展名的配置文件将被视为 YAML 格式（而不是之前的 TOML）
2. 所有新创建的模型实例将默认以 YAML 格式保存
3. 配置文件优先级顺序为：
   - modsrv.yaml
   - modsrv.yml
   - modsrv.toml
   - 默认使用 modsrv.yaml

## 新行为说明

### 配置文件加载

当启动服务时，系统会按照以下顺序查找配置文件：

```bash
# 如果存在 modsrv.yaml，加载它
# 否则，如果存在 modsrv.yml，加载它
# 否则，如果存在 modsrv.toml，加载它
# 否则，尝试加载默认的 modsrv.yaml（如果不存在，将使用内置默认配置）
```

如果使用 `-c` 或 `--config` 选项指定配置文件，无论文件扩展名如何，都将优先使用 YAML 解析器解析该文件，除非文件明确带有 `.toml` 扩展名。

### 模板加载与保存

- 所有无扩展名的模板文件将被视为 YAML 格式
- 新创建的模型实例将以 YAML 格式保存到存储系统
- 所有通过 `create_instance` 和 `create_instances` 命令创建的实例都将使用 YAML 格式

### 兼容性

系统仍然完全支持 TOML 和 JSON 格式，但 YAML 是首选和默认格式。如果您有现有的 TOML 或 JSON 配置文件，它们将继续正常工作，但我们建议在将来的配置中使用 YAML 格式。

## 为什么选择 YAML 作为默认格式？

- YAML 比 JSON 和 TOML 更加人类可读
- YAML 支持复杂的数据结构，如嵌套对象、列表和多行字符串
- YAML 允许注释，这使得配置文件更加清晰易懂
- YAML 的缩进结构更符合直觉，更易于编辑和维护

## 示例 YAML 配置

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
