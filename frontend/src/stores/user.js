import { defineStore } from 'pinia'
import { login, getUserInfo, logout as logoutApi } from '@/api/auth'
import { getPermissionsByRole, ROLES } from '@/utils/permission'

export const useUserStore = defineStore('user', {
  state: () => ({
    token: localStorage.getItem('token') || null,
    userInfo: null,
    role: null, // 使用 ROLES 常量定义的角色
    permissions: []
  }),

  getters: {
    isLoggedIn: (state) => !!state.token,
    isOperator: (state) => state.role === ROLES.OPS_ENGINEER,
    isEngineer: (state) => state.role === ROLES.OPS_ENGINEER,
    isAdmin: (state) => [ROLES.SUPER_ADMIN, ROLES.SYSTEM_ADMIN].includes(state.role),
    isSuperAdmin: (state) => state.role === ROLES.SUPER_ADMIN,
    canControl: (state) => [ROLES.OPS_ENGINEER, ROLES.SYSTEM_ADMIN, ROLES.SUPER_ADMIN].includes(state.role),
    canConfig: (state) => [ROLES.SYSTEM_ADMIN, ROLES.SUPER_ADMIN].includes(state.role),
    hasPermission: (state) => (permission) => {
      if (state.role === ROLES.SUPER_ADMIN) return true
      return state.permissions.includes(permission)
    }
  },

  actions: {
    async login(credentials) {
      try {
        const { data } = await login(credentials)
        this.token = data.token
        localStorage.setItem('token', data.token)
        await this.fetchUserInfo()
        return { success: true }
      } catch (error) {
        this.reset()
        return { success: false, error: error.message }
      }
    },

    async fetchUserInfo() {
      if (!this.token) return
      
      try {
        const { data } = await getUserInfo()
        this.userInfo = data.userInfo
        this.role = data.userInfo.role
        this.permissions = data.permissions || []
        
        // 根据角色生成权限
        this.generatePermissionsByRole()
      } catch (error) {
        this.reset()
        throw error
      }
    },

    generatePermissionsByRole() {
      // 使用权限系统定义的角色权限映射
      this.permissions = getPermissionsByRole(this.role)
    },

    async logout() {
      try {
        await logoutApi()
      } catch (error) {
        console.error('Logout API error:', error)
      } finally {
        this.reset()
      }
    },

    reset() {
      this.token = null
      this.userInfo = null
      this.role = null
      this.permissions = []
      localStorage.removeItem('token')
    },

    checkPermission(permission) {
      if (this.role === ROLES.SUPER_ADMIN) return true
      return this.permissions.includes(permission)
    }
  }
})