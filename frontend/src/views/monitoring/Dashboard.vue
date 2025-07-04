<template>
  <div class="dashboard-container">
    <!-- 页面标题 -->
    <div class="page-header">
      <h1>{{ $t('menu.dashboard') }}</h1>
      <div class="header-actions">
        <el-button @click="refreshData" :loading="loading">
          <el-icon><Refresh /></el-icon>
          {{ $t('common.refresh') }}
        </el-button>
      </div>
    </div>

    <!-- 关键指标卡片 -->
    <el-row :gutter="20" class="metric-cards">
      <el-col :xs="24" :sm="12" :md="6" v-for="metric in metrics" :key="metric.key">
        <el-card class="metric-card" :class="metric.class">
          <div class="metric-content">
            <div class="metric-icon">
              <el-icon :size="32">
                <component :is="metric.icon" />
              </el-icon>
            </div>
            <div class="metric-info">
              <div class="metric-value">{{ metric.value }}</div>
              <div class="metric-label">{{ metric.label }}</div>
            </div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 图表区域 -->
    <el-row :gutter="20" class="chart-row">
      <!-- 实时功率曲线 -->
      <el-col :xs="24" :lg="16">
        <el-card class="chart-card">
          <template #header>
            <div class="card-header">
              <span>{{ $t('dashboard.powerCurve', '实时功率曲线') }}</span>
              <el-radio-group v-model="timeRange" size="small" @change="updateCharts">
                <el-radio-button label="1h">1小时</el-radio-button>
                <el-radio-button label="24h">24小时</el-radio-button>
                <el-radio-button label="7d">7天</el-radio-button>
              </el-radio-group>
            </div>
          </template>
          <div id="powerChart" class="chart"></div>
        </el-card>
      </el-col>

      <!-- 告警级别分布 -->
      <el-col :xs="24" :lg="8">
        <el-card class="chart-card">
          <template #header>
            <span>{{ $t('dashboard.alarmDistribution', '告警级别分布') }}</span>
          </template>
          <div id="alarmChart" class="chart"></div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 设备状态和快捷操作 -->
    <el-row :gutter="20">
      <!-- 设备类型分布 -->
      <el-col :xs="24" :md="12">
        <el-card class="chart-card">
          <template #header>
            <span>{{ $t('dashboard.deviceDistribution', '设备类型分布') }}</span>
          </template>
          <div id="deviceChart" class="chart-small"></div>
        </el-card>
      </el-col>

      <!-- 快捷操作 -->
      <el-col :xs="24" :md="12">
        <el-card>
          <template #header>
            <span>{{ $t('dashboard.quickActions', '快捷操作') }}</span>
          </template>
          <div class="quick-actions">
            <el-button 
              v-for="action in quickActions" 
              :key="action.key"
              :type="action.type"
              :icon="action.icon"
              @click="handleQuickAction(action)"
              v-permission="action.permission"
            >
              {{ action.label }}
            </el-button>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- Grafana集成 -->
    <el-row :gutter="20" class="grafana-row" v-if="showGrafana">
      <el-col :span="24">
        <el-card>
          <template #header>
            <div class="card-header">
              <span>{{ $t('dashboard.systemMonitoring', '系统监控') }}</span>
              <el-button text @click="openGrafana">
                <el-icon><Link /></el-icon>
                {{ $t('grafana.openInGrafana') }}
              </el-button>
            </div>
          </template>
          <GrafanaEmbedEnhanced
            dashboard-uid="voltage-overview"
            :height="400"
            :refresh-interval="30"
          />
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import * as echarts from 'echarts'
// import { useUserStore } from '@/stores/user'
import { useAlarmStore } from '@/stores/alarm'
// import { useRealtimeStore } from '@/stores/realtime'
import GrafanaEmbedEnhanced from '@/components/GrafanaEmbedEnhanced.vue'

const router = useRouter()
const { t } = useI18n()
// const userStore = useUserStore()
const alarmStore = useAlarmStore()
// const realtimeStore = useRealtimeStore() // 预留给实时数据使用

// 响应式数据
const loading = ref(false)
const timeRange = ref('24h')
const showGrafana = ref(true)

// 图表实例
let powerChart = null
let alarmChart = null
let deviceChart = null

// 关键指标
const metrics = computed(() => [
  {
    key: 'devices',
    icon: 'Monitor',
    label: t('dashboard.totalDevices', '设备总数'),
    value: '156',
    class: 'metric-primary'
  },
  {
    key: 'online',
    icon: 'CircleCheck',
    label: t('dashboard.onlineRate', '在线率'),
    value: '98.5%',
    class: 'metric-success'
  },
  {
    key: 'energy',
    icon: 'DataLine',
    label: t('dashboard.todayEnergy', '今日能耗'),
    value: '1,234 kWh',
    class: 'metric-warning'
  },
  {
    key: 'alarms',
    icon: 'WarnTriangleFilled',
    label: t('dashboard.activeAlarms', '活跃告警'),
    value: alarmStore.activeCount || '3',
    class: 'metric-danger'
  }
])

// 快捷操作
const quickActions = computed(() => [
  {
    key: 'newDevice',
    label: t('dashboard.newDevice', '新建设备'),
    icon: 'Plus',
    type: 'primary',
    permission: 'devices.create',
    route: '/config/channels'
  },
  {
    key: 'exportReport',
    label: t('dashboard.exportReport', '导出报表'),
    icon: 'Download',
    type: 'default',
    permission: 'report.export'
  },
  {
    key: 'systemCheck',
    label: t('dashboard.systemCheck', '系统巡检'),
    icon: 'Search',
    type: 'default',
    permission: 'system.check'
  },
  {
    key: 'alarmRules',
    label: t('dashboard.alarmRules', '告警规则'),
    icon: 'Setting',
    type: 'default',
    permission: 'alarms.config',
    route: '/config/alarms'
  }
])

// 初始化图表
const initCharts = () => {
  // 功率曲线图
  powerChart = echarts.init(document.getElementById('powerChart'))
  const powerOption = {
    tooltip: {
      trigger: 'axis'
    },
    grid: {
      left: '3%',
      right: '4%',
      bottom: '3%',
      containLabel: true
    },
    xAxis: {
      type: 'category',
      boundaryGap: false,
      data: generateTimeLabels()
    },
    yAxis: {
      type: 'value',
      name: 'kW'
    },
    series: [
      {
        name: t('dashboard.activePower', '有功功率'),
        type: 'line',
        smooth: true,
        data: generatePowerData(),
        areaStyle: {
          opacity: 0.3
        }
      }
    ]
  }
  powerChart.setOption(powerOption)

  // 告警分布图
  alarmChart = echarts.init(document.getElementById('alarmChart'))
  const alarmOption = {
    tooltip: {
      trigger: 'item'
    },
    legend: {
      orient: 'vertical',
      left: 'left'
    },
    series: [
      {
        name: t('dashboard.alarmLevel', '告警级别'),
        type: 'pie',
        radius: ['40%', '70%'],
        avoidLabelOverlap: false,
        itemStyle: {
          borderRadius: 10,
          borderColor: '#fff',
          borderWidth: 2
        },
        label: {
          show: false,
          position: 'center'
        },
        emphasis: {
          label: {
            show: true,
            fontSize: '20',
            fontWeight: 'bold'
          }
        },
        labelLine: {
          show: false
        },
        data: [
          { value: 2, name: t('alarms.levels.critical'), itemStyle: { color: '#f56c6c' } },
          { value: 5, name: t('alarms.levels.high'), itemStyle: { color: '#e6a23c' } },
          { value: 8, name: t('alarms.levels.medium'), itemStyle: { color: '#f4d03f' } },
          { value: 12, name: t('alarms.levels.info'), itemStyle: { color: '#409eff' } }
        ]
      }
    ]
  }
  alarmChart.setOption(alarmOption)

  // 设备分布图
  deviceChart = echarts.init(document.getElementById('deviceChart'))
  const deviceOption = {
    tooltip: {
      trigger: 'item'
    },
    series: [
      {
        name: t('dashboard.deviceType', '设备类型'),
        type: 'pie',
        radius: '50%',
        data: [
          { value: 45, name: 'PCS' },
          { value: 38, name: 'BMS' },
          { value: 32, name: t('devices.types.powerMeter') },
          { value: 28, name: t('devices.types.temperatureSensor') },
          { value: 13, name: t('common.other', '其他') }
        ],
        emphasis: {
          itemStyle: {
            shadowBlur: 10,
            shadowOffsetX: 0,
            shadowColor: 'rgba(0, 0, 0, 0.5)'
          }
        }
      }
    ]
  }
  deviceChart.setOption(deviceOption)
}

// 生成时间标签
const generateTimeLabels = () => {
  const labels = []
  const now = new Date()
  const hours = timeRange.value === '1h' ? 1 : timeRange.value === '24h' ? 24 : 168
  const interval = hours <= 24 ? 1 : 24
  
  for (let i = hours; i >= 0; i -= interval) {
    const time = new Date(now - i * 60 * 60 * 1000)
    labels.push(time.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' }))
  }
  
  return labels
}

// 生成功率数据
const generatePowerData = () => {
  const data = []
  const hours = timeRange.value === '1h' ? 1 : timeRange.value === '24h' ? 24 : 168
  const points = hours <= 24 ? hours + 1 : 8
  
  for (let i = 0; i < points; i++) {
    data.push(Math.floor(Math.random() * 50) + 100)
  }
  
  return data
}

// 刷新数据
const refreshData = async () => {
  loading.value = true
  try {
    await Promise.all([
      alarmStore.fetchAlarms(),
      // 其他数据获取
    ])
    updateCharts()
    ElMessage.success(t('common.refreshSuccess', '刷新成功'))
  } catch (error) {
    ElMessage.error(t('common.refreshFailed', '刷新失败'))
  } finally {
    loading.value = false
  }
}

// 更新图表
const updateCharts = () => {
  if (powerChart) {
    powerChart.setOption({
      xAxis: {
        data: generateTimeLabels()
      },
      series: [{
        data: generatePowerData()
      }]
    })
  }
}

// 处理快捷操作
const handleQuickAction = (action) => {
  if (action.route) {
    router.push(action.route)
  } else {
    ElMessage.info(`${action.label} - ${t('common.developing', '功能开发中')}`)
  }
}

// 打开Grafana
const openGrafana = () => {
  window.open('/grafana/', '_blank')
}

// 处理窗口大小变化
const handleResize = () => {
  powerChart?.resize()
  alarmChart?.resize()
  deviceChart?.resize()
}

// 生命周期
onMounted(() => {
  initCharts()
  window.addEventListener('resize', handleResize)
  
  // 获取初始数据
  refreshData()
  
  // 定时刷新
  const timer = setInterval(refreshData, 60000) // 每分钟刷新
  
  onUnmounted(() => {
    clearInterval(timer)
    window.removeEventListener('resize', handleResize)
    powerChart?.dispose()
    alarmChart?.dispose()
    deviceChart?.dispose()
  })
})
</script>

<style lang="scss" scoped>
@import '@/styles/design-tokens.scss';

.dashboard-container {
  min-height: 100%;
}

// Apple 风格页面头部
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--space-8);
  
  h1 {
    margin: 0;
    font-size: var(--font-size-4xl);
    font-weight: var(--font-weight-bold);
    color: var(--color-text-primary);
    letter-spacing: -0.02em;
  }
  
  .header-actions {
    .el-button {
      height: 36px;
      padding: 0 var(--space-4);
      background: var(--color-background-elevated);
      border: 1px solid var(--color-border-light);
      border-radius: var(--radius-lg);
      color: var(--color-primary);
      font-weight: var(--font-weight-medium);
      transition: all var(--duration-fast) var(--ease-in-out);
      
      &:hover {
        background: var(--color-primary);
        color: var(--color-text-inverse);
        border-color: var(--color-primary);
      }
      
      .el-icon {
        margin-right: var(--space-1);
      }
    }
  }
}

// Tesla 风格指标卡片
.metric-cards {
  margin-bottom: var(--space-8);
  
  .metric-card {
    height: 120px;
    background: var(--color-background-elevated);
    border: 1px solid var(--color-border-light);
    border-radius: var(--radius-xl);
    box-shadow: var(--shadow-xs);
    transition: all var(--duration-normal) var(--ease-in-out);
    position: relative;
    overflow: hidden;
    
    &::before {
      content: '';
      position: absolute;
      top: 0;
      left: 0;
      right: 0;
      height: 3px;
      background: currentColor;
      transform: scaleX(0);
      transform-origin: left;
      transition: transform var(--duration-normal) var(--ease-in-out);
    }
    
    &:hover {
      transform: translateY(-4px);
      box-shadow: var(--shadow-lg);
      border-color: transparent;
      
      &::before {
        transform: scaleX(1);
      }
    }
    
    &.metric-primary {
      color: var(--color-primary);
    }
    
    &.metric-success {
      color: var(--color-success);
    }
    
    &.metric-warning {
      color: var(--color-warning);
    }
    
    &.metric-danger {
      color: var(--color-danger);
    }
    
    :deep(.el-card__body) {
      height: 100%;
      padding: var(--space-5);
    }
    
    .metric-content {
      display: flex;
      align-items: center;
      height: 100%;
      gap: var(--space-4);
      
      .metric-icon {
        width: 56px;
        height: 56px;
        display: flex;
        align-items: center;
        justify-content: center;
        background: currentColor;
        border-radius: var(--radius-lg);
        opacity: 0.1;
        
        .el-icon {
          font-size: 28px;
          color: var(--color-text-primary);
          opacity: 1;
        }
      }
      
      .metric-info {
        flex: 1;
        
        .metric-value {
          font-size: var(--font-size-3xl);
          font-weight: var(--font-weight-bold);
          color: var(--color-text-primary);
          line-height: 1.2;
          letter-spacing: -0.02em;
        }
        
        .metric-label {
          font-size: var(--font-size-sm);
          color: var(--color-text-tertiary);
          margin-top: var(--space-1);
          font-weight: var(--font-weight-medium);
        }
      }
    }
  }
}

// 图表区域
.chart-row {
  margin-bottom: var(--space-8);
}

.chart-card {
  height: 100%;
  background: var(--color-background-elevated);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-xs);
  
  :deep(.el-card__header) {
    padding: var(--space-5) var(--space-6);
    border-bottom: 1px solid var(--color-border-light);
  }
  
  :deep(.el-card__body) {
    padding: var(--space-6);
  }
  
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    
    span {
      font-size: var(--font-size-lg);
      font-weight: var(--font-weight-semibold);
      color: var(--color-text-primary);
    }
    
    .el-radio-group {
      :deep(.el-radio-button__inner) {
        padding: var(--space-2) var(--space-3);
        border: none;
        background: var(--color-gray-100);
        color: var(--color-text-secondary);
        font-weight: var(--font-weight-medium);
        border-radius: var(--radius-md);
        
        &:hover {
          color: var(--color-text-primary);
        }
      }
      
      :deep(.el-radio-button__original-radio:checked + .el-radio-button__inner) {
        background: var(--color-primary);
        color: var(--color-text-inverse);
        box-shadow: none;
      }
      
      :deep(.el-radio-button:first-child .el-radio-button__inner) {
        border-radius: var(--radius-md) 0 0 var(--radius-md);
      }
      
      :deep(.el-radio-button:last-child .el-radio-button__inner) {
        border-radius: 0 var(--radius-md) var(--radius-md) 0;
      }
    }
  }
}

.chart {
  height: 400px;
}

.chart-small {
  height: 320px;
}

// 快捷操作区域
.quick-actions {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: var(--space-3);
  
  .el-button {
    height: 48px;
    border-radius: var(--radius-lg);
    font-weight: var(--font-weight-medium);
    transition: all var(--duration-fast) var(--ease-in-out);
    position: relative;
    overflow: hidden;
    
    &::before {
      content: '';
      position: absolute;
      inset: 0;
      background: linear-gradient(45deg, transparent, rgba(255, 255, 255, 0.1), transparent);
      transform: translateX(-100%);
      transition: transform 0.6s;
    }
    
    &:hover::before {
      transform: translateX(100%);
    }
    
    &:hover {
      transform: translateY(-2px);
      box-shadow: var(--shadow-md);
    }
    
    &:active {
      transform: translateY(0);
    }
    
    .el-icon {
      margin-right: var(--space-2);
      font-size: 18px;
    }
  }
}

// Grafana 集成卡片
.grafana-row {
  margin-top: var(--space-8);
  
  :deep(.el-card) {
    background: var(--color-background-elevated);
    border: 1px solid var(--color-border-light);
    border-radius: var(--radius-xl);
    box-shadow: var(--shadow-xs);
  }
  
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    
    span {
      font-size: var(--font-size-lg);
      font-weight: var(--font-weight-semibold);
      color: var(--color-text-primary);
    }
    
    .el-button {
      color: var(--color-primary);
      font-weight: var(--font-weight-medium);
      
      &:hover {
        color: var(--color-primary-hover);
        background: var(--color-primary-light);
      }
    }
  }
}

// 响应式设计
@media (max-width: 1024px) {
  .quick-actions {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 768px) {
  .page-header {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-4);
    
    h1 {
      font-size: var(--font-size-3xl);
    }
  }
  
  .metric-cards {
    .el-col {
      margin-bottom: var(--space-4);
    }
  }
  
  .chart {
    height: 300px;
  }
  
  .chart-small {
    height: 250px;
  }
}

// 动画效果
@keyframes pulse {
  0% {
    opacity: 1;
  }
  50% {
    opacity: 0.8;
  }
  100% {
    opacity: 1;
  }
}

// 加载动画
.loading-animation {
  animation: pulse 2s ease-in-out infinite;
}
</style>