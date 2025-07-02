# Grafana 前端集成指南

## 概述

本文档说明如何在 VoltageEMS 前端中直接使用 Grafana 进行数据可视化。

## 核心组件

### 1. GrafanaEmbed 组件

位置：`src/components/GrafanaIntegration/index.tsx`

主要功能：
- 通过 iframe 嵌入 Grafana 仪表板
- 自动处理认证
- 支持动态参数传递（时间范围、变量等）
- 提供加载状态提示

使用示例：
```tsx
<GrafanaEmbed
  dashboardUid="voltage-ems-overview"
  height="600px"
  timeRange={{
    from: '2024-01-01T00:00:00Z',
    to: '2024-01-02T00:00:00Z'
  }}
  variables={{
    device: 'device_001',
    metric: 'temperature'
  }}
  theme="light"
  refresh="10s"
/>
```

### 2. GrafanaService 服务

位置：`src/services/GrafanaService.ts`

主要功能：
- 管理 Grafana API 认证
- 提供仪表板的 CRUD 操作
- 构建仪表板 URL
- 处理组织和用户管理

关键方法：
```typescript
// 确保认证
await grafanaService.ensureAuth();

// 获取仪表板列表
const dashboards = await grafanaService.getDashboards();

// 创建仪表板
await grafanaService.createDashboard(dashboardConfig);

// 构建 URL
const url = grafanaService.buildDashboardUrl(uid, params);
```

## 页面实现

### 1. 历史数据分析页面

位置：`src/pages/Monitoring/HistoryAnalysis.tsx`

特点：
- 预定义多个分析仪表板（系统总览、设备分析、能耗分析等）
- 支持设备选择和时间范围选择
- 支持自定义仪表板
- 提供导出功能

### 2. 监控中心大屏

位置：`src/pages/Dashboard/index.tsx`

特点：
- 可自定义的仪表板布局
- 混合展示统计数据和 Grafana 图表
- 支持动态添加/移除仪表板
- 响应式布局

## 认证流程

```
1. 用户登录 VoltageEMS
   ↓
2. 前端调用 ensureGrafanaAuth()
   ↓
3. 后端为用户创建 Grafana API Key
   ↓
4. 前端存储 API Key 到 sessionStorage
   ↓
5. 设置 grafana_session cookie
   ↓
6. iframe 自动使用 cookie 认证
```

## 使用步骤

### 1. 基础使用

```tsx
import { GrafanaEmbed } from '@/components/GrafanaIntegration';

function MyComponent() {
  return (
    <GrafanaEmbed
      dashboardUid="my-dashboard"
      height="500px"
    />
  );
}
```

### 2. 带参数的使用

```tsx
function DeviceMonitoring({ deviceId }) {
  const [timeRange, setTimeRange] = useState({
    from: 'now-6h',
    to: 'now'
  });

  return (
    <GrafanaEmbed
      dashboardUid="device-monitoring"
      height="600px"
      timeRange={timeRange}
      variables={{
        device_id: deviceId,
        threshold: '80'
      }}
      refresh="30s"
    />
  );
}
```

### 3. 动态仪表板管理

```tsx
function DashboardManager() {
  const [dashboards, setDashboards] = useState([]);

  useEffect(() => {
    // 加载用户的仪表板
    grafanaService.getDashboards().then(setDashboards);
  }, []);

  const createDashboard = async () => {
    const newDashboard = await grafanaService.createDashboard({
      title: '我的仪表板',
      panels: [...]
    });
    setDashboards([...dashboards, newDashboard]);
  };

  return (
    <div>
      {dashboards.map(d => (
        <GrafanaEmbed key={d.uid} dashboardUid={d.uid} />
      ))}
    </div>
  );
}
```

## 配置要求

### 1. Nginx 配置

```nginx
location /grafana/ {
  rewrite ^/grafana/(.*) /$1 break;
  proxy_pass http://grafana:3000;
  proxy_set_header Authorization $http_authorization;
  
  # 支持 WebSocket
  proxy_http_version 1.1;
  proxy_set_header Upgrade $http_upgrade;
  proxy_set_header Connection "upgrade";
}
```

### 2. Grafana 配置

```ini
[auth]
disable_login_form = true

[auth.proxy]
enabled = true
header_name = X-User
auto_sign_up = true

[security]
allow_embedding = true
cookie_samesite = disabled
```

### 3. 环境变量

```env
# .env
REACT_APP_GRAFANA_URL=/grafana
REACT_APP_API_URL=/api
```

## 最佳实践

### 1. 性能优化

- 使用 `kiosk=tv` 模式减少 iframe 渲染内容
- 合理设置 `refresh` 间隔避免频繁刷新
- 使用 `maxDataPoints` 限制数据量

### 2. 用户体验

- 提供预设的时间范围选项
- 使用 Loading 状态提升感知
- 处理 iframe 加载失败的情况

### 3. 安全考虑

- API Key 仅存储在 sessionStorage
- 设置合理的 Key 过期时间
- 使用 HTTPS 传输

## 故障排查

### 1. Grafana 无法加载

```typescript
// 检查认证状态
const checkAuth = async () => {
  try {
    await grafanaService.ensureAuth();
    console.log('Grafana auth successful');
  } catch (error) {
    console.error('Grafana auth failed:', error);
  }
};
```

### 2. 仪表板显示空白

- 检查 dashboardUid 是否正确
- 验证用户是否有访问权限
- 查看浏览器控制台错误

### 3. 变量不生效

- 确保变量名称匹配（注意 `var-` 前缀）
- 检查仪表板是否配置了对应变量
- 验证变量值格式是否正确

## 扩展功能

### 1. 仪表板模板

```typescript
// 创建设备监控模板
const deviceTemplate = {
  title: '设备监控模板',
  panels: [
    {
      title: '温度趋势',
      targets: [{ target: '$device.temperature' }],
      type: 'graph'
    }
  ],
  templating: {
    list: [{
      name: 'device',
      type: 'query',
      query: 'tag_values(source_id)'
    }]
  }
};
```

### 2. 批量操作

```typescript
// 批量创建仪表板
const createDashboardsForDevices = async (devices: string[]) => {
  const promises = devices.map(device => 
    grafanaService.createDashboard({
      ...deviceTemplate,
      title: `设备 ${device} 监控`,
      uid: `device-${device}`
    })
  );
  
  await Promise.all(promises);
};
```

### 3. 导出功能

```typescript
// 导出仪表板快照
const exportDashboard = async (uid: string) => {
  const snapshot = await grafanaService.createSnapshot(uid, 'export');
  window.open(snapshot.url, '_blank');
};
```