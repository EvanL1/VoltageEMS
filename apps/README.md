# EdgeEMS - 边缘能源管理系统

<div align="center">

![EdgeEMS Logo](src/assets/images/login-logo.png)

**EdgeEMS** 是一个现代化的边缘能源管理系统，专为分布式能源设备监控、管理和优化而设计。

[![Vue 3](https://img.shields.io/badge/Vue-3.5.17-4FC08D?style=flat-square&logo=vue.js)](https://vuejs.org/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.8.0-3178C6?style=flat-square&logo=typescript)](https://www.typescriptlang.org/)
[![Vite](https://img.shields.io/badge/Vite-7.0.0-646CFF?style=flat-square&logo=vite)](https://vitejs.dev/)
[![Element Plus](https://img.shields.io/badge/Element%20Plus-2.10.4-409EFF?style=flat-square)](https://element-plus.org/)

</div>

## ✨ 项目特色

EdgeEMS 是一个企业级的能源管理解决方案，提供：

- 🔋 **多设备监控** - 支持光伏、储能、柴发等多种能源设备
- 📊 **实时数据可视化** - 基于 ECharts 的丰富图表展示
- 🎯 **智能告警系统** - 实时监控设备状态，及时预警
- 📱 **响应式设计** - 完美适配各种屏幕尺寸
- 🔐 **权限管理** - 多角色用户权限控制
- ⚡ **高性能** - 基于 Vue 3 + Vite 的现代化架构

## 🏗️ 系统架构

### 核心技术栈

- ⚡️ **[Vite 7.0](https://vitejs.dev/)** - 下一代前端构建工具
- 🖖 **[Vue 3.5](https://vuejs.org/)** - 渐进式 JavaScript 框架
- 🎯 **[TypeScript 5.8](https://www.typescriptlang.org/)** - 类型安全的 JavaScript
- 🎨 **[Element Plus 2.10](https://element-plus.org/)** - Vue 3 企业级组件库
- 📦 **[Pinia 3.0](https://pinia.vuejs.org/)** - Vue 状态管理
- 🛣️ **[Vue Router 4.5](https://router.vuejs.org/)** - 官方路由管理器
- 📊 **[ECharts 6.0](https://echarts.apache.org/)** - 数据可视化图表库
- 🎨 **[SCSS](https://sass-lang.com/)** - CSS 预处理器
- 🔧 **[ESLint](https://eslint.org/)** + **[Prettier](https://prettier.io/)** - 代码质量保证

### 特色功能

- 📱 **响应式适配** - 基于 autofit.js 的多屏幕适配方案
- 🔄 **状态持久化** - 使用 pinia-plugin-persistedstate
- 📦 **自动导入** - unplugin-auto-import 提升开发效率
- 🗜️ **构建优化** - Gzip 压缩 + Bundle 分析
- 🧪 **单元测试** - Vitest 测试框架

## 📁 项目结构

```
EdgeEMS/
├── src/
│   ├── api/                    # API 接口层
│   │   ├── user.ts            # 用户相关接口
│   │   └── widgets.ts         # 组件数据接口
│   ├── assets/                # 静态资源
│   │   ├── fonts/             # 字体文件
│   │   ├── icons/             # SVG 图标
│   │   ├── images/            # 图片资源
│   │   └── styles/            # 全局样式
│   ├── components/            # 公共组件
│   │   ├── card/              # 卡片组件
│   │   ├── charts/            # 图表组件
│   │   └── dialog/            # 对话框组件
│   ├── composables/           # 组合式函数
│   │   └── useTableData.ts    # 表格数据处理
│   ├── layout/                # 布局组件
│   │   ├── Header.vue         # 顶部导航
│   │   ├── Sidebar.vue        # 侧边栏
│   │   └── MainLayout.vue     # 主布局
│   ├── router/                # 路由配置
│   ├── stores/                # 状态管理
│   │   ├── user.ts            # 用户状态
│   │   └── counter.ts         # 计数器状态
│   ├── types/                 # TypeScript 类型定义
│   │   ├── user.ts            # 用户类型
│   │   ├── home.ts            # 首页类型
│   │   └── alarm.ts           # 告警类型
│   ├── utils/                 # 工具函数
│   │   ├── auth.ts            # 认证工具
│   │   ├── request.ts         # HTTP 请求封装
│   │   └── responsive.ts      # 响应式工具
│   ├── views/                 # 页面组件
│   │   ├── HomeView/          # 首页
│   │   ├── LoginView/         # 登录页
│   │   ├── DeviceBattery/     # 储能设备
│   │   ├── DevicesPV/         # 光伏设备
│   │   ├── DieselGenerator/   # 柴油发电机
│   │   ├── Statistics/        # 统计分析
│   │   ├── UserManagement/    # 用户管理
│   │   └── SystemSetting/     # 系统设置
│   ├── App.vue                # 根组件
│   └── main.ts                # 应用入口
├── public/                    # 公共资源
├── docs/                      # 项目文档
└── test/                      # 测试文件
```

## 🚀 核心功能模块

### 🏠 首页仪表板

- **实时监控面板** - 显示系统整体运行状态
- **能源流向图** - 可视化能源流动方向和数量
- **关键指标卡片** - PV发电量、储能状态、负载功率等
- **告警信息** - 实时显示系统告警和通知

### ⚡ 设备管理

- **光伏设备 (PV)** - 光伏板监控、发电量统计、效率分析
- **储能系统 (ESS)** - 电池状态、充放电管理、容量监控
- **柴油发电机 (DG)** - 发电机状态、燃油监控、运行参数
- **电表监控** - 多路电表数据采集和分析

### 📊 数据分析

- **运行曲线** - 历史数据趋势分析
- **统计报表** - 日/月/年度能源统计
- **运行日志** - 系统操作记录
- **性能分析** - 设备效率和性能评估

### 🔧 系统管理

- **用户管理** - 多角色权限控制
- **规则配置** - 自动化控制规则设置
- **系统设置** - 参数配置、时间设置
- **告警管理** - 告警规则配置和历史记录

## 🎨 UI/UX 设计

### 响应式适配系统

```typescript
// 基于 autofit.js 的自适应方案
import { pxToResponsive, pxToRem } from '@/utils/responsive'

// 设计稿 px 转换为响应式 px
const width = pxToResponsive(200) // 自动适配当前屏幕

// 设计稿 px 转换为 rem
const fontSize = pxToRem(16) // 转换为 rem 单位
```

### 主题色彩

- **主色调**: `#409EFF` (Element Plus 蓝)
- **成功色**: `#67C23A`
- **警告色**: `#E6A23C`
- **危险色**: `#F56C6C`
- **背景色**: `#F5F7FA` / `#1A1A1A` (暗色模式)

### 组件设计

- **卡片式布局** - 模块化信息展示
- **数据可视化** - ECharts 图表集成
- **交互反馈** - 加载状态、操作提示
- **无障碍设计** - 键盘导航、屏幕阅读器支持

## 🚀 快速开始

### 环境要求

- **Node.js** >= 18.0.0
- **npm** >= 8.0.0 或 **yarn** >= 1.22.0
- **现代浏览器** (Chrome 88+, Firefox 78+, Safari 14+)

### 安装部署

1. **克隆项目**

```bash
git clone https://github.com/your-username/EdgeEMS.git
cd EdgeEMS
```

2. **安装依赖**

```bash
npm install
# 或使用 yarn
yarn install
```

3. **环境配置**

```bash
# 复制环境配置文件
cp .env.local.example .env.local

# 编辑配置文件，设置 API 地址等
vim .env.local
```

4. **启动开发服务器**

```bash
npm run dev
```

5. **访问应用**
   打开浏览器访问 `http://localhost:5173`

### 生产部署

```bash
# 构建生产版本
npm run build

# 预览构建结果
npm run preview

# 部署到服务器
# 将 dist/ 目录内容上传到 Web 服务器
```

## 🛠️ 开发指南

### 代码规范

- **Vue 3 Composition API** - 使用组合式 API 编写组件
- **TypeScript** - 严格类型检查，提升代码质量
- **ESLint + Prettier** - 统一代码风格
- **BEM 命名规范** - CSS 类名规范
- **Git 提交规范** - 使用 Conventional Commits

### 开发工作流

```bash
# 开发环境
npm run dev              # 启动开发服务器
npm run type-check       # TypeScript 类型检查
npm run lint             # ESLint 代码检查
npm run format           # Prettier 代码格式化

# 测试
npm run test:unit        # 运行单元测试

# 构建
npm run build            # 生产构建
npm run build:type-check # 构建时类型检查
npm run preview          # 预览构建结果
```

### 项目配置

- **Vite 配置** - `vite.config.ts`
- **TypeScript 配置** - `tsconfig.json`
- **ESLint 配置** - `eslint.config.ts`
- **样式配置** - `postcss.config.js`

### API 接口

```typescript
// 示例：用户登录接口
import { userApi } from '@/api/user'

const login = async (credentials: LoginForm) => {
  try {
    const response = await userApi.login(credentials)
    // 处理登录成功
  } catch (error) {
    // 处理登录失败
  }
}
```

## 📸 系统截图

<div align="center">

### 登录页面

![登录页面](docs/screenshots/login.png)

### 首页仪表板

![首页仪表板](docs/screenshots/dashboard.png)

### 设备监控

![设备监控](docs/screenshots/devices.png)

</div>

## 🤝 贡献指南

我们欢迎所有形式的贡献！请遵循以下步骤：

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 创建 Pull Request

### 提交信息规范

```
feat: 新功能
fix: 修复问题
docs: 文档更新
style: 代码格式调整
refactor: 代码重构
test: 测试相关
chore: 构建/工具链更新
```

## 📄 许可证

本项目基于 [MIT License](LICENSE) 开源协议。

## 📞 联系我们

- **项目主页**: [https://github.com/your-username/EdgeEMS](https://github.com/your-username/EdgeEMS)
- **问题反馈**: [Issues](https://github.com/your-username/EdgeEMS/issues)
- **功能建议**: [Discussions](https://github.com/your-username/EdgeEMS/discussions)

---

<div align="center">

**EdgeEMS** - 让能源管理更智能 ⚡

Made with ❤️ by EdgeEMS Team

</div>
