<template>
  <div class="alarm-overview-container">
    <!-- 页面头部 -->
    <div class="page-header">
      <h1>{{ $t('alarmOverview.title') }}</h1>
      <div class="header-actions">
        <el-date-picker
          v-model="timeRange"
          type="datetimerange"
          :range-separator="$t('common.to')"
          :start-placeholder="$t('common.startTime')"
          :end-placeholder="$t('common.endTime')"
          format="YYYY-MM-DD HH:mm:ss"
          value-format="YYYY-MM-DD HH:mm:ss"
          @change="handleTimeRangeChange"
        />
        <el-button @click="handleRefresh">
          <el-icon><Refresh /></el-icon>
          {{ $t('common.refresh') }}
        </el-button>
      </div>
    </div>

    <!-- 告警统计卡片 -->
    <div class="alarm-stats">
      <el-row :gutter="20">
        <el-col :xs="24" :sm="12" :md="6">
          <el-card class="stat-card critical">
            <div class="stat-content">
              <div class="stat-icon">
                <el-icon :size="32"><Warning /></el-icon>
              </div>
              <div class="stat-info">
                <div class="stat-value">{{ stats.critical }}</div>
                <div class="stat-label">{{ $t('alarmOverview.criticalAlarms') }}</div>
                <div class="stat-trend">
                  <span :class="getTrendClass(trends.critical)">
                    <el-icon><component :is="getTrendIcon(trends.critical)" /></el-icon>
                    {{ Math.abs(trends.critical) }}%
                  </span>
                  <span class="vs">{{ $t('alarmOverview.vsYesterday') }}</span>
                </div>
              </div>
            </div>
          </el-card>
        </el-col>
        <el-col :xs="24" :sm="12" :md="6">
          <el-card class="stat-card major">
            <div class="stat-content">
              <div class="stat-icon">
                <el-icon :size="32"><WarningFilled /></el-icon>
              </div>
              <div class="stat-info">
                <div class="stat-value">{{ stats.major }}</div>
                <div class="stat-label">{{ $t('alarmOverview.majorAlarms') }}</div>
                <div class="stat-trend">
                  <span :class="getTrendClass(trends.major)">
                    <el-icon><component :is="getTrendIcon(trends.major)" /></el-icon>
                    {{ Math.abs(trends.major) }}%
                  </span>
                  <span class="vs">{{ $t('alarmOverview.vsYesterday') }}</span>
                </div>
              </div>
            </div>
          </el-card>
        </el-col>
        <el-col :xs="24" :sm="12" :md="6">
          <el-card class="stat-card minor">
            <div class="stat-content">
              <div class="stat-icon">
                <el-icon :size="32"><InfoFilled /></el-icon>
              </div>
              <div class="stat-info">
                <div class="stat-value">{{ stats.minor }}</div>
                <div class="stat-label">{{ $t('alarmOverview.minorAlarms') }}</div>
                <div class="stat-trend">
                  <span :class="getTrendClass(trends.minor)">
                    <el-icon><component :is="getTrendIcon(trends.minor)" /></el-icon>
                    {{ Math.abs(trends.minor) }}%
                  </span>
                  <span class="vs">{{ $t('alarmOverview.vsYesterday') }}</span>
                </div>
              </div>
            </div>
          </el-card>
        </el-col>
        <el-col :xs="24" :sm="12" :md="6">
          <el-card class="stat-card hint">
            <div class="stat-content">
              <div class="stat-icon">
                <el-icon :size="32"><QuestionFilled /></el-icon>
              </div>
              <div class="stat-info">
                <div class="stat-value">{{ stats.hint }}</div>
                <div class="stat-label">{{ $t('alarmOverview.hintAlarms') }}</div>
                <div class="stat-trend">
                  <span :class="getTrendClass(trends.hint)">
                    <el-icon><component :is="getTrendIcon(trends.hint)" /></el-icon>
                    {{ Math.abs(trends.hint) }}%
                  </span>
                  <span class="vs">{{ $t('alarmOverview.vsYesterday') }}</span>
                </div>
              </div>
            </div>
          </el-card>
        </el-col>
      </el-row>
    </div>

    <!-- 图表区域 -->
    <el-row :gutter="20" class="charts-row">
      <!-- 告警趋势图 -->
      <el-col :xs="24" :lg="14">
        <el-card class="chart-card">
          <template #header>
            <div class="card-header">
              <span>{{ $t('alarmOverview.alarmTrend') }}</span>
              <el-radio-group v-model="trendPeriod" size="small" @change="updateTrendChart">
                <el-radio-button label="day">{{ $t('alarmOverview.today') }}</el-radio-button>
                <el-radio-button label="week">{{ $t('alarmOverview.thisWeek') }}</el-radio-button>
                <el-radio-button label="month">{{ $t('alarmOverview.thisMonth') }}</el-radio-button>
              </el-radio-group>
            </div>
          </template>
          <div ref="trendChartRef" class="chart-container"></div>
        </el-card>
      </el-col>

      <!-- 告警分布饼图 -->
      <el-col :xs="24" :lg="10">
        <el-card class="chart-card">
          <template #header>
            <span>{{ $t('alarmOverview.alarmDistribution') }}</span>
          </template>
          <div ref="distributionChartRef" class="chart-container"></div>
        </el-card>
      </el-col>
    </el-row>

    <el-row :gutter="20" class="charts-row">
      <!-- 设备告警排行 -->
      <el-col :xs="24" :lg="12">
        <el-card class="chart-card">
          <template #header>
            <span>{{ $t('alarmOverview.deviceAlarmRanking') }}</span>
          </template>
          <div ref="deviceRankingRef" class="chart-container"></div>
        </el-card>
      </el-col>

      <!-- 告警类型分析 -->
      <el-col :xs="24" :lg="12">
        <el-card class="chart-card">
          <template #header>
            <span>{{ $t('alarmOverview.alarmTypeAnalysis') }}</span>
          </template>
          <div ref="typeAnalysisRef" class="chart-container"></div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 最新告警列表 -->
    <el-card class="alarm-list-card">
      <template #header>
        <div class="card-header">
          <span>{{ $t('alarmOverview.latestAlarms') }}</span>
          <el-button type="primary" link @click="gotoAlarmManagement">
            {{ $t('alarmOverview.viewAll') }}
            <el-icon><ArrowRight /></el-icon>
          </el-button>
        </div>
      </template>
      <el-table 
        :data="latestAlarms" 
        stripe
        style="width: 100%"
        :empty-text="$t('common.noData')"
      >
        <el-table-column prop="time" :label="$t('alarmOverview.alarmTime')" width="180">
          <template #default="{ row }">
            {{ formatTime(row.time) }}
          </template>
        </el-table-column>
        <el-table-column prop="level" :label="$t('alarmOverview.level')" width="100">
          <template #default="{ row }">
            <el-tag :type="getLevelType(row.level)" size="small">
              {{ getLevelLabel(row.level) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="source" :label="$t('alarmOverview.source')" width="150" />
        <el-table-column prop="message" :label="$t('alarmOverview.message')" min-width="200" show-overflow-tooltip />
        <el-table-column prop="status" :label="$t('common.status')" width="100">
          <template #default="{ row }">
            <el-tag :type="getStatusType(row.status)" size="small">
              {{ getStatusLabel(row.status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column :label="$t('common.actions')" width="120" fixed="right">
          <template #default="{ row }">
            <el-button 
              type="primary" 
              link 
              size="small" 
              @click="handleViewAlarm(row)"
            >
              {{ $t('common.details') }}
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 告警响应统计 -->
    <el-row :gutter="20" class="response-stats">
      <el-col :xs="24" :sm="12" :md="6">
        <el-card>
          <div class="response-stat">
            <div class="stat-title">{{ $t('alarmOverview.avgResponseTime') }}</div>
            <div class="stat-value primary">{{ responseStats.avgTime }} {{ $t('alarmOverview.minutes') }}</div>
            <el-progress :percentage="responseStats.avgTimeProgress" :show-text="false" />
          </div>
        </el-card>
      </el-col>
      <el-col :xs="24" :sm="12" :md="6">
        <el-card>
          <div class="response-stat">
            <div class="stat-title">{{ $t('alarmOverview.confirmRate') }}</div>
            <div class="stat-value success">{{ responseStats.confirmRate }}%</div>
            <el-progress :percentage="responseStats.confirmRate" :show-text="false" status="success" />
          </div>
        </el-card>
      </el-col>
      <el-col :xs="24" :sm="12" :md="6">
        <el-card>
          <div class="response-stat">
            <div class="stat-title">{{ $t('alarmOverview.clearRate') }}</div>
            <div class="stat-value warning">{{ responseStats.clearRate }}%</div>
            <el-progress :percentage="responseStats.clearRate" :show-text="false" status="warning" />
          </div>
        </el-card>
      </el-col>
      <el-col :xs="24" :sm="12" :md="6">
        <el-card>
          <div class="response-stat">
            <div class="stat-title">{{ $t('alarmOverview.falseAlarmRate') }}</div>
            <div class="stat-value danger">{{ responseStats.falseRate }}%</div>
            <el-progress :percentage="responseStats.falseRate" :show-text="false" status="exception" />
          </div>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted, nextTick, shallowRef } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { 
  Refresh, Warning, WarningFilled, InfoFilled, QuestionFilled,
  ArrowRight, Top, Bottom
} from '@element-plus/icons-vue'
import * as echarts from 'echarts'
import dayjs from 'dayjs'

const { t } = useI18n()
const router = useRouter()

// 图标组件引用
const trendIcons = shallowRef({ Top, Bottom })

// 时间范围
const timeRange = ref([
  dayjs().startOf('day').format('YYYY-MM-DD HH:mm:ss'),
  dayjs().format('YYYY-MM-DD HH:mm:ss')
])

// 趋势周期
const trendPeriod = ref('day')

// 统计数据
const stats = ref({
  critical: 12,
  major: 28,
  minor: 45,
  hint: 67
})

// 趋势数据
const trends = ref({
  critical: -15,
  major: 8,
  minor: -5,
  hint: 12
})

// 响应统计
const responseStats = ref({
  avgTime: 15.3,
  avgTimeProgress: 65,
  confirmRate: 92,
  clearRate: 78,
  falseRate: 5.2
})

// 最新告警
const latestAlarms = ref([])

// 图表实例
let trendChart = null
let distributionChart = null
let deviceRankingChart = null
let typeAnalysisChart = null

// 图表DOM引用
const trendChartRef = ref(null)
const distributionChartRef = ref(null)
const deviceRankingRef = ref(null)
const typeAnalysisRef = ref(null)

// 自动刷新定时器
let refreshTimer = null

// 获取趋势类名
const getTrendClass = (trend) => {
  return trend > 0 ? 'trend-up' : 'trend-down'
}

// 获取趋势图标
const getTrendIcon = (trend) => {
  return trend > 0 ? trendIcons.value.Top : trendIcons.value.Bottom
}

// 获取级别类型
const getLevelType = (level) => {
  const typeMap = {
    critical: 'danger',
    major: 'warning',
    minor: 'info',
    hint: 'info'
  }
  return typeMap[level] || 'info'
}

// 获取级别标签
const getLevelLabel = (level) => {
  const labelMap = {
    critical: t('alarmManagement.critical'),
    major: t('alarmManagement.major'),
    minor: t('alarmManagement.minor'),
    hint: t('alarmManagement.hint')
  }
  return labelMap[level] || level
}

// 获取状态类型
const getStatusType = (status) => {
  const typeMap = {
    active: 'danger',
    confirmed: 'warning',
    cleared: 'success'
  }
  return typeMap[status] || 'info'
}

// 获取状态标签
const getStatusLabel = (status) => {
  const labelMap = {
    active: t('alarmManagement.active'),
    confirmed: t('alarmManagement.confirmed'),
    cleared: t('alarmManagement.cleared')
  }
  return labelMap[status] || status
}

// 格式化时间
const formatTime = (time) => {
  return dayjs(time).format('YYYY-MM-DD HH:mm:ss')
}

// 处理时间范围变化
const handleTimeRangeChange = () => {
  loadData()
}

// 刷新数据
const handleRefresh = () => {
  ElMessage.info(t('alarmOverview.refreshing'))
  loadData()
}

// 查看告警详情
const handleViewAlarm = (alarm) => {
  router.push({ 
    name: 'AlarmManagement',
    query: { alarmId: alarm.id }
  })
}

// 跳转到告警管理
const gotoAlarmManagement = () => {
  router.push({ name: 'AlarmManagement' })
}

// 初始化趋势图
const initTrendChart = () => {
  trendChart = echarts.init(trendChartRef.value)
  
  const option = {
    tooltip: {
      trigger: 'axis',
      axisPointer: {
        type: 'shadow'
      }
    },
    legend: {
      data: [
        t('alarmManagement.critical'),
        t('alarmManagement.major'),
        t('alarmManagement.minor'),
        t('alarmManagement.hint')
      ]
    },
    grid: {
      left: '3%',
      right: '4%',
      bottom: '3%',
      containLabel: true
    },
    xAxis: {
      type: 'category',
      data: generateTimeLabels()
    },
    yAxis: {
      type: 'value',
      name: t('alarmOverview.alarmCount')
    },
    series: [
      {
        name: t('alarmManagement.critical'),
        type: 'bar',
        stack: 'total',
        data: generateRandomData(7, 5, 15),
        itemStyle: { color: '#F56C6C' }
      },
      {
        name: t('alarmManagement.major'),
        type: 'bar',
        stack: 'total',
        data: generateRandomData(7, 10, 30),
        itemStyle: { color: '#E6A23C' }
      },
      {
        name: t('alarmManagement.minor'),
        type: 'bar',
        stack: 'total',
        data: generateRandomData(7, 15, 50),
        itemStyle: { color: '#409EFF' }
      },
      {
        name: t('alarmManagement.hint'),
        type: 'bar',
        stack: 'total',
        data: generateRandomData(7, 20, 70),
        itemStyle: { color: '#909399' }
      }
    ]
  }
  
  trendChart.setOption(option)
}

// 初始化分布饼图
const initDistributionChart = () => {
  distributionChart = echarts.init(distributionChartRef.value)
  
  const option = {
    tooltip: {
      trigger: 'item',
      formatter: '{a} <br/>{b}: {c} ({d}%)'
    },
    legend: {
      orient: 'vertical',
      left: 'left'
    },
    series: [
      {
        name: t('alarmOverview.alarmDistribution'),
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
            fontSize: 20,
            fontWeight: 'bold'
          }
        },
        labelLine: {
          show: false
        },
        data: [
          { value: stats.value.critical, name: t('alarmManagement.critical'), itemStyle: { color: '#F56C6C' }},
          { value: stats.value.major, name: t('alarmManagement.major'), itemStyle: { color: '#E6A23C' }},
          { value: stats.value.minor, name: t('alarmManagement.minor'), itemStyle: { color: '#409EFF' }},
          { value: stats.value.hint, name: t('alarmManagement.hint'), itemStyle: { color: '#909399' }}
        ]
      }
    ]
  }
  
  distributionChart.setOption(option)
}

// 初始化设备排行图
const initDeviceRankingChart = () => {
  deviceRankingChart = echarts.init(deviceRankingRef.value)
  
  const devices = [
    'PLC Controller 1', 'Power Meter 2', 'Temperature Sensor 3',
    'Pressure Sensor 1', 'Flow Meter 1', 'PLC Controller 2',
    'Valve Actuator 1', 'Motor Drive 1', 'Level Sensor 1', 'PLC Controller 3'
  ]
  
  const option = {
    tooltip: {
      trigger: 'axis',
      axisPointer: {
        type: 'shadow'
      }
    },
    grid: {
      left: '3%',
      right: '4%',
      bottom: '3%',
      containLabel: true
    },
    xAxis: {
      type: 'value',
      name: t('alarmOverview.alarmCount')
    },
    yAxis: {
      type: 'category',
      data: devices.slice(0, 10).reverse()
    },
    series: [
      {
        type: 'bar',
        data: generateRandomData(10, 20, 100).reverse(),
        itemStyle: {
          color: new echarts.graphic.LinearGradient(0, 0, 1, 0, [
            { offset: 0, color: '#409EFF' },
            { offset: 1, color: '#67C23A' }
          ])
        }
      }
    ]
  }
  
  deviceRankingChart.setOption(option)
}

// 初始化类型分析图
const initTypeAnalysisChart = () => {
  typeAnalysisChart = echarts.init(typeAnalysisRef.value)
  
  const option = {
    tooltip: {
      trigger: 'item',
      formatter: '{a} <br/>{b}: {c} ({d}%)'
    },
    series: [
      {
        name: t('alarmOverview.alarmType'),
        type: 'pie',
        radius: '70%',
        data: [
          { value: 135, name: t('alarmOverview.limitAlarm') },
          { value: 98, name: t('alarmOverview.rateAlarm') },
          { value: 76, name: t('alarmOverview.statusAlarm') },
          { value: 54, name: t('alarmOverview.qualityAlarm') },
          { value: 32, name: t('alarmOverview.complexAlarm') }
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
  
  typeAnalysisChart.setOption(option)
}

// 生成时间标签
const generateTimeLabels = () => {
  const labels = []
  if (trendPeriod.value === 'day') {
    for (let i = 23; i >= 0; i--) {
      labels.push(`${i}:00`)
    }
  } else if (trendPeriod.value === 'week') {
    for (let i = 6; i >= 0; i--) {
      labels.push(dayjs().subtract(i, 'day').format('MM-DD'))
    }
  } else {
    for (let i = 29; i >= 0; i--) {
      labels.push(dayjs().subtract(i, 'day').format('MM-DD'))
    }
  }
  return labels
}

// 生成随机数据
const generateRandomData = (count, min, max) => {
  return Array.from({ length: count }, () => 
    Math.floor(Math.random() * (max - min + 1)) + min
  )
}

// 更新趋势图
const updateTrendChart = () => {
  if (trendChart) {
    const option = trendChart.getOption()
    option.xAxis[0].data = generateTimeLabels()
    
    const dataLength = trendPeriod.value === 'day' ? 24 : 
                      trendPeriod.value === 'week' ? 7 : 30
    
    option.series[0].data = generateRandomData(dataLength, 5, 15)
    option.series[1].data = generateRandomData(dataLength, 10, 30)
    option.series[2].data = generateRandomData(dataLength, 15, 50)
    option.series[3].data = generateRandomData(dataLength, 20, 70)
    
    trendChart.setOption(option)
  }
}

// 加载数据
const loadData = async () => {
  // 生成最新告警数据
  const levels = ['critical', 'major', 'minor', 'hint']
  const sources = ['PLC-001', 'METER-002', 'SENSOR-003', 'ACTUATOR-004', 'PLC-005']
  const messages = [
    'Temperature exceeds limit',
    'Communication timeout',
    'Power factor below threshold',
    'Device offline',
    'Data quality issue'
  ]
  const statuses = ['active', 'confirmed', 'cleared']
  
  latestAlarms.value = Array.from({ length: 10 }, (_, i) => ({
    id: `ALARM${String(i + 1).padStart(5, '0')}`,
    time: new Date(Date.now() - Math.random() * 3600000),
    level: levels[Math.floor(Math.random() * levels.length)],
    source: sources[Math.floor(Math.random() * sources.length)],
    message: messages[Math.floor(Math.random() * messages.length)],
    status: statuses[Math.floor(Math.random() * statuses.length)]
  })).sort((a, b) => b.time - a.time)
  
  // 更新图表
  if (distributionChart) {
    const option = distributionChart.getOption()
    option.series[0].data = [
      { value: stats.value.critical, name: t('alarmManagement.critical'), itemStyle: { color: '#F56C6C' }},
      { value: stats.value.major, name: t('alarmManagement.major'), itemStyle: { color: '#E6A23C' }},
      { value: stats.value.minor, name: t('alarmManagement.minor'), itemStyle: { color: '#409EFF' }},
      { value: stats.value.hint, name: t('alarmManagement.hint'), itemStyle: { color: '#909399' }}
    ]
    distributionChart.setOption(option)
  }
  
  ElMessage.success(t('common.refreshSuccess'))
}

// 处理窗口大小变化
const handleResize = () => {
  trendChart?.resize()
  distributionChart?.resize()
  deviceRankingChart?.resize()
  typeAnalysisChart?.resize()
}

// 启动自动刷新
const startAutoRefresh = () => {
  refreshTimer = setInterval(() => {
    // 更新统计数据
    stats.value.critical = Math.max(0, stats.value.critical + Math.floor(Math.random() * 5 - 2))
    stats.value.major = Math.max(0, stats.value.major + Math.floor(Math.random() * 7 - 3))
    stats.value.minor = Math.max(0, stats.value.minor + Math.floor(Math.random() * 10 - 5))
    stats.value.hint = Math.max(0, stats.value.hint + Math.floor(Math.random() * 15 - 7))
    
    // 更新分布图
    if (distributionChart) {
      const option = distributionChart.getOption()
      option.series[0].data = [
        { value: stats.value.critical, name: t('alarmManagement.critical'), itemStyle: { color: '#F56C6C' }},
        { value: stats.value.major, name: t('alarmManagement.major'), itemStyle: { color: '#E6A23C' }},
        { value: stats.value.minor, name: t('alarmManagement.minor'), itemStyle: { color: '#409EFF' }},
        { value: stats.value.hint, name: t('alarmManagement.hint'), itemStyle: { color: '#909399' }}
      ]
      distributionChart.setOption(option)
    }
  }, 10000)
}

onMounted(async () => {
  await nextTick()
  
  // 初始化图表
  initTrendChart()
  initDistributionChart()
  initDeviceRankingChart()
  initTypeAnalysisChart()
  
  // 加载数据
  loadData()
  
  // 启动自动刷新
  startAutoRefresh()
  
  // 监听窗口大小变化
  window.addEventListener('resize', handleResize)
})

onUnmounted(() => {
  // 销毁图表
  trendChart?.dispose()
  distributionChart?.dispose()
  deviceRankingChart?.dispose()
  typeAnalysisChart?.dispose()
  
  // 清除定时器
  if (refreshTimer) {
    clearInterval(refreshTimer)
  }
  
  // 移除事件监听
  window.removeEventListener('resize', handleResize)
})
</script>

<style lang="scss" scoped>
@import '@/styles/design-tokens.scss';

.alarm-overview-container {
  min-height: 100%;
}

// Apple 风格页面头部
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--space-8);
  flex-wrap: wrap;
  gap: var(--space-4);
  
  h1 {
    margin: 0;
    font-size: var(--font-size-4xl);
    font-weight: var(--font-weight-bold);
    color: var(--color-text-primary);
    letter-spacing: -0.02em;
  }
  
  .header-actions {
    display: flex;
    gap: var(--space-3);
    align-items: center;
    flex-wrap: wrap;
    
    :deep(.el-date-editor) {
      .el-input__wrapper {
        background: var(--color-background-elevated);
        border: 1px solid var(--color-border-light);
        border-radius: var(--radius-lg);
        box-shadow: none;
        transition: all var(--duration-fast) var(--ease-in-out);
        
        &:hover {
          border-color: var(--color-border);
        }
        
        &.is-focus {
          border-color: var(--color-primary);
          box-shadow: 0 0 0 3px var(--color-primary-light);
        }
      }
    }
    
    .el-button {
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
        transform: translateY(-1px);
        box-shadow: var(--shadow-md);
      }
    }
  }
}

// Tesla 风格统计卡片
.alarm-stats {
  margin-bottom: var(--space-8);
  
  .stat-card {
    height: 140px;
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
    
    &.critical {
      color: var(--color-danger);
      
      .stat-icon {
        background: var(--color-danger-light);
        color: var(--color-danger);
      }
    }
    
    &.major {
      color: var(--color-warning);
      
      .stat-icon {
        background: var(--color-warning-light);
        color: var(--color-warning);
      }
    }
    
    &.minor {
      color: var(--color-primary);
      
      .stat-icon {
        background: var(--color-primary-light);
        color: var(--color-primary);
      }
    }
    
    &.hint {
      color: var(--color-text-secondary);
      
      .stat-icon {
        background: var(--color-gray-100);
        color: var(--color-text-secondary);
      }
    }
    
    :deep(.el-card__body) {
      height: 100%;
      padding: var(--space-5);
    }
    
    .stat-content {
      display: flex;
      align-items: center;
      gap: var(--space-4);
      height: 100%;
      
      .stat-icon {
        width: 56px;
        height: 56px;
        border-radius: var(--radius-xl);
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 28px;
        transition: all var(--duration-fast) var(--ease-in-out);
      }
      
      .stat-info {
        flex: 1;
        
        .stat-value {
          font-size: var(--font-size-4xl);
          font-weight: var(--font-weight-bold);
          line-height: 1;
          margin-bottom: var(--space-1);
          letter-spacing: -0.02em;
        }
        
        .stat-label {
          font-size: var(--font-size-sm);
          color: var(--color-text-tertiary);
          margin-bottom: var(--space-2);
          font-weight: var(--font-weight-medium);
        }
        
        .stat-trend {
          display: flex;
          align-items: center;
          font-size: var(--font-size-xs);
          gap: var(--space-1);
          
          .trend-up,
          .trend-down {
            display: flex;
            align-items: center;
            font-weight: var(--font-weight-semibold);
            
            .el-icon {
              font-size: 16px;
            }
          }
          
          .trend-up {
            color: var(--color-danger);
          }
          
          .trend-down {
            color: var(--color-success);
          }
          
          .vs {
            color: var(--color-text-tertiary);
          }
        }
      }
    }
  }
}

// 图表行
.charts-row {
  margin-bottom: var(--space-8);
}

// 图表卡片
.chart-card {
  height: 100%;
  background: var(--color-background-elevated);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-xs);
  
  :deep(.el-card__header) {
    padding: var(--space-5) var(--space-6);
    border-bottom: 1px solid var(--color-border-light);
    background: var(--color-gray-50);
  }
  
  :deep(.el-card__body) {
    padding: var(--space-6);
  }
  
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    
    > span {
      font-size: var(--font-size-lg);
      font-weight: var(--font-weight-semibold);
      color: var(--color-text-primary);
    }
    
    .el-radio-group {
      :deep(.el-radio-button__inner) {
        padding: var(--space-1) var(--space-3);
        border: none;
        background: var(--color-gray-100);
        color: var(--color-text-secondary);
        font-weight: var(--font-weight-medium);
        font-size: var(--font-size-sm);
        height: 28px;
        line-height: 26px;
        
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
  
  .chart-container {
    height: 350px;
    padding: var(--space-2) 0;
  }
}

// 告警列表卡片
.alarm-list-card {
  margin-bottom: var(--space-8);
  background: var(--color-background-elevated);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-xs);
  
  :deep(.el-card__header) {
    padding: var(--space-5) var(--space-6);
    border-bottom: 1px solid var(--color-border-light);
    background: var(--color-gray-50);
  }
  
  :deep(.el-card__body) {
    padding: var(--space-6);
  }
  
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    
    > span {
      font-size: var(--font-size-lg);
      font-weight: var(--font-weight-semibold);
      color: var(--color-text-primary);
    }
  }
  
  // 表格样式优化
  :deep(.el-table) {
    font-size: var(--font-size-sm);
    
    .el-table__header {
      th {
        background: var(--color-gray-50);
        color: var(--color-text-secondary);
        font-weight: var(--font-weight-semibold);
        text-transform: uppercase;
        font-size: var(--font-size-xs);
        letter-spacing: 0.05em;
      }
    }
    
    .el-table__row {
      &:hover {
        background: var(--color-gray-50);
      }
    }
  }
}

// 响应统计
.response-stats {
  background: var(--color-background-elevated);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-xs);
  
  :deep(.el-card__body) {
    padding: var(--space-6);
  }
  
  .response-stat {
    text-align: center;
    
    .stat-title {
      font-size: var(--font-size-sm);
      color: var(--color-text-tertiary);
      margin-bottom: var(--space-3);
      font-weight: var(--font-weight-medium);
    }
    
    .stat-value {
      font-size: var(--font-size-2xl);
      font-weight: var(--font-weight-bold);
      margin-bottom: var(--space-3);
      letter-spacing: -0.02em;
      
      &.primary {
        color: var(--color-primary);
      }
      
      &.success {
        color: var(--color-success);
      }
      
      &.warning {
        color: var(--color-warning);
      }
      
      &.danger {
        color: var(--color-danger);
      }
    }
    
    :deep(.el-progress) {
      .el-progress__text {
        font-size: var(--font-size-xs);
        font-weight: var(--font-weight-medium);
      }
    }
  }
}

// 响应式布局
@media (max-width: 768px) {
  .page-header {
    h1 {
      font-size: var(--font-size-3xl);
    }
    
    .header-actions {
      width: 100%;
      
      :deep(.el-date-editor) {
        width: 100%;
      }
    }
  }
  
  .alarm-stats {
    .el-col {
      margin-bottom: var(--space-4);
    }
  }
  
  .chart-container {
    height: 300px !important;
  }
}
</style>