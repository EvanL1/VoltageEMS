<template>
  <div class="realtime-chart">
    <!-- Point Selection -->
    <div class="chart-controls">
      <el-select 
        v-model="selectedPointIds" 
        multiple 
        placeholder="Select points to display"
        style="width: 400px"
        @change="updateChart"
      >
        <el-option
          v-for="point in availablePoints"
          :key="point.point_id"
          :label="`${point.point_id} - ${point.description || 'No description'}`"
          :value="point.point_id"
        />
      </el-select>
      
      <el-button-group>
        <el-button 
          :type="timeRange === 60 ? 'primary' : ''"
          @click="setTimeRange(60)"
        >
          1 Min
        </el-button>
        <el-button 
          :type="timeRange === 300 ? 'primary' : ''"
          @click="setTimeRange(300)"
        >
          5 Min
        </el-button>
        <el-button 
          :type="timeRange === 600 ? 'primary' : ''"
          @click="setTimeRange(600)"
        >
          10 Min
        </el-button>
      </el-button-group>
    </div>
    
    <!-- Chart Container -->
    <div ref="chartContainer" class="chart-container"></div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed } from 'vue'
import * as echarts from 'echarts'
import type { EChartsOption } from 'echarts'
import type { PointData } from '@/types/realtime'
import dayjs from 'dayjs'

const props = defineProps<{
  channelId: number
  points: PointData[]
}>()

// Local state
const chartContainer = ref<HTMLElement>()
const chart = ref<echarts.ECharts>()
const selectedPointIds = ref<number[]>([])
const timeRange = ref(300) // seconds
const dataHistory = ref<Map<number, Array<[number, number]>>>(new Map())
const updateTimer = ref<number | null>(null)

// Computed
const availablePoints = computed(() => 
  props.points.filter(p => p.point_type === 'YC' || p.point_type === 'YT')
)

// Initialize chart
onMounted(() => {
  if (chartContainer.value) {
    chart.value = echarts.init(chartContainer.value)
    
    // Select first 5 points by default
    selectedPointIds.value = availablePoints.value
      .slice(0, 5)
      .map(p => p.point_id)
    
    initChart()
    updateChart()
    
    // Start real-time updates
    updateTimer.value = window.setInterval(() => {
      updateRealtimeData()
    }, 1000) // Update every second
  }
})

onUnmounted(() => {
  if (updateTimer.value) {
    clearInterval(updateTimer.value)
  }
  if (chart.value) {
    chart.value.dispose()
  }
})

// Watch for point changes
watch(() => props.points, () => {
  updateRealtimeData()
}, { deep: true })

function initChart() {
  const option: EChartsOption = {
    title: {
      text: 'Real-time Data',
      left: 'center'
    },
    tooltip: {
      trigger: 'axis',
      formatter: function(params: any) {
        let result = dayjs(params[0].value[0]).format('HH:mm:ss') + '<br/>'
        params.forEach((item: any) => {
          result += `${item.marker} ${item.seriesName}: ${item.value[1].toFixed(2)}<br/>`
        })
        return result
      }
    },
    legend: {
      data: [],
      bottom: 0
    },
    xAxis: {
      type: 'time',
      splitLine: {
        show: false
      },
      axisLabel: {
        formatter: (value: number) => dayjs(value).format('HH:mm:ss')
      }
    },
    yAxis: {
      type: 'value',
      boundaryGap: [0, '10%']
    },
    dataZoom: [
      {
        type: 'inside',
        start: 50,
        end: 100
      },
      {
        start: 50,
        end: 100
      }
    ],
    series: []
  }
  
  chart.value?.setOption(option)
}

function updateChart() {
  if (!chart.value) return
  
  const series: any[] = []
  const legendData: string[] = []
  
  selectedPointIds.value.forEach(pointId => {
    const point = props.points.find(p => p.point_id === pointId)
    if (!point) return
    
    const seriesName = `${pointId} - ${point.description || 'Point'}`
    legendData.push(seriesName)
    
    const data = dataHistory.value.get(pointId) || []
    
    series.push({
      name: seriesName,
      type: 'line',
      smooth: true,
      symbol: 'none',
      data: data
    })
  })
  
  chart.value.setOption({
    legend: {
      data: legendData
    },
    series: series
  })
}

function updateRealtimeData() {
  const now = Date.now()
  const cutoffTime = now - timeRange.value * 1000
  
  // Update data history for selected points
  selectedPointIds.value.forEach(pointId => {
    const point = props.points.find(p => p.point_id === pointId)
    if (!point || typeof point.value !== 'number') return
    
    let history = dataHistory.value.get(pointId)
    if (!history) {
      history = []
      dataHistory.value.set(pointId, history)
    }
    
    // Add new data point
    history.push([now, point.value])
    
    // Remove old data points
    while (history.length > 0 && history[0][0] < cutoffTime) {
      history.shift()
    }
  })
  
  updateChart()
}

function setTimeRange(seconds: number) {
  timeRange.value = seconds
  
  // Clear old data outside the new range
  const now = Date.now()
  const cutoffTime = now - seconds * 1000
  
  dataHistory.value.forEach((history, pointId) => {
    const filtered = history.filter(([time]) => time >= cutoffTime)
    dataHistory.value.set(pointId, filtered)
  })
  
  updateChart()
}
</script>

<style scoped lang="scss">
.realtime-chart {
  height: 100%;
  display: flex;
  flex-direction: column;
  
  .chart-controls {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 20px;
  }
  
  .chart-container {
    flex: 1;
    min-height: 400px;
  }
}
</style>