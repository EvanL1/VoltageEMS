# VoltageEMS 前端应用

VoltageEMS 前端是一个基于 Vue.js 3 和 Element Plus 构建的现代化工业物联网管理界面，提供实时监控、设备管理、告警处理等功能。

## 功能特性

- 🎨 **现代化界面** - 基于 Element Plus 的响应式设计
- 📊 **实时监控** - 系统拓扑图和实时数据展示
- 🔧 **服务管理** - 可视化服务状态监控和控制
- 📱 **设备管理** - 多协议工业设备统一管理
- 🔔 **告警系统** - 多级告警分类和批量处理
- 📈 **数据可视化** - 集成 Grafana 图表展示
- 🖥️ **跨平台支持** - Web 和 Electron 桌面应用

## 技术栈

- **Vue.js 3** - 渐进式 JavaScript 框架
- **Element Plus** - Vue 3 组件库
- **Vue Router** - 官方路由管理器
- **Vuex** - 状态管理模式
- **Axios** - HTTP 客户端
- **Electron** - 跨平台桌面应用（可选）

## 快速开始

### 前置要求

- Node.js >= 14.x
- npm >= 6.x

### 安装依赖

```bash
npm install
```

### 开发模式

```bash
npm run serve
```

应用将在 http://localhost:8080 启动

### 生产构建

```bash
npm run build
```

构建文件将生成在 `dist/` 目录

### 代码检查

```bash
npm run lint
```

## 项目结构

```
frontend/
├── public/                 # 静态资源
├── src/
│   ├── assets/            # 资源文件（图片、样式等）
│   ├── components/        # 可复用组件
│   │   └── electron/      # Electron 相关组件
│   ├── router/            # 路由配置
│   ├── store/             # Vuex 状态管理
│   ├── utils/             # 工具函数
│   ├── views/             # 页面组件
│   │   ├── Home.vue       # 首页仪表盘
│   │   ├── Services.vue   # 服务管理
│   │   ├── Devices.vue    # 设备管理
│   │   ├── Alarms.vue     # 告警管理
│   │   ├── System.vue     # 系统配置
│   │   ├── Activity.vue   # 活动日志
│   │   └── config/        # 各服务配置组件
│   ├── App.vue            # 根组件
│   └── main.js            # 应用入口
├── electron/              # Electron 主进程
├── babel.config.js        # Babel 配置
├── vue.config.js          # Vue CLI 配置
└── package.json           # 项目依赖

```

## 核心功能

### 1. 首页仪表盘 (/)

- **系统拓扑图**：实时展示 PV、PCS、电池、负载等设备状态
- **告警列表**：显示当前活跃告警
- **趋势图表**：能量和 SOC 变化趋势

### 2. 服务管理 (/services)

- **服务状态卡片**：展示 5 个核心服务运行状态
  - comsrv - 通信服务
  - modsrv - 模型服务  
  - hissrv - 历史服务
  - netsrv - 网络服务
  - alarmsrv - 告警服务
- **数据流向图**：可视化系统架构
- **关键指标**：设备数、点位数、消息吞吐量等

### 3. 设备管理 (/devices)

- **设备统计**：总数、在线、离线、异常统计
- **设备列表**：支持筛选、搜索、分页
- **设备详情**：查看点位信息和实时数据
- **协议支持**：Modbus、CAN、IEC60870、GPIO

### 4. 告警管理 (/alarms)

- **告警分级**：紧急、重要、次要、提示
- **告警分类**：环境、电力、通信、系统、安全
- **批量操作**：批量确认、导出
- **处理记录**：告警处理历史追踪

### 5. 系统配置 (/system)

- **服务配置**：各服务参数配置
- **通道管理**：通信通道配置
- **点表管理**：四遥点表配置

## API 集成

前端通过 RESTful API 与后端服务通信：

```javascript
// API 基础配置
const API_BASE_URL = process.env.VUE_APP_API_URL || 'http://localhost:8000'

// 服务状态
GET /api/v1/services
GET /api/v1/services/{service}/status

// 设备管理
GET /api/v1/devices
GET /api/v1/devices/{id}
POST /api/v1/devices/{id}/control

// 实时数据
GET /api/v1/realtime/{channel}/{device}
WS /api/v1/ws/realtime

// 告警管理
GET /api/v1/alarms
PUT /api/v1/alarms/{id}/confirm
POST /api/v1/alarms/batch/confirm
```

## 配置说明

### 环境变量

创建 `.env.local` 文件：

```env
# API 服务地址
VUE_APP_API_URL=http://localhost:8000

# WebSocket 地址
VUE_APP_WS_URL=ws://localhost:8000

# Grafana 地址
VUE_APP_GRAFANA_URL=http://localhost:3000

# 刷新间隔（毫秒）
VUE_APP_REFRESH_INTERVAL=5000
```

### Vue 配置

`vue.config.js` 主要配置：

```javascript
module.exports = {
  devServer: {
    proxy: {
      '/api': {
        target: 'http://localhost:8000',
        changeOrigin: true
      }
    }
  }
}
```

## Electron 桌面应用

### 构建桌面应用

```bash
# 开发模式
npm run electron:serve

# 构建安装包
npm run electron:build
```

### 支持平台

- Windows (x64)
- macOS (x64, arm64)
- Linux (x64)

## 开发指南

### 组件开发

1. 组件放在 `src/components/` 目录
2. 使用组合式 API (Composition API)
3. 遵循单文件组件规范

### 状态管理

使用 Vuex 管理全局状态：

```javascript
// store/modules/services.js
const state = {
  services: [],
  loading: false
}

const mutations = {
  SET_SERVICES(state, services) {
    state.services = services
  }
}
```

### 路由配置

在 `router/index.js` 添加新路由：

```javascript
{
  path: '/new-page',
  name: 'NewPage',
  component: () => import('../views/NewPage.vue')
}
```

### 样式规范

- 使用 scoped 样式避免污染
- 遵循 BEM 命名规范
- 优先使用 Element Plus 内置样式

## 性能优化

- 路由懒加载
- 组件按需引入
- 图片懒加载
- 虚拟滚动（大数据列表）
- 防抖/节流（频繁操作）

## 部署

### Nginx 配置

```nginx
server {
    listen 80;
    server_name your-domain.com;
    root /var/www/voltage-ems;
    
    location / {
        try_files $uri $uri/ /index.html;
    }
    
    location /api {
        proxy_pass http://backend:8000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### Docker 部署

```bash
# 构建镜像
docker build -t voltage-ems-frontend .

# 运行容器
docker run -d -p 80:80 voltage-ems-frontend
```

## 常见问题

### 1. 开发服务器启动失败

检查端口 8080 是否被占用：
```bash
lsof -i:8080
```

### 2. API 请求跨域

确保后端服务已启用 CORS 或配置代理

### 3. Electron 构建失败

清理缓存重试：
```bash
npm run clean
npm install
npm run electron:build
```

## 贡献指南

1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 创建 Pull Request

## 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](../LICENSE) 文件了解详情
