# Grafana 集成方案

本文档描述了如何将 Grafana 嵌入到 VoltageEMS 前端服务中，并与 Hissrv 历史数据服务集成。

## 1. 整体架构

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   前端服务      │     │    Grafana      │     │    Hissrv       │
│   (React/Vue)   │────▶│   (Docker)      │────▶│   历史服务      │
│                 │     │                 │     │                 │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │                        │                        │
        └────────────────────────┴────────────────────────┘
                          统一认证/代理
```

### 关键特性
- **统一入口**：用户通过前端服务访问所有功能
- **无缝集成**：Grafana 作为 iframe 嵌入，用户无需感知
- **统一认证**：前端服务统一处理认证，自动登录 Grafana
- **数据对接**：Grafana 通过自定义数据源直接查询 Hissrv

## 2. 实现方案

### 2.1 前端嵌入方式

#### 方式一：iframe 嵌入（推荐）

```typescript
// components/GrafanaDashboard.tsx
import React from 'react';
import { useParams } from 'react-router-dom';

interface GrafanaDashboardProps {
  dashboardId: string;
  timeRange?: {
    from: string;
    to: string;
  };
  variables?: Record<string, string>;
}

const GrafanaDashboard: React.FC<GrafanaDashboardProps> = ({ 
  dashboardId, 
  timeRange,
  variables = {}
}) => {
  // 构建 Grafana URL
  const buildGrafanaUrl = () => {
    const params = new URLSearchParams({
      orgId: '1',
      kiosk: 'tv', // 隐藏 Grafana UI
      theme: 'light',
      ...(timeRange && {
        from: timeRange.from,
        to: timeRange.to
      })
    });
    
    // 添加变量参数
    Object.entries(variables).forEach(([key, value]) => {
      params.append(`var-${key}`, value);
    });
    
    return `/grafana/d/${dashboardId}?${params.toString()}`;
  };
  
  return (
    <div className="grafana-container">
      <iframe
        src={buildGrafanaUrl()}
        width="100%"
        height="600px"
        frameBorder="0"
        style={{ border: 'none' }}
      />
    </div>
  );
};

export default GrafanaDashboard;
```

#### 方式二：Grafana API 集成

```typescript
// services/GrafanaService.ts
export class GrafanaService {
  private apiUrl = '/grafana/api';
  
  // 获取仪表板配置
  async getDashboard(uid: string) {
    const response = await fetch(`${this.apiUrl}/dashboards/uid/${uid}`, {
      headers: {
        'Authorization': `Bearer ${this.getGrafanaToken()}`
      }
    });
    return response.json();
  }
  
  // 查询面板数据
  async queryPanelData(datasourceId: number, query: any, timeRange: TimeRange) {
    const payload = {
      queries: [{
        datasourceId,
        ...query,
        intervalMs: 1000,
        maxDataPoints: 500
      }],
      from: timeRange.from,
      to: timeRange.to
    };
    
    const response = await fetch(`${this.apiUrl}/ds/query`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${this.getGrafanaToken()}`
      },
      body: JSON.stringify(payload)
    });
    
    return response.json();
  }
  
  // 自动创建 API Token
  private async getGrafanaToken(): Promise<string> {
    const cached = sessionStorage.getItem('grafana_token');
    if (cached) return cached;
    
    const response = await fetch('/api/auth/grafana-token', {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${getUserToken()}`
      }
    });
    
    const { token } = await response.json();
    sessionStorage.setItem('grafana_token', token);
    return token;
  }
}
```

### 2.2 反向代理配置

```nginx
# nginx.conf
upstream frontend {
    server frontend:3000;
}

upstream grafana {
    server grafana:3000;
}

upstream hissrv {
    server hissrv:8080;
}

server {
    listen 80;
    server_name localhost;
    
    # 前端应用
    location / {
        proxy_pass http://frontend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
    
    # Grafana 代理
    location /grafana/ {
        rewrite ^/grafana/(.*) /$1 break;
        proxy_pass http://grafana;
        
        # 认证头转发
        proxy_set_header Authorization $http_authorization;
        
        # WebSocket 支持（实时更新）
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        
        # 隐藏 Grafana UI 元素
        sub_filter '</head>' '<style>
            .sidemenu { display: none !important; }
            .navbar { display: none !important; }
            .dashboard-header { display: none !important; }
        </style></head>';
        sub_filter_once on;
    }
    
    # Hissrv API 代理
    location /api/hissrv/ {
        rewrite ^/api/hissrv/(.*) /api/v1/$1 break;
        proxy_pass http://hissrv;
        proxy_set_header Authorization $http_authorization;
    }
}
```

### 2.3 Grafana 配置

#### 数据源配置

```yaml
# grafana/provisioning/datasources/hissrv.yaml
apiVersion: 1

datasources:
  - name: Hissrv
    type: simplejson
    access: proxy
    url: http://hissrv:8080/grafana
    isDefault: true
    jsonData:
      httpMethod: POST
      httpHeaderName1: 'Authorization'
    secureJsonData:
      httpHeaderValue1: '${HISSRV_API_KEY}'
```

#### Grafana 环境配置

```ini
# grafana.ini
[server]
domain = localhost
root_url = %(protocol)s://%(domain)s/grafana/
serve_from_sub_path = true

[auth]
disable_login_form = true
disable_signout_menu = true

[auth.anonymous]
enabled = true
org_role = Viewer

[auth.proxy]
enabled = true
header_name = X-User
header_property = username
auto_sign_up = true

[security]
allow_embedding = true
cookie_secure = false
cookie_samesite = disabled
```

## 3. Hissrv 数据源适配

### 3.1 SimpleJSON 数据源实现

```rust
// services/Hissrv/src/api/handlers_grafana.rs
use axum::{Json, extract::{Query, State}};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct GrafanaQueryRequest {
    pub targets: Vec<GrafanaTarget>,
    pub range: GrafanaTimeRange,
    pub interval: Option<String>,
    pub interval_ms: Option<i64>,
    pub max_data_points: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct GrafanaTarget {
    pub target: String,
    #[serde(rename = "type")]
    pub query_type: Option<String>,
    pub refId: String,
}

#[derive(Debug, Deserialize)]
pub struct GrafanaTimeRange {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct GrafanaTimeSeries {
    pub target: String,
    pub datapoints: Vec<[f64; 2]>,
}

// 测试连接
pub async fn grafana_test() -> &'static str {
    "OK"
}

// 查询可用指标
pub async fn grafana_search(
    State(app_state): State<Arc<AppState>>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<Vec<String>>, Error> {
    // 获取所有可用的数据点
    let metrics = app_state.storage
        .get_available_metrics()
        .await?;
    
    // 格式化为 Grafana 期望的格式: source_id.point_name
    let formatted_metrics: Vec<String> = metrics
        .into_iter()
        .map(|m| format!("{}.{}", m.source_id, m.point_name))
        .collect();
    
    Ok(Json(formatted_metrics))
}

// 查询时序数据
pub async fn grafana_query(
    State(app_state): State<Arc<AppState>>,
    Json(request): Json<GrafanaQueryRequest>,
) -> Result<Json<Vec<GrafanaTimeSeries>>, Error> {
    let mut result = Vec::new();
    
    for target in request.targets {
        // 解析 target 格式: "device_001.temperature"
        let parts: Vec<&str> = target.target.split('.').collect();
        if parts.len() != 2 {
            continue;
        }
        
        let (source_id, point_name) = (parts[0], parts[1]);
        
        // 构建查询参数
        let params = HistoryQueryParams {
            source_id: Some(source_id.to_string()),
            point_name: Some(point_name.to_string()),
            start_time: request.range.from,
            end_time: request.range.to,
            aggregation: target.query_type
                .as_ref()
                .and_then(|t| AggregationType::from_str(t).ok()),
            interval: request.interval.clone(),
            limit: request.max_data_points.map(|n| n as i32),
            ..Default::default()
        };
        
        // 查询历史数据
        let history_data = app_state.storage
            .query_history(params)
            .await?;
        
        // 转换为 Grafana 格式
        let datapoints: Vec<[f64; 2]> = history_data.data
            .iter()
            .filter_map(|point| {
                point.value.as_numeric().map(|v| [
                    v,
                    point.timestamp.timestamp_millis() as f64
                ])
            })
            .collect();
        
        result.push(GrafanaTimeSeries {
            target: target.target,
            datapoints,
        });
    }
    
    Ok(Json(result))
}

// 查询标签键
pub async fn grafana_tag_keys(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<Vec<serde_json::Value>>, Error> {
    Ok(Json(vec![
        json!({"type": "string", "text": "source_id"}),
        json!({"type": "string", "text": "point_name"}),
        json!({"type": "string", "text": "quality"}),
    ]))
}

// 查询标签值
pub async fn grafana_tag_values(
    State(app_state): State<Arc<AppState>>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<Vec<serde_json::Value>>, Error> {
    let key = request["key"].as_str().unwrap_or("");
    
    let values = match key {
        "source_id" => {
            app_state.storage
                .get_unique_source_ids()
                .await?
                .into_iter()
                .map(|v| json!({"text": v}))
                .collect()
        },
        "point_name" => {
            app_state.storage
                .get_unique_point_names()
                .await?
                .into_iter()
                .map(|v| json!({"text": v}))
                .collect()
        },
        _ => vec![]
    };
    
    Ok(Json(values))
}
```

### 3.2 路由配置

```rust
// services/Hissrv/src/api/routes.rs
pub fn grafana_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(handlers_grafana::grafana_test))
        .route("/search", post(handlers_grafana::grafana_search))
        .route("/query", post(handlers_grafana::grafana_query))
        .route("/tag-keys", post(handlers_grafana::grafana_tag_keys))
        .route("/tag-values", post(handlers_grafana::grafana_tag_values))
}

// 在主路由中添加
pub fn create_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .nest("/api/v1", api_routes())
        .nest("/grafana", grafana_routes())
        .layer(middleware::from_fn(auth_middleware))
        .with_state(app_state)
}
```

## 4. 前端集成示例

### 4.1 路由配置

```typescript
// routes/index.tsx
import { Routes, Route } from 'react-router-dom';
import MonitoringLayout from '@/layouts/MonitoringLayout';
import RealtimeView from '@/views/monitoring/RealtimeView';
import HistoryView from '@/views/monitoring/HistoryView';
import GrafanaView from '@/views/monitoring/GrafanaView';

export default function AppRoutes() {
  return (
    <Routes>
      <Route path="/monitoring" element={<MonitoringLayout />}>
        <Route path="realtime" element={<RealtimeView />} />
        <Route path="history" element={<HistoryView />} />
        <Route path="dashboard/:dashboardId" element={<GrafanaView />} />
      </Route>
    </Routes>
  );
}
```

### 4.2 Grafana 视图组件

```typescript
// views/monitoring/GrafanaView.tsx
import React, { useState, useEffect } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import { Card, Select, DatePicker, Space, Spin } from 'antd';
import GrafanaDashboard from '@/components/GrafanaDashboard';
import { useAuth } from '@/hooks/useAuth';

const { RangePicker } = DatePicker;

const GrafanaView: React.FC = () => {
  const { dashboardId } = useParams();
  const [searchParams, setSearchParams] = useSearchParams();
  const { ensureGrafanaAuth } = useAuth();
  const [loading, setLoading] = useState(true);
  const [timeRange, setTimeRange] = useState({
    from: searchParams.get('from') || 'now-6h',
    to: searchParams.get('to') || 'now'
  });
  const [variables, setVariables] = useState<Record<string, string>>({});

  useEffect(() => {
    // 确保 Grafana 认证
    ensureGrafanaAuth().then(() => {
      setLoading(false);
    });
  }, []);

  const handleTimeRangeChange = (dates: any) => {
    if (dates && dates.length === 2) {
      const newTimeRange = {
        from: dates[0].toISOString(),
        to: dates[1].toISOString()
      };
      setTimeRange(newTimeRange);
      setSearchParams({
        from: newTimeRange.from,
        to: newTimeRange.to
      });
    }
  };

  const handleVariableChange = (key: string, value: string) => {
    setVariables(prev => ({
      ...prev,
      [key]: value
    }));
  };

  if (loading) {
    return <Spin size="large" />;
  }

  return (
    <div className="grafana-view">
      <Card
        title="历史数据分析"
        extra={
          <Space>
            <Select
              placeholder="选择设备"
              style={{ width: 200 }}
              onChange={(value) => handleVariableChange('device', value)}
            >
              <Select.Option value="all">所有设备</Select.Option>
              <Select.Option value="device_001">设备001</Select.Option>
              <Select.Option value="device_002">设备002</Select.Option>
            </Select>
            <RangePicker
              showTime
              onChange={handleTimeRangeChange}
            />
          </Space>
        }
      >
        <GrafanaDashboard
          dashboardId={dashboardId!}
          timeRange={timeRange}
          variables={variables}
        />
      </Card>
    </div>
  );
};

export default GrafanaView;
```

### 4.3 认证服务

```typescript
// services/AuthService.ts
export class AuthService {
  // 确保 Grafana 认证
  async ensureGrafanaAuth(): Promise<void> {
    const token = this.getAuthToken();
    
    // 创建 Grafana API Key
    const response = await fetch('/api/auth/grafana-key', {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${token}`,
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        name: `user-${this.getCurrentUserId()}`,
        role: 'Viewer',
        secondsToLive: 86400 // 24小时
      })
    });
    
    if (!response.ok) {
      throw new Error('Failed to create Grafana API key');
    }
    
    const { key } = await response.json();
    
    // 设置 Grafana cookie
    document.cookie = `grafana_session=${key}; path=/grafana; max-age=86400`;
  }
  
  // 获取当前用户的 Grafana 组织 ID
  async getGrafanaOrgId(): Promise<number> {
    const response = await fetch('/api/auth/grafana-org', {
      headers: {
        'Authorization': `Bearer ${this.getAuthToken()}`
      }
    });
    
    const { orgId } = await response.json();
    return orgId;
  }
}
```

## 5. Docker Compose 配置

```yaml
version: '3.8'

services:
  frontend:
    build:
      context: ./frontend
      dockerfile: Dockerfile
    environment:
      - REACT_APP_API_URL=http://nginx/api
      - REACT_APP_GRAFANA_URL=http://nginx/grafana
    depends_on:
      - hissrv
      - grafana

  grafana:
    image: grafana/grafana:latest
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
      - GF_USERS_ALLOW_SIGN_UP=false
      - GF_AUTH_ANONYMOUS_ENABLED=true
      - GF_AUTH_ANONYMOUS_ORG_ROLE=Viewer
      - GF_AUTH_PROXY_ENABLED=true
      - GF_AUTH_PROXY_HEADER_NAME=X-User
      - GF_AUTH_PROXY_HEADER_PROPERTY=username
      - GF_AUTH_PROXY_AUTO_SIGN_UP=true
      - GF_SECURITY_ALLOW_EMBEDDING=true
      - GF_SERVER_ROOT_URL=%(protocol)s://%(domain)s/grafana/
      - GF_SERVER_SERVE_FROM_SUB_PATH=true
    volumes:
      - ./grafana/provisioning:/etc/grafana/provisioning
      - grafana-storage:/var/lib/grafana
    depends_on:
      - hissrv

  hissrv:
    build:
      context: ./services/Hissrv
      dockerfile: Dockerfile
    environment:
      - DATABASE_URL=postgres://user:pass@postgres:5432/hissrv
      - INFLUXDB_URL=http://influxdb:8086
      - REDIS_URL=redis://redis:6379
    volumes:
      - ./services/Hissrv/hissrv.yaml:/app/hissrv.yaml
    depends_on:
      - postgres
      - influxdb
      - redis

  nginx:
    image: nginx:alpine
    ports:
      - "80:80"
    volumes:
      - ./nginx/nginx.conf:/etc/nginx/nginx.conf
    depends_on:
      - frontend
      - grafana
      - hissrv

  postgres:
    image: postgres:15
    environment:
      - POSTGRES_DB=hissrv
      - POSTGRES_USER=user
      - POSTGRES_PASSWORD=pass
    volumes:
      - postgres-data:/var/lib/postgresql/data

  influxdb:
    image: influxdb:2.7
    environment:
      - DOCKER_INFLUXDB_INIT_MODE=setup
      - DOCKER_INFLUXDB_INIT_USERNAME=admin
      - DOCKER_INFLUXDB_INIT_PASSWORD=password123
      - DOCKER_INFLUXDB_INIT_ORG=voltageems
      - DOCKER_INFLUXDB_INIT_BUCKET=history
    volumes:
      - influxdb-data:/var/lib/influxdb2

  redis:
    image: redis:7-alpine
    volumes:
      - redis-data:/data

volumes:
  grafana-storage:
  postgres-data:
  influxdb-data:
  redis-data:
```

## 6. 使用示例

### 6.1 创建自定义仪表板

```json
{
  "dashboard": {
    "title": "设备历史数据分析",
    "panels": [
      {
        "title": "温度趋势",
        "targets": [
          {
            "target": "device_001.temperature",
            "refId": "A",
            "type": "timeseries"
          }
        ],
        "type": "graph"
      },
      {
        "title": "压力分布",
        "targets": [
          {
            "target": "device_001.pressure",
            "refId": "B",
            "type": "timeseries"
          }
        ],
        "type": "graph"
      }
    ]
  }
}
```

### 6.2 前端调用示例

```typescript
// 在组件中使用
function DeviceMonitoring() {
  return (
    <div>
      <h2>设备监控</h2>
      <GrafanaDashboard
        dashboardId="device-overview"
        timeRange={{
          from: 'now-24h',
          to: 'now'
        }}
        variables={{
          device: 'device_001',
          metric: 'temperature'
        }}
      />
    </div>
  );
}
```

## 7. 安全考虑

1. **认证传递**：使用 JWT Token 在服务间传递身份信息
2. **权限控制**：基于用户角色限制可访问的仪表板和数据
3. **数据隔离**：确保用户只能查看有权限的数据源
4. **API 限流**：防止恶意查询导致服务过载

## 8. 性能优化

1. **缓存策略**
   - Grafana 查询结果缓存
   - 浏览器端缓存仪表板配置
   - CDN 加速静态资源

2. **查询优化**
   - 自动聚合长时间范围查询
   - 限制单次查询数据点数量
   - 使用流式传输大数据集

3. **预加载**
   - 预加载常用仪表板
   - 预取时间范围数据
   - 懒加载非关键面板

## 9. 故障排查

### 常见问题

1. **Grafana 无法加载**
   - 检查 nginx 代理配置
   - 验证 Grafana 服务状态
   - 查看浏览器控制台错误

2. **数据查询失败**
   - 检查 Hissrv 服务状态
   - 验证数据源配置
   - 查看 Grafana 数据源测试结果

3. **认证问题**
   - 确认 JWT Token 有效性
   - 检查 Grafana API Key
   - 验证代理头配置

### 日志位置

- Frontend: `/var/log/frontend/app.log`
- Grafana: `/var/log/grafana/grafana.log`
- Hissrv: `/var/log/hissrv/hissrv.log`
- Nginx: `/var/log/nginx/access.log`, `/var/log/nginx/error.log`

## 10. 未来扩展

1. **高级功能**
   - 支持 Grafana 告警集成
   - 实现仪表板模板管理
   - 添加数据标注功能

2. **性能提升**
   - 实现 WebSocket 实时数据推送
   - 添加数据预聚合服务
   - 支持分布式查询

3. **用户体验**
   - 自定义主题支持
   - 移动端适配
   - 导出 PDF 报表功能