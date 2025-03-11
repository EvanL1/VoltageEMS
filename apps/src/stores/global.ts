import { defineStore } from 'pinia'
import type { RouteItem } from '@/types/menu'
import { ref, type Ref } from 'vue'

// 定义全局store，包含菜单数据和根据角色筛选菜单的方法
export const useGlobalStore = defineStore('global', () => {
  const isCollapse = ref(false)

  // 菜单数据
  const allRoutesList = ref<RouteItem[]>([
    {
      path: 'home',
      name: 'home',
      component: () => import('../views/HomeView/index.vue'),
      meta: {
        isSubMenu: false,
        activeNav: '/home',
        iconName: 'home',
        title: 'Home',
        roles: ['admin', 'operator', 'engineer'],
      },
    },
    {
      path: 'devices',
      name: 'devices',
      meta: {
        isSubMenu: true,
        activeNav: '/devices',
        iconName: 'devices',
        title: 'Devices',
        roles: ['admin', 'operator', 'engineer'],
      },
      children: [
        {
          path: 'devicesPV',
          name: 'devicesPV',
          redirect: '/devices/devicesPV/overview',
          meta: {
            title: 'PV',
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
            title: 'Battery',
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
            title: 'PCS',
            activeNav: '/devices/devicePCS',
            roles: ['admin', 'operator', 'engineer'],
          },
        },
        {
          path: 'devicemeter1',
          name: 'devicemeter1',
          component: () => import('../views/DeviceMeter1/index.vue'),
          meta: {
            title: 'Meter1',
            activeNav: '/devices/devicemeter1',
            roles: ['admin', 'operator', 'engineer'],
          },
        },
        {
          path: 'devicemeter2',
          name: 'devicemeter2',
          component: () => import('../views/DeviceMeter2/index.vue'),
          meta: {
            title: 'Meter2',
            activeNav: '/devices/devicemeter2',
            roles: ['admin', 'operator', 'engineer'],
          },
        },
        {
          path: 'dieselGenerator',
          name: 'dieselGenerator',
          component: () => import('../views/DieselGenerator/index.vue'),
          redirect: '/devices/dieselGenerator/overview',
          meta: {
            title: 'Diesel Generator',
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
        isSubMenu: true,
        activeNav: '/alarm',
        iconName: 'warning',
        title: 'Alarm',
        roles: ['admin', 'operator', 'engineer'],
      },
      children: [
        {
          path: 'alarmRecords',
          name: 'alarmRecords',
          component: () => import('../views/AlarmRecords/index.vue'),
          meta: {
            title: 'Alarm Records',
            activeNav: '/alarm/alarmRecords',
            roles: ['admin', 'operator', 'engineer'],
          },
        },
        {
          path: 'ruleManagement',
          name: 'ruleManagement',
          component: () => import('../views/RulesManagement/index.vue'),
          meta: {
            title: 'Rule Management',
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
        isSubMenu: false,
        title: 'Statistics',
        activeNav: '/statistics',
        iconName: 'statistics',
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
        isSubMenu: true,
        activeNav: '/setting',
        iconName: 'setting',
        title: 'Setting',
        roles: ['admin'],
      },
      children: [
        {
          path: 'systemSetting',
          name: 'systemSetting',
          component: () => import('../views/SystemSetting/index.vue'),
          meta: {
            title: 'System Setting',
            activeNav: '/setting/systemSetting',
            roles: ['admin'],
          },
        },
        {
          path: 'userManagement',
          name: 'userManagement',
          component: () => import('../views/UserManagement/index.vue'),
          meta: {
            title: 'User Management',
            activeNav: '/setting/userManagement',
            roles: ['admin'],
          },
        },
      ],
    },
  ])
  const activeMenuPath = ref('/home')
  const filterRoutesList = ref<RouteItem[]>(allRoutesList.value)
  /**
   * 递归过滤菜单，根据角色类型筛选可访问的菜单
   * @param routes 需要过滤的路由数组
   * @param role 用户角色（如 'admin'、'operator'、'engineer'）
   * @returns 过滤后的路由数组
   */
  function filterRoutesByRole(routes: RouteItem[], role: string): RouteItem[] {
    return routes
      .filter((route) => {
        // 判断当前路由是否有meta.roles，并且包含当前角色
        const hasRole = route.meta?.roles?.includes(role)
        // 如果有children，递归过滤子路由
        let filteredChildren: RouteItem[] | undefined
        if (route.children && route.children.length > 0) {
          filteredChildren = filterRoutesByRole(route.children, role)
        }
        // 只返回有权限的路由，或者其子路由有权限
        if (hasRole || (filteredChildren && filteredChildren.length > 0)) {
          return true
        }
        return false
      })
      .map((route) => {
        // 递归处理children
        let newRoute = { ...route }
        if (route.children && route.children.length > 0) {
          newRoute.children = filterRoutesByRole(route.children, role)
        }
        return newRoute
      })
  }

  /**
   * 获取当前角色可访问的菜单
   * @param role 用户角色
   * @returns RouteItem[]
   */
  function getRoutesByRole(role: string): RouteItem[] {
    filterRoutesList.value = filterRoutesByRole(allRoutesList.value, role)
    return filterRoutesList.value
  }

  return {
    isCollapse,
    activeMenuPath,
    allRoutesList,
    filterRoutesList,
    getRoutesByRole, // 暴露方法
  }
})
