import { defineStore } from 'pinia'
import type { UserInfo, LoginParams } from '@/types/user'
import { mockUserApi } from '@/api/user'
import { useGlobalStore } from '@/stores/global'
// 用户状态管理
export const useUserStore = defineStore(
  'user',
  () => {
    // 状态
    const token = ref<string>('')
    const userInfo = ref<UserInfo | null>(null)

    // 计算属性
    const isLoggedIn = computed(() => !!token.value && !!userInfo.value)
    const isAdmin = computed(() => userInfo.value?.role === 'admin')
    const displayName = computed(() => userInfo.value?.nickname || userInfo.value?.username || '')
    const avatar = computed(() => userInfo.value?.avatar || '')

    // 检查Token有效性
    const checkTokenValidity = (): boolean => {
      if (!token.value) return false

      try {
        // 解析JWT Token（这里假设Token包含过期时间）
        const payload = JSON.parse(atob(token.value.split('.')[1]))
        const exp = payload.exp * 1000 // 转换为毫秒
        const now = Date.now()

        return exp > now
      } catch (error) {
        console.error('Token解析失败:', error)
        return false
      }
    }

    // 用户登录
    const login = async (params: LoginParams) => {
      try {
        const response = await mockUserApi.login(params)

        if (response.success) {
          const { token: newToken, user } = response.data

          // 更新状态（会自动持久化）
          token.value = newToken
          userInfo.value = user

          return { success: true, message: response.message }
        } else {
          return { success: false, message: response.message }
        }
      } catch (error: any) {
        return { success: false, message: error.message || '登录失败' }
      }
    }

    // 用户登出
    const logout = async () => {
      try {
        // 调用登出API
        if (token.value) {
          await mockUserApi.logout()
        }
      } catch (error) {
        console.error('登出API调用失败:', error)
      } finally {
        // 清除状态（会自动清除持久化数据）
        token.value = ''
        userInfo.value = null
      }
    }

    // 获取用户信息
    const getUserInfo = async () => {
      try {
        const response = await mockUserApi.getUserInfo()

        if (response.success) {
          userInfo.value = response.data
          return { success: true, message: response.message }
        } else {
          return { success: false, message: response.message }
        }
      } catch (error: any) {
        return { success: false, message: error.message || '获取用户信息失败' }
      }
    }

    // 刷新用户Token
    const refreshUserToken = async () => {
      try {
        // 这里应该调用刷新Token的API
        // 暂时返回成功，实际项目中需要实现真实的刷新逻辑
        return { success: true, message: 'Token刷新成功' }
      } catch (error: any) {
        return { success: false, message: error.message || 'Token刷新失败' }
      }
    }

    // 清除用户数据（手动清除持久化数据）
    const clearUserData = () => {
      token.value = ''
      userInfo.value = null
    }

    // 清除存储（别名方法）
    const clearStorage = () => {
      clearUserData()
    }

    // 获取用户信息（别名方法）
    const fetchUserInfo = () => {
      return getUserInfo()
    }
    // 监听 userInfo 的 role 字段变化，发生变化时调用 getRoutesByRole 方法
    watch(
      () => userInfo.value?.role,
      (newRole, oldRole) => {
        if (newRole && newRole !== oldRole) {
          const globalStore = useGlobalStore()
          globalStore.getRoutesByRole(newRole)
        }
      },
    )
    return {
      // 状态
      token,
      userInfo,

      // 计算属性
      isLoggedIn,
      isAdmin,
      displayName,
      avatar,

      // 方法
      login,
      logout,
      getUserInfo,
      clearUserData,
      refreshUserToken,
      clearStorage,
      fetchUserInfo,
      checkTokenValidity,
    }
  },
  {
    persist: true,
  },
)
