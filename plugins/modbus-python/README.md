# Modbus Python Plugin for ComSrv

这是一个使用 Python 实现的 Modbus 协议插件示例，通过 gRPC 与 ComSrv 通信。

## 功能特性

- 支持 Modbus TCP 协议
- 实现批量读取功能
- 支持多种数据类型（uint16, int16, uint32, int32, float32 等）
- 连接池管理
- 健康检查机制

## 开发环境设置

1. 安装依赖：
```bash
pip install -r requirements.txt
```

2. 编译 protobuf：
```bash
./compile_proto.sh
```

3. 运行插件：
```bash
python -m src.server
```

## Docker 部署

构建镜像：
```bash
docker build -t modbus-plugin .
```

运行容器：
```bash
docker run -d \
  --name modbus-plugin \
  -p 50051:50051 \
  -e GRPC_PORT=50051 \
  modbus-plugin
```

## 配置 ComSrv 使用插件

在 ComSrv 的通道配置中添加：

```yaml
channels:
  - id: 1003
    name: "gRPC Modbus Channel"
    protocol: "grpc_modbus"
    enabled: true
    parameters:
      endpoint: "http://modbus-plugin:50051"
      protocol: "modbus_tcp"
      host: "192.168.1.100"
      port: 502
      slave_id: 1
```

## 扩展插件

要添加新的功能：

1. 修改 `modbus_plugin.py` 中的相关方法
2. 更新 protobuf 定义（如需要）
3. 重新编译和部署

## 测试

使用 gRPC 客户端工具测试：

```bash
# 安装 grpcurl
brew install grpcurl

# 查看服务
grpcurl -plaintext localhost:50051 list

# 获取插件信息
grpcurl -plaintext localhost:50051 comsrv.plugin.v1.ProtocolPlugin/GetInfo

# 健康检查
grpcurl -plaintext localhost:50051 comsrv.plugin.v1.ProtocolPlugin/HealthCheck
```