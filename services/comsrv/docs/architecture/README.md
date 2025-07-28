# ComSrv 架构设计文档

本目录包含 ComSrv (Communication Service) 的架构设计文档。

## 文档列表

- [architecture-overview.md](./architecture-overview.md) - 整体架构设计
- [reconnect-design.md](./reconnect-design.md) - 重连机制设计
- [config-refactor.md](./config-refactor.md) - 配置重构方案
- [migration-guide.md](./migration-guide.md) - 迁移指南

## 设计原则

1. **简洁性优于灵活性** - 避免过度设计
2. **协议独立性** - 每个协议插件独立管理自己的连接
3. **配置分离** - 四遥配置与协议配置分离
4. **可靠性** - 统一的重连机制提高系统稳定性

## 版本历史

- v2.0.0 (2025-07) - 重构架构，移除 Transport 层，实现重连机制
- v1.0.0 (2024) - 初始版本