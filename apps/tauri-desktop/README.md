# VoltageEMS Desktop Application

基于 Tauri + Vue 3 + TypeScript 的工业物联网能源管理系统桌面应用。

## 快速开始

### 使用 bun 运行

```bash
# 安装依赖
bun install

# 启动开发服务器
bun run dev

# 在另一个终端启动 Tauri 应用
bun run tauri dev
```

### 或者使用提供的脚本

```bash
# 直接运行
./run-dev.sh
```

### 登录信息

- 用户名: `admin`
- 密码: `admin123`

## 功能特性

- 🚀 实时数据监控 (WebSocket)
- 📊 历史数据查询与导出
- 🎛️ 设备控制面板
- ⚙️ 通道和点表配置
- 🔧 规则引擎编辑器
- 🔔 告警管理系统
- 👥 用户权限管理
- 📝 系统日志查看
- 📈 数据可视化仪表板
- 🌐 多语言支持

## 技术栈

- **Frontend**: Vue 3 + TypeScript
- **UI**: Element Plus
- **Desktop**: Tauri
- **Charts**: ECharts
- **State**: Pinia
- **Build**: Vite + Bun

## 项目结构

```
src/
├── api/          # API 配置和请求
├── components/   # 通用组件
├── layouts/      # 布局组件
├── router/       # 路由配置
├── stores/       # Pinia 状态管理
├── views/        # 页面视图
│   ├── Dashboard/      # 仪表板
│   ├── Monitor/        # 实时监控
│   ├── History/        # 历史数据
│   ├── Control/        # 设备控制
│   ├── Config/         # 配置管理
│   ├── Rules/          # 规则引擎
│   ├── Alarm/          # 告警中心
│   ├── System/         # 系统管理
│   ├── User/           # 用户管理
│   └── Login/          # 登录页面
└── App.vue      # 根组件
```

## 构建

```bash
# 构建 Web 版本
bun run build

# 构建 Tauri 应用
bun run tauri build
```

## 环境变量

创建 `.env.local` 文件：

```env
VITE_API_BASE_URL=http://localhost:8080
VITE_WS_URL=ws://localhost:8080/ws
```

## 主要页面截图

### 登录页面
- 简洁的登录界面
- 默认账号: admin/admin123

### 仪表板
- KPI 卡片显示关键指标
- 实时图表展示能源数据
- 系统健康监控

### 实时监控
- 三种视图模式（网格、表格、图表）
- WebSocket 实时数据更新
- 通道订阅管理

### 设备控制
- 二进制控制（开关）
- 模拟量控制（滑块）
- 批量控制功能
- 两步确认机制

### 告警中心
- 多级别告警显示
- 告警确认工作流
- 声音和桌面通知
- 告警历史记录

## 注意事项

1. 确保已安装 Rust 和 Tauri 依赖
2. 开发时需要启动后端 API Gateway 服务
3. WebSocket 连接会自动重连
4. 所有 API 请求都通过 API Gateway

## License

MIT