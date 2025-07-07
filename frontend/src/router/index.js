import { createRouter, createWebHistory } from 'vue-router'
import MainLayout from '@/layouts/MainLayout.vue'
import { PERMISSIONS } from '@/utils/permission'
import { setupPermissionGuard } from './permission'

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
          permissions: [PERMISSIONS.MONITOR.DASHBOARD_VIEW]
        }
      },
      {
        path: 'monitoring/realtime',
        name: 'RealtimeMonitoring',
        component: () => import('@/views/monitoring/Realtime.vue'),
        meta: { 
          title: 'menu.realtime',
          icon: 'Timer',
          permissions: [PERMISSIONS.MONITOR.REALTIME_VIEW]
        }
      },
      {
        path: 'monitoring/devices',
        name: 'DeviceStatus',
        component: () => import('@/views/monitoring/DeviceStatus.vue'),
        meta: { 
          title: 'menu.devices',
          icon: 'Connection',
          permissions: [PERMISSIONS.MONITOR.DEVICE_VIEW]
        }
      },
      {
        path: 'monitoring/energy',
        name: 'EnergyStats',
        component: () => import('@/views/monitoring/EnergyStats.vue'),
        meta: { 
          title: 'menu.energy',
          icon: 'DataLine',
          permissions: [PERMISSIONS.MONITOR.STATS_VIEW]
        }
      },
      {
        path: 'monitoring/alarms',
        name: 'AlarmOverview',
        component: () => import('@/views/monitoring/AlarmOverview.vue'),
        meta: { 
          title: 'menu.alarms',
          icon: 'WarnTriangleFilled',
          permissions: [PERMISSIONS.CONTROL.ALARM_VIEW]
        }
      },
      {
        path: 'monitoring/topology',
        name: 'SystemTopology',
        component: () => import('@/views/monitoring/Topology.vue'),
        meta: { 
          title: 'menu.topology',
          icon: 'Share',
          permissions: [PERMISSIONS.MONITOR.TOPOLOGY_VIEW]
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
          permissions: [PERMISSIONS.CONTROL.DEVICE_VIEW]
        }
      },
      {
        path: 'control/alarms',
        name: 'AlarmManagement',
        component: () => import('@/views/control/AlarmManagement.vue'),
        meta: { 
          title: 'menu.alarmManagement',
          icon: 'Bell',
          permissions: [PERMISSIONS.CONTROL.ALARM_VIEW]
        }
      },
      {
        path: 'control/batch',
        name: 'BatchControl',
        component: () => import('@/views/control/BatchControl.vue'),
        meta: { 
          title: 'menu.batchControl',
          icon: 'CopyDocument',
          permissions: [PERMISSIONS.CONTROL.BATCH_VIEW]
        }
      },
      {
        path: 'control/schedule',
        name: 'ScheduledTasks',
        component: () => import('@/views/control/ScheduledTasks.vue'),
        meta: { 
          title: 'menu.scheduledTasks',
          icon: 'Calendar',
          permissions: [PERMISSIONS.CONTROL.TASK_VIEW]
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
          permissions: [PERMISSIONS.CONFIG.CHANNEL_VIEW]
        }
      },
      {
        path: 'config/points',
        name: 'PointTable',
        component: () => import('@/views/config/PointTable.vue'),
        meta: { 
          title: 'menu.pointTable',
          icon: 'Grid',
          permissions: [PERMISSIONS.CONFIG.POINT_VIEW]
        }
      },
      {
        path: 'config/models',
        name: 'ModelConfig',
        component: () => import('@/views/config/ModelConfig.vue'),
        meta: { 
          title: 'menu.modelConfig',
          icon: 'DataBoard',
          permissions: [PERMISSIONS.CONFIG.MODEL_VIEW]
        }
      },
      {
        path: 'config/alarms',
        name: 'AlarmRules',
        component: () => import('@/views/config/AlarmRules.vue'),
        meta: { 
          title: 'menu.alarmRules',
          icon: 'Warning',
          permissions: [PERMISSIONS.CONFIG.ALARM_VIEW]
        }
      },
      {
        path: 'config/storage',
        name: 'StoragePolicy',
        component: () => import('@/views/config/StoragePolicy.vue'),
        meta: { 
          title: 'menu.storagePolicy',
          icon: 'Files',
          permissions: [PERMISSIONS.CONFIG.STORAGE_VIEW]
        }
      },
      {
        path: 'config/network',
        name: 'NetworkForward',
        component: () => import('@/views/config/NetworkForward.vue'),
        meta: { 
          title: 'menu.networkForward',
          icon: 'Upload',
          permissions: [PERMISSIONS.CONFIG.NETWORK_VIEW]
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
          permissions: [PERMISSIONS.SYSTEM.USER_VIEW]
        }
      },
      {
        path: 'system/settings',
        name: 'SystemSettings',
        component: () => import('@/views/system/SystemSettings.vue'),
        meta: { 
          title: 'menu.systemSettings',
          icon: 'SetUp',
          permissions: [PERMISSIONS.SYSTEM.SETTINGS_VIEW]
        }
      },
      {
        path: 'system/audit',
        name: 'AuditLogs',
        component: () => import('@/views/system/AuditLogs.vue'),
        meta: { 
          title: 'menu.auditLogs',
          icon: 'Document',
          permissions: [PERMISSIONS.SYSTEM.AUDIT_VIEW]
        }
      },
      {
        path: 'system/services',
        name: 'ServiceMonitor',
        component: () => import('@/views/system/ServiceMonitor.vue'),
        meta: { 
          title: 'menu.serviceMonitor',
          icon: 'Cpu',
          permissions: [PERMISSIONS.SYSTEM.SERVICE_VIEW]
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

// 设置权限守卫
setupPermissionGuard(router)

export default router