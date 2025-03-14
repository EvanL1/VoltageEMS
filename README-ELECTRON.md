# Voltage EMS Desktop Application

这是Voltage EMS的桌面应用版本，基于Electron构建，集成了所有后端服务。

## 功能特点

- 集成所有后端服务(modsrv, comsrv, hissrv, netsrv)
- 统一的服务管理界面
- 跨平台支持(Windows, macOS, Linux)
- 自动更新功能
- 离线运行能力

## 系统要求

- Windows 10/11, macOS 10.13+, 或 Ubuntu 18.04+
- 4GB RAM 以上
- 500MB 磁盘空间

## 安装

### Windows

1. 下载最新的 `VoltageEMS-Setup-x.x.x.exe` 安装文件
2. 双击安装文件并按照提示完成安装
3. 从开始菜单或桌面快捷方式启动应用

### macOS

1. 下载最新的 `VoltageEMS-x.x.x.dmg` 文件
2. 打开DMG文件并将应用拖到Applications文件夹
3. 从Launchpad或Applications文件夹启动应用

### Linux

1. 下载最新的 `VoltageEMS-x.x.x.AppImage` 或 `.deb` 文件
2. 对于AppImage: 添加执行权限 (`chmod +x VoltageEMS-x.x.x.AppImage`) 并双击运行
3. 对于DEB: 使用包管理器安装 (`sudo dpkg -i VoltageEMS-x.x.x.deb`)

## 开发指南

### 环境设置

```bash
# 克隆仓库
git clone https://github.com/voltage/ems.git
cd ems

# 安装依赖
npm install

# 安装前端依赖
cd frontend
npm install
cd ..
```

### 开发模式

```bash
# 启动开发服务器和Electron
npm run dev
```

### 构建应用

```bash
# 构建所有组件(前端、后端服务和Electron应用)
npm run build:all

# 仅构建Electron应用(假设前端和服务已构建)
npm run build
```

### 项目结构

```
voltage-ems/
├── electron/             # Electron主进程代码
│   ├── main.js           # 主进程入口
│   ├── preload.js        # 预加载脚本
│   └── services/         # 服务管理
├── frontend/             # Vue.js前端代码
├── services/             # 后端服务
│   ├── modsrv/           # 模型服务
│   ├── comsrv/           # 通信服务
│   ├── hissrv/           # 历史服务
│   └── netsrv/           # 网络服务
├── build/                # 构建配置和脚本
└── config/               # 配置文件
```

## 服务管理

桌面应用集成了所有后端服务，可以通过服务管理界面控制:

1. 启动/停止/重启单个服务
2. 启动/停止所有服务
3. 查看服务状态和日志
4. 配置服务参数

## 故障排除

### 常见问题

1. **应用无法启动**
   - 检查日志文件 (`%APPDATA%\voltage-ems\logs` 或 `~/.config/voltage-ems/logs`)
   - 确保没有端口冲突

2. **服务启动失败**
   - 检查服务日志
   - 验证配置文件是否正确

3. **界面无响应**
   - 重启应用
   - 检查系统资源使用情况

### 日志位置

- Windows: `%APPDATA%\voltage-ems\logs`
- macOS: `~/Library/Logs/voltage-ems`
- Linux: `~/.config/voltage-ems/logs`

## 许可证

Copyright © 2025 Voltage, LLC. 保留所有权利。 