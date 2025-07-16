<template>
  <div class="data-query">
    <!-- Query Form -->
    <el-card class="query-card">
      <template #header>
        <span>History Data Query</span>
      </template>
      
      <el-form :model="queryForm" label-width="120px">
        <el-row :gutter="20">
          <el-col :span="8">
            <el-form-item label="Channel">
              <el-select 
                v-model="queryForm.channelId" 
                placeholder="Select channel"
                filterable
              >
                <el-option
                  v-for="channel in channels"
                  :key="channel.channel_id"
                  :label="channel.name"
                  :value="channel.channel_id"
                />
              </el-select>
            </el-form-item>
          </el-col>
          
          <el-col :span="8">
            <el-form-item label="Point Type">
              <el-select 
                v-model="queryForm.pointType" 
                placeholder="Select point type"
                multiple
              >
                <el-option label="Measurements (YC)" value="YC" />
                <el-option label="Signals (YX)" value="YX" />
                <el-option label="Controls (YK)" value="YK" />
                <el-option label="Adjustments (YT)" value="YT" />
              </el-select>
            </el-form-item>
          </el-col>
          
          <el-col :span="8">
            <el-form-item label="Points">
              <el-select 
                v-model="queryForm.pointIds" 
                placeholder="Select points"
                multiple
                filterable
                :disabled="!queryForm.channelId"
              >
                <el-option
                  v-for="point in availablePoints"
                  :key="point.point_id"
                  :label="`${point.point_id} - ${point.description}`"
                  :value="point.point_id"
                />
              </el-select>
            </el-form-item>
          </el-col>
        </el-row>
        
        <el-row :gutter="20">
          <el-col :span="8">
            <el-form-item label="Time Range">
              <el-date-picker
                v-model="queryForm.timeRange"
                type="datetimerange"
                range-separator="To"
                start-placeholder="Start time"
                end-placeholder="End time"
                format="YYYY-MM-DD HH:mm:ss"
                value-format="YYYY-MM-DD HH:mm:ss"
                :shortcuts="timeShortcuts"
              />
            </el-form-item>
          </el-col>
          
          <el-col :span="8">
            <el-form-item label="Aggregation">
              <el-select v-model="queryForm.aggregation" placeholder="Select aggregation">
                <el-option label="Raw Data" value="none" />
                <el-option label="1 Minute Average" value="1m" />
                <el-option label="5 Minutes Average" value="5m" />
                <el-option label="15 Minutes Average" value="15m" />
                <el-option label="1 Hour Average" value="1h" />
                <el-option label="1 Day Average" value="1d" />
              </el-select>
            </el-form-item>
          </el-col>
          
          <el-col :span="8">
            <el-form-item label="Data Format">
              <el-radio-group v-model="queryForm.format">
                <el-radio label="table">Table</el-radio>
                <el-radio label="chart">Chart</el-radio>
                <el-radio label="both">Both</el-radio>
              </el-radio-group>
            </el-form-item>
          </el-col>
        </el-row>
        
        <el-form-item>
          <el-button type="primary" @click="queryData" :loading="loading">
            <el-icon><Search /></el-icon>
            Query
          </el-button>
          <el-button @click="resetForm">
            <el-icon><Refresh /></el-icon>
            Reset
          </el-button>
          <el-button type="success" @click="exportData" :disabled="!hasData">
            <el-icon><Download /></el-icon>
            Export
          </el-button>
        </el-form-item>
      </el-form>
    </el-card>
    
    <!-- Results -->
    <div v-if="hasData" class="results-container">
      <!-- Chart View -->
      <el-card v-if="showChart" class="result-card">
        <template #header>
          <div class="card-header">
            <span>Data Chart</span>
            <el-button-group>
              <el-button size="small" @click="zoomIn">
                <el-icon><ZoomIn /></el-icon>
              </el-button>
              <el-button size="small" @click="zoomOut">
                <el-icon><ZoomOut /></el-icon>
              </el-button>
              <el-button size="small" @click="resetZoom">
                <el-icon><Refresh /></el-icon>
              </el-button>
            </el-button-group>
          </div>
        </template>
        
        <div ref="chartContainer" class="chart-container"></div>
      </el-card>
      
      <!-- Table View -->
      <el-card v-if="showTable" class="result-card">
        <template #header>
          <div class="card-header">
            <span>Data Table ({{ tableData.length }} records)</span>
            <el-space>
              <el-input
                v-model="tableSearch"
                placeholder="Search..."
                :prefix-icon="Search"
                clearable
                style="width: 200px"
              />
              <el-button size="small" @click="downloadCSV">
                <el-icon><Document /></el-icon>
                CSV
              </el-button>
              <el-button size="small" @click="downloadExcel">
                <el-icon><Document /></el-icon>
                Excel
              </el-button>
            </el-space>
          </div>
        </template>
        
        <el-table 
          :data="filteredTableData" 
          style="width: 100%"
          max-height="400"
          stripe
        >
          <el-table-column prop="timestamp" label="Timestamp" width="180" fixed />
          <el-table-column prop="channelName" label="Channel" width="150" />
          <el-table-column prop="pointId" label="Point ID" width="100" />
          <el-table-column prop="description" label="Description" min-width="200" />
          <el-table-column prop="value" label="Value" width="120">
            <template #default="{ row }">
              <span :class="getValueClass(row)">{{ formatValue(row) }}</span>
            </template>
          </el-table-column>
          <el-table-column prop="quality" label="Quality" width="100">
            <template #default="{ row }">
              <el-tag :type="getQualityType(row.quality)" size="small">
                {{ getQualityText(row.quality) }}
              </el-tag>
            </template>
          </el-table-column>
        </el-table>
        
        <el-pagination
          v-model:current-page="currentPage"
          v-model:page-size="pageSize"
          :page-sizes="[50, 100, 200, 500]"
          :total="filteredTableData.length"
          layout="total, sizes, prev, pager, next, jumper"
          style="margin-top: 20px"
        />
      </el-card>
    </div>
    
    <!-- Export Dialog -->
    <el-dialog
      v-model="showExportDialog"
      title="Export Data"
      width="500px"
    >
      <el-form :model="exportForm" label-width="120px">
        <el-form-item label="Export Format">
          <el-radio-group v-model="exportForm.format">
            <el-radio label="csv">CSV</el-radio>
            <el-radio label="excel">Excel</el-radio>
            <el-radio label="json">JSON</el-radio>
          </el-radio-group>
        </el-form-item>
        
        <el-form-item label="Data Range">
          <el-radio-group v-model="exportForm.range">
            <el-radio label="all">All Data</el-radio>
            <el-radio label="filtered">Filtered Data</el-radio>
            <el-radio label="selected">Selected Rows</el-radio>
          </el-radio-group>
        </el-form-item>
        
        <el-form-item label="Include Headers">
          <el-switch v-model="exportForm.includeHeaders" />
        </el-form-item>
        
        <el-form-item label="File Name">
          <el-input v-model="exportForm.fileName" />
        </el-form-item>
      </el-form>
      
      <template #footer>
        <el-button @click="showExportDialog = false">Cancel</el-button>
        <el-button type="primary" @click="confirmExport">Export</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { 
  Search, 
  Refresh, 
  Download,
  Document,
  ZoomIn,
  ZoomOut
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import * as echarts from 'echarts'
import dayjs from 'dayjs'
import { useRealtimeStore } from '@/stores/realtime'

const realtimeStore = useRealtimeStore()

// Query form
const queryForm = ref({
  channelId: null as number | null,
  pointType: [] as string[],
  pointIds: [] as number[],
  timeRange: [] as string[],
  aggregation: 'none',
  format: 'both'
})

// Data
const channels = computed(() => realtimeStore.channels)
const availablePoints = ref<any[]>([])
const tableData = ref<any[]>([])
const loading = ref(false)

// Display control
const showChart = computed(() => queryForm.value.format === 'chart' || queryForm.value.format === 'both')
const showTable = computed(() => queryForm.value.format === 'table' || queryForm.value.format === 'both')
const hasData = computed(() => tableData.value.length > 0)

// Table
const tableSearch = ref('')
const currentPage = ref(1)
const pageSize = ref(100)

const filteredTableData = computed(() => {
  let filtered = tableData.value
  
  if (tableSearch.value) {
    const search = tableSearch.value.toLowerCase()
    filtered = filtered.filter(row => 
      row.description?.toLowerCase().includes(search) ||
      row.pointId?.toString().includes(search) ||
      row.value?.toString().includes(search)
    )
  }
  
  const start = (currentPage.value - 1) * pageSize.value
  const end = start + pageSize.value
  return filtered.slice(start, end)
})

// Chart
const chartContainer = ref<HTMLElement>()
const chart = ref<echarts.ECharts>()

// Export
const showExportDialog = ref(false)
const exportForm = ref({
  format: 'csv',
  range: 'all',
  includeHeaders: true,
  fileName: `history_data_${dayjs().format('YYYYMMDD_HHmmss')}`
})

// Time shortcuts
const timeShortcuts = [
  {
    text: 'Last 1 hour',
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setTime(start.getTime() - 3600 * 1000)
      return [start, end]
    }
  },
  {
    text: 'Last 24 hours',
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setTime(start.getTime() - 3600 * 1000 * 24)
      return [start, end]
    }
  },
  {
    text: 'Last 7 days',
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setTime(start.getTime() - 3600 * 1000 * 24 * 7)
      return [start, end]
    }
  },
  {
    text: 'Last 30 days',
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setTime(start.getTime() - 3600 * 1000 * 24 * 30)
      return [start, end]
    }
  }
]

// Watch channel selection
watch(() => queryForm.value.channelId, async (channelId) => {
  if (channelId) {
    // Load available points for selected channel
    // TODO: Fetch from API
    availablePoints.value = [
      { point_id: 10001, description: 'Voltage Phase A', type: 'YC' },
      { point_id: 10002, description: 'Voltage Phase B', type: 'YC' },
      { point_id: 10003, description: 'Voltage Phase C', type: 'YC' },
      { point_id: 10004, description: 'Current Phase A', type: 'YC' },
      { point_id: 10005, description: 'Current Phase B', type: 'YC' },
      { point_id: 20001, description: 'Circuit Breaker Status', type: 'YX' },
      { point_id: 30001, description: 'Circuit Breaker Control', type: 'YK' },
      { point_id: 40001, description: 'Power Setpoint', type: 'YT' }
    ]
  } else {
    availablePoints.value = []
    queryForm.value.pointIds = []
  }
})

onMounted(() => {
  // Set default time range to last 24 hours
  const end = new Date()
  const start = new Date()
  start.setTime(start.getTime() - 3600 * 1000 * 24)
  queryForm.value.timeRange = [
    dayjs(start).format('YYYY-MM-DD HH:mm:ss'),
    dayjs(end).format('YYYY-MM-DD HH:mm:ss')
  ]
})

// Methods
async function queryData() {
  if (!queryForm.value.channelId) {
    ElMessage.warning('Please select a channel')
    return
  }
  
  if (queryForm.value.timeRange.length !== 2) {
    ElMessage.warning('Please select time range')
    return
  }
  
  loading.value = true
  
  try {
    // TODO: Replace with actual API call
    // Simulate data generation
    const data = generateMockData()
    tableData.value = data
    
    if (showChart.value) {
      setTimeout(() => {
        initChart()
        updateChart(data)
      }, 100)
    }
    
    ElMessage.success(`Query completed. Found ${data.length} records.`)
  } catch (error) {
    ElMessage.error('Query failed')
  } finally {
    loading.value = false
  }
}

function generateMockData() {
  const data = []
  const channel = channels.value.find(c => c.channel_id === queryForm.value.channelId)
  const [startTime, endTime] = queryForm.value.timeRange.map(t => new Date(t).getTime())
  const interval = 60000 // 1 minute
  
  for (const pointId of queryForm.value.pointIds) {
    const point = availablePoints.value.find(p => p.point_id === pointId)
    if (!point) continue
    
    for (let time = startTime; time <= endTime; time += interval) {
      data.push({
        timestamp: dayjs(time).format('YYYY-MM-DD HH:mm:ss'),
        channelId: queryForm.value.channelId,
        channelName: channel?.name || 'Unknown',
        pointId: pointId,
        description: point.description,
        type: point.type,
        value: point.type === 'YX' || point.type === 'YK' 
          ? Math.random() > 0.5 ? 1 : 0
          : (Math.random() * 100).toFixed(2),
        quality: Math.random() > 0.9 ? 128 : 192
      })
    }
  }
  
  return data
}

function resetForm() {
  queryForm.value = {
    channelId: null,
    pointType: [],
    pointIds: [],
    timeRange: [],
    aggregation: 'none',
    format: 'both'
  }
  tableData.value = []
  if (chart.value) {
    chart.value.clear()
  }
}

function exportData() {
  showExportDialog.value = true
}

function confirmExport() {
  // TODO: Implement export functionality
  ElMessage.success(`Exported ${tableData.value.length} records as ${exportForm.value.format.toUpperCase()}`)
  showExportDialog.value = false
}

function downloadCSV() {
  // TODO: Implement CSV download
  ElMessage.success('Downloading CSV file...')
}

function downloadExcel() {
  // TODO: Implement Excel download
  ElMessage.success('Downloading Excel file...')
}

// Chart methods
function initChart() {
  if (!chartContainer.value) return
  
  if (chart.value) {
    chart.value.dispose()
  }
  
  chart.value = echarts.init(chartContainer.value)
  
  const option = {
    title: {
      text: 'Historical Data Chart',
      left: 'center'
    },
    tooltip: {
      trigger: 'axis',
      axisPointer: {
        type: 'cross'
      }
    },
    legend: {
      bottom: 0,
      data: []
    },
    toolbox: {
      feature: {
        dataZoom: {
          yAxisIndex: 'none'
        },
        restore: {},
        saveAsImage: {}
      }
    },
    xAxis: {
      type: 'time',
      boundaryGap: false
    },
    yAxis: {
      type: 'value',
      boundaryGap: [0, '100%']
    },
    dataZoom: [
      {
        type: 'inside',
        start: 0,
        end: 100
      },
      {
        start: 0,
        end: 100
      }
    ],
    series: []
  }
  
  chart.value.setOption(option)
  
  // Handle resize
  window.addEventListener('resize', () => {
    chart.value?.resize()
  })
}

function updateChart(data: any[]) {
  if (!chart.value) return
  
  // Group data by point
  const groupedData = data.reduce((acc, item) => {
    if (!acc[item.pointId]) {
      acc[item.pointId] = {
        name: `${item.pointId} - ${item.description}`,
        data: []
      }
    }
    acc[item.pointId].data.push([item.timestamp, parseFloat(item.value)])
    return acc
  }, {} as Record<string, any>)
  
  const series = Object.values(groupedData).map(item => ({
    name: item.name,
    type: 'line',
    smooth: true,
    symbol: 'none',
    data: item.data
  }))
  
  chart.value.setOption({
    legend: {
      data: series.map(s => s.name)
    },
    series
  })
}

function zoomIn() {
  if (!chart.value) return
  const option = chart.value.getOption() as any
  const zoom = option.dataZoom[0]
  const range = zoom.end - zoom.start
  const newRange = range * 0.8
  const center = (zoom.start + zoom.end) / 2
  chart.value.setOption({
    dataZoom: [{
      start: center - newRange / 2,
      end: center + newRange / 2
    }]
  })
}

function zoomOut() {
  if (!chart.value) return
  const option = chart.value.getOption() as any
  const zoom = option.dataZoom[0]
  const range = zoom.end - zoom.start
  const newRange = Math.min(range * 1.2, 100)
  const center = (zoom.start + zoom.end) / 2
  chart.value.setOption({
    dataZoom: [{
      start: Math.max(0, center - newRange / 2),
      end: Math.min(100, center + newRange / 2)
    }]
  })
}

function resetZoom() {
  if (!chart.value) return
  chart.value.setOption({
    dataZoom: [{
      start: 0,
      end: 100
    }]
  })
}

// Utility methods
function formatValue(row: any) {
  if (row.type === 'YX' || row.type === 'YK') {
    return row.value === 1 || row.value === '1' ? 'ON' : 'OFF'
  }
  return row.value
}

function getValueClass(row: any) {
  if (row.type === 'YX' || row.type === 'YK') {
    return row.value === 1 || row.value === '1' ? 'value-on' : 'value-off'
  }
  return ''
}

function getQualityType(quality: number) {
  if (quality >= 192) return 'success'
  if (quality >= 128) return 'warning'
  return 'danger'
}

function getQualityText(quality: number) {
  if (quality >= 192) return 'Good'
  if (quality >= 128) return 'Fair'
  return 'Poor'
}
</script>

<style lang="scss" scoped>
.data-query {
  .query-card {
    margin-bottom: 20px;
  }
  
  .results-container {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  
  .result-card {
    .card-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
    }
    
    .chart-container {
      height: 400px;
    }
  }
  
  .value-on {
    color: #67C23A;
    font-weight: bold;
  }
  
  .value-off {
    color: #909399;
  }
}
</style>