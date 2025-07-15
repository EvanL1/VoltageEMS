import { createRouter, createWebHistory } from 'vue-router'
import type { RouteRecordRaw } from 'vue-router'
import DataMonitor from '@/components/DataMonitor.vue'

const routes: RouteRecordRaw[] = [
  {
    path: '/',
    name: 'home',
    redirect: '/monitor'
  },
  {
    path: '/monitor',
    name: 'monitor',
    component: DataMonitor
  }
]

const router = createRouter({
  history: createWebHistory(),
  routes
})

export default router