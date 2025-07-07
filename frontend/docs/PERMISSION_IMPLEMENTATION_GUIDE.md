# Monarch Hub 权限实施指南

## 一、权限实施概览

### 1.1 实施目标
- **安全性**: 确保只有授权用户才能访问相应功能
- **灵活性**: 支持权限的动态调整和扩展
- **可维护性**: 权限配置清晰，易于管理
- **用户体验**: 权限控制对用户透明，不影响正常操作

### 1.2 技术架构
```
前端权限控制
├── 路由级权限（Vue Router Guards）
├── 页面级权限（组件级控制）
├── 功能级权限（按钮/操作控制）
├── 数据级权限（API 层面控制）
└── UI 展示控制（动态菜单/元素显隐）
```

## 二、具体功能模块权限实施

### 2.1 监控模块 (Monitoring)

#### 仪表板 (Dashboard)
```javascript
权限标识: MONITORING.DASHBOARD_VIEW

功能明细:
- 查看实时数据面板 ✓ 所有角色
- 自定义面板布局 ✓ monitor及以上
- 导出数据 ✓ monitor及以上
- 分享面板 ✓ ops_engineer及以上

实施要点:
1. 路由守卫检查基础查看权限
2. 自定义功能根据角色动态加载
3. 导出按钮根据权限显隐
```

#### 实时监控 (Realtime)
```javascript
权限标识: MONITORING.REALTIME_VIEW

功能明细:
- 查看实时数据流 ✓ 所有角色
- 设置刷新频率 ✓ monitor及以上
- 订阅数据推送 ✓ ops_engineer及以上
- 配置监控点位 ✓ ops_engineer及以上

实施要点:
1. WebSocket连接根据权限级别限流
2. 配置面板仅对工程师可见
3. 数据粒度根据角色调整
```

#### Grafana 集成
```javascript
权限标识: MONITORING.GRAFANA_VIEW

功能明细:
- 查看嵌入式仪表板 ✓ 所有角色
- 编辑仪表板 ✓ ops_engineer及以上
- 创建新仪表板 ✓ system_admin及以上
- 管理数据源 ✓ system_admin及以上

实施要点:
1. Grafana iframe 根据权限传递不同参数
2. 编辑模式通过 URL 参数控制
3. API 代理层进行权限验证
```

### 2.2 控制模块 (Control)

#### 设备控制 (Device Control)
```javascript
权限标识: CONTROL.DEVICE_CONTROL

功能明细:
- 查看控制面板 ✓ monitor及以上
- 执行单点控制 ✓ ops_engineer及以上
- 批量控制操作 ✓ ops_engineer及以上
- 紧急停机操作 ✓ ops_engineer及以上

实施要点:
1. 控制按钮根据权限启用/禁用
2. 关键操作需要二次确认
3. 操作日志实时记录
4. WebSocket 推送操作结果
```

#### 告警管理 (Alarm Management)
```javascript
权限标识: CONTROL.ALARM_MANAGE

功能明细:
- 查看告警列表 ✓ monitor及以上
- 确认告警 ✓ monitor及以上
- 处理告警 ✓ ops_engineer及以上
- 配置告警规则 ✓ system_admin及以上

实施要点:
1. 告警推送根据角色过滤
2. 处理流程根据权限分级
3. 历史记录完整保留
```

### 2.3 配置模块 (Configuration)

#### 通道配置 (Channel Config)
```javascript
权限标识: CONFIG.CHANNEL_CONFIG

功能明细:
- 查看通道列表 ✓ ops_engineer及以上
- 修改通道参数 ✓ ops_engineer及以上
- 新增通道 ✓ system_admin及以上
- 删除通道 ✓ system_admin及以上

实施要点:
1. 敏感参数（如密码）加密显示
2. 修改操作保留历史版本
3. 删除操作需要确认
```

#### 点表管理 (Point Table)
```javascript
权限标识: CONFIG.POINT_TABLE

功能明细:
- 查看点表 ✓ monitor及以上
- 编辑点位信息 ✓ ops_engineer及以上
- 批量导入/导出 ✓ ops_engineer及以上
- 点位映射配置 ✓ system_admin及以上

实施要点:
1. 支持 Excel/CSV 导入导出
2. 批量操作需要预览确认
3. 变更历史可追溯
```

### 2.4 系统管理 (System)

#### 用户管理 (User Management)
```javascript
权限标识: SYSTEM.USER_*

功能明细:
- 查看用户列表 ✓ system_admin及以上
- 创建用户 ✓ system_admin及以上
- 编辑用户 ✓ system_admin及以上
- 删除用户 ✓ system_admin及以上*
- 重置密码 ✓ system_admin及以上*

*限制: 不能操作更高级别用户

实施要点:
1. 角色分配受当前用户角色限制
2. 批量操作支持但需审核
3. 密码策略强制执行
```

## 三、前端实施细节

### 3.1 路由权限控制
```javascript
// router/index.js
{
  path: '/monitoring/dashboard',
  component: Dashboard,
  meta: { 
    permissions: [PERMISSIONS.MONITORING.DASHBOARD_VIEW]
  }
}

// 路由守卫
router.beforeEach((to, from, next) => {
  const requiredPermissions = to.meta.permissions || []
  if (checkPermissions(requiredPermissions)) {
    next()
  } else {
    next('/403')
  }
})
```

### 3.2 组件权限控制
```vue
<!-- 使用 v-permission 指令 -->
<el-button 
  v-permission="PERMISSIONS.CONTROL.DEVICE_CONTROL"
  @click="handleControl"
>
  执行控制
</el-button>

<!-- 使用组合式函数 -->
<script setup>
const { checkPermission } = usePermission()
const canEdit = checkPermission(PERMISSIONS.CONFIG.CHANNEL_CONFIG)
</script>
```

### 3.3 菜单动态生成
```javascript
// 根据权限过滤菜单
const filterMenuByPermissions = (routes, permissions) => {
  return routes.filter(route => {
    if (route.meta?.permissions) {
      return hasAnyPermission(route.meta.permissions, permissions)
    }
    if (route.children) {
      route.children = filterMenuByPermissions(route.children, permissions)
    }
    return true
  })
}
```

## 四、数据权限控制

### 4.1 行级数据权限
```javascript
// 不同角色看到不同的数据范围
数据权限规则:
- guest: 仅公开数据
- monitor: 所负责区域的数据
- ops_engineer: 所有设备数据
- system_admin: 全系统数据
- super_admin: 包含系统配置数据
```

### 4.2 字段级权限
```javascript
// 敏感字段根据权限显示
字段权限规则:
- 设备密码: system_admin及以上
- 用户手机: system_admin及以上
- 系统密钥: super_admin
- 成本数据: ops_engineer及以上
```

## 五、权限配置示例

### 5.1 典型角色配置
```javascript
// 监控人员典型配置
{
  role: 'monitor',
  permissions: [
    'monitoring.dashboard_view',
    'monitoring.realtime_view',
    'monitoring.grafana_view',
    'control.alarm_view',
    'control.alarm_confirm'
  ],
  dataScope: 'assigned_area',
  allowedIPs: ['10.0.0.0/8'],
  sessionTimeout: 3600
}

// 运维工程师典型配置
{
  role: 'ops_engineer',
  permissions: [
    ...monitorPermissions,
    'control.device_control',
    'control.batch_control',
    'config.channel_config',
    'config.point_table'
  ],
  dataScope: 'all_devices',
  allowedIPs: ['*'],
  sessionTimeout: 7200
}
```

## 六、最佳实践

### 6.1 权限设计原则
1. **默认拒绝**: 未明确授权的默认拒绝访问
2. **细粒度控制**: 权限粒度到具体操作级别
3. **动态加载**: 权限可在线调整，无需重启
4. **缓存优化**: 权限判断结果适当缓存

### 6.2 安全加固
1. **会话管理**: 
   - 不同角色不同超时时间
   - 敏感操作需要重新认证
   
2. **审计日志**:
   - 记录所有权限相关操作
   - 包括授权失败的尝试
   
3. **异常检测**:
   - 检测异常访问模式
   - 自动锁定可疑账号

### 6.3 用户体验
1. **友好提示**: 无权限时给出明确提示
2. **降级方案**: 部分权限不影响主流程
3. **快捷申请**: 支持权限申请工作流

## 七、故障排查

### 7.1 常见问题
1. **权限不生效**
   - 检查缓存是否更新
   - 确认权限配置正确
   - 查看控制台错误

2. **菜单不显示**
   - 检查路由 meta 配置
   - 确认用户权限列表
   - 查看过滤逻辑

3. **按钮误显示**
   - 检查 v-permission 指令
   - 确认权限常量引用
   - 查看条件渲染逻辑

### 7.2 调试工具
```javascript
// 开发环境权限调试
if (process.env.NODE_ENV === 'development') {
  window.__checkPermission = checkPermission
  window.__currentPermissions = () => store.state.user.permissions
  window.__switchRole = (role) => store.dispatch('user/switchRole', role)
}
```

## 八、迁移指南

### 8.1 从旧版本升级
1. 备份现有权限配置
2. 运行权限迁移脚本
3. 验证权限映射正确
4. 分批更新用户权限
5. 监控异常访问

### 8.2 权限模型调整
1. 新增权限点
   - 更新权限常量
   - 分配给相应角色
   - 更新前端控制
   
2. 角色调整
   - 评估影响范围
   - 制定迁移计划
   - 通知相关用户