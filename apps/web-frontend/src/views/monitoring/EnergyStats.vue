<template>
  <div class="energy-stats-container">
    <!-- 页面头部 -->
    <div class="page-header">
      <h1>{{ $t('energyStats.title') }}</h1>
      <div class="header-actions">
        <el-date-picker
          v-model="dateRange"
          type="daterange"
          :range-separator="$t('common.to')"
          :start-placeholder="$t('common.startTime')"
          :end-placeholder="$t('common.endTime')"
          format="YYYY-MM-DD"
          value-format="YYYY-MM-DD"
          @change="handleDateChange"
        />
        <el-button type="primary" @click="handleExport">
          <el-icon><Download /></el-icon>
          {{ $t('energyStats.exportReport') }}
        </el-button>
      </div>
    </div>

    <!-- 能耗概览 -->
    <div class="energy-overview">
      <el-row :gutter="20">
        <el-col :xs="24" :sm="12" :md="6">
          <el-card class="overview-card">
            <div class="card-content">
              <div class="card-icon electricity">
                <el-icon :size="32"><Lightning /></el-icon>
              </div>
              <div class="card-info">
                <div class="value">{{ overview.totalEnergy.toLocaleString() }}</div>
                <div class="label">{{ $t('energyStats.totalEnergy') }}</div>
                <div class="unit">kWh</div>
              </div>
            </div>
            <div class="card-footer">
              <span class="trend up">
                <el-icon><Top /></el-icon>
                {{ overview.energyTrend }}%
              </span>
              <span class="vs">{{ $t('energyStats.vsLastPeriod') }}</span>
            </div>
          </el-card>
        </el-col>
        <el-col :xs="24" :sm="12" :md="6">
          <el-card class="overview-card">
            <div class="card-content">
              <div class="card-icon power">
                <el-icon :size="32"><Odometer /></el-icon>
              </div>
              <div class="card-info">
                <div class="value">{{ overview.avgPower.toFixed(2) }}</div>
                <div class="label">{{ $t('energyStats.avgPower') }}</div>
                <div class="unit">kW</div>
              </div>
            </div>
            <div class="card-footer">
              <span class="trend down">
                <el-icon><Bottom /></el-icon>
                {{ overview.powerTrend }}%
              </span>
              <span class="vs">{{ $t('energyStats.vsLastPeriod') }}</span>
            </div>
          </el-card>
        </el-col>
        <el-col :xs="24" :sm="12" :md="6">
          <el-card class="overview-card">
            <div class="card-content">
              <div class="card-icon peak">
                <el-icon :size="32"><TrendCharts /></el-icon>
              </div>
              <div class="card-info">
                <div class="value">{{ overview.peakPower.toFixed(2) }}</div>
                <div class="label">{{ $t('energyStats.peakPower') }}</div>
                <div class="unit">kW</div>
              </div>
            </div>
            <div class="card-footer">
              <span class="time">{{ formatPeakTime(overview.peakTime) }}</span>
            </div>
          </el-card>
        </el-col>
        <el-col :xs="24" :sm="12" :md="6">
          <el-card class="overview-card">
            <div class="card-content">
              <div class="card-icon cost">
                <el-icon :size="32"><Coin /></el-icon>
              </div>
              <div class="card-info">
                <div class="value">{{ overview.totalCost.toLocaleString() }}</div>
                <div class="label">{{ $t('energyStats.totalCost') }}</div>
                <div class="unit">¥</div>
              </div>
            </div>
            <div class="card-footer">
              <span class="trend up">
                <el-icon><Top /></el-icon>
                {{ overview.costTrend }}%
              </span>
              <span class="vs">{{ $t('energyStats.vsLastPeriod') }}</span>
            </div>
          </el-card>
        </el-col>
      </el-row>
    </div>

    <!-- 图表区域 -->
    <el-row :gutter="20" class="charts-row">
      <!-- 能耗趋势图 -->
      <el-col :span="24">
        <el-card class="chart-card">
          <template #header>
            <div class="card-header">
              <span>{{ $t('energyStats.energyTrend') }}</span>
              <el-radio-group v-model="trendType" size="small" @change="updateTrendChart">
                <el-radio-button label="day">{{ $t('energyStats.daily') }}</el-radio-button>
                <el-radio-button label="hour">{{ $t('energyStats.hourly') }}</el-radio-button>
                <el-radio-button label="month">{{ $t('energyStats.monthly') }}</el-radio-button>
              </el-radio-group>
            </div>
          </template>
          <div ref="trendChartRef" class="chart-container"></div>
        </el-card>
      </el-col>
    </el-row>

    <el-row :gutter="20" class="charts-row">
      <!-- 分类能耗饼图 -->
      <el-col :xs="24" :md="12">
        <el-card class="chart-card">
          <template #header>
            <span>{{ $t('energyStats.energyByCategory') }}</span>
          </template>
          <div ref="categoryChartRef" class="chart-container"></div>
        </el-card>
      </el-col>

      <!-- 能效指标 -->
      <el-col :xs="24" :md="12">
        <el-card class="chart-card">
          <template #header>
            <span>{{ $t('energyStats.energyEfficiency') }}</span>
          </template>
          <div ref="efficiencyChartRef" class="chart-container"></div>
        </el-card>
      </el-col>
    </el-row>

    <el-row :gutter="20" class="charts-row">
      <!-- 设备能耗排行 -->
      <el-col :xs="24" :md="12">
        <el-card class="chart-card">
          <template #header>
            <div class="card-header">
              <span>{{ $t('energyStats.deviceRanking') }}</span>
              <el-switch 
                v-model="showTopConsumers" 
                :active-text="$t('energyStats.top10')"
                :inactive-text="$t('energyStats.bottom10')"
                @change="updateRankingChart"
              />
            </div>
          </template>
          <div ref="rankingChartRef" class="chart-container"></div>
        </el-card>
      </el-col>

      <!-- 对比分析 -->
      <el-col :xs="24" :md="12">
        <el-card class="chart-card">
          <template #header>
            <span>{{ $t('energyStats.comparison') }}</span>
          </template>
          <div ref="comparisonChartRef" class="chart-container"></div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 详细数据表格 -->
    <el-card class="data-table-card">
      <template #header>
        <div class="card-header">
          <span>{{ $t('energyStats.detailData') }}</span>
          <el-button type="primary" link @click="toggleTableExpand">
            <el-icon>
              <component :is="tableExpanded ? arrowIcons.ArrowUp : arrowIcons.ArrowDown" />
            </el-icon>
            {{ tableExpanded ? $t('common.collapse') : $t('common.expand') }}
          </el-button>
        </div>
      </template>
      <el-collapse-transition>
        <div v-show="tableExpanded">
          <el-table :data="tableData" stripe height="400">
            <el-table-column prop="date" :label="$t('energyStats.date')" width="150" />
            <el-table-column prop="deviceGroup" :label="$t('energyStats.deviceGroup')" width="150" />
            <el-table-column prop="energy" :label="$t('energyStats.energy') + ' (kWh)'" width="120" />
            <el-table-column prop="avgPower" :label="$t('energyStats.avgPower') + ' (kW)'" width="120" />
            <el-table-column prop="peakPower" :label="$t('energyStats.peakPower') + ' (kW)'" width="120" />
            <el-table-column prop="runTime" :label="$t('energyStats.runTime') + ' (h)'" width="100" />
            <el-table-column prop="cost" :label="$t('energyStats.cost') + ' (¥)'" width="100" />
            <el-table-column prop="efficiency" :label="$t('energyStats.efficiency')" width="100">
              <template #default="{ row }">
                <el-progress :percentage="row.efficiency" :color="getEfficiencyColor(row.efficiency)" />
              </template>
            </el-table-column>
          </el-table>
        </div>
      </el-collapse-transition>
    </el-card>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted, nextTick, shallowRef } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { 
  Download, Lightning, Odometer, TrendCharts, Coin,
  Top, Bottom, ArrowUp, ArrowDown
} from '@element-plus/icons-vue'
import * as echarts from 'echarts'
import dayjs from 'dayjs'

const { t } = useI18n()

// 图标组件引用
const arrowIcons = shallowRef({ ArrowUp, ArrowDown })

// 日期范围
const dateRange = ref([
  dayjs().subtract(7, 'day').format('YYYY-MM-DD'),
  dayjs().format('YYYY-MM-DD')
])

// 能耗概览数据
const overview = ref({
  totalEnergy: 12456.8,
  energyTrend: 5.2,
  avgPower: 245.6,
  powerTrend: -2.1,
  peakPower: 876.5,
  peakTime: new Date('2024-01-03 14:30:00'),
  totalCost: 8719.8,
  costTrend: 4.8
})

// 图表相关
const trendType = ref('day')
const showTopConsumers = ref(true)
const tableExpanded = ref(false)

// 图表实例
let trendChart = null
let categoryChart = null
let efficiencyChart = null
let rankingChart = null
let comparisonChart = null

// 图表DOM引用
const trendChartRef = ref(null)
const categoryChartRef = ref(null)
const efficiencyChartRef = ref(null)
const rankingChartRef = ref(null)
const comparisonChartRef = ref(null)

// 表格数据
const tableData = ref([])

// 格式化峰值时间
const formatPeakTime = (time) => {
  return dayjs(time).format('MM-DD HH:mm')
}

// 获取效率颜色
const getEfficiencyColor = (efficiency) => {
  if (efficiency >= 90) return '#67C23A'
  if (efficiency >= 70) return '#E6A23C'
  return '#F56C6C'
}

// 处理日期变化
const handleDateChange = () => {
  loadData()
}

// 导出报表
const handleExport = () => {
  ElMessage.success(t('energyStats.exportSuccess'))
}

// 切换表格展开
const toggleTableExpand = () => {
  tableExpanded.value = !tableExpanded.value
}

// 初始化趋势图
const initTrendChart = () => {
  trendChart = echarts.init(trendChartRef.value)
  
  const option = {
    tooltip: {
      trigger: 'axis',
      axisPointer: {
        type: 'cross'
      }
    },
    legend: {
      data: [t('energyStats.energy'), t('energyStats.power')]
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
    yAxis: [
      {
        type: 'value',
        name: t('energyStats.energy') + ' (kWh)',
        position: 'left'
      },
      {
        type: 'value',
        name: t('energyStats.power') + ' (kW)',
        position: 'right'
      }
    ],
    series: [
      {
        name: t('energyStats.energy'),
        type: 'bar',
        data: generateEnergyData(),
        itemStyle: {
          color: '#409EFF'
        }
      },
      {
        name: t('energyStats.power'),
        type: 'line',
        yAxisIndex: 1,
        data: generatePowerData(),
        smooth: true,
        itemStyle: {
          color: '#67C23A'
        }
      }
    ]
  }
  
  trendChart.setOption(option)
}

// 初始化分类饼图
const initCategoryChart = () => {
  categoryChart = echarts.init(categoryChartRef.value)
  
  const option = {
    tooltip: {
      trigger: 'item',
      formatter: '{a} <br/>{b}: {c} kWh ({d}%)'
    },
    legend: {
      orient: 'vertical',
      left: 'left'
    },
    series: [
      {
        name: t('energyStats.energyByCategory'),
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
          { value: 4235, name: t('energyStats.production') },
          { value: 2810, name: t('energyStats.lighting') },
          { value: 2348, name: t('energyStats.airConditioning') },
          { value: 1835, name: t('energyStats.office') },
          { value: 1228, name: t('energyStats.other') }
        ]
      }
    ]
  }
  
  categoryChart.setOption(option)
}

// 初始化能效图表
const initEfficiencyChart = () => {
  efficiencyChart = echarts.init(efficiencyChartRef.value)
  
  const option = {
    tooltip: {
      trigger: 'axis',
      axisPointer: {
        type: 'shadow'
      }
    },
    radar: {
      indicator: [
        { name: t('energyStats.loadRate'), max: 100 },
        { name: t('energyStats.powerFactor'), max: 100 },
        { name: t('energyStats.efficiency'), max: 100 },
        { name: t('energyStats.utilization'), max: 100 },
        { name: t('energyStats.stability'), max: 100 }
      ]
    },
    series: [
      {
        name: t('energyStats.energyEfficiency'),
        type: 'radar',
        data: [
          {
            value: [85, 92, 88, 78, 95],
            name: t('energyStats.currentPeriod'),
            areaStyle: {
              color: 'rgba(64, 158, 255, 0.3)'
            },
            lineStyle: {
              color: '#409EFF'
            }
          },
          {
            value: [82, 88, 85, 75, 92],
            name: t('energyStats.lastPeriod'),
            areaStyle: {
              color: 'rgba(103, 194, 58, 0.3)'
            },
            lineStyle: {
              color: '#67C23A'
            }
          }
        ]
      }
    ]
  }
  
  efficiencyChart.setOption(option)
}

// 初始化排行图表
const initRankingChart = () => {
  rankingChart = echarts.init(rankingChartRef.value)
  updateRankingChart()
}

// 更新排行图表
const updateRankingChart = () => {
  const data = showTopConsumers.value ? 
    generateTopConsumers() : generateBottomConsumers()
  
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
      name: t('energyStats.energy') + ' (kWh)'
    },
    yAxis: {
      type: 'category',
      data: data.map(item => item.name)
    },
    series: [
      {
        type: 'bar',
        data: data.map(item => ({
          value: item.value,
          itemStyle: {
            color: showTopConsumers.value ? '#F56C6C' : '#67C23A'
          }
        }))
      }
    ]
  }
  
  rankingChart.setOption(option)
}

// 初始化对比图表
const initComparisonChart = () => {
  comparisonChart = echarts.init(comparisonChartRef.value)
  
  const option = {
    tooltip: {
      trigger: 'axis',
      axisPointer: {
        type: 'shadow'
      }
    },
    legend: {
      data: [t('energyStats.thisWeek'), t('energyStats.lastWeek')]
    },
    grid: {
      left: '3%',
      right: '4%',
      bottom: '3%',
      containLabel: true
    },
    xAxis: {
      type: 'category',
      data: [
        t('energyStats.monday'),
        t('energyStats.tuesday'),
        t('energyStats.wednesday'),
        t('energyStats.thursday'),
        t('energyStats.friday'),
        t('energyStats.saturday'),
        t('energyStats.sunday')
      ]
    },
    yAxis: {
      type: 'value',
      name: t('energyStats.energy') + ' (kWh)'
    },
    series: [
      {
        name: t('energyStats.thisWeek'),
        type: 'bar',
        data: [1820, 1932, 1901, 1934, 2090, 1330, 1020],
        itemStyle: {
          color: '#409EFF'
        }
      },
      {
        name: t('energyStats.lastWeek'),
        type: 'bar',
        data: [1720, 1832, 1801, 1834, 1990, 1230, 920],
        itemStyle: {
          color: '#67C23A'
        }
      }
    ]
  }
  
  comparisonChart.setOption(option)
}

// 生成时间标签
const generateTimeLabels = () => {
  const labels = []
  if (trendType.value === 'hour') {
    for (let i = 0; i < 24; i++) {
      labels.push(`${i}:00`)
    }
  } else if (trendType.value === 'day') {
    for (let i = 6; i >= 0; i--) {
      labels.push(dayjs().subtract(i, 'day').format('MM-DD'))
    }
  } else {
    for (let i = 11; i >= 0; i--) {
      labels.push(dayjs().subtract(i, 'month').format('YYYY-MM'))
    }
  }
  return labels
}

// 生成能耗数据
const generateEnergyData = () => {
  const count = trendType.value === 'hour' ? 24 : 
                trendType.value === 'day' ? 7 : 12
  return Array.from({ length: count }, () => 
    Math.floor(Math.random() * 2000) + 1000
  )
}

// 生成功率数据
const generatePowerData = () => {
  const count = trendType.value === 'hour' ? 24 : 
                trendType.value === 'day' ? 7 : 12
  return Array.from({ length: count }, () => 
    Math.floor(Math.random() * 500) + 200
  )
}

// 生成高耗能设备
const generateTopConsumers = () => {
  const devices = [
    'Production Line 1', 'Air Compressor 1', 'Chiller Unit 1',
    'Production Line 2', 'Heating System', 'Air Compressor 2',
    'Elevator System', 'Production Line 3', 'Lighting System A',
    'Ventilation System'
  ]
  return devices.map(name => ({
    name,
    value: Math.floor(Math.random() * 1000) + 500
  })).sort((a, b) => b.value - a.value)
}

// 生成低耗能设备
const generateBottomConsumers = () => {
  const devices = [
    'Office Area A', 'Emergency Lighting', 'Security System',
    'Office Area B', 'Server Room UPS', 'Water Pumps',
    'Office Area C', 'Parking Lighting', 'Access Control',
    'Meeting Rooms'
  ]
  return devices.map(name => ({
    name,
    value: Math.floor(Math.random() * 100) + 20
  })).sort((a, b) => a.value - b.value)
}

// 更新趋势图
const updateTrendChart = () => {
  if (trendChart) {
    const option = trendChart.getOption()
    option.xAxis[0].data = generateTimeLabels()
    option.series[0].data = generateEnergyData()
    option.series[1].data = generatePowerData()
    trendChart.setOption(option)
  }
}

// 加载数据
const loadData = () => {
  // 生成表格数据
  const groups = ['Production Area', 'Office Area', 'Warehouse', 'Utility Room']
  tableData.value = Array.from({ length: 20 }, (_, i) => ({
    date: dayjs().subtract(i, 'day').format('YYYY-MM-DD'),
    deviceGroup: groups[i % groups.length],
    energy: Math.floor(Math.random() * 1000) + 500,
    avgPower: Math.floor(Math.random() * 200) + 100,
    peakPower: Math.floor(Math.random() * 300) + 200,
    runTime: Math.floor(Math.random() * 20) + 4,
    cost: Math.floor(Math.random() * 700) + 300,
    efficiency: Math.floor(Math.random() * 30) + 70
  }))
}

// 处理窗口大小变化
const handleResize = () => {
  trendChart?.resize()
  categoryChart?.resize()
  efficiencyChart?.resize()
  rankingChart?.resize()
  comparisonChart?.resize()
}

onMounted(async () => {
  await nextTick()
  
  // 初始化图表
  initTrendChart()
  initCategoryChart()
  initEfficiencyChart()
  initRankingChart()
  initComparisonChart()
  
  // 加载数据
  loadData()
  
  // 监听窗口大小变化
  window.addEventListener('resize', handleResize)
})

onUnmounted(() => {
  // 销毁图表
  trendChart?.dispose()
  categoryChart?.dispose()
  efficiencyChart?.dispose()
  rankingChart?.dispose()
  comparisonChart?.dispose()
  
  // 移除事件监听
  window.removeEventListener('resize', handleResize)
})
</script>

<style lang="scss" scoped>
@import '@/styles/design-tokens.scss';

.energy-stats-container {
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
      background: var(--color-primary);
      border-color: var(--color-primary);
      color: var(--color-text-inverse);
      border-radius: var(--radius-lg);
      font-weight: var(--font-weight-medium);
      transition: all var(--duration-fast) var(--ease-in-out);
      
      &:hover {
        background: var(--color-primary-hover);
        border-color: var(--color-primary-hover);
        transform: translateY(-1px);
        box-shadow: var(--shadow-md);
      }
    }
  }
}

// Tesla 风格能耗概览卡片
.energy-overview {
  margin-bottom: var(--space-8);
  
  .overview-card {
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
      opacity: 0.8;
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
    
    :deep(.el-card__body) {
      height: 100%;
      padding: var(--space-5);
    }
    
    .card-content {
      display: flex;
      align-items: center;
      margin-bottom: var(--space-4);
      
      .card-icon {
        width: 56px;
        height: 56px;
        border-radius: var(--radius-xl);
        display: flex;
        align-items: center;
        justify-content: center;
        margin-right: var(--space-4);
        font-size: 28px;
        backdrop-filter: blur(10px);
        -webkit-backdrop-filter: blur(10px);
        transition: all var(--duration-fast) var(--ease-in-out);
        
        &.electricity {
          background: rgba(var(--color-primary-rgb), 0.1);
          color: var(--color-primary);
        }
        
        &.power {
          background: rgba(var(--color-success-rgb), 0.1);
          color: var(--color-success);
        }
        
        &.peak {
          background: rgba(var(--color-warning-rgb), 0.1);
          color: var(--color-warning);
        }
        
        &.cost {
          background: rgba(var(--color-danger-rgb), 0.1);
          color: var(--color-danger);
        }
      }
      
      .card-info {
        flex: 1;
        
        .value {
          font-size: var(--font-size-3xl);
          font-weight: var(--font-weight-bold);
          line-height: 1.2;
          margin-bottom: var(--space-1);
          color: var(--color-text-primary);
          letter-spacing: -0.02em;
        }
        
        .label {
          font-size: var(--font-size-sm);
          color: var(--color-text-secondary);
          margin-bottom: var(--space-1);
          font-weight: var(--font-weight-medium);
        }
        
        .unit {
          font-size: var(--font-size-xs);
          color: var(--color-text-tertiary);
        }
      }
    }
    
    .card-footer {
      display: flex;
      align-items: center;
      font-size: var(--font-size-xs);
      
      .trend {
        display: flex;
        align-items: center;
        margin-right: var(--space-2);
        font-weight: var(--font-weight-semibold);
        
        &.up {
          color: var(--color-danger);
        }
        
        &.down {
          color: var(--color-success);
        }
        
        .el-icon {
          margin-right: var(--space-1);
          font-size: 16px;
        }
      }
      
      .vs, .time {
        color: var(--color-text-tertiary);
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
    background: var(--color-background-secondary);
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
        background: var(--color-background-secondary);
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

// 数据表格卡片
.data-table-card {
  background: var(--color-background-elevated);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-xs);
  
  :deep(.el-card__header) {
    padding: var(--space-5) var(--space-6);
    border-bottom: 1px solid var(--color-border-light);
    background: var(--color-background-secondary);
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
    
    .el-button {
      background: var(--color-background);
      border: 1px solid var(--color-border-light);
      border-radius: var(--radius-lg);
      color: var(--color-primary);
      font-weight: var(--font-weight-medium);
      font-size: var(--font-size-sm);
      
      &:hover {
        background: var(--color-primary);
        color: var(--color-text-inverse);
        border-color: var(--color-primary);
      }
    }
  }
  
  // 表格样式优化
  :deep(.el-table) {
    font-size: var(--font-size-sm);
    
    .el-table__header {
      th {
        background: var(--color-background-secondary);
        color: var(--color-text-secondary);
        font-weight: var(--font-weight-semibold);
        text-transform: uppercase;
        font-size: var(--font-size-xs);
        letter-spacing: 0.05em;
      }
    }
    
    .el-table__row {
      &:hover {
        background: var(--color-background-hover);
      }
    }
    
    .el-progress {
      width: 100px;
      
      :deep(.el-progress__text) {
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
  
  .energy-overview {
    .el-col {
      margin-bottom: var(--space-4);
    }
    
    .overview-card {
      height: 120px;
    }
  }
  
  .chart-container {
    height: 300px !important;
  }
}
</style>