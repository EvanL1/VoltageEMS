/**
 * 权限管理工具类
 */

import { useUserStore } from '@/stores/user'

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
    AUDIT_CLEAR: 'system.audit.clear',
    SERVICE_VIEW: 'system.service.view',
    SERVICE_CONTROL: 'system.service.control',
    SERVICE_CONFIG: 'system.service.config'
  },
  
  // 配置管理
  CONFIG: {
    CHANNEL_VIEW: 'config.channel.view',
    CHANNEL_CREATE: 'config.channel.create',
    CHANNEL_EDIT: 'config.channel.edit',
    CHANNEL_DELETE: 'config.channel.delete',
    POINT_VIEW: 'config.point.view',
    POINT_IMPORT: 'config.point.import',
    POINT_EDIT: 'config.point.edit',
    POINT_DELETE: 'config.point.delete',
    MODEL_VIEW: 'config.model.view',
    MODEL_CREATE: 'config.model.create',
    MODEL_EDIT: 'config.model.edit',
    MODEL_DELETE: 'config.model.delete',
    ALARM_VIEW: 'config.alarm.view',
    ALARM_CREATE: 'config.alarm.create',
    ALARM_EDIT: 'config.alarm.edit',
    ALARM_DELETE: 'config.alarm.delete',
    STORAGE_VIEW: 'config.storage.view',
    STORAGE_EDIT: 'config.storage.edit',
    NETWORK_VIEW: 'config.network.view',
    NETWORK_EDIT: 'config.network.edit'
  },
  
  // 监控功能
  MONITOR: {
    REALTIME_VIEW: 'monitor.realtime.view',
    REALTIME_EXPORT: 'monitor.realtime.export',
    HISTORY_VIEW: 'monitor.history.view',
    HISTORY_EXPORT: 'monitor.history.export',
    DEVICE_VIEW: 'monitor.device.view',
    TOPOLOGY_VIEW: 'monitor.topology.view',
    TOPOLOGY_EDIT: 'monitor.topology.edit',
    STATS_VIEW: 'monitor.stats.view',
    STATS_EXPORT: 'monitor.stats.export',
    DASHBOARD_VIEW: 'monitor.dashboard.view'
  },
  
  // 控制功能
  CONTROL: {
    DEVICE_VIEW: 'control.device.view',
    DEVICE_CONTROL: 'control.device.control',
    BATCH_VIEW: 'control.batch.view',
    BATCH_CONTROL: 'control.batch.control',
    BATCH_APPROVE: 'control.batch.approve',
    TASK_VIEW: 'control.task.view',
    TASK_CREATE: 'control.task.create',
    TASK_EDIT: 'control.task.edit',
    TASK_DELETE: 'control.task.delete',
    TASK_EXECUTE: 'control.task.execute',
    ALARM_VIEW: 'control.alarm.view',
    ALARM_CONFIRM: 'control.alarm.confirm',
    ALARM_HANDLE: 'control.alarm.handle',
    ALARM_DELETE: 'control.alarm.delete'
  }
}

// 角色定义
export const ROLES = {
  SUPER_ADMIN: 'super_admin',
  SYSTEM_ADMIN: 'system_admin',
  OPS_ENGINEER: 'ops_engineer',
  MONITOR: 'monitor',
  GUEST: 'guest'
}

// 角色权限映射
export const ROLE_PERMISSIONS = {
  [ROLES.SUPER_ADMIN]: Object.values(PERMISSIONS).flatMap(group => Object.values(group)),
  
  [ROLES.SYSTEM_ADMIN]: [
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
    PERMISSIONS.SYSTEM.SERVICE_CONTROL,
    // 配置管理（全部）
    ...Object.values(PERMISSIONS.CONFIG),
    // 监控功能（全部）
    ...Object.values(PERMISSIONS.MONITOR),
    // 控制功能（大部分）
    PERMISSIONS.CONTROL.DEVICE_VIEW,
    PERMISSIONS.CONTROL.DEVICE_CONTROL,
    PERMISSIONS.CONTROL.BATCH_VIEW,
    PERMISSIONS.CONTROL.BATCH_CONTROL,
    PERMISSIONS.CONTROL.TASK_VIEW,
    PERMISSIONS.CONTROL.TASK_CREATE,
    PERMISSIONS.CONTROL.TASK_EDIT,
    PERMISSIONS.CONTROL.TASK_DELETE,
    PERMISSIONS.CONTROL.TASK_EXECUTE,
    PERMISSIONS.CONTROL.ALARM_VIEW,
    PERMISSIONS.CONTROL.ALARM_CONFIRM,
    PERMISSIONS.CONTROL.ALARM_HANDLE
  ],
  
  [ROLES.OPS_ENGINEER]: [
    // 系统管理（极少）
    PERMISSIONS.SYSTEM.SETTINGS_VIEW,
    PERMISSIONS.SYSTEM.SERVICE_VIEW,
    PERMISSIONS.SYSTEM.AUDIT_VIEW,
    // 配置管理（只读和部分编辑）
    PERMISSIONS.CONFIG.CHANNEL_VIEW,
    PERMISSIONS.CONFIG.POINT_VIEW,
    PERMISSIONS.CONFIG.POINT_EDIT,
    PERMISSIONS.CONFIG.MODEL_VIEW,
    PERMISSIONS.CONFIG.ALARM_VIEW,
    // 监控功能（全部）
    ...Object.values(PERMISSIONS.MONITOR),
    // 控制功能（执行权限）
    PERMISSIONS.CONTROL.DEVICE_VIEW,
    PERMISSIONS.CONTROL.DEVICE_CONTROL,
    PERMISSIONS.CONTROL.BATCH_VIEW,
    PERMISSIONS.CONTROL.TASK_VIEW,
    PERMISSIONS.CONTROL.TASK_EXECUTE,
    PERMISSIONS.CONTROL.ALARM_VIEW,
    PERMISSIONS.CONTROL.ALARM_CONFIRM,
    PERMISSIONS.CONTROL.ALARM_HANDLE
  ],
  
  [ROLES.MONITOR]: [
    // 配置管理（极少只读）
    PERMISSIONS.CONFIG.POINT_VIEW,
    PERMISSIONS.CONFIG.ALARM_VIEW,
    // 监控功能（只读）
    PERMISSIONS.MONITOR.REALTIME_VIEW,
    PERMISSIONS.MONITOR.HISTORY_VIEW,
    PERMISSIONS.MONITOR.DEVICE_VIEW,
    PERMISSIONS.MONITOR.TOPOLOGY_VIEW,
    PERMISSIONS.MONITOR.STATS_VIEW,
    PERMISSIONS.MONITOR.DASHBOARD_VIEW,
    // 控制功能（只看告警）
    PERMISSIONS.CONTROL.TASK_VIEW,
    PERMISSIONS.CONTROL.ALARM_VIEW,
    PERMISSIONS.CONTROL.ALARM_CONFIRM
  ],
  
  [ROLES.GUEST]: [
    // 监控功能（受限）
    PERMISSIONS.MONITOR.REALTIME_VIEW,
    PERMISSIONS.MONITOR.DEVICE_VIEW,
    PERMISSIONS.MONITOR.TOPOLOGY_VIEW,
    PERMISSIONS.MONITOR.STATS_VIEW,
    PERMISSIONS.MONITOR.DASHBOARD_VIEW
  ]
}

// 角色显示名称
export const ROLE_NAMES = {
  [ROLES.SUPER_ADMIN]: '超级管理员',
  [ROLES.SYSTEM_ADMIN]: '系统管理员',
  [ROLES.OPS_ENGINEER]: '运维工程师',
  [ROLES.MONITOR]: '监控人员',
  [ROLES.GUEST]: '访客'
}

// 角色描述
export const ROLE_DESCRIPTIONS = {
  [ROLES.SUPER_ADMIN]: '拥有系统所有权限，可以管理所有功能和数据',
  [ROLES.SYSTEM_ADMIN]: '负责系统日常管理和配置，但不能修改核心系统设置',
  [ROLES.OPS_ENGINEER]: '负责设备运维和日常操作，重点在监控和控制',
  [ROLES.MONITOR]: '只能查看监控数据和告警，不能执行控制操作',
  [ROLES.GUEST]: '临时访问权限，只能查看基本信息'
}

/**
 * 检查用户是否有某个权限
 * @param {string|string[]} permission - 权限标识或权限数组
 * @param {string[]} userPermissions - 用户拥有的权限列表
 * @returns {boolean}
 */
export function hasPermission(permission, userPermissions = []) {
  if (!permission) return true
  
  if (Array.isArray(permission)) {
    // 检查是否拥有所有权限（AND逻辑）
    return permission.every(p => userPermissions.includes(p))
  }
  
  return userPermissions.includes(permission)
}

/**
 * 检查用户是否有任一权限
 * @param {string[]} permissions - 权限数组
 * @param {string[]} userPermissions - 用户拥有的权限列表
 * @returns {boolean}
 */
export function hasAnyPermission(permissions, userPermissions = []) {
  if (!permissions || permissions.length === 0) return true
  
  // 检查是否拥有任一权限（OR逻辑）
  return permissions.some(p => userPermissions.includes(p))
}

/**
 * 根据角色获取权限列表
 * @param {string} role - 角色标识
 * @returns {string[]}
 */
export function getPermissionsByRole(role) {
  return ROLE_PERMISSIONS[role] || []
}

/**
 * 获取用户的所有权限（合并角色权限和额外权限）
 * @param {string[]} roles - 用户角色列表
 * @param {string[]} extraPermissions - 额外权限列表
 * @returns {string[]}
 */
export function getUserPermissions(roles = [], extraPermissions = []) {
  const rolePermissions = roles.flatMap(role => getPermissionsByRole(role))
  const allPermissions = [...new Set([...rolePermissions, ...extraPermissions])]
  return allPermissions
}

/**
 * 权限指令注册
 */
export function registerPermissionDirective(app) {
  app.directive('permission', {
    mounted(el, binding) {
      const { value } = binding
      const userStore = useUserStore()
      const userPermissions = userStore.permissions || []
      
      if (value && value.length > 0) {
        const hasPermissions = hasPermission(value, userPermissions)
        
        if (!hasPermissions) {
          el.style.display = 'none'
          // 或者直接移除元素
          // el.parentNode && el.parentNode.removeChild(el)
        }
      }
    },
    updated(el, binding) {
      const { value } = binding
      const userStore = useUserStore()
      const userPermissions = userStore.permissions || []
      
      if (value && value.length > 0) {
        const hasPermissions = hasPermission(value, userPermissions)
        
        if (!hasPermissions) {
          el.style.display = 'none'
        } else {
          el.style.display = ''
        }
      }
    }
  })
}

/**
 * 菜单权限过滤
 * @param {Array} routes - 路由配置
 * @param {string[]} permissions - 用户权限列表
 * @returns {Array}
 */
export function filterRoutesByPermission(routes, permissions) {
  return routes.filter(route => {
    // 如果路由没有权限要求，则显示
    if (!route.meta || !route.meta.permissions) {
      return true
    }
    
    // 检查权限
    const hasPermissions = hasAnyPermission(route.meta.permissions, permissions)
    
    // 如果有子路由，递归过滤
    if (hasPermissions && route.children && route.children.length > 0) {
      route.children = filterRoutesByPermission(route.children, permissions)
    }
    
    return hasPermissions
  })
}

/**
 * 按钮权限控制
 * @param {string|string[]} permission - 权限标识
 * @returns {boolean}
 */
export function checkButtonPermission(permission) {
  const userStore = useUserStore()
  const userPermissions = userStore.permissions || []
  return hasPermission(permission, userPermissions)
}

// 导出给模板使用
export default {
  PERMISSIONS,
  ROLES,
  ROLE_NAMES,
  ROLE_DESCRIPTIONS,
  hasPermission,
  hasAnyPermission,
  getPermissionsByRole,
  getUserPermissions,
  filterRoutesByPermission,
  checkButtonPermission
}