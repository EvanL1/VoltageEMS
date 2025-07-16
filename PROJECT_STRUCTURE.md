# VoltageEMS 项目结构说明

## 目录结构

```
VoltageEMS/
├── apps/                    # 应用程序
│   ├── tauri-desktop/      # Tauri桌面应用
│   ├── web-frontend/       # Web前端应用
│   └── config-ui/          # 配置管理界面
├── services/               # 微服务
│   ├── comsrv/            # 通信服务
│   ├── modsrv/            # 模型服务
│   ├── hissrv/            # 历史数据服务
│   ├── alarmsrv/          # 告警服务
│   ├── rulesrv/           # 规则引擎服务
│   ├── netsrv/            # 网络转发服务
│   ├── apigateway/        # API网关
│   └── config-framework/   # 配置框架
├── libs/                   # 共享库
│   └── voltage-common/     # 公共库
├── scripts/                # 脚本文件
│   ├── check_rulesrv.sh   # 规则服务检查脚本
│   └── test_build_rulesrv.sh # 规则服务构建测试
├── docs/                   # 文档
│   └── fixlog/            # 修复日志
└── archived/               # 归档文件
```

## Git Worktree 布局

所有worktree都位于 `/Users/lyf/dev/` 下：

- `VoltageEMS` - 主仓库 (develop分支)
- `VoltageEMS-apigateway` - API网关开发
- `VoltageEMS-bugfix` - Bug修复
- `VoltageEMS-frontend` - 前端开发
- `VoltageEMS-hissrv` - 历史服务开发
- `VoltageEMS-monitoring` - 监控开发
- `VoltageEMS-modsrv` - 模型服务开发
- `VoltageEMS-predsrv` - 预测服务开发
- `VoltageEMS-tauri-ui` - Tauri界面开发
- `VoltageEMS-websocket` - WebSocket开发

## 开发说明

1. **应用程序** 位于 `apps/` 目录下
2. **微服务** 位于 `services/` 目录下
3. **脚本文件** 统一放在 `scripts/` 目录下
4. **文档** 统一放在 `docs/` 目录下

## 构建和运行

参考各个服务和应用的 README.md 文件。