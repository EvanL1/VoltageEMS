import { createRouter, createWebHistory } from 'vue-router'

const routes = [
  {
    path: '/',
    name: 'Home',
    component: () => import('../views/Home.vue')
  },
  {
    path: '/system',
    name: 'System',
    component: () => import('../views/System.vue')
  },
  {
    path: '/activity',
    name: 'Activity',
    component: () => import('../views/Activity.vue')
  },
  // 重定向旧的配置页面路由到系统页面
  {
    path: '/config/:service',
    redirect: to => {
      return { path: '/system', query: { service: to.params.service } }
    }
  },
  // 兼容旧路由
  {
    path: '/dashboard',
    redirect: '/'
  }
]

const router = createRouter({
  history: createWebHistory(process.env.BASE_URL),
  routes
})

export default router 