import { defineStore } from 'pinia'
import { login, getUserInfo, logout as logoutApi } from '@/api/auth'

export const useUserStore = defineStore('user', {
  state: () => ({
    token: localStorage.getItem('token') || null,
    userInfo: null,
    role: null, // 'operator' | 'engineer' | 'admin'
    permissions: []
  }),

  getters: {
    isLoggedIn: (state) => !!state.token,
    isOperator: (state) => state.role === 'operator',
    isEngineer: (state) => state.role === 'engineer',
    isAdmin: (state) => state.role === 'admin',
    canControl: (state) => ['engineer', 'admin'].includes(state.role),
    canConfig: (state) => state.role === 'admin',
    hasPermission: (state) => (permission) => state.permissions.includes(permission)
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
      const rolePermissions = {
        operator: [
          'monitoring.view',
          'dashboard.view',
          'realtime.view',
          'devices.view',
          'energy.view',
          'alarms.view',
          'topology.view'
        ],
        engineer: [
          // 继承operator权限
          'monitoring.view',
          'dashboard.view',
          'realtime.view',
          'realtime.control',
          'devices.view',
          'devices.control',
          'energy.view',
          'alarms.view',
          'alarms.handle',
          'topology.view',
          // 工程师特有权限
          'control.access',
          'batch.execute',
          'schedule.manage',
          'audit.view.self',
          'services.view'
        ],
        admin: [
          // 所有权限
          '*'
        ]
      }

      if (this.role === 'admin') {
        this.permissions = ['*']
      } else {
        this.permissions = rolePermissions[this.role] || []
      }
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
      if (this.permissions.includes('*')) return true
      return this.permissions.includes(permission)
    }
  }
})