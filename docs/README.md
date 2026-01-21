# VoltageEMS 文档

## 快速开始

- [开发环境搭建](./GETTING_STARTED_DEVELOPMENT.md) - 从零开始运行项目
- [Monarch CLI 指南](./MONARCH_CLI_GUIDE.md) - 命令行管理工具

## API 文档

- [HTTP API 参考](./API_REFERENCE.md) - 完整的 REST API 说明
- [WebSocket API](./websocket-rule-monitor-api.md) - 实时数据推送接口

## 配置说明

- [配置格式指南](./CONFIG_FORMAT_GUIDE.md) - YAML、CSV、JSON 配置规范

## 运维参考

- [运维日志](./operations-log.md) - 问题记录与解决方案

---

## 常用命令

```bash
# 初始化并启动
monarch init && monarch sync
monarch services start

# 检查系统状态
monarch doctor

# 查看帮助
monarch --help
```

## 环境变量

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `VOLTAGE_REDIS_URL` | Redis 连接 | `redis://localhost:6379` |
| `VOLTAGE_COMSRV_URL` | Comsrv 服务地址 | `http://localhost:6001` |
| `VOLTAGE_MODSRV_URL` | Modsrv 服务地址 | `http://localhost:6002` |
| `VOLTAGE_CONFIG_PATH` | 配置文件目录 | 自动检测 |
| `VOLTAGE_DATA_PATH` | 数据文件目录 | 自动检测 |
