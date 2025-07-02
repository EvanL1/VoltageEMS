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
  {
    path: '/history',
    name: 'HistoryAnalysis',
    component: () => import('../views/HistoryAnalysisDemo.vue')
  },
  {
    path: '/grafana-test',
    name: 'GrafanaTest',
    component: () => import('../views/GrafanaTest.vue')
  },
  {
    path: '/grafana-live',
    name: 'GrafanaLive',
    component: () => import('../views/GrafanaLive.vue')
  },
  {
    path: '/grafana-realtime',
    name: 'GrafanaRealtime',
    component: () => import('../views/GrafanaRealtime.vue')
  },
  {
    path: '/simple',
    name: 'SimpleMonitor',
    component: () => import('../views/SimpleMonitor.vue')
  },
  {
    path: '/ultra',
    name: 'UltraSimple',
    component: () => import('../views/UltraSimple.vue')
  },
  {
    path: '/grafana-embedded',
    name: 'GrafanaEmbedded',
    component: () => import('../views/GrafanaEmbedded.vue')
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