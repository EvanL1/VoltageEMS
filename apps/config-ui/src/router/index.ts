import { createRouter, createWebHistory } from 'vue-router';
import type { RouteRecordRaw } from 'vue-router';

const routes: RouteRecordRaw[] = [
  {
    path: '/',
    name: 'dashboard',
    component: () => import('@/views/Dashboard.vue'),
    meta: { title: '仪表盘', description: '系统配置概览和状态监控' }
  },
  // 实时监控
  {
    path: '/monitor/realtime',
    name: 'monitor-realtime',
    component: () => import('@/views/monitor/RealtimeData.vue'),
    meta: { title: '实时数据', category: '实时监控', description: '实时数据监控和查看' }
  },
  {
    path: '/monitor/channels',
    name: 'monitor-channels',
    component: () => import('@/views/monitor/ChannelsStatus.vue'),
    meta: { title: '通道状态', category: '实时监控', description: '监控通道运行状态' }
  },
  {
    path: '/monitor/alarms',
    name: 'monitor-alarms',
    component: () => import('@/views/monitor/AlarmsMonitor.vue'),
    meta: { title: '告警监控', category: '实时监控', description: '实时告警监控和处理' }
  },
  {
    path: '/monitor/trends',
    name: 'monitor-trends',
    component: () => import('@/views/monitor/TrendsChart.vue'),
    meta: { title: '历史趋势', category: '实时监控', description: '查看历史数据趋势' }
  },
  // 服务配置
  {
    path: '/service/:name',
    name: 'service-config',
    component: () => import('@/views/ServiceConfig.vue'),
    props: true,
    meta: { category: '服务配置' }
  },
  // 数据配置
  {
    path: '/channels',
    name: 'channels',
    component: () => import('@/views/ChannelManager.vue'),
    meta: { title: '通道管理', category: '数据配置', description: '配置和管理数据采集通道及点表' }
  },
  {
    path: '/data-flow',
    name: 'data-flow',
    component: () => import('@/views/DataFlow.vue'),
    meta: { title: '数据流配置', category: '数据配置', description: '配置数据流转和处理规则' }
  },
  {
    path: '/calculations',
    name: 'calculations',
    component: () => import('@/views/Calculations.vue'),
    meta: { title: '计算配置', category: '数据配置', description: '配置计算公式和规则' }
  },
  // 告警配置
  {
    path: '/alarm-rules',
    name: 'alarm-rules',
    component: () => import('@/views/AlarmRules.vue'),
    meta: { title: '告警规则', category: '告警配置', description: '配置告警触发条件和规则' }
  },
  {
    path: '/alarm-levels',
    name: 'alarm-levels',
    component: () => import('@/views/AlarmLevels.vue'),
    meta: { title: '告警等级', category: '告警配置', description: '管理告警等级和优先级' }
  },
  {
    path: '/alarm-notifications',
    name: 'alarm-notifications',
    component: () => import('@/views/AlarmNotifications.vue'),
    meta: { title: '通知配置', category: '告警配置', description: '配置告警通知方式和接收人' }
  },
  // 系统配置
  {
    path: '/settings',
    redirect: '/settings/global'
  },
  {
    path: '/settings/global',
    name: 'settings-global',
    component: () => import('@/views/settings/GlobalSettings.vue'),
    meta: { title: '全局设置', category: '系统配置', description: '系统全局参数配置' }
  },
  {
    path: '/settings/redis',
    name: 'settings-redis',
    component: () => import('@/views/settings/RedisSettings.vue'),
    meta: { title: 'Redis配置', category: '系统配置', description: 'Redis连接和参数配置' }
  },
  {
    path: '/settings/influxdb',
    name: 'settings-influxdb',
    component: () => import('@/views/settings/InfluxDBSettings.vue'),
    meta: { title: 'InfluxDB配置', category: '系统配置', description: 'InfluxDB连接和存储策略配置' }
  },
  {
    path: '/settings/backup',
    name: 'settings-backup',
    component: () => import('@/views/settings/BackupRestore.vue'),
    meta: { title: '备份恢复', category: '系统配置', description: '配置备份和恢复管理' }
  },
  {
    path: '/settings/templates',
    name: 'settings-templates',
    component: () => import('@/views/settings/ConfigTemplates.vue'),
    meta: { title: '配置模板', category: '系统配置', description: '管理和应用配置模板' }
  },
];

const router = createRouter({
  history: createWebHistory(),
  routes,
});

// 动态设置页面标题
router.beforeEach((to, from, next) => {
  if (to.meta.title) {
    document.title = `${to.meta.title} - VoltageEMS 配置中心`;
  }
  next();
});

export default router;