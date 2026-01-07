import { router } from './index'
import { useUserStore } from '@/stores/user'
import { ensureRoutesInjected } from './injector'
import { cancelAllPendingRequests } from '@/utils/request'

const WHITE_LIST = ['/login']

router.beforeEach(async (to, from, next) => {
  // 取消所有pending的请求
  cancelAllPendingRequests()

  const user = useUserStore()

  // 如果是登录页，直接放行
  if (WHITE_LIST.includes(to.path)) {
    return next()
  }

  // 如果不是登录页，需要先获取用户信息
  try {
    // 如果没有 token，跳转到登录页
    if (!user.token || !user.userInfo) {
      // 如果有 refreshToken，尝试刷新 token（可选，根据实际需求）
      // 这里直接跳转到登录页
      if (!user.refreshToken) {
        return next({ path: '/login' })
      } else {
        const result = await user.refreshUserToken()
        if (result.success) {
          const res = await user.getUserInfo()
          if (!res.success) {
            user.clearUserData()
            return next({ path: '/login' })
          }
        } else {
          user.clearUserData()
          return next({ path: '/login' })
        }
      }
    }
    // 确保已注入动态路由
    if (!user.routesInjected) {
      await ensureRoutesInjected()
      return next({ ...to, replace: true }) // 路由重定向
    } else {
      next()
    }
  } catch (e) {
    // 发生错误，清除数据并跳转到登录页
    console.error('Route guard error:', e)
    user.clearUserData()
    next({ path: '/login' })
  }
})
