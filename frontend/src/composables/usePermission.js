/**
 * 权限管理组合式函数
 */
import { computed } from 'vue'
import { useUserStore } from '@/stores/user'
import { 
  PERMISSIONS, 
  ROLES, 
  ROLE_NAMES,
  hasPermission, 
  hasAnyPermission,
  checkButtonPermission 
} from '@/utils/permission'

export function usePermission() {
  const userStore = useUserStore()
  
  // 当前用户权限列表
  const userPermissions = computed(() => userStore.permissions || [])
  
  // 当前用户角色
  const userRole = computed(() => userStore.role)
  
  // 当前用户角色名称
  const userRoleName = computed(() => ROLE_NAMES[userStore.role] || '未知角色')
  
  // 是否是超级管理员
  const isSuperAdmin = computed(() => userStore.role === ROLES.SUPER_ADMIN)
  
  // 是否是系统管理员（包括超级管理员）
  const isSystemAdmin = computed(() => 
    [ROLES.SUPER_ADMIN, ROLES.SYSTEM_ADMIN].includes(userStore.role)
  )
  
  // 是否是运维人员（包括更高权限）
  const isOperator = computed(() => 
    [ROLES.SUPER_ADMIN, ROLES.SYSTEM_ADMIN, ROLES.OPS_ENGINEER].includes(userStore.role)
  )
  
  /**
   * 检查单个权限
   * @param {string} permission - 权限标识
   * @returns {boolean}
   */
  const checkPermission = (permission) => {
    return hasPermission(permission, userPermissions.value)
  }
  
  /**
   * 检查多个权限（AND逻辑）
   * @param {string[]} permissions - 权限数组
   * @returns {boolean}
   */
  const checkPermissions = (permissions) => {
    return hasPermission(permissions, userPermissions.value)
  }
  
  /**
   * 检查任一权限（OR逻辑）
   * @param {string[]} permissions - 权限数组
   * @returns {boolean}
   */
  const checkAnyPermission = (permissions) => {
    return hasAnyPermission(permissions, userPermissions.value)
  }
  
  /**
   * 检查按钮权限
   * @param {string|string[]} permission - 权限标识
   * @returns {boolean}
   */
  const canClick = (permission) => {
    return checkButtonPermission(permission)
  }
  
  // 功能权限快捷检查
  const can = {
    // 系统管理
    viewUsers: computed(() => checkPermission(PERMISSIONS.SYSTEM.USER_VIEW)),
    createUser: computed(() => checkPermission(PERMISSIONS.SYSTEM.USER_CREATE)),
    editUser: computed(() => checkPermission(PERMISSIONS.SYSTEM.USER_EDIT)),
    deleteUser: computed(() => checkPermission(PERMISSIONS.SYSTEM.USER_DELETE)),
    manageRoles: computed(() => checkPermission(PERMISSIONS.SYSTEM.ROLE_MANAGE)),
    editSettings: computed(() => checkPermission(PERMISSIONS.SYSTEM.SETTINGS_EDIT)),
    viewAuditLogs: computed(() => checkPermission(PERMISSIONS.SYSTEM.AUDIT_VIEW)),
    exportAuditLogs: computed(() => checkPermission(PERMISSIONS.SYSTEM.AUDIT_EXPORT)),
    controlServices: computed(() => checkPermission(PERMISSIONS.SYSTEM.SERVICE_CONTROL)),
    
    // 配置管理
    manageChannels: computed(() => checkPermission(PERMISSIONS.CONFIG.CHANNEL_CREATE)),
    editChannels: computed(() => checkPermission(PERMISSIONS.CONFIG.CHANNEL_EDIT)),
    deleteChannels: computed(() => checkPermission(PERMISSIONS.CONFIG.CHANNEL_DELETE)),
    importPoints: computed(() => checkPermission(PERMISSIONS.CONFIG.POINT_IMPORT)),
    editPoints: computed(() => checkPermission(PERMISSIONS.CONFIG.POINT_EDIT)),
    manageModels: computed(() => checkPermission(PERMISSIONS.CONFIG.MODEL_CREATE)),
    manageAlarmRules: computed(() => checkPermission(PERMISSIONS.CONFIG.ALARM_CREATE)),
    
    // 监控功能
    viewRealtime: computed(() => checkPermission(PERMISSIONS.MONITOR.REALTIME_VIEW)),
    exportHistory: computed(() => checkPermission(PERMISSIONS.MONITOR.HISTORY_EXPORT)),
    editTopology: computed(() => checkPermission(PERMISSIONS.MONITOR.TOPOLOGY_EDIT)),
    exportStats: computed(() => checkPermission(PERMISSIONS.MONITOR.STATS_EXPORT)),
    
    // 控制功能
    controlDevices: computed(() => checkPermission(PERMISSIONS.CONTROL.DEVICE_CONTROL)),
    batchControl: computed(() => checkPermission(PERMISSIONS.CONTROL.BATCH_CONTROL)),
    approveBatch: computed(() => checkPermission(PERMISSIONS.CONTROL.BATCH_APPROVE)),
    manageTasks: computed(() => checkPermission(PERMISSIONS.CONTROL.TASK_CREATE)),
    executeTasks: computed(() => checkPermission(PERMISSIONS.CONTROL.TASK_EXECUTE)),
    handleAlarms: computed(() => checkPermission(PERMISSIONS.CONTROL.ALARM_HANDLE)),
    deleteAlarms: computed(() => checkPermission(PERMISSIONS.CONTROL.ALARM_DELETE))
  }
  
  return {
    // 权限常量
    PERMISSIONS,
    ROLES,
    ROLE_NAMES,
    
    // 用户信息
    userPermissions,
    userRole,
    userRoleName,
    
    // 角色判断
    isSuperAdmin,
    isSystemAdmin,
    isOperator,
    
    // 权限检查方法
    checkPermission,
    checkPermissions,
    checkAnyPermission,
    canClick,
    
    // 功能权限
    can
  }
}

// 导出权限常量，方便直接引用
export { PERMISSIONS, ROLES, ROLE_NAMES } from '@/utils/permission'