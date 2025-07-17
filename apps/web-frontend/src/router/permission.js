/**
 * 路由权限守卫
 */
import { ElMessage } from 'element-plus'
import { useUserStore } from '@/stores/user'
import { hasAnyPermission } from '@/utils/permission'

/**
 * 路由权限守卫
 * @param {Object} router - Vue Router 实例
 */
export function setupPermissionGuard(router) {
  router.beforeEach(async (to, from, next) => {
    const userStore = useUserStore()
    
    // 白名单路由，不需要权限
    const whiteList = ['/login', '/404', '/403']
    if (whiteList.includes(to.path)) {
      next()
      return
    }
    
    // 检查是否登录
    if (!userStore.token) {
      next(`/login?redirect=${to.path}`)
      return
    }
    
    // 如果用户信息不完整，尝试获取
    if (!userStore.permissions || userStore.permissions.length === 0) {
      try {
        await userStore.getUserInfo()
      } catch (error) {
        // 获取用户信息失败，清除token并跳转到登录页
        userStore.logout()
        ElMessage.error('获取用户信息失败，请重新登录')
        next(`/login?redirect=${to.path}`)
        return
      }
    }
    
    // 检查路由权限
    if (to.meta && to.meta.permissions) {
      const hasPermission = hasAnyPermission(to.meta.permissions, userStore.permissions)
      
      if (!hasPermission) {
        // 没有权限，跳转到403页面
        ElMessage.error('您没有访问该页面的权限')
        next('/403')
        return
      }
    }
    
    // 有权限，允许访问
    next()
  })
  
  // 路由后置守卫
  router.afterEach((to) => {
    // 设置页面标题
    const title = to.meta.title || 'Monarch Hub'
    document.title = `${title} - Monarch Hub`
  })
}

/**
 * 根据用户权限动态生成路由
 * @param {Array} routes - 原始路由配置
 * @param {Array} permissions - 用户权限列表
 * @returns {Array} 过滤后的路由
 */
export function filterAsyncRoutes(routes, permissions) {
  const res = []
  
  routes.forEach(route => {
    const tmp = { ...route }
    
    // 如果路由需要权限
    if (tmp.meta && tmp.meta.permissions) {
      // 检查是否有权限
      if (hasAnyPermission(tmp.meta.permissions, permissions)) {
        // 如果有子路由，递归过滤
        if (tmp.children) {
          tmp.children = filterAsyncRoutes(tmp.children, permissions)
        }
        res.push(tmp)
      }
    } else {
      // 不需要权限的路由
      if (tmp.children) {
        tmp.children = filterAsyncRoutes(tmp.children, permissions)
      }
      res.push(tmp)
    }
  })
  
  return res
}

/**
 * 重置路由
 * @param {Object} router - Vue Router 实例
 * @param {Function} createRouterFunc - createRouter 函数
 */
export function resetRouter(router, createRouterFunc) {
  const newRouter = createRouterFunc()
  router.matcher = newRouter.matcher
}

/**
 * 添加动态路由
 * @param {Object} router - Vue Router 实例
 * @param {Array} routes - 要添加的路由
 */
export function addDynamicRoutes(router, routes) {
  routes.forEach(route => {
    router.addRoute(route)
  })
}