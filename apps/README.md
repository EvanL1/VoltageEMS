# Frontend Applications

这个目录用于存放 VoltageEMS 的前端应用程序。

## 目录结构

```
apps/
├── web/              # Web 管理界面 (Vue.js/React)
├── mobile/           # 移动端应用 (React Native/Flutter)
└── desktop/          # 桌面客户端 (Electron)
```

## 开发说明

每个前端应用都应该有自己的独立目录和构建系统。

### Web 应用示例结构
```
apps/web/
├── src/              # 源代码
├── public/           # 静态资源
├── package.json      # 依赖管理
├── vite.config.js    # 构建配置
└── Dockerfile        # 容器化配置
```

## 部署

前端应用通过 Nginx 反向代理访问后端服务：

- Web 界面: http://localhost (通过 Nginx)
- API Gateway: http://localhost/api (代理到 apigateway:6005)
- WebSocket: ws://localhost/ws (代理到相应服务)

## 技术栈建议

- **Web**: Vue 3 + TypeScript + Vite + Element Plus
- **Mobile**: React Native 或 Flutter
- **Desktop**: Electron + Vue/React

## 开发命令

```bash
# 安装依赖
cd apps/web
npm install

# 开发模式
npm run dev

# 构建生产版本
npm run build

# Docker 构建
docker build -t voltageems-web:latest .
```