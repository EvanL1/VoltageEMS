import { useUserStore } from '@/stores/user'

export const permissionDirective = {
  mounted(el, binding) {
    const { value } = binding
    const userStore = useUserStore()
    
    if (value && value instanceof Array && value.length > 0) {
      const requiredRoles = value
      const hasRole = requiredRoles.includes(userStore.role)
      
      if (!hasRole && el.parentNode) {
        el.parentNode.removeChild(el)
      }
    } else if (value && typeof value === 'string') {
      const hasPermission = userStore.checkPermission(value)
      
      if (!hasPermission && el.parentNode) {
        el.parentNode.removeChild(el)
      }
    } else {
      throw new Error('v-permission value must be a role array or permission string')
    }
  }
}

// 检查权限的函数
export function hasPermission(permission) {
  const userStore = useUserStore()
  return userStore.checkPermission(permission)
}

// 检查角色的函数
export function hasRole(roles) {
  const userStore = useUserStore()
  if (typeof roles === 'string') {
    return userStore.role === roles
  }
  if (Array.isArray(roles)) {
    return roles.includes(userStore.role)
  }
  return false
}