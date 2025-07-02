# Grafana 嵌入式监控解决方案

## 概述

本解决方案将 Grafana 完全嵌入到 VoltageEMS 前端系统中，提供实时数据监控和可视化功能。

## 系统架构

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Vue 前端      │────▶│    Grafana      │────▶│  Mock 数据服务   │
│  (端口 8080)    │     │  (端口 3000)    │     │  (端口 3001)    │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │                        │                        │
        │                        │                        │
        ▼                        ▼                        ▼
   嵌入式视图              可视化引擎              实时数据生成
```

## 关键组件

### 1. 前端组件 (GrafanaEmbedded.vue)
- 位置：`/frontend/src/views/GrafanaEmbedded.vue`
- 功能：
  - 通过 iframe 嵌入 Grafana 仪表板
  - 提供时间范围选择器
  - 支持多个仪表板切换
  - 隐藏 Grafana 原生 UI（kiosk 模式）

### 2. 数据服务 (mock-data-server.js)
- 位置：`/mock-data-server.js`
- 功能：
  - 提供 SimpleJSON 兼容的数据接口
  - 生成模拟的温度、电压、电流、功率数据
  - 支持时间序列查询
  - 自动生成实时数据

### 3. Grafana 配置
- 数据源：SimpleJSON 数据源插件
- 仪表板：
  - `simple-view`: 温度监控仪表板
  - `voltage-realtime`: 综合监控仪表板

## 使用方法

### 1. 启动服务

```bash
# 1. 启动 Grafana (如果未运行)
docker run -d -p 3000:3000 --name grafana grafana/grafana:latest

# 2. 启动模拟数据服务
node mock-data-server.js

# 3. 启动前端服务
npm run serve

# 4. 初始化 Grafana
./init-grafana.sh
```

### 2. 访问界面

- **嵌入式监控页面**: http://localhost:8080/grafana-embedded
- **Grafana 原生界面**: http://localhost:3000 (admin/admin)
- **其他测试页面**:
  - `/simple`: 简单监控视图
  - `/ultra`: 极简监控视图
  - `/grafana-live`: 实时仪表板测试

### 3. 功能特性

#### 实时数据刷新
- 自动每 5 秒刷新数据
- 支持手动刷新按钮

#### 时间范围选择
- 5 分钟
- 15 分钟
- 30 分钟
- 1 小时

#### 仪表板切换
- 温度监控（专注于温度数据）
- 综合监控（包含温度、功率、电压、电流）

## 集成到生产环境

### 1. 替换数据源

将 `mock-data-server.js` 替换为真实的 Hissrv API：

```javascript
// 修改 Grafana 数据源配置
{
  "name": "Hissrv",
  "type": "grafana-simple-json-datasource",
  "url": "http://hissrv:8080/api/v1/grafana",
  "access": "proxy"
}
```

### 2. 认证集成

在 `GrafanaEmbedded.vue` 中添加认证 token：

```javascript
const grafanaUrl = computed(() => {
  const params = new URLSearchParams({
    orgId: '1',
    from: timeRange.value,
    to: 'now',
    refresh: '5s',
    kiosk: 'tv',
    auth_token: getAuthToken() // 添加认证
  })
  return `http://localhost:3000/d/${currentDashboard.value}?${params.toString()}`
})
```

### 3. Docker Compose 配置

```yaml
services:
  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_AUTH_ANONYMOUS_ENABLED=true
      - GF_AUTH_ANONYMOUS_ORG_ROLE=Viewer
      - GF_SECURITY_ALLOW_EMBEDDING=true
    volumes:
      - grafana-storage:/var/lib/grafana
      - ./grafana-provisioning:/etc/grafana/provisioning
```

## 自定义和扩展

### 添加新的数据点

1. 在数据服务中添加新的指标：
```javascript
const metrics = [
  // 现有指标...
  'humidity',    // 湿度
  'pressure'     // 压力
]
```

2. 在 Grafana 中创建新的面板使用这些指标

### 自定义样式

通过修改 `GrafanaEmbedded.vue` 中的样式来调整外观：

```css
.grafana-container {
  /* 自定义容器样式 */
}

.toolbar {
  /* 自定义工具栏样式 */
}
```

## 故障排除

### 常见问题

1. **Grafana 无法加载**
   - 检查 Grafana 容器是否运行：`docker ps | grep grafana`
   - 检查端口 3000 是否被占用

2. **没有数据显示**
   - 确认数据服务运行：`curl http://localhost:3001/`
   - 检查 Grafana 数据源配置

3. **跨域问题**
   - 确保 Grafana 配置了 `GF_SECURITY_ALLOW_EMBEDDING=true`
   - 检查前端 proxy 配置

### 日志查看

```bash
# Grafana 日志
docker logs grafana

# 数据服务日志
# 查看 mock-data-server.js 的控制台输出

# 前端日志
# 查看浏览器控制台
```

## 总结

这个解决方案提供了一个完整的 Grafana 嵌入式监控系统，具有以下优势：

1. **无缝集成**: 完全嵌入到现有前端，用户体验一致
2. **实时更新**: 自动刷新，实时显示最新数据
3. **灵活配置**: 易于添加新的仪表板和数据源
4. **生产就绪**: 可以轻松切换到真实数据源

现在系统已经完全配置好并运行，你醒来后可以：
- 访问 http://localhost:8080/grafana-embedded 查看嵌入式监控
- 所有服务都在正常运行
- 数据每 5 秒自动刷新
- 可以在仪表板之间切换
- 支持不同的时间范围查看