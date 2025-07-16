# VoltageEMS 页面权限矩阵

## 权限矩阵总览

| 页面分类 | 页面名称 | 路径 | 操作员 | 工程师 | 管理员 |
|---------|---------|------|--------|--------|--------|
| **监控展示类** | | | | | |
| | 系统总览 | /dashboard | ✓查看 | ✓查看 | ✓查看 |
| | 实时监控 | /monitoring/realtime | ✓查看 | ✓查看+控制 | ✓查看+控制 |
| | 设备状态 | /monitoring/devices | ✓查看 | ✓查看 | ✓查看 |
| | 能耗统计 | /monitoring/energy | ✓查看 | ✓查看 | ✓查看 |
| | 告警总览 | /monitoring/alarms | ✓查看 | ✓查看 | ✓查看 |
| | 系统拓扑 | /monitoring/topology | ✓查看 | ✓查看 | ✓查看 |
| **控制操作类** | | | | | |
| | 设备控制 | /control/devices | ✗ | ✓完全访问 | ✓完全访问 |
| | 告警处理 | /control/alarms | ✗ | ✓完全访问 | ✓完全访问 |
| | 批量操作 | /control/batch | ✗ | ✓完全访问 | ✓完全访问 |
| | 计划任务 | /control/schedule | ✗ | ✓完全访问 | ✓完全访问 |
| **配置管理类** | | | | | |
| | 通道配置 | /config/channels | ✗ | ✗ | ✓完全访问 |
| | 点表管理 | /config/points | ✗ | ✗ | ✓完全访问 |
| | 模型配置 | /config/models | ✗ | ✗ | ✓完全访问 |
| | 告警规则 | /config/alarms | ✗ | ✗ | ✓完全访问 |
| | 存储策略 | /config/storage | ✗ | ✗ | ✓完全访问 |
| | 网络转发 | /config/network | ✗ | ✗ | ✓完全访问 |
| **系统管理类** | | | | | |
| | 用户管理 | /system/users | ✗ | ✗ | ✓完全访问 |
| | 系统设置 | /system/settings | ✗ | ✗ | ✓完全访问 |
| | 日志审计 | /system/audit | ✗ | ✓查看自己 | ✓查看所有 |
| | 服务监控 | /system/services | ✗ | ✓查看 | ✓查看+控制 |

## 详细权限说明

### 1. 操作员 (Operator) 权限

#### 可访问页面（6个）
- **系统总览**：查看系统整体运行状态
- **实时监控**：查看实时数据，不能进行控制操作
- **设备状态**：查看设备在线/离线状态
- **能耗统计**：查看能耗数据和统计图表
- **告警总览**：查看告警信息，不能确认或处理
- **系统拓扑**：查看系统架构和连接关系

#### 功能限制
- 所有页面仅有查看权限
- 不显示任何控制按钮
- 不能修改任何配置
- 不能处理告警
- 不能查看日志

### 2. 工程师 (Engineer) 权限

#### 可访问页面（14个）
- **继承操作员所有权限**（6个监控页面）
- **设备控制**：执行遥控、遥调操作
- **告警处理**：确认、清除、处理告警
- **批量操作**：批量控制多个设备
- **计划任务**：创建和管理定时控制任务
- **日志审计**：仅查看自己的操作日志
- **服务监控**：查看服务状态，不能重启服务

#### 功能权限
- 监控页面：查看 + 控制操作
- 控制页面：完全访问权限
- 配置页面：无权限访问
- 系统页面：部分查看权限

### 3. 管理员 (Admin) 权限

#### 可访问页面（20个）
- **所有页面完全访问权限**

#### 功能权限
- 所有功能无限制
- 可以创建和管理用户
- 可以修改所有系统配置
- 可以查看所有操作日志
- 可以控制服务启停

## 页面内权限控制

### 1. 实时监控页面
```javascript
// 权限控制示例
{
  // 数据查看
  viewData: ['operator', 'engineer', 'admin'],

  // 远程控制
  remoteControl: ['engineer', 'admin'],

  // 参数设置
  parameterSetting: ['engineer', 'admin'],

  // 导出数据
  exportData: ['engineer', 'admin']
}
```

### 2. 告警总览页面
```javascript
{
  // 查看告警
  viewAlarms: ['operator', 'engineer', 'admin'],

  // 确认告警
  acknowledgeAlarm: ['engineer', 'admin'],

  // 清除告警
  clearAlarm: ['engineer', 'admin'],

  // 配置规则
  configureRules: ['admin']
}
```

### 3. 服务监控页面
```javascript
{
  // 查看状态
  viewStatus: ['engineer', 'admin'],

  // 查看日志
  viewLogs: ['engineer', 'admin'],

  // 重启服务
  restartService: ['admin'],

  // 修改配置
  modifyConfig: ['admin']
}
```

## API权限映射

### 1. 查询类API
| API路径 | 操作员 | 工程师 | 管理员 |
|--------|--------|--------|--------|
| GET /api/realtime/* | ✓ | ✓ | ✓ |
| GET /api/alarms | ✓ | ✓ | ✓ |
| GET /api/devices | ✓ | ✓ | ✓ |
| GET /api/energy/* | ✓ | ✓ | ✓ |
| GET /api/topology | ✓ | ✓ | ✓ |

### 2. 控制类API
| API路径 | 操作员 | 工程师 | 管理员 |
|--------|--------|--------|--------|
| POST /api/control/* | ✗ | ✓ | ✓ |
| PUT /api/alarms/* | ✗ | ✓ | ✓ |
| POST /api/batch/* | ✗ | ✓ | ✓ |
| POST /api/schedule/* | ✗ | ✓ | ✓ |

### 3. 配置类API
| API路径 | 操作员 | 工程师 | 管理员 |
|--------|--------|--------|--------|
| POST /api/config/* | ✗ | ✗ | ✓ |
| PUT /api/config/* | ✗ | ✗ | ✓ |
| DELETE /api/config/* | ✗ | ✗ | ✓ |

### 4. 系统类API
| API路径 | 操作员 | 工程师 | 管理员 |
|--------|--------|--------|--------|
| GET /api/audit/self | ✗ | ✓ | ✓ |
| GET /api/audit/all | ✗ | ✗ | ✓ |
| POST /api/users/* | ✗ | ✗ | ✓ |
| PUT /api/system/* | ✗ | ✗ | ✓ |
| POST /api/services/restart | ✗ | ✗ | ✓ |

## 实现建议

### 1. 前端路由守卫
```typescript
router.beforeEach((to, from, next) => {
  const userRole = store.state.user.role;
  const requiredRoles = to.meta.roles || [];

  if (requiredRoles.length === 0) {
    next(); // 公开页面
  } else if (requiredRoles.includes(userRole)) {
    next(); // 有权限
  } else {
    next('/403'); // 无权限
  }
});
```

### 2. 组件权限指令
```typescript
// v-permission 指令
app.directive('permission', {
  mounted(el, binding) {
    const userRole = store.state.user.role;
    const permissions = binding.value;

    if (!permissions.includes(userRole)) {
      el.style.display = 'none';
    }
  }
});
```

### 3. API请求拦截
```typescript
axios.interceptors.response.use(
  response => response,
  error => {
    if (error.response?.status === 403) {
      ElMessage.error('您没有权限执行此操作');
    }
    return Promise.reject(error);
  }
);
```

## 注意事项

1. **权限继承**：高级角色自动继承低级角色的所有权限
2. **动态菜单**：根据用户角色动态生成导航菜单
3. **按钮控制**：根据权限显示/隐藏操作按钮
4. **数据过滤**：某些数据根据角色进行过滤（如日志）
5. **审计要求**：所有权限相关操作必须记录审计日志
