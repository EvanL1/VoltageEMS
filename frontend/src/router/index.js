import { createRouter, createWebHistory } from 'vue-router'

const routes = [
  {
    path: '/',
    name: 'Home',
    component: () => import('../views/Home.vue')
  },
  {
    path: '/config/modsrv',
    name: 'ModsrvConfig',
    component: () => import('../views/config/ModsrvConfig.vue')
  },
  {
    path: '/config/netsrv',
    name: 'NetsrvConfig',
    component: () => import('../views/config/NetsrvConfig.vue')
  },
  {
    path: '/config/comsrv',
    name: 'ComsrvConfig',
    component: () => import('../views/config/ComsrvConfig.vue')
  },
  {
    path: '/config/hissrv',
    name: 'HissrvConfig',
    component: () => import('../views/config/HissrvConfig.vue')
  },
  {
    path: '/config/mosquitto',
    name: 'MosquittoConfig',
    component: () => import('../views/config/MosquittoConfig.vue')
  },
  {
    path: '/dashboard',
    name: 'Dashboard',
    component: () => import('../views/Dashboard.vue')
  }
]

const router = createRouter({
  history: createWebHistory(process.env.BASE_URL),
  routes
})

export default router 