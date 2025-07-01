import { createRouter, createWebHistory } from 'vue-router'

const routes = [
  {
    path: '/',
    name: 'Home',
    component: () => import('../views/Home.vue')
  },
  {
    path: '/services',
    name: 'Services',
    component: () => import('../views/Services.vue')
  },
  {
    path: '/devices',
    name: 'Devices',
    component: () => import('../views/Devices.vue')
  },
  {
    path: '/alarms',
    name: 'Alarms',
    component: () => import('../views/Alarms.vue')
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
  // Redirect old configuration page routes to system page
  {
    path: '/config/:service',
    redirect: to => {
      return { path: '/system', query: { service: to.params.service } }
    }
  },
  // Compatible with old routes
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