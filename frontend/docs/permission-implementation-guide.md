# Monarch Hub 权限系统实施指南

## 目录

1. [概述](#概述)
2. [权限系统架构](#权限系统架构)
3. [实施步骤](#实施步骤)
4. [各模块权限配置](#各模块权限配置)
5. [前端组件改造](#前端组件改造)
6. [API接口权限](#api接口权限)
7. [测试方案](#测试方案)
8. [常见问题](#常见问题)

## 概述

Monarch Hub 权限系统采用 RBAC（基于角色的访问控制）模型，通过角色分配权限，用户通过角色获得相应权限。系统支持细粒度的功能权限控制，包括页面访问、按钮操作、数据查看等。

### 核心概念

- **用户 (User)**: 系统使用者
- **角色 (Role)**: 权限集合的载体
- **权限 (Permission)**: 具体的功能操作权限
- **资源 (Resource)**: 受保护的功能或数据

## 权限系统架构

```
┌─────────────┐     ┌─────────────┐     ┌──────────────┐
│    用户     │ ──> │    角色     │ ──> │    权限      │
│   (User)    │     │   (Role)    │     │ (Permission) │
└─────────────┘     └─────────────┘     └──────────────┘
                           │                      │
                           ▼                      ▼
                    ┌─────────────┐     ┌──────────────┐
                    │  角色权限    │     │   资源权限    │
                    │  映射表      │     │   检查器      │
                    └─────────────┘     └──────────────┘
```

## 实施步骤

### 1. 安装和配置

#### 1.1 注册权限指令

在 `main.js` 中注册权限指令：

```javascript
import { createApp } from 'vue'
import App from './App.vue'
import { registerPermissionDirective } from '@/utils/permission'

const app = createApp(App)

// 注册权限指令
registerPermissionDirective(app)

app.mount('#app')
```

#### 1.2 配置路由守卫

在 `router/index.js` 中配置权限守卫：

```javascript
import { createRouter, createWebHistory } from 'vue-router'
import { setupPermissionGuard } from './permission'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    // 路由配置
  ]
})

// 设置权限守卫
setupPermissionGuard(router)

export default router
```

#### 1.3 配置用户状态管理

更新 `stores/user.js`：

```javascript
import { defineStore } from 'pinia'
import { getUserPermissions } from '@/utils/permission'

export const useUserStore = defineStore('user', {
  state: () => ({
    token: '',
    userInfo: {},
    role: '',
    roles: [],
    permissions: []
  }),
  
  actions: {
    // 设置用户信息
    setUserInfo(info) {
      this.userInfo = info
      this.role = info.role
      this.roles = info.roles || [info.role]
      // 根据角色获取权限
      this.permissions = getUserPermissions(this.roles, info.extraPermissions || [])
    },
    
    // 登出
    logout() {
      this.token = ''
      this.userInfo = {}
      this.role = ''
      this.roles = []
      this.permissions = []
    }
  }
})
```

### 2. 路由权限配置

更新路由配置，添加权限元信息：

```javascript
// router/modules/system.js
import { PERMISSIONS } from '@/utils/permission'

export default {
  path: '/system',
  component: Layout,
  meta: {
    title: '系统管理',
    icon: 'Setting',
    permissions: [PERMISSIONS.SYSTEM.USER_VIEW] // 需要查看用户权限
  },
  children: [
    {
      path: 'users',
      name: 'UserManagement',
      component: () => import('@/views/system/UserManagement.vue'),
      meta: {
        title: '用户管理',
        permissions: [PERMISSIONS.SYSTEM.USER_VIEW]
      }
    },
    {
      path: 'roles',
      name: 'RoleManagement',
      component: () => import('@/views/system/RoleManagement.vue'),
      meta: {
        title: '角色管理',
        permissions: [PERMISSIONS.SYSTEM.ROLE_VIEW]
      }
    },
    {
      path: 'settings',
      name: 'SystemSettings',
      component: () => import('@/views/system/SystemSettings.vue'),
      meta: {
        title: '系统设置',
        permissions: [PERMISSIONS.SYSTEM.SETTINGS_VIEW]
      }
    },
    {
      path: 'audit',
      name: 'AuditLogs',
      component: () => import('@/views/system/AuditLogs.vue'),
      meta: {
        title: '审计日志',
        permissions: [PERMISSIONS.SYSTEM.AUDIT_VIEW]
      }
    }
  ]
}
```

## 各模块权限配置

### 系统管理模块

| 页面 | 路由 | 查看权限 | 操作权限 |
|-----|------|---------|---------|
| 用户管理 | /system/users | system.user.view | create/edit/delete |
| 角色管理 | /system/roles | system.role.view | system.role.manage |
| 系统设置 | /system/settings | system.settings.view | system.settings.edit |
| 审计日志 | /system/audit | system.audit.view | export/clear |
| 服务监控 | /system/services | system.service.view | control/config |

### 配置管理模块

| 页面 | 路由 | 查看权限 | 操作权限 |
|-----|------|---------|---------|
| 通道配置 | /config/channels | config.channel.view | create/edit/delete |
| 点表管理 | /config/points | config.point.view | import/edit/delete |
| 模型配置 | /config/models | config.model.view | create/edit/delete |
| 告警规则 | /config/alarms | config.alarm.view | create/edit/delete |
| 存储策略 | /config/storage | config.storage.view | edit |
| 网络转发 | /config/network | config.network.view | edit |

### 监控功能模块

| 页面 | 路由 | 查看权限 | 操作权限 |
|-----|------|---------|---------|
| 仪表盘 | /monitoring/dashboard | monitor.dashboard.view | - |
| 实时监控 | /monitoring/realtime | monitor.realtime.view | export |
| 历史分析 | /monitoring/history | monitor.history.view | export |
| 设备状态 | /monitoring/devices | monitor.device.view | - |
| 系统拓扑 | /monitoring/topology | monitor.topology.view | edit |
| 能耗统计 | /monitoring/stats | monitor.stats.view | export |

### 控制功能模块

| 页面 | 路由 | 查看权限 | 操作权限 |
|-----|------|---------|---------|
| 设备控制 | /control/devices | control.device.view | control |
| 批量控制 | /control/batch | control.batch.view | control/approve |
| 定时任务 | /control/tasks | control.task.view | create/edit/delete/execute |
| 告警管理 | /control/alarms | control.alarm.view | confirm/handle/delete |

## 前端组件改造

### 1. 用户管理组件改造示例

```vue
<!-- views/system/UserManagement.vue -->
<template>
  <div class="user-management">
    <!-- 页面标题 -->
    <div class="page-header">
      <h1>{{ $t('menu.userManagement') }}</h1>
      <div class="header-actions">
        <!-- 创建按钮：需要创建权限 -->
        <el-button 
          v-permission="PERMISSIONS.SYSTEM.USER_CREATE"
          type="primary" 
          @click="handleCreate"
        >
          <el-icon><Plus /></el-icon>
          {{ $t('common.create') }}
        </el-button>
      </div>
    </div>

    <!-- 用户列表 -->
    <el-table :data="userList" stripe>
      <!-- ... 表格列 ... -->
      
      <el-table-column label="操作" width="200" fixed="right">
        <template #default="{ row }">
          <!-- 编辑按钮：需要编辑权限 -->
          <el-button 
            v-if="can.editUser.value"
            type="primary" 
            size="small"
            @click="handleEdit(row)"
          >
            编辑
          </el-button>
          
          <!-- 删除按钮：需要删除权限 -->
          <el-button 
            v-if="checkPermission(PERMISSIONS.SYSTEM.USER_DELETE)"
            type="danger" 
            size="small"
            @click="handleDelete(row)"
          >
            删除
          </el-button>
          
          <!-- 重置密码：编程式权限控制 -->
          <el-button 
            size="small"
            :disabled="!canResetPassword(row)"
            @click="handleResetPassword(row)"
          >
            重置密码
          </el-button>
        </template>
      </el-table-column>
    </el-table>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import { usePermission } from '@/composables/usePermission'

const { 
  PERMISSIONS, 
  can, 
  checkPermission,
  isSystemAdmin 
} = usePermission()

// 是否可以重置密码
const canResetPassword = (user) => {
  // 系统管理员可以重置普通用户密码
  // 超级管理员可以重置所有用户密码
  if (isSystemAdmin.value) {
    return user.role !== 'super_admin'
  }
  return false
}

// ... 其他逻辑
</script>
```

### 2. 菜单权限过滤

```vue
<!-- layouts/components/Sidebar.vue -->
<template>
  <el-menu>
    <template v-for="route in filteredRoutes" :key="route.path">
      <el-sub-menu v-if="route.children" :index="route.path">
        <template #title>
          <el-icon><component :is="route.meta.icon" /></el-icon>
          <span>{{ route.meta.title }}</span>
        </template>
        <el-menu-item 
          v-for="child in route.children" 
          :key="child.path"
          :index="child.path"
        >
          {{ child.meta.title }}
        </el-menu-item>
      </el-sub-menu>
      <el-menu-item v-else :index="route.path">
        <el-icon><component :is="route.meta.icon" /></el-icon>
        <span>{{ route.meta.title }}</span>
      </el-menu-item>
    </template>
  </el-menu>
</template>

<script setup>
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { useUserStore } from '@/stores/user'
import { filterRoutesByPermission } from '@/utils/permission'
import routes from '@/router/routes'

const route = useRoute()
const userStore = useUserStore()

// 根据权限过滤路由
const filteredRoutes = computed(() => {
  return filterRoutesByPermission(routes, userStore.permissions)
})
</script>
```

### 3. 通用权限按钮组件

```vue
<!-- components/PermissionButton.vue -->
<template>
  <el-button
    v-if="hasPermission"
    v-bind="$attrs"
    :disabled="disabled || !hasPermission"
  >
    <slot />
  </el-button>
</template>

<script setup>
import { computed } from 'vue'
import { usePermission } from '@/composables/usePermission'

const props = defineProps({
  permission: {
    type: [String, Array],
    required: true
  },
  disabled: {
    type: Boolean,
    default: false
  }
})

const { checkPermission, checkAnyPermission } = usePermission()

const hasPermission = computed(() => {
  if (Array.isArray(props.permission)) {
    return checkAnyPermission(props.permission)
  }
  return checkPermission(props.permission)
})
</script>
```

## API接口权限

### 1. 请求拦截器配置

```javascript
// utils/request.js
import axios from 'axios'
import { ElMessage } from 'element-plus'
import { useUserStore } from '@/stores/user'

const service = axios.create({
  baseURL: process.env.VUE_APP_BASE_API,
  timeout: 15000
})

// 请求拦截器
service.interceptors.request.use(
  config => {
    const userStore = useUserStore()
    
    // 添加认证token
    if (userStore.token) {
      config.headers['Authorization'] = `Bearer ${userStore.token}`
    }
    
    // 添加用户角色信息（可选）
    if (userStore.role) {
      config.headers['X-User-Role'] = userStore.role
    }
    
    return config
  },
  error => {
    return Promise.reject(error)
  }
)

// 响应拦截器
service.interceptors.response.use(
  response => {
    return response.data
  },
  error => {
    if (error.response) {
      switch (error.response.status) {
        case 401:
          // 未授权，跳转登录
          userStore.logout()
          router.push('/login')
          break
        case 403:
          // 无权限
          ElMessage.error('您没有权限执行此操作')
          break
        default:
          ElMessage.error(error.response.data.message || '请求失败')
      }
    }
    return Promise.reject(error)
  }
)

export default service
```

### 2. API权限封装

```javascript
// api/system/user.js
import request from '@/utils/request'
import { checkPermission } from '@/utils/permission'
import { PERMISSIONS } from '@/utils/permission'

export const userApi = {
  // 获取用户列表
  getUsers(params) {
    return request({
      url: '/api/users',
      method: 'get',
      params
    })
  },
  
  // 创建用户（前端权限检查）
  createUser(data) {
    if (!checkPermission(PERMISSIONS.SYSTEM.USER_CREATE)) {
      return Promise.reject(new Error('无创建用户权限'))
    }
    
    return request({
      url: '/api/users',
      method: 'post',
      data
    })
  },
  
  // 更新用户
  updateUser(id, data) {
    if (!checkPermission(PERMISSIONS.SYSTEM.USER_EDIT)) {
      return Promise.reject(new Error('无编辑用户权限'))
    }
    
    return request({
      url: `/api/users/${id}`,
      method: 'put',
      data
    })
  },
  
  // 删除用户
  deleteUser(id) {
    if (!checkPermission(PERMISSIONS.SYSTEM.USER_DELETE)) {
      return Promise.reject(new Error('无删除用户权限'))
    }
    
    return request({
      url: `/api/users/${id}`,
      method: 'delete'
    })
  }
}
```

## 测试方案

### 1. 单元测试

```javascript
// tests/unit/permission.spec.js
import { describe, it, expect } from 'vitest'
import { hasPermission, hasAnyPermission, getUserPermissions } from '@/utils/permission'
import { PERMISSIONS, ROLES, ROLE_PERMISSIONS } from '@/utils/permission'

describe('权限工具函数', () => {
  describe('hasPermission', () => {
    it('应该正确检查单个权限', () => {
      const userPermissions = ['system.user.view', 'system.user.create']
      
      expect(hasPermission('system.user.view', userPermissions)).toBe(true)
      expect(hasPermission('system.user.delete', userPermissions)).toBe(false)
    })
    
    it('应该正确检查多个权限（AND逻辑）', () => {
      const userPermissions = ['system.user.view', 'system.user.create']
      
      expect(hasPermission(['system.user.view', 'system.user.create'], userPermissions)).toBe(true)
      expect(hasPermission(['system.user.view', 'system.user.delete'], userPermissions)).toBe(false)
    })
  })
  
  describe('角色权限', () => {
    it('超级管理员应该拥有所有权限', () => {
      const permissions = ROLE_PERMISSIONS[ROLES.SUPER_ADMIN]
      
      expect(permissions).toContain(PERMISSIONS.SYSTEM.USER_DELETE)
      expect(permissions).toContain(PERMISSIONS.CONFIG.CHANNEL_CREATE)
      expect(permissions).toContain(PERMISSIONS.CONTROL.DEVICE_CONTROL)
    })
    
    it('访客应该只有基础查看权限', () => {
      const permissions = ROLE_PERMISSIONS[ROLES.GUEST]
      
      expect(permissions).toContain(PERMISSIONS.MONITOR.REALTIME_VIEW)
      expect(permissions).not.toContain(PERMISSIONS.CONTROL.DEVICE_CONTROL)
      expect(permissions).not.toContain(PERMISSIONS.SYSTEM.USER_CREATE)
    })
  })
})
```

### 2. 集成测试

```javascript
// tests/e2e/permission.cy.js
describe('权限系统集成测试', () => {
  beforeEach(() => {
    cy.visit('/login')
  })
  
  it('超级管理员可以访问所有功能', () => {
    // 使用超级管理员登录
    cy.login('admin', 'password')
    
    // 检查所有菜单可见
    cy.get('.el-menu-item').contains('用户管理').should('exist')
    cy.get('.el-menu-item').contains('系统设置').should('exist')
    cy.get('.el-menu-item').contains('设备控制').should('exist')
    
    // 检查操作按钮可见
    cy.visit('/system/users')
    cy.get('button').contains('创建').should('exist')
    cy.get('button').contains('删除').should('exist')
  })
  
  it('监控人员只能查看不能操作', () => {
    // 使用监控人员登录
    cy.login('monitor', 'password')
    
    // 检查只有监控菜单可见
    cy.get('.el-menu-item').contains('实时监控').should('exist')
    cy.get('.el-menu-item').contains('用户管理').should('not.exist')
    
    // 访问实时监控页面
    cy.visit('/monitoring/realtime')
    
    // 检查没有控制按钮
    cy.get('button').contains('控制').should('not.exist')
    cy.get('button').contains('导出').should('not.exist')
  })
  
  it('无权限访问时应跳转到403页面', () => {
    // 使用访客登录
    cy.login('guest', 'password')
    
    // 尝试访问用户管理页面
    cy.visit('/system/users')
    
    // 应该跳转到403页面
    cy.url().should('include', '/403')
    cy.contains('403').should('exist')
    cy.contains('您没有权限访问此页面').should('exist')
  })
})
```

### 3. 权限测试清单

| 测试项 | 测试内容 | 预期结果 |
|--------|---------|----------|
| 路由权限 | 不同角色访问受限页面 | 无权限时跳转403 |
| 菜单显示 | 不同角色查看菜单 | 只显示有权限的菜单 |
| 按钮权限 | 操作按钮的显示/禁用 | 无权限时隐藏或禁用 |
| API权限 | 调用受限API | 返回403错误 |
| 数据权限 | 查看受限数据 | 只能看到权限范围内的数据 |

## 常见问题

### Q1: 如何添加新的权限？

1. 在 `utils/permission.js` 中的 `PERMISSIONS` 对象添加新权限
2. 在相应角色的权限列表中添加该权限
3. 在需要控制的组件/路由中使用该权限

### Q2: 如何创建自定义角色？

1. 在 `ROLES` 中添加角色常量
2. 在 `ROLE_PERMISSIONS` 中定义角色权限
3. 在 `ROLE_NAMES` 中添加角色显示名称

### Q3: 如何实现数据级权限？

可以通过以下方式实现：

```javascript
// 在用户信息中添加数据权限
{
  userId: 1,
  role: 'ops_engineer',
  dataPermissions: {
    deviceGroups: [1, 2, 3], // 可访问的设备组
    regions: ['east', 'west'], // 可访问的区域
    timeRange: 30 // 可查询的历史数据天数
  }
}

// 在API请求时带上数据权限参数
const getDeviceData = (params) => {
  const userStore = useUserStore()
  return request({
    url: '/api/devices',
    params: {
      ...params,
      deviceGroups: userStore.dataPermissions.deviceGroups
    }
  })
}
```

### Q4: 如何处理权限变更？

1. 实时更新：通过WebSocket接收权限变更通知
2. 定期检查：每隔一定时间重新获取用户权限
3. 手动刷新：提供刷新按钮让用户主动更新权限

### Q5: 如何优化权限检查性能？

1. 使用计算属性缓存权限检查结果
2. 避免在模板中进行复杂的权限计算
3. 使用权限指令而不是 v-if 进行简单的显示控制

## 下一步计划

1. **实施权限系统**：按照本文档逐步改造现有组件
2. **添加审计日志**：记录所有权限相关操作
3. **优化用户体验**：添加权限不足的友好提示
4. **性能优化**：实施权限缓存机制
5. **安全加固**：添加权限防护和反作弊机制