# Monarch Hub 角色权限定义

## 角色概述

系统定义了5个主要角色，从高到低分别为：

1. **超级管理员 (Super Admin)**
2. **系统管理员 (System Admin)**
3. **运维工程师 (Operations Engineer)**
4. **监控人员 (Monitor)**
5. **访客 (Guest)**

## 详细权限矩阵

### 1. 超级管理员 (Super Admin)
**角色代码**: `super_admin`
**描述**: 拥有系统所有权限，可以管理所有功能和数据

| 功能模块 | 权限 | 说明 |
|---------|------|------|
| **系统管理** | | |
| 用户管理 | ✅ 完全控制 | 创建、编辑、删除、重置密码、分配角色 |
| 角色管理 | ✅ 完全控制 | 创建、编辑、删除角色及权限 |
| 系统设置 | ✅ 完全控制 | 修改所有系统配置 |
| 审计日志 | ✅ 完全访问 | 查看、导出、清理日志 |
| 服务监控 | ✅ 完全控制 | 启停服务、查看状态、修改配置 |
| **配置管理** | | |
| 通道配置 | ✅ 完全控制 | 创建、编辑、删除通道 |
| 点表管理 | ✅ 完全控制 | 导入、编辑、删除点表 |
| 模型配置 | ✅ 完全控制 | 创建、编辑、删除计算模型 |
| 告警规则 | ✅ 完全控制 | 创建、编辑、删除告警规则 |
| 存储策略 | ✅ 完全控制 | 配置数据存储策略 |
| 网络转发 | ✅ 完全控制 | 配置数据转发规则 |
| **监控功能** | | |
| 实时监控 | ✅ 完全访问 | 查看所有数据 |
| 历史数据 | ✅ 完全访问 | 查询、导出历史数据 |
| 设备状态 | ✅ 完全访问 | 查看所有设备状态 |
| 系统拓扑 | ✅ 完全访问 | 查看、编辑拓扑图 |
| 能耗统计 | ✅ 完全访问 | 查看、导出统计报表 |
| **控制功能** | | |
| 设备控制 | ✅ 完全控制 | 执行所有控制操作 |
| 批量控制 | ✅ 完全控制 | 批量操作设备 |
| 定时任务 | ✅ 完全控制 | 创建、编辑、删除定时任务 |
| 告警管理 | ✅ 完全控制 | 确认、忽略、删除告警 |

### 2. 系统管理员 (System Admin)
**角色代码**: `system_admin`
**描述**: 负责系统日常管理和配置，但不能修改核心系统设置

| 功能模块 | 权限 | 说明 |
|---------|------|------|
| **系统管理** | | |
| 用户管理 | ✅ 部分控制 | 创建、编辑普通用户，不能删除用户 |
| 角色管理 | ❌ 只读 | 只能查看角色定义 |
| 系统设置 | ⚠️ 受限 | 修改部分系统配置 |
| 审计日志 | ✅ 查看导出 | 查看、导出日志，不能清理 |
| 服务监控 | ⚠️ 受限 | 查看状态、重启服务，不能修改配置 |
| **配置管理** | | |
| 通道配置 | ✅ 完全控制 | 创建、编辑、删除通道 |
| 点表管理 | ✅ 完全控制 | 导入、编辑、删除点表 |
| 模型配置 | ✅ 完全控制 | 创建、编辑、删除计算模型 |
| 告警规则 | ✅ 完全控制 | 创建、编辑、删除告警规则 |
| 存储策略 | ✅ 查看编辑 | 编辑策略，不能删除 |
| 网络转发 | ✅ 查看编辑 | 编辑规则，不能删除 |
| **监控功能** | | |
| 实时监控 | ✅ 完全访问 | 查看所有数据 |
| 历史数据 | ✅ 完全访问 | 查询、导出历史数据 |
| 设备状态 | ✅ 完全访问 | 查看所有设备状态 |
| 系统拓扑 | ✅ 查看编辑 | 查看、编辑拓扑图 |
| 能耗统计 | ✅ 完全访问 | 查看、导出统计报表 |
| **控制功能** | | |
| 设备控制 | ✅ 完全控制 | 执行所有控制操作 |
| 批量控制 | ✅ 完全控制 | 批量操作设备 |
| 定时任务 | ✅ 完全控制 | 创建、编辑、删除定时任务 |
| 告警管理 | ✅ 完全控制 | 确认、忽略告警，不能删除 |

### 3. 运维工程师 (Operations Engineer)
**角色代码**: `ops_engineer`
**描述**: 负责设备运维和日常操作，重点在监控和控制

| 功能模块 | 权限 | 说明 |
|---------|------|------|
| **系统管理** | | |
| 用户管理 | ❌ 无权限 | - |
| 角色管理 | ❌ 无权限 | - |
| 系统设置 | ❌ 只读 | 只能查看系统信息 |
| 审计日志 | ⚠️ 受限 | 只能查看自己的操作日志 |
| 服务监控 | ✅ 只读 | 查看服务状态 |
| **配置管理** | | |
| 通道配置 | ❌ 只读 | 只能查看配置 |
| 点表管理 | ⚠️ 受限 | 编辑点位属性，不能增删 |
| 模型配置 | ❌ 只读 | 只能查看模型 |
| 告警规则 | ⚠️ 受限 | 启用/禁用规则，不能增删改 |
| 存储策略 | ❌ 只读 | 只能查看策略 |
| 网络转发 | ❌ 只读 | 只能查看规则 |
| **监控功能** | | |
| 实时监控 | ✅ 完全访问 | 查看所有数据 |
| 历史数据 | ✅ 查询导出 | 查询、导出历史数据 |
| 设备状态 | ✅ 完全访问 | 查看所有设备状态 |
| 系统拓扑 | ✅ 只读 | 只能查看拓扑图 |
| 能耗统计 | ✅ 查看导出 | 查看、导出统计报表 |
| **控制功能** | | |
| 设备控制 | ✅ 执行控制 | 执行控制操作，有审计记录 |
| 批量控制 | ⚠️ 受限 | 需要审批的批量操作 |
| 定时任务 | ✅ 查看执行 | 执行任务，不能创建修改 |
| 告警管理 | ✅ 确认处理 | 确认、处理告警，添加备注 |

### 4. 监控人员 (Monitor)
**角色代码**: `monitor`
**描述**: 只能查看监控数据和告警，不能执行控制操作

| 功能模块 | 权限 | 说明 |
|---------|------|------|
| **系统管理** | | |
| 用户管理 | ❌ 无权限 | - |
| 角色管理 | ❌ 无权限 | - |
| 系统设置 | ❌ 无权限 | - |
| 审计日志 | ❌ 无权限 | - |
| 服务监控 | ❌ 无权限 | - |
| **配置管理** | | |
| 通道配置 | ❌ 无权限 | - |
| 点表管理 | ❌ 只读 | 只能查看点表 |
| 模型配置 | ❌ 无权限 | - |
| 告警规则 | ❌ 只读 | 只能查看规则 |
| 存储策略 | ❌ 无权限 | - |
| 网络转发 | ❌ 无权限 | - |
| **监控功能** | | |
| 实时监控 | ✅ 只读 | 查看实时数据 |
| 历史数据 | ✅ 查询 | 查询历史数据，不能导出 |
| 设备状态 | ✅ 只读 | 查看设备状态 |
| 系统拓扑 | ✅ 只读 | 查看拓扑图 |
| 能耗统计 | ✅ 只读 | 查看统计数据 |
| **控制功能** | | |
| 设备控制 | ❌ 无权限 | - |
| 批量控制 | ❌ 无权限 | - |
| 定时任务 | ❌ 只读 | 只能查看任务状态 |
| 告警管理 | ✅ 查看确认 | 查看、确认告警 |

### 5. 访客 (Guest)
**角色代码**: `guest`
**描述**: 临时访问权限，只能查看基本信息

| 功能模块 | 权限 | 说明 |
|---------|------|------|
| **系统管理** | ❌ 无权限 | 所有系统管理功能不可见 |
| **配置管理** | ❌ 无权限 | 所有配置管理功能不可见 |
| **监控功能** | | |
| 实时监控 | ⚠️ 受限 | 只能查看指定设备的数据 |
| 历史数据 | ❌ 无权限 | - |
| 设备状态 | ⚠️ 受限 | 只能查看概览信息 |
| 系统拓扑 | ✅ 只读 | 查看拓扑图 |
| 能耗统计 | ⚠️ 受限 | 只能查看汇总数据 |
| **控制功能** | ❌ 无权限 | 所有控制功能不可见 |

## 权限继承关系

```
超级管理员
    ↓ (继承所有权限)
系统管理员
    ↓ (继承监控和部分配置权限)
运维工程师
    ↓ (继承监控权限)
监控人员
    ↓ (继承基础查看权限)
访客
```

## 特殊权限说明

### 1. 数据权限隔离
- **设备组权限**: 可以将用户限制在特定的设备组内
- **区域权限**: 按照物理区域限制数据访问
- **时间权限**: 限制历史数据的查询时间范围

### 2. 操作审计
- 所有控制操作都需要记录操作人、时间、操作内容
- 敏感操作（如删除、批量控制）需要二次确认
- 部分高危操作需要双人审核

### 3. 临时授权
- 支持临时提升用户权限
- 设置授权时效（如8小时、24小时）
- 授权到期自动回收

### 4. API访问权限
- 每个角色对应不同的API访问权限
- 支持API Key认证
- 限制API调用频率

## 实施建议

1. **最小权限原则**: 默认给予用户完成工作所需的最小权限
2. **定期审查**: 每季度审查用户权限，移除不必要的权限
3. **权限申请流程**: 建立标准的权限申请和审批流程
4. **应急响应**: 准备应急账号，但平时禁用，紧急情况下启用

## 前端实现示例

```javascript
// 权限常量定义
export const PERMISSIONS = {
  // 系统管理
  SYSTEM: {
    USER_VIEW: 'system.user.view',
    USER_CREATE: 'system.user.create',
    USER_EDIT: 'system.user.edit',
    USER_DELETE: 'system.user.delete',
    ROLE_VIEW: 'system.role.view',
    ROLE_MANAGE: 'system.role.manage',
    SETTINGS_VIEW: 'system.settings.view',
    SETTINGS_EDIT: 'system.settings.edit',
    AUDIT_VIEW: 'system.audit.view',
    AUDIT_EXPORT: 'system.audit.export',
    SERVICE_VIEW: 'system.service.view',
    SERVICE_CONTROL: 'system.service.control'
  },
  
  // 配置管理
  CONFIG: {
    CHANNEL_VIEW: 'config.channel.view',
    CHANNEL_MANAGE: 'config.channel.manage',
    POINT_VIEW: 'config.point.view',
    POINT_MANAGE: 'config.point.manage',
    MODEL_VIEW: 'config.model.view',
    MODEL_MANAGE: 'config.model.manage',
    ALARM_VIEW: 'config.alarm.view',
    ALARM_MANAGE: 'config.alarm.manage'
  },
  
  // 监控功能
  MONITOR: {
    REALTIME_VIEW: 'monitor.realtime.view',
    HISTORY_VIEW: 'monitor.history.view',
    HISTORY_EXPORT: 'monitor.history.export',
    DEVICE_VIEW: 'monitor.device.view',
    TOPOLOGY_VIEW: 'monitor.topology.view',
    TOPOLOGY_EDIT: 'monitor.topology.edit',
    STATS_VIEW: 'monitor.stats.view',
    STATS_EXPORT: 'monitor.stats.export'
  },
  
  // 控制功能
  CONTROL: {
    DEVICE_CONTROL: 'control.device.control',
    BATCH_CONTROL: 'control.batch.control',
    TASK_VIEW: 'control.task.view',
    TASK_MANAGE: 'control.task.manage',
    ALARM_VIEW: 'control.alarm.view',
    ALARM_HANDLE: 'control.alarm.handle'
  }
}

// 角色权限映射
export const ROLE_PERMISSIONS = {
  super_admin: Object.values(PERMISSIONS).flatMap(group => Object.values(group)),
  
  system_admin: [
    // 系统管理（部分）
    PERMISSIONS.SYSTEM.USER_VIEW,
    PERMISSIONS.SYSTEM.USER_CREATE,
    PERMISSIONS.SYSTEM.USER_EDIT,
    PERMISSIONS.SYSTEM.ROLE_VIEW,
    PERMISSIONS.SYSTEM.SETTINGS_VIEW,
    PERMISSIONS.SYSTEM.SETTINGS_EDIT,
    PERMISSIONS.SYSTEM.AUDIT_VIEW,
    PERMISSIONS.SYSTEM.AUDIT_EXPORT,
    PERMISSIONS.SYSTEM.SERVICE_VIEW,
    // 配置管理（全部）
    ...Object.values(PERMISSIONS.CONFIG),
    // 监控功能（全部）
    ...Object.values(PERMISSIONS.MONITOR),
    // 控制功能（全部）
    ...Object.values(PERMISSIONS.CONTROL)
  ],
  
  ops_engineer: [
    // 系统管理（极少）
    PERMISSIONS.SYSTEM.SETTINGS_VIEW,
    PERMISSIONS.SYSTEM.SERVICE_VIEW,
    // 配置管理（只读）
    PERMISSIONS.CONFIG.CHANNEL_VIEW,
    PERMISSIONS.CONFIG.POINT_VIEW,
    PERMISSIONS.CONFIG.MODEL_VIEW,
    PERMISSIONS.CONFIG.ALARM_VIEW,
    // 监控功能（全部）
    ...Object.values(PERMISSIONS.MONITOR),
    // 控制功能（大部分）
    PERMISSIONS.CONTROL.DEVICE_CONTROL,
    PERMISSIONS.CONTROL.BATCH_CONTROL,
    PERMISSIONS.CONTROL.TASK_VIEW,
    PERMISSIONS.CONTROL.ALARM_VIEW,
    PERMISSIONS.CONTROL.ALARM_HANDLE
  ],
  
  monitor: [
    // 配置管理（极少只读）
    PERMISSIONS.CONFIG.POINT_VIEW,
    PERMISSIONS.CONFIG.ALARM_VIEW,
    // 监控功能（大部分）
    PERMISSIONS.MONITOR.REALTIME_VIEW,
    PERMISSIONS.MONITOR.HISTORY_VIEW,
    PERMISSIONS.MONITOR.DEVICE_VIEW,
    PERMISSIONS.MONITOR.TOPOLOGY_VIEW,
    PERMISSIONS.MONITOR.STATS_VIEW,
    // 控制功能（只看告警）
    PERMISSIONS.CONTROL.TASK_VIEW,
    PERMISSIONS.CONTROL.ALARM_VIEW,
    PERMISSIONS.CONTROL.ALARM_HANDLE
  ],
  
  guest: [
    // 监控功能（受限）
    PERMISSIONS.MONITOR.REALTIME_VIEW,
    PERMISSIONS.MONITOR.DEVICE_VIEW,
    PERMISSIONS.MONITOR.TOPOLOGY_VIEW,
    PERMISSIONS.MONITOR.STATS_VIEW
  ]
}
```