# VoltageEMS 开发指南

## 快速开始

### 环境要求

- **Rust**: 1.70+ (推荐使用 rustup)
- **Redis**: 7.0+
- **Docker**: 20.10+ (可选，用于容器化部署)
- **Git**: 2.30+

### 安装开发工具

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装开发工具
cargo install cargo-watch cargo-nextest cargo-udeps

# 安装代码格式化和检查工具
rustup component add rustfmt clippy

# macOS 用户
brew install redis

# Linux 用户
sudo apt-get install redis-server  # Debian/Ubuntu
sudo yum install redis             # RHEL/CentOS
```

### 克隆代码

```bash
git clone https://github.com/VoltageEMS/VoltageEMS.git
cd VoltageEMS
```

### 初始化开发环境

```bash
# 复制环境配置
cp .env.example .env

# 安装 Git hooks
lefthook install

# 验证环境
cargo --version
redis-cli --version
```

## 项目结构

```
VoltageEMS/
├── services/           # 微服务
│   ├── comsrv/        # 通信服务
│   ├── modsrv/        # 计算服务
│   ├── hissrv/        # 历史数据服务
│   ├── netsrv/        # 云网关服务
│   ├── alarmsrv/      # 告警服务
│   └── apigateway/    # API 网关
├── libs/              # 共享库
│   └── voltage-common/
├── docs/              # 文档
├── scripts/           # 脚本工具
├── config/            # 配置文件
└── tests/             # 集成测试
```

## 开发流程

### 1. 启动 Redis

```bash
# Docker 方式
docker run -d --name redis-dev -p 6379:6379 redis:7-alpine

# 本地方式
redis-server
```

### 2. 启动单个服务

```bash
# 进入服务目录
cd services/comsrv

# 开发模式运行（自动重载）
cargo watch -x run

# 或直接运行
cargo run

# 指定日志级别
RUST_LOG=debug cargo run
RUST_LOG=comsrv=debug,voltage_common=info cargo run
```

### 3. 运行测试

```bash
# 运行所有测试
cargo test --workspace

# 运行特定服务的测试
cargo test -p comsrv

# 运行并显示输出
cargo test -- --nocapture

# 使用 nextest（更快）
cargo nextest run

# 运行特定测试
cargo test test_modbus_client
```

### 4. 代码检查

```bash
# 格式化代码
cargo fmt --all

# 运行 clippy
cargo clippy --all-targets --all-features -- -D warnings

# 检查未使用的依赖
cargo udeps

# 运行所有检查（推荐在提交前运行）
./scripts/check-all.sh
```

## 服务开发

### comsrv - 通信服务开发

#### 添加新协议

1. 在 `plugins/protocols/` 创建新模块
2. 实现 `ProtocolPlugin` trait
3. 注册到 `PluginRegistry`

```rust
// plugins/protocols/my_protocol/mod.rs
use crate::plugins::plugin_trait::*;

pub struct MyProtocolPlugin {
    // 插件状态
}

#[async_trait]
impl ProtocolPlugin for MyProtocolPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "my_protocol".to_string(),
            version: "1.0.0".to_string(),
            protocol_type: "my_protocol".to_string(),
            // ...
        }
    }
    
    async fn start(&mut self) -> Result<()> {
        // 启动逻辑
    }
}
```

#### 添加新传输层

1. 在 `core/transport/` 创建新模块
2. 实现 `Transport` trait
3. 在 `TransportFactory` 中注册

```rust
// core/transport/my_transport.rs
#[async_trait]
impl Transport for MyTransport {
    async fn connect(&mut self) -> Result<()> {
        // 连接逻辑
    }
    
    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize> {
        // 读取逻辑
    }
}
```

### modsrv - 模型服务开发

#### 创建设备模型

1. 在 `config/models/` 创建 YAML 文件
2. 定义模型结构

```yaml
# config/models/my_device.yaml
id: my_device_v1
name: 我的设备
version: 1.0.0
device_type: custom

properties:
  - identifier: serial_number
    name: 序列号
    data_type: string
    required: true

telemetry:
  - identifier: temperature
    name: 温度
    data_type: float64
    unit: °C
    mapping:
      channel_id: 1001
      point_type: m
      point_id: 10001

calculations:
  - identifier: temp_avg
    name: 平均温度
    inputs: [temp1, temp2, temp3]
    outputs: [avg_temp]
    expression:
      built_in:
        function: avg
```

#### 添加计算函数

```rust
// device_model/calculation.rs
fn my_calculation(
    inputs: HashMap<String, Value>,
    params: HashMap<String, Value>,
) -> Result<Value> {
    // 自定义计算逻辑
    Ok(json!(result))
}

// 注册函数
engine.register_function("my_calc", my_calculation);
```

## 调试技巧

### 1. 日志调试

```rust
use tracing::{debug, info, warn, error};

// 添加调试日志
debug!("Processing point: {:?}", point);
info!(channel_id = %channel_id, "Channel started");
warn!("Connection timeout: {:?}", duration);
error!("Failed to parse: {}", e);

// 结构化日志
info!(
    point_id = %point_id,
    value = %value,
    latency_ms = %latency.as_millis(),
    "Point updated"
);
```

### 2. Redis 监控

```bash
# 监控所有命令
redis-cli monitor

# 监控特定模式
redis-cli monitor | grep "1001:m:"

# 查看键空间
redis-cli --scan --pattern "1001:*"

# 查看特定键
redis-cli get "1001:m:10001"

# 订阅调试
redis-cli psubscribe "cmd:*"
```

### 3. 性能分析

```bash
# CPU 分析
cargo build --release
perf record --call-graph=dwarf target/release/comsrv
perf report

# 内存分析
valgrind --tool=massif target/release/comsrv
ms_print massif.out.*

# 火焰图
cargo install flamegraph
cargo flamegraph --bin comsrv
```

## 常见问题

### 1. 编译错误

```bash
# 清理缓存
cargo clean

# 更新依赖
cargo update

# 检查特定 target
cargo check --target x86_64-unknown-linux-gnu
```

### 2. Redis 连接失败

```bash
# 检查 Redis 是否运行
redis-cli ping

# 检查连接
redis-cli -h localhost -p 6379 info

# 查看 Redis 日志
redis-server --loglevel debug
```

### 3. 测试失败

```bash
# 单独运行失败的测试
cargo test failing_test_name -- --exact --nocapture

# 忽略集成测试
cargo test --lib

# 设置测试超时
cargo test -- --test-threads=1
```

## 代码规范

### Rust 编码规范

1. **命名规范**
   - 类型名：`PascalCase`
   - 函数/方法：`snake_case`
   - 常量：`SCREAMING_SNAKE_CASE`
   - 模块：`snake_case`

2. **错误处理**
   - 使用 `Result<T, E>` 而非 panic
   - 提供有意义的错误信息
   - 实现 `From` trait 进行错误转换

3. **异步编程**
   - 优先使用 `tokio::spawn` 而非阻塞
   - 正确处理任务取消
   - 避免在异步代码中使用阻塞操作

4. **性能考虑**
   - 使用 `Arc` 共享大对象
   - 批量操作优于单个操作
   - 合理使用缓存

### Git 工作流

1. **分支管理**
   ```bash
   # 功能分支
   git checkout -b feature/my-feature
   
   # 修复分支
   git checkout -b fix/issue-123
   
   # 发布分支
   git checkout -b release/v1.2.0
   ```

2. **提交规范**
   ```bash
   # 格式：<type>(<scope>): <subject>
   
   feat(comsrv): 添加 OPC UA 协议支持
   fix(modsrv): 修复内存泄漏问题
   docs(readme): 更新安装说明
   refactor(common): 重构错误处理
   test(comsrv): 添加 Modbus 集成测试
   ```

3. **代码审查**
   - 确保所有测试通过
   - 运行格式化和 lint
   - 更新相关文档
   - 添加必要的测试

## 集成开发环境

### VS Code

推荐扩展：
- rust-analyzer
- Even Better TOML
- crates
- Error Lens

配置示例（`.vscode/settings.json`）：
```json
{
    "rust-analyzer.cargo.features": "all",
    "rust-analyzer.checkOnSave.command": "clippy",
    "editor.formatOnSave": true,
    "[rust]": {
        "editor.defaultFormatter": "rust-lang.rust-analyzer"
    }
}
```

### IntelliJ IDEA / CLion

- 安装 Rust 插件
- 配置 Cargo 项目
- 启用格式化和检查

## 部署准备

### 构建生产版本

```bash
# 优化构建
cargo build --release --workspace

# 交叉编译
cargo build --release --target aarch64-unknown-linux-gnu

# 静态链接
RUSTFLAGS='-C target-feature=+crt-static' cargo build --release
```

### Docker 镜像

```bash
# 构建镜像
docker build -t voltageems/comsrv:latest services/comsrv

# 多阶段构建（推荐）
docker build -f services/comsrv/Dockerfile.multi -t voltageems/comsrv:latest .
```

## 资源链接

- [Rust 官方文档](https://doc.rust-lang.org/)
- [Tokio 异步编程](https://tokio.rs/)
- [Redis 命令参考](https://redis.io/commands/)
- [Docker 最佳实践](https://docs.docker.com/develop/dev-best-practices/)

## 获取帮助

- GitHub Issues: [项目问题追踪](https://github.com/VoltageEMS/VoltageEMS/issues)
- 文档: [在线文档](https://docs.voltageems.com)
- 社区: [Discord 频道](https://discord.gg/voltageems)