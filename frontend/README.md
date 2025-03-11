# EMS 前端配置管理平台

## 项目概述

EMS 前端配置管理平台是一个基于 Vue.js 的 Web 应用，用于集中管理能源管理系统(EMS)各个服务组件的配置文件。该平台提供了直观的用户界面，使用户能够方便地查看、编辑和保存各服务的配置，同时通过嵌入 Grafana 实现数据可视化功能。

## 技术栈

- **框架**: Vue 3
- **UI 组件库**: Element Plus
- **状态管理**: Vuex
- **路由管理**: Vue Router
- **HTTP 客户端**: Axios
- **构建工具**: Vue CLI

## 目录结构

```
frontend/
├── public/                 # 静态资源目录
│   └── index.html          # HTML 模板
├── src/                    # 源代码目录
│   ├── api/                # API 请求封装
│   ├── components/         # 通用组件
│   ├── router/             # 路由配置
│   ├── store/              # Vuex 状态管理
│   ├── utils/              # 工具函数
│   ├── views/              # 页面组件
│   │   ├── config/         # 配置页面组件
│   │   │   ├── ModsrvConfig.vue    # modsrv 配置页面
│   │   │   ├── NetsrvConfig.vue    # netsrv 配置页面
│   │   │   ├── ComsrvConfig.vue    # comsrv 配置页面
│   │   │   ├── HissrvConfig.vue    # hissrv 配置页面
│   │   │   └── MosquittoConfig.vue # Mosquitto 配置页面
│   │   ├── Dashboard.vue   # 数据看板页面
│   │   └── Home.vue        # 首页
│   ├── App.vue             # 根组件
│   └── main.js             # 入口文件
├── .eslintrc.js            # ESLint 配置
├── babel.config.js         # Babel 配置
├── package.json            # 项目依赖和脚本
└── vue.config.js           # Vue CLI 配置
```

## 关键文件说明

### 配置文件

1. **package.json**
   - 定义项目依赖包和版本
   - 配置 npm 脚本命令（serve, build, lint）
   - 包含项目元数据（名称、版本等）

2. **.eslintrc.js**
   - ESLint 代码规范检查配置
   - 定义代码风格规则
   - 配置 Vue 特定的 lint 规则

3. **babel.config.js**
   - Babel 转译器配置
   - 确保 JavaScript 代码兼容不同浏览器

4. **vue.config.js**
   - Vue CLI 项目配置
   - 配置开发服务器（端口、代理等）
   - 设置构建选项

### 核心文件

1. **public/index.html**
   - 应用的 HTML 模板
   - 包含根 DOM 元素 `<div id="app">`
   - 设置页面标题和元数据

2. **src/main.js**
   - 应用入口文件
   - 创建 Vue 实例
   - 挂载全局插件（Element Plus、Vue Router、Vuex）
   - 注册 Element Plus 图标组件

3. **src/App.vue**
   - 根组件
   - 定义应用的基本布局（侧边栏、头部、内容区）
   - 包含全局导航菜单

4. **src/router/index.js**
   - 路由配置
   - 定义页面路由和组件映射
   - 配置路由模式（history 模式）

5. **src/store/index.js**
   - Vuex 状态管理
   - 定义全局状态（配置数据、加载状态、错误信息）
   - 实现配置数据的获取和保存逻辑
   - 包含模拟数据，用于前端开发阶段

## 页面组件

1. **Home.vue**
   - 首页组件
   - 展示系统概览
   - 提供各服务配置的快速入口

2. **Dashboard.vue**
   - 数据看板页面
   - 嵌入 Grafana 仪表盘
   - 展示系统运行数据和指标

3. **配置页面组件**
   - **ModsrvConfig.vue**: 模型服务配置页面
   - **NetsrvConfig.vue**: 网络服务配置页面
   - **ComsrvConfig.vue**: 通信服务配置页面
   - **HissrvConfig.vue**: 历史数据服务配置页面
   - **MosquittoConfig.vue**: MQTT 消息代理配置页面

## 状态管理

Vuex 用于管理应用的全局状态，主要包括：

1. **状态 (state)**
   - 各服务的配置数据
   - 加载状态标志
   - 错误信息

2. **getter**
   - 获取特定服务的配置
   - 检查加载状态
   - 检查错误状态

3. **mutation**
   - SET_CONFIG: 设置配置数据
   - SET_LOADING: 设置加载状态
   - SET_ERROR: 设置错误信息

4. **action**
   - fetchConfig: 获取配置数据
   - saveConfig: 保存配置数据

## 开发模式

在开发阶段，前端使用模拟数据进行开发，无需依赖后端 API。这是通过在 store/index.js 中设置 `useBackend = false` 实现的。模拟数据存储在 `mockConfigs` 对象中，包含了各服务的示例配置。

## 运行项目

```bash
# 安装依赖
npm install

# 启动开发服务器
npm run serve

# 构建生产版本
npm run build

# 代码检查
npm run lint
```

## 部署

项目使用 Docker 进行容器化部署：

1. 构建阶段使用 Node.js 环境构建前端应用
2. 生产阶段使用 Nginx 服务器提供静态文件
3. Nginx 配置了反向代理，将 API 请求转发到后端服务

## Mosquitto 说明

Mosquitto 是一个轻量级的 MQTT 消息代理，在 EMS 系统中用于：

1. **设备通信**：与现场设备进行数据交换
2. **服务间通信**：作为各微服务之间的消息中间件
3. **数据传输**：用于实时数据的发布与订阅

通过配置管理平台，用户可以方便地配置 Mosquitto 的监听端口、认证方式、持久化设置和日志选项等。

## 扩展开发

要添加新的服务配置页面，需要：

1. 在 `src/views/config/` 目录下创建新的配置组件
2. 在 `src/router/index.js` 中添加对应的路由
3. 在 `src/store/index.js` 的 `mockConfigs` 中添加模拟数据
4. 在 `src/App.vue` 的导航菜单中添加新的菜单项

## 注意事项

- 前端应用默认在 8080 端口运行
- 开发模式下，API 请求会被代理到 http://localhost:3001
- Grafana 请求会被代理到 http://localhost:3000
- 生产环境中，这些代理由 Nginx 配置处理 