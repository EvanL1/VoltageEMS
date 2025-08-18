import { createRouter, createWebHistory } from 'vue-router'
import { useUserStore } from '@/stores/user'
import { useGlobalStore } from '@/stores/global'

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/',
      component: () => import('../layout/MainLayout.vue'),
      redirect: '/home',
      children: [
        {
          path: 'home',
          name: 'home',
          component: () => import('../views/HomeView/index.vue'),
          meta: {
            activeNav: '/home',
            roles: ['admin', 'operator', 'engineer'],
          },
        },
        {
          path: 'devices',
          name: 'devices',
          meta: {
            activeNav: '/devices',
            roles: ['admin', 'operator', 'engineer'],
          },
          children: [
            {
              path: 'devicesPV',
              name: 'devicesPV',
              component: () => import('../views/DevicesPV/index.vue'),
              redirect: '/devices/devicesPV/overview',
              meta: {
                activeNav: '/devices/devicesPV',
                roles: ['admin', 'operator', 'engineer'],
              },
              children: [
                {
                  path: 'overview',
                  name: 'devicesPVOverview',
                  component: () => import('../views/DevicesPV/PVOverview.vue'),
                  meta: {
                    activeNav: '/devices/devicesPV',
                    roles: ['admin', 'operator', 'engineer'],
                  },
                },
                {
                  path: 'monitoring',
                  name: 'devicesPVMonitoring',
                  component: () => import('../views/DevicesPV/PVValueMonitoring.vue'),
                  meta: {
                    activeNav: '/devices/devicesPV',
                    roles: ['admin', 'operator', 'engineer'],
                  },
                },
              ],
            },
            {
              path: 'deviceBattery',
              name: 'deviceBattery',
              component: () => import('../views/DeviceBattery/index.vue'),
              redirect: '/devices/deviceBattery/overview',
              meta: {
                activeNav: '/devices/deviceBattery',
                roles: ['admin', 'operator', 'engineer'],
              },
              children: [
                {
                  path: 'overview',
                  name: 'deviceBatteryOverview',
                  component: () => import('../views/DeviceBattery/BatteryOverview.vue'),
                  meta: {
                    activeNav: '/devices/deviceBattery',
                    roles: ['admin', 'operator', 'engineer'],
                  },
                },
                {
                  path: 'value',
                  name: 'deviceBatteryValue',
                  component: () => import('../views/DeviceBattery/BatteryValue.vue'),
                  meta: {
                    activeNav: '/devices/deviceBattery',
                    roles: ['admin', 'operator', 'engineer'],
                  },
                },
                {
                  path: 'management',
                  name: 'deviceBatteryManagement',
                  component: () => import('../views/DeviceBattery/BatteryManagement.vue'),
                  meta: {
                    activeNav: '/devices/deviceBattery',
                    roles: ['admin', 'operator', 'engineer'],
                  },
                },
              ],
            },
            {
              path: 'devicePCS',
              name: 'devicePCS',
              component: () => import('../views/DevicePCS/index.vue'),
              meta: {
                activeNav: '/devices/devicePCS',
                roles: ['admin', 'operator', 'engineer'],
              },
            },
            {
              path: 'devicemeter2',
              name: 'devicemeter2',
              component: () => import('../views/DeviceMeter2/index.vue'),
              meta: {
                activeNav: '/devices/devicemeter2',
                roles: ['admin', 'operator', 'engineer'],
              },
            },
            {
              path: 'devicemeter1',
              name: 'devicemeter1',
              component: () => import('../views/DeviceMeter1/index.vue'),
              meta: {
                activeNav: '/devices/devicemeter1',
                roles: ['admin', 'operator', 'engineer'],
              },
            },
            {
              path: 'dieselGenerator',
              name: 'dieselGenerator',
              component: () => import('../views/DieselGenerator/index.vue'),
              redirect: '/devices/dieselGenerator/overview',
              meta: {
                activeNav: '/devices/dieselGenerator',
                roles: ['admin', 'operator', 'engineer'],
              },
              children: [
                {
                  path: 'overview',
                  name: 'dieselGeneratorOverview',
                  component: () => import('../views/DieselGenerator/DieselOverview.vue'),
                  meta: {
                    activeNav: '/devices/dieselGenerator',
                    roles: ['admin', 'operator', 'engineer'],
                  },
                },
                {
                  path: 'monitoring',
                  name: 'dieselGeneratorMonitoring',
                  component: () => import('../views/DieselGenerator/DieselValueMonitoring.vue'),
                  meta: {
                    activeNav: '/devices/dieselGenerator',
                    roles: ['admin', 'operator', 'engineer'],
                  },
                },
              ],
            },
          ],
        },
        {
          path: 'alarm',
          name: 'alarm',
          meta: {
            activeNav: '/alarm',
            roles: ['admin', 'operator', 'engineer'],
          },
          children: [
            {
              path: 'alarmRecords',
              name: 'alarmRecords',
              component: () => import('../views/AlarmRecords/index.vue'),
              meta: {
                activeNav: '/alarm/alarmRecords',
                roles: ['admin', 'operator', 'engineer'],
              },
            },
            {
              path: 'ruleManagement',
              name: 'ruleManagement',
              component: () => import('../views/RulesManagement/index.vue'),
              meta: {
                activeNav: '/alarm/ruleManagement',
                roles: ['admin', 'operator'],
              },
            },
          ],
        },
        {
          path: 'statistics',
          name: 'statistics',
          component: () => import('../views/Statistics/index.vue'),
          redirect: '/statistics/overview',
          meta: {
            activeNav: '/statistics',
            roles: ['admin', 'operator', 'engineer'],
          },
          children: [
            {
              path: 'overview',
              name: 'statisticsOverview',
              component: () => import('../views/Statistics/Overview.vue'),
              meta: {
                activeNav: '/statistics',
                roles: ['admin', 'operator', 'engineer'],
              },
            },
            {
              path: 'curves',
              name: 'statisticsCurves',
              component: () => import('../views/Statistics/Curves.vue'),
              meta: {
                activeNav: '/statistics',
                roles: ['admin', 'operator', 'engineer'],
              },
            },
            {
              path: 'operationLog',
              name: 'statisticsOperationLog',
              component: () => import('../views/Statistics/OperationLog.vue'),
              meta: {
                activeNav: '/statistics',
                roles: ['admin', 'operator'],
              },
            },
            {
              path: 'runingLog',
              name: 'statisticsRuningLog',
              component: () => import('../views/Statistics/RuningLog.vue'),
              meta: {
                activeNav: '/statistics',
                roles: ['admin', 'operator'],
              },
            },
          ],
        },
        {
          path: 'setting',
          name: 'setting',
          meta: {
            activeNav: '/setting',
            roles: ['admin'],
          },
          children: [
            {
              path: 'systemSetting',
              name: 'systemSetting',
              component: () => import('../views/SystemSetting/index.vue'),
              meta: {
                activeNav: '/setting/systemSetting',
                roles: ['admin'],
              },
            },
            {
              path: 'userManagement',
              name: 'userManagement',
              component: () => import('../views/UserManagement/index.vue'),
              meta: {
                activeNav: '/setting/userManagement',
                roles: ['admin'],
              },
            },
          ],
        },
      ],
    },
    {
      path: '/login',
      name: 'login',
      component: () => import('../views/LoginView/index.vue'),
      meta: {
        title: '登录页面',
      },
    },
  ],
})

// 路由守卫
router.beforeEach(async (to, from, next) => {
  // const userStore = useUserStore()
  // const globalStore = useGlobalStore()

  // 检查是否需要登录
  // if (to.path === '/login') {
  //   // 如果已经登录，重定向到首页
  //   if (userStore.isLoggedIn) {
  //     next('/')
  //   } else {
  //     next()
  //   }
  //   return
  // }

  // 检查是否已登录
  // if (!userStore.isLoggedIn) {
  //   next('/login')
  //   return
  // }

  // 检查路由权限
  // if (to.meta?.roles) {
  //   const userRole = userStore.userInfo?.role || 'engineer'
  //   const allowedRoles = to.meta.roles as string[]

  //   if (!hasRoutePermission(allowedRoles, userRole)) {
  //     // 权限不足，重定向到首页
  //     next('/')
  //     return
  //   }
  // }

  next()
})

export default router
