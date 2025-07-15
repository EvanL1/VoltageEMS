import { createRouter, createWebHistory } from 'vue-router'
import type { RouteRecordRaw } from 'vue-router'
import MainLayout from '@/layouts/MainLayout.vue'

const routes: RouteRecordRaw[] = [
  {
    path: '/',
    component: MainLayout,
    redirect: '/dashboard',
    children: [
      {
        path: 'dashboard',
        name: 'Dashboard',
        component: () => import('@/views/Dashboard/index.vue'),
        meta: { title: 'Dashboard', icon: 'Odometer' }
      },
      {
        path: 'monitor',
        name: 'Monitor',
        redirect: '/monitor/realtime',
        meta: { title: 'Monitoring', icon: 'Monitor' },
        children: [
          {
            path: 'realtime',
            name: 'RealtimeMonitor',
            component: () => import('@/views/Monitor/RealtimeMonitor.vue'),
            meta: { title: 'Realtime Data' }
          },
          {
            path: 'history',
            name: 'HistoryQuery',
            component: () => import('@/views/History/DataQuery.vue'),
            meta: { title: 'Historical Data' }
          }
        ]
      },
      {
        path: 'control',
        name: 'Control',
        component: () => import('@/views/Control/DeviceControl.vue'),
        meta: { title: 'Device Control', icon: 'Operation' }
      },
      {
        path: 'config',
        name: 'Config',
        redirect: '/config/channel',
        meta: { title: 'Configuration', icon: 'Setting' },
        children: [
          {
            path: 'channel',
            name: 'ChannelConfig',
            component: () => import('@/views/Config/ChannelConfig.vue'),
            meta: { title: 'Channels' }
          },
          {
            path: 'points',
            name: 'PointTable',
            component: () => import('@/views/Config/PointTable.vue'),
            meta: { title: 'Point Table' }
          }
        ]
      },
      {
        path: 'rules',
        name: 'Rules',
        component: () => import('@/views/Rules/RuleEditor.vue'),
        meta: { title: 'Rule Engine', icon: 'SetUp' }
      },
      {
        path: 'alarm',
        name: 'Alarm',
        component: () => import('@/views/Alarm/AlarmCenter.vue'),
        meta: { title: 'Alarm Center', icon: 'Warning' }
      },
      {
        path: 'system',
        name: 'System',
        redirect: '/system/service',
        meta: { title: 'System', icon: 'Tools' },
        children: [
          {
            path: 'service',
            name: 'ServiceStatus',
            component: () => import('@/views/System/ServiceStatus.vue'),
            meta: { title: 'Service Status' }
          },
          {
            path: 'logs',
            name: 'SystemLog',
            component: () => import('@/views/System/SystemLog.vue'),
            meta: { title: 'System Logs' }
          }
        ]
      },
      {
        path: 'user',
        name: 'User',
        component: () => import('@/views/User/UserManagement.vue'),
        meta: { title: 'User Management', icon: 'User' }
      }
    ]
  },
  {
    path: '/login',
    name: 'Login',
    component: () => import('@/views/Login/index.vue'),
    meta: { title: 'Login' }
  },
  {
    path: '/:pathMatch(.*)*',
    name: 'NotFound',
    component: () => import('@/views/Error/404.vue'),
    meta: { title: 'Not Found' }
  }
]

const router = createRouter({
  history: createWebHistory(),
  routes
})

// Navigation guards
router.beforeEach((to, from, next) => {
  // Update document title
  if (to.meta.title) {
    document.title = `${to.meta.title} - VoltageEMS`
  }
  
  // Check authentication (simplified for demo)
  const token = localStorage.getItem('token')
  if (to.path !== '/login' && !token) {
    next('/login')
  } else {
    next()
  }
})

export default router