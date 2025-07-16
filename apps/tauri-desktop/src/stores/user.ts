import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export const useUserStore = defineStore('user', () => {
  // State
  const token = ref(localStorage.getItem('token') || '')
  const username = ref(localStorage.getItem('username') || '')
  const role = ref('admin')
  const permissions = ref<string[]>([])
  
  // Getters
  const isAuthenticated = computed(() => !!token.value)
  
  // Actions
  function setToken(newToken: string) {
    token.value = newToken
    localStorage.setItem('token', newToken)
  }
  
  function setUser(user: { username: string; role?: string; permissions?: string[] }) {
    username.value = user.username
    localStorage.setItem('username', user.username)
    
    if (user.role) {
      role.value = user.role
    }
    
    if (user.permissions) {
      permissions.value = user.permissions
    }
  }
  
  function logout() {
    token.value = ''
    username.value = ''
    role.value = ''
    permissions.value = []
    
    localStorage.removeItem('token')
    localStorage.removeItem('username')
  }
  
  function hasPermission(permission: string): boolean {
    return role.value === 'admin' || permissions.value.includes(permission)
  }
  
  return {
    // State
    token,
    username,
    role,
    permissions,
    
    // Getters
    isAuthenticated,
    
    // Actions
    setToken,
    setUser,
    logout,
    hasPermission
  }
})