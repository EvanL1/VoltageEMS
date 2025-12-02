import { router } from './index'
import { useUserStore } from '@/stores/user'
import { ensureRoutesInjected } from './injector'
import { cancelAllPendingRequests } from '@/utils/request'

const WHITE_LIST = ['/login']

router.beforeEach(async (to, from, next) => {
  // 取消所有pending的请求
  cancelAllPendingRequests()

  const user = useUserStore()

  // 未登录：只允许进入白名单
  // if (!user.token && !user.userInfo) {
  //   if (WHITE_LIST.includes(to.path) || to.meta.public) return next()
  //   return next({ path: '/login', query: { redirect: to.path } })
  // }

  // 已登录：确保已注入动态路由
  try {
    if (!user.routesInjected) {
      await ensureRoutesInjected()
      return next({ ...to, replace: true }) // 路由重定向
    } else {
      next()
    }
  } catch (e) {
    // 注入失败，保险起见登出
    user.clearUserData()
    next('/login')
  }
})
