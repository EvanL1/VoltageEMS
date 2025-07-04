import { createRouter, createWebHistory } from 'vue-router'
import { useUserStore } from '@/stores/user'
import { ElMessage } from 'element-plus'
import MainLayout from '@/layouts/MainLayout.vue'

// 公共路由
const publicRoutes = [
  {
    path: '/login',
    name: 'Login',
    component: () => import('@/views/Login.vue'),
    meta: { title: 'common.login', public: true }
  },
  {
    path: '/403',
    name: 'Forbidden',
    component: () => import('@/views/error/403.vue'),
    meta: { title: 'error.forbidden', public: true }
  }
]

// 需要登录的路由
const authRoutes = [
  {
    path: '/',
    component: MainLayout,
    redirect: '/dashboard',
    children: [
      // 监控展示类 - 所有用户可访问
      {
        path: 'dashboard',
        name: 'Dashboard',
        component: () => import('@/views/monitoring/Dashboard.vue'),
        meta: { 
          title: 'menu.dashboard',
          icon: 'DataAnalysis',
          roles: ['operator', 'engineer', 'admin']
        }
      },
      {
        path: 'monitoring/realtime',
        name: 'RealtimeMonitoring',
        component: () => import('@/views/monitoring/Realtime.vue'),
        meta: { 
          title: 'menu.realtime',
          icon: 'Timer',
          roles: ['operator', 'engineer', 'admin']
        }
      },
      {
        path: 'monitoring/devices',
        name: 'DeviceStatus',
        component: () => import('@/views/monitoring/DeviceStatus.vue'),
        meta: { 
          title: 'menu.devices',
          icon: 'Connection',
          roles: ['operator', 'engineer', 'admin']
        }
      },
      {
        path: 'monitoring/energy',
        name: 'EnergyStats',
        component: () => import('@/views/monitoring/EnergyStats.vue'),
        meta: { 
          title: 'menu.energy',
          icon: 'DataLine',
          roles: ['operator', 'engineer', 'admin']
        }
      },
      {
        path: 'monitoring/alarms',
        name: 'AlarmOverview',
        component: () => import('@/views/monitoring/AlarmOverview.vue'),
        meta: { 
          title: 'menu.alarms',
          icon: 'WarnTriangleFilled',
          roles: ['operator', 'engineer', 'admin']
        }
      },
      {
        path: 'monitoring/topology',
        name: 'SystemTopology',
        component: () => import('@/views/monitoring/Topology.vue'),
        meta: { 
          title: 'menu.topology',
          icon: 'Share',
          roles: ['operator', 'engineer', 'admin']
        }
      },

      // 控制操作类 - 工程师和管理员
      {
        path: 'control/devices',
        name: 'DeviceControl',
        component: () => import('@/views/control/DeviceControl.vue'),
        meta: { 
          title: 'menu.deviceControl',
          icon: 'SwitchButton',
          roles: ['engineer', 'admin']
        }
      },
      {
        path: 'control/alarms',
        name: 'AlarmManagement',
        component: () => import('@/views/control/AlarmManagement.vue'),
        meta: { 
          title: 'menu.alarmManagement',
          icon: 'Bell',
          roles: ['engineer', 'admin']
        }
      },
      {
        path: 'control/batch',
        name: 'BatchControl',
        component: () => import('@/views/control/BatchControl.vue'),
        meta: { 
          title: 'menu.batchControl',
          icon: 'CopyDocument',
          roles: ['engineer', 'admin']
        }
      },
      {
        path: 'control/schedule',
        name: 'ScheduledTasks',
        component: () => import('@/views/control/ScheduledTasks.vue'),
        meta: { 
          title: 'menu.scheduledTasks',
          icon: 'Calendar',
          roles: ['engineer', 'admin']
        }
      },

      // 配置管理类 - 仅管理员
      {
        path: 'config/channels',
        name: 'ChannelConfig',
        component: () => import('@/views/config/ChannelConfig.vue'),
        meta: { 
          title: 'menu.channelConfig',
          icon: 'Link',
          roles: ['admin']
        }
      },
      {
        path: 'config/points',
        name: 'PointTable',
        component: () => import('@/views/config/PointTable.vue'),
        meta: { 
          title: 'menu.pointTable',
          icon: 'Grid',
          roles: ['admin']
        }
      },
      {
        path: 'config/models',
        name: 'ModelConfig',
        component: () => import('@/views/config/ModelConfig.vue'),
        meta: { 
          title: 'menu.modelConfig',
          icon: 'DataBoard',
          roles: ['admin']
        }
      },
      {
        path: 'config/alarms',
        name: 'AlarmRules',
        component: () => import('@/views/config/AlarmRules.vue'),
        meta: { 
          title: 'menu.alarmRules',
          icon: 'Warning',
          roles: ['admin']
        }
      },
      {
        path: 'config/storage',
        name: 'StoragePolicy',
        component: () => import('@/views/config/StoragePolicy.vue'),
        meta: { 
          title: 'menu.storagePolicy',
          icon: 'Files',
          roles: ['admin']
        }
      },
      {
        path: 'config/network',
        name: 'NetworkForward',
        component: () => import('@/views/config/NetworkForward.vue'),
        meta: { 
          title: 'menu.networkForward',
          icon: 'Upload',
          roles: ['admin']
        }
      },

      // 系统管理类
      {
        path: 'system/users',
        name: 'UserManagement',
        component: () => import('@/views/system/UserManagement.vue'),
        meta: { 
          title: 'menu.userManagement',
          icon: 'User',
          roles: ['admin']
        }
      },
      {
        path: 'system/settings',
        name: 'SystemSettings',
        component: () => import('@/views/system/SystemSettings.vue'),
        meta: { 
          title: 'menu.systemSettings',
          icon: 'SetUp',
          roles: ['admin']
        }
      },
      {
        path: 'system/audit',
        name: 'AuditLogs',
        component: () => import('@/views/system/AuditLogs.vue'),
        meta: { 
          title: 'menu.auditLogs',
          icon: 'Document',
          roles: ['engineer', 'admin']
        }
      },
      {
        path: 'system/services',
        name: 'ServiceMonitor',
        component: () => import('@/views/system/ServiceMonitor.vue'),
        meta: { 
          title: 'menu.serviceMonitor',
          icon: 'Cpu',
          roles: ['engineer', 'admin']
        }
      },

      // 用户个人页面
      {
        path: 'profile',
        name: 'Profile',
        component: () => import('@/views/Profile.vue'),
        meta: { 
          title: 'common.profile',
          hidden: true
        }
      },
      {
        path: 'settings',
        name: 'Settings',
        component: () => import('@/views/Settings.vue'),
        meta: { 
          title: 'common.settings',
          hidden: true
        }
      }
    ]
  },

  // 兼容旧路由
  {
    path: '/services',
    redirect: '/system/services'
  },
  {
    path: '/devices',
    redirect: '/monitoring/devices'
  },
  {
    path: '/alarms',
    redirect: '/monitoring/alarms'
  },
  {
    path: '/system',
    redirect: '/system/settings'
  }
]

const router = createRouter({
  history: createWebHistory(process.env.BASE_URL),
  routes: [...publicRoutes, ...authRoutes]
})

// 路由守卫
router.beforeEach(async (to, from, next) => {
  const userStore = useUserStore()
  
  // 公开路由直接放行
  if (to.meta.public) {
    next()
    return
  }

  // 检查是否登录
  if (!userStore.isLoggedIn) {
    next('/login')
    return
  }

  // 如果没有用户信息，尝试获取
  if (!userStore.userInfo) {
    try {
      await userStore.fetchUserInfo()
    } catch (error) {
      ElMessage.error('获取用户信息失败')
      next('/login')
      return
    }
  }

  // 检查权限
  const requiredRoles = to.meta.roles
  if (requiredRoles && requiredRoles.length > 0) {
    const hasRole = requiredRoles.includes(userStore.role)
    if (!hasRole) {
      ElMessage.error('您没有权限访问该页面')
      next('/403')
      return
    }
  }

  next()
})

export default router