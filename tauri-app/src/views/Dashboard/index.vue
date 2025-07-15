<template>
  <div class="dashboard">
    <!-- Dashboard Header -->
    <div class="dashboard-header">
      <h2>Energy Management Dashboard</h2>
      <el-space>
        <el-date-picker
          v-model="selectedDate"
          type="date"
          placeholder="Select date"
          format="YYYY-MM-DD"
          value-format="YYYY-MM-DD"
        />
        <el-button @click="refreshDashboard" :loading="refreshing">
          <el-icon><Refresh /></el-icon>
          Refresh
        </el-button>
        <el-button @click="toggleFullscreen">
          <el-icon><FullScreen /></el-icon>
          Fullscreen
        </el-button>
        <el-button @click="exportDashboard">
          <el-icon><Download /></el-icon>
          Export
        </el-button>
      </el-space>
    </div>
    
    <!-- KPI Cards -->
    <el-row :gutter="20" class="kpi-row">
      <el-col :span="6">
        <el-card class="kpi-card">
          <div class="kpi-content">
            <div class="kpi-icon" style="background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);">
              <el-icon size="30"><Lightning /></el-icon>
            </div>
            <div class="kpi-info">
              <div class="kpi-value">{{ totalPower.toFixed(1) }}</div>
              <div class="kpi-label">Total Power (MW)</div>
              <div class="kpi-trend">
                <el-icon :color="totalPowerTrend > 0 ? '#67C23A' : '#F56C6C'">
                  <TrendCharts />
                </el-icon>
                <span :style="{ color: totalPowerTrend > 0 ? '#67C23A' : '#F56C6C' }">
                  {{ Math.abs(totalPowerTrend).toFixed(1) }}%
                </span>
              </div>
            </div>
          </div>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card class="kpi-card">
          <div class="kpi-content">
            <div class="kpi-icon" style="background: linear-gradient(135deg, #f093fb 0%, #f5576c 100%);">
              <el-icon size="30"><Sunny /></el-icon>
            </div>
            <div class="kpi-info">
              <div class="kpi-value">{{ solarGeneration.toFixed(1) }}</div>
              <div class="kpi-label">Solar Generation (MW)</div>
              <div class="kpi-trend">
                <el-icon color="#67C23A"><TrendCharts /></el-icon>
                <span style="color: #67C23A">+12.5%</span>
              </div>
            </div>
          </div>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card class="kpi-card">
          <div class="kpi-content">
            <div class="kpi-icon" style="background: linear-gradient(135deg, #4facfe 0%, #00f2fe 100%);">
              <el-icon size="30"><Box /></el-icon>
            </div>
            <div class="kpi-info">
              <div class="kpi-value">{{ batterySOC.toFixed(0) }}</div>
              <div class="kpi-label">Battery SOC (%)</div>
              <el-progress 
                :percentage="batterySOC" 
                :show-text="false"
                :stroke-width="4"
                :color="getBatteryColor(batterySOC)"
              />
            </div>
          </div>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card class="kpi-card">
          <div class="kpi-content">
            <div class="kpi-icon" style="background: linear-gradient(135deg, #fa709a 0%, #fee140 100%);">
              <el-icon size="30"><PriceTag /></el-icon>
            </div>
            <div class="kpi-info">
              <div class="kpi-value">${{ energyCost.toFixed(2) }}</div>
              <div class="kpi-label">Energy Cost/kWh</div>
              <div class="kpi-trend">
                <el-icon color="#F56C6C"><TrendCharts /></el-icon>
                <span style="color: #F56C6C">+5.2%</span>
              </div>
            </div>
          </div>
        </el-card>
      </el-col>
    </el-row>
    
    <!-- Charts Row 1 -->
    <el-row :gutter="20" class="chart-row">
      <el-col :span="16">
        <el-card>
          <template #header>
            <div class="card-header">
              <span>Power Generation Overview</span>
              <el-radio-group v-model="powerChartType" size="small">
                <el-radio-button label="line">Line</el-radio-button>
                <el-radio-button label="area">Area</el-radio-button>
                <el-radio-button label="stack">Stack</el-radio-button>
              </el-radio-group>
            </div>
          </template>
          <div ref="powerChartContainer" class="chart-container-large"></div>
        </el-card>
      </el-col>
      
      <el-col :span="8">
        <el-card>
          <template #header>
            <span>Energy Sources Distribution</span>
          </template>
          <div ref="energyPieContainer" class="chart-container-large"></div>
        </el-card>
      </el-col>
    </el-row>
    
    <!-- Charts Row 2 -->
    <el-row :gutter="20" class="chart-row">
      <el-col :span="8">
        <el-card>
          <template #header>
            <span>Load Profile</span>
          </template>
          <div ref="loadProfileContainer" class="chart-container-medium"></div>
        </el-card>
      </el-col>
      
      <el-col :span="8">
        <el-card>
          <template #header>
            <span>System Efficiency</span>
          </template>
          <div ref="efficiencyGaugeContainer" class="chart-container-medium"></div>
        </el-card>
      </el-col>
      
      <el-col :span="8">
        <el-card>
          <template #header>
            <span>Alarm Statistics</span>
          </template>
          <div ref="alarmStatsContainer" class="chart-container-medium"></div>
        </el-card>
      </el-col>
    </el-row>
    
    <!-- Real-time Monitoring -->
    <el-card class="realtime-card">
      <template #header>
        <div class="card-header">
          <span>Real-time Monitoring</span>
          <el-switch v-model="realtimeEnabled" active-text="Live" />
        </div>
      </template>
      
      <el-row :gutter="20">
        <el-col :span="6" v-for="meter in realtimeMeters" :key="meter.id">
          <div class="meter-card">
            <div class="meter-header">
              <span class="meter-name">{{ meter.name }}</span>
              <el-tag :type="meter.status === 'normal' ? 'success' : 'warning'" size="small">
                {{ meter.status }}
              </el-tag>
            </div>
            <div class="meter-value">
              <span class="value">{{ meter.value.toFixed(2) }}</span>
              <span class="unit">{{ meter.unit }}</span>
            </div>
            <el-progress 
              :percentage="(meter.value / meter.max) * 100" 
              :show-text="false"
              :color="getMeterColor(meter.value, meter.max)"
            />
            <div class="meter-info">
              <span>Min: {{ meter.min }}</span>
              <span>Max: {{ meter.max }}</span>
            </div>
          </div>
        </el-col>
      </el-row>
    </el-card>
    
    <!-- System Health -->
    <el-card>
      <template #header>
        <span>System Health Monitor</span>
      </template>
      
      <el-row :gutter="20">
        <el-col :span="6" v-for="service in systemServices" :key="service.name">
          <div class="service-health">
            <div class="service-icon" :style="{ background: getServiceColor(service.health) }">
              <el-icon size="24"><Monitor /></el-icon>
            </div>
            <div class="service-info">
              <div class="service-name">{{ service.name }}</div>
              <el-progress 
                :percentage="service.health" 
                :color="getHealthColor(service.health)"
                :format="() => `${service.health}%`"
              />
              <div class="service-stats">
                <span>CPU: {{ service.cpu }}%</span>
                <span>MEM: {{ service.memory }}MB</span>
              </div>
            </div>
          </div>
        </el-col>
      </el-row>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import {
  Refresh,
  FullScreen,
  Download,
  Lightning,
  Sunny,
  Box,
  PriceTag,
  TrendCharts,
  Monitor
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import * as echarts from 'echarts'
import dayjs from 'dayjs'

// Dashboard state
const selectedDate = ref(dayjs().format('YYYY-MM-DD'))
const refreshing = ref(false)
const realtimeEnabled = ref(true)

// KPI values
const totalPower = ref(85.2)
const totalPowerTrend = ref(8.5)
const solarGeneration = ref(32.5)
const batterySOC = ref(75)
const energyCost = ref(0.12)

// Chart settings
const powerChartType = ref('area')

// Charts
const powerChartContainer = ref<HTMLElement>()
const energyPieContainer = ref<HTMLElement>()
const loadProfileContainer = ref<HTMLElement>()
const efficiencyGaugeContainer = ref<HTMLElement>()
const alarmStatsContainer = ref<HTMLElement>()

const powerChart = ref<echarts.ECharts>()
const energyPieChart = ref<echarts.ECharts>()
const loadProfileChart = ref<echarts.ECharts>()
const efficiencyGaugeChart = ref<echarts.ECharts>()
const alarmStatsChart = ref<echarts.ECharts>()

// Real-time meters
const realtimeMeters = ref([
  {
    id: 1,
    name: 'Grid Voltage',
    value: 220.5,
    unit: 'V',
    min: 200,
    max: 240,
    status: 'normal'
  },
  {
    id: 2,
    name: 'Grid Current',
    value: 125.8,
    unit: 'A',
    min: 0,
    max: 200,
    status: 'normal'
  },
  {
    id: 3,
    name: 'Power Factor',
    value: 0.95,
    unit: '',
    min: 0.8,
    max: 1.0,
    status: 'normal'
  },
  {
    id: 4,
    name: 'Frequency',
    value: 50.02,
    unit: 'Hz',
    min: 49.5,
    max: 50.5,
    status: 'normal'
  }
])

// System services
const systemServices = ref([
  {
    name: 'API Gateway',
    health: 98,
    cpu: 12.5,
    memory: 256
  },
  {
    name: 'Communication',
    health: 100,
    cpu: 8.2,
    memory: 128
  },
  {
    name: 'Computation',
    health: 95,
    cpu: 25.8,
    memory: 512
  },
  {
    name: 'Historical',
    health: 92,
    cpu: 15.3,
    memory: 384
  }
])

// Update intervals
let updateInterval: number | null = null

onMounted(() => {
  initCharts()
  
  if (realtimeEnabled.value) {
    startRealtimeUpdates()
  }
  
  // Handle resize
  window.addEventListener('resize', handleResize)
})

onUnmounted(() => {
  stopRealtimeUpdates()
  
  // Dispose charts
  powerChart.value?.dispose()
  energyPieChart.value?.dispose()
  loadProfileChart.value?.dispose()
  efficiencyGaugeChart.value?.dispose()
  alarmStatsChart.value?.dispose()
  
  window.removeEventListener('resize', handleResize)
})

// Watch realtime toggle
watch(realtimeEnabled, (enabled) => {
  if (enabled) {
    startRealtimeUpdates()
  } else {
    stopRealtimeUpdates()
  }
})

// Watch chart type change
watch(powerChartType, () => {
  updatePowerChart()
})

// Methods
async function refreshDashboard() {
  refreshing.value = true
  
  try {
    // TODO: Fetch latest data from API
    await new Promise(resolve => setTimeout(resolve, 1000))
    
    // Update all charts
    updateAllCharts()
    
    ElMessage.success('Dashboard refreshed')
  } finally {
    refreshing.value = false
  }
}

function toggleFullscreen() {
  if (!document.fullscreenElement) {
    document.documentElement.requestFullscreen()
  } else {
    document.exitFullscreen()
  }
}

function exportDashboard() {
  // TODO: Implement dashboard export
  ElMessage.success('Exporting dashboard...')
}

function startRealtimeUpdates() {
  updateInterval = window.setInterval(() => {
    updateKPIs()
    updateRealtimeMeters()
    updateChartData()
  }, 2000)
}

function stopRealtimeUpdates() {
  if (updateInterval) {
    clearInterval(updateInterval)
    updateInterval = null
  }
}

function updateKPIs() {
  // Simulate KPI updates
  totalPower.value += (Math.random() - 0.5) * 2
  totalPowerTrend.value = (Math.random() - 0.5) * 20
  solarGeneration.value = Math.max(0, solarGeneration.value + (Math.random() - 0.5) * 1)
  batterySOC.value = Math.max(0, Math.min(100, batterySOC.value + (Math.random() - 0.5) * 2))
  energyCost.value = Math.max(0.08, Math.min(0.20, energyCost.value + (Math.random() - 0.5) * 0.01))
}

function updateRealtimeMeters() {
  realtimeMeters.value.forEach(meter => {
    // Simulate value changes
    const change = (Math.random() - 0.5) * (meter.max - meter.min) * 0.02
    meter.value = Math.max(meter.min, Math.min(meter.max, meter.value + change))
    
    // Update status
    const range = meter.max - meter.min
    const normalMin = meter.min + range * 0.1
    const normalMax = meter.max - range * 0.1
    meter.status = meter.value >= normalMin && meter.value <= normalMax ? 'normal' : 'warning'
  })
}

function updateChartData() {
  // Update power chart with new data point
  if (powerChart.value) {
    const option = powerChart.value.getOption() as any
    const now = new Date()
    
    option.series.forEach((series: any) => {
      series.data.push([
        now,
        Math.random() * 20 + 10
      ])
      
      // Keep only last 100 points
      if (series.data.length > 100) {
        series.data.shift()
      }
    })
    
    powerChart.value.setOption(option)
  }
}

// Chart initialization
function initCharts() {
  initPowerChart()
  initEnergyPieChart()
  initLoadProfileChart()
  initEfficiencyGaugeChart()
  initAlarmStatsChart()
}

function initPowerChart() {
  if (!powerChartContainer.value) return
  
  powerChart.value = echarts.init(powerChartContainer.value)
  updatePowerChart()
}

function updatePowerChart() {
  if (!powerChart.value) return
  
  const option = {
    tooltip: {
      trigger: 'axis',
      axisPointer: {
        type: 'cross'
      }
    },
    legend: {
      data: ['Solar', 'Grid', 'Battery', 'Diesel']
    },
    xAxis: {
      type: 'time',
      boundaryGap: false
    },
    yAxis: {
      type: 'value',
      name: 'Power (MW)',
      min: 0
    },
    series: [
      {
        name: 'Solar',
        type: powerChartType.value === 'stack' ? 'line' : powerChartType.value,
        smooth: true,
        stack: powerChartType.value === 'stack' ? 'total' : undefined,
        areaStyle: powerChartType.value !== 'line' ? {} : undefined,
        data: generateTimeSeriesData(30, 10, 40)
      },
      {
        name: 'Grid',
        type: powerChartType.value === 'stack' ? 'line' : powerChartType.value,
        smooth: true,
        stack: powerChartType.value === 'stack' ? 'total' : undefined,
        areaStyle: powerChartType.value !== 'line' ? {} : undefined,
        data: generateTimeSeriesData(20, 10, 30)
      },
      {
        name: 'Battery',
        type: powerChartType.value === 'stack' ? 'line' : powerChartType.value,
        smooth: true,
        stack: powerChartType.value === 'stack' ? 'total' : undefined,
        areaStyle: powerChartType.value !== 'line' ? {} : undefined,
        data: generateTimeSeriesData(10, -10, 20)
      },
      {
        name: 'Diesel',
        type: powerChartType.value === 'stack' ? 'line' : powerChartType.value,
        smooth: true,
        stack: powerChartType.value === 'stack' ? 'total' : undefined,
        areaStyle: powerChartType.value !== 'line' ? {} : undefined,
        data: generateTimeSeriesData(5, 0, 15)
      }
    ]
  }
  
  powerChart.value.setOption(option)
}

function initEnergyPieChart() {
  if (!energyPieContainer.value) return
  
  energyPieChart.value = echarts.init(energyPieContainer.value)
  
  const option = {
    tooltip: {
      trigger: 'item',
      formatter: '{b}: {c} MW ({d}%)'
    },
    legend: {
      bottom: '0',
      left: 'center'
    },
    series: [
      {
        name: 'Energy Sources',
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
          { value: 32.5, name: 'Solar', itemStyle: { color: '#f6ca57' } },
          { value: 25.8, name: 'Grid', itemStyle: { color: '#409eff' } },
          { value: 15.2, name: 'Battery', itemStyle: { color: '#67c23a' } },
          { value: 11.7, name: 'Diesel', itemStyle: { color: '#909399' } }
        ]
      }
    ]
  }
  
  energyPieChart.value.setOption(option)
}

function initLoadProfileChart() {
  if (!loadProfileContainer.value) return
  
  loadProfileChart.value = echarts.init(loadProfileContainer.value)
  
  const hours = Array.from({ length: 24 }, (_, i) => `${i}:00`)
  const loadData = [
    45, 42, 40, 38, 35, 32, 38, 45, 52, 58, 
    65, 70, 72, 75, 73, 70, 68, 72, 75, 70, 
    65, 58, 52, 48
  ]
  
  const option = {
    tooltip: {
      trigger: 'axis',
      formatter: '{b}: {c} MW'
    },
    xAxis: {
      type: 'category',
      data: hours,
      axisLabel: {
        interval: 3
      }
    },
    yAxis: {
      type: 'value',
      name: 'Load (MW)'
    },
    series: [
      {
        type: 'line',
        smooth: true,
        symbol: 'none',
        areaStyle: {
          color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
            { offset: 0, color: 'rgba(64, 158, 255, 0.5)' },
            { offset: 1, color: 'rgba(64, 158, 255, 0.1)' }
          ])
        },
        lineStyle: {
          color: '#409eff',
          width: 2
        },
        data: loadData
      }
    ]
  }
  
  loadProfileChart.value.setOption(option)
}

function initEfficiencyGaugeChart() {
  if (!efficiencyGaugeContainer.value) return
  
  efficiencyGaugeChart.value = echarts.init(efficiencyGaugeContainer.value)
  
  const option = {
    series: [
      {
        type: 'gauge',
        startAngle: 180,
        endAngle: 0,
        min: 0,
        max: 100,
        splitNumber: 10,
        radius: '90%',
        center: ['50%', '70%'],
        axisLine: {
          lineStyle: {
            width: 20,
            color: [
              [0.6, '#F56C6C'],
              [0.8, '#E6A23C'],
              [1, '#67C23A']
            ]
          }
        },
        pointer: {
          itemStyle: {
            color: 'inherit'
          }
        },
        axisTick: {
          distance: -30,
          length: 8,
          lineStyle: {
            color: '#fff',
            width: 2
          }
        },
        splitLine: {
          distance: -30,
          length: 30,
          lineStyle: {
            color: '#fff',
            width: 4
          }
        },
        axisLabel: {
          color: 'inherit',
          distance: 40,
          fontSize: 12
        },
        detail: {
          valueAnimation: true,
          formatter: '{value}%',
          color: 'inherit',
          fontSize: 24,
          offsetCenter: [0, '-10%']
        },
        data: [
          {
            value: 88.5,
            name: 'Efficiency'
          }
        ]
      }
    ]
  }
  
  efficiencyGaugeChart.value.setOption(option)
}

function initAlarmStatsChart() {
  if (!alarmStatsContainer.value) return
  
  alarmStatsChart.value = echarts.init(alarmStatsContainer.value)
  
  const option = {
    tooltip: {
      trigger: 'axis',
      axisPointer: {
        type: 'shadow'
      }
    },
    legend: {
      data: ['Critical', 'Major', 'Minor', 'Warning']
    },
    xAxis: {
      type: 'category',
      data: ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']
    },
    yAxis: {
      type: 'value',
      name: 'Count'
    },
    series: [
      {
        name: 'Critical',
        type: 'bar',
        stack: 'total',
        itemStyle: { color: '#F56C6C' },
        data: [2, 1, 0, 1, 0, 1, 0]
      },
      {
        name: 'Major',
        type: 'bar',
        stack: 'total',
        itemStyle: { color: '#E6A23C' },
        data: [5, 3, 4, 2, 3, 2, 1]
      },
      {
        name: 'Minor',
        type: 'bar',
        stack: 'total',
        itemStyle: { color: '#F6CA57' },
        data: [8, 6, 7, 5, 6, 4, 3]
      },
      {
        name: 'Warning',
        type: 'bar',
        stack: 'total',
        itemStyle: { color: '#909399' },
        data: [12, 10, 8, 9, 11, 7, 5]
      }
    ]
  }
  
  alarmStatsChart.value.setOption(option)
}

function updateAllCharts() {
  updatePowerChart()
  // Update other charts as needed
}

function handleResize() {
  powerChart.value?.resize()
  energyPieChart.value?.resize()
  loadProfileChart.value?.resize()
  efficiencyGaugeChart.value?.resize()
  alarmStatsChart.value?.resize()
}

// Utility functions
function generateTimeSeriesData(baseValue: number, minValue: number, maxValue: number) {
  const data = []
  const now = Date.now()
  
  for (let i = 99; i >= 0; i--) {
    const time = now - i * 60000 // 1 minute intervals
    const value = baseValue + (Math.random() - 0.5) * (maxValue - minValue)
    data.push([time, Math.max(minValue, Math.min(maxValue, value))])
  }
  
  return data
}

function getBatteryColor(soc: number) {
  if (soc < 20) return '#F56C6C'
  if (soc < 50) return '#E6A23C'
  return '#67C23A'
}

function getMeterColor(value: number, max: number) {
  const percentage = (value / max) * 100
  if (percentage > 90) return '#F56C6C'
  if (percentage > 80) return '#E6A23C'
  return '#67C23A'
}

function getHealthColor(health: number) {
  if (health >= 90) return '#67C23A'
  if (health >= 70) return '#E6A23C'
  return '#F56C6C'
}

function getServiceColor(health: number) {
  if (health >= 90) return 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)'
  if (health >= 70) return 'linear-gradient(135deg, #f093fb 0%, #f5576c 100%)'
  return 'linear-gradient(135deg, #F56C6C 0%, #E6A23C 100%)'
}
</script>

<style lang="scss" scoped>
.dashboard {
  height: 100%;
  overflow-y: auto;
  padding: 20px;
  background: #f5f7fa;
  
  .dashboard-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 20px;
    
    h2 {
      margin: 0;
      font-size: 24px;
      font-weight: 600;
    }
  }
  
  .kpi-row {
    margin-bottom: 20px;
    
    .kpi-card {
      height: 120px;
      
      :deep(.el-card__body) {
        height: 100%;
        padding: 20px;
      }
      
      .kpi-content {
        display: flex;
        align-items: center;
        height: 100%;
        gap: 20px;
        
        .kpi-icon {
          width: 60px;
          height: 60px;
          border-radius: 12px;
          display: flex;
          align-items: center;
          justify-content: center;
          color: white;
          flex-shrink: 0;
        }
        
        .kpi-info {
          flex: 1;
          
          .kpi-value {
            font-size: 28px;
            font-weight: 600;
            color: #303133;
            line-height: 1;
            margin-bottom: 5px;
          }
          
          .kpi-label {
            font-size: 14px;
            color: #909399;
            margin-bottom: 8px;
          }
          
          .kpi-trend {
            display: flex;
            align-items: center;
            gap: 5px;
            font-size: 14px;
          }
        }
      }
    }
  }
  
  .chart-row {
    margin-bottom: 20px;
  }
  
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  
  .chart-container-large {
    height: 400px;
  }
  
  .chart-container-medium {
    height: 300px;
  }
  
  .realtime-card {
    margin-bottom: 20px;
    
    .meter-card {
      background: #f5f7fa;
      border-radius: 8px;
      padding: 15px;
      
      .meter-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 10px;
        
        .meter-name {
          font-weight: 500;
          color: #606266;
        }
      }
      
      .meter-value {
        margin-bottom: 10px;
        
        .value {
          font-size: 24px;
          font-weight: 600;
          color: #303133;
        }
        
        .unit {
          font-size: 14px;
          color: #909399;
          margin-left: 5px;
        }
      }
      
      .meter-info {
        display: flex;
        justify-content: space-between;
        margin-top: 10px;
        font-size: 12px;
        color: #909399;
      }
    }
  }
  
  .service-health {
    display: flex;
    align-items: center;
    gap: 15px;
    padding: 15px;
    background: #f5f7fa;
    border-radius: 8px;
    
    .service-icon {
      width: 50px;
      height: 50px;
      border-radius: 10px;
      display: flex;
      align-items: center;
      justify-content: center;
      color: white;
      flex-shrink: 0;
    }
    
    .service-info {
      flex: 1;
      
      .service-name {
        font-weight: 500;
        margin-bottom: 5px;
      }
      
      .service-stats {
        display: flex;
        gap: 10px;
        margin-top: 5px;
        font-size: 12px;
        color: #909399;
      }
    }
  }
}
</style>