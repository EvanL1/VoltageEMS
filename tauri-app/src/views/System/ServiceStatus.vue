<template>
  <div class="service-status">
    <!-- Service Overview -->
    <el-row :gutter="20" class="status-overview">
      <el-col :span="6">
        <el-card>
          <el-statistic title="Total Services" :value="totalServices">
            <template #prefix>
              <el-icon color="#409EFF"><Server /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card>
          <el-statistic title="Running" :value="runningServices">
            <template #prefix>
              <el-icon color="#67C23A"><CircleCheck /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card>
          <el-statistic title="Stopped" :value="stoppedServices">
            <template #prefix>
              <el-icon color="#F56C6C"><CircleClose /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card>
          <el-statistic title="System Uptime" :value="systemUptime">
            <template #suffix>hours</template>
            <template #prefix>
              <el-icon color="#E6A23C"><Clock /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
    </el-row>
    
    <!-- Service List -->
    <el-card class="service-list-card">
      <template #header>
        <div class="card-header">
          <span>Service Management</span>
          <el-space>
            <el-button @click="refreshServices" :loading="refreshing">
              <el-icon><Refresh /></el-icon>
              Refresh
            </el-button>
            <el-button type="danger" @click="restartAll">
              <el-icon><RefreshRight /></el-icon>
              Restart All
            </el-button>
          </el-space>
        </div>
      </template>
      
      <el-table :data="services" style="width: 100%" v-loading="loading">
        <el-table-column type="expand">
          <template #default="{ row }">
            <div class="service-details">
              <el-descriptions :column="3" border>
                <el-descriptions-item label="PID">{{ row.pid || 'N/A' }}</el-descriptions-item>
                <el-descriptions-item label="CPU Usage">{{ row.cpuUsage }}%</el-descriptions-item>
                <el-descriptions-item label="Memory Usage">{{ row.memoryUsage }} MB</el-descriptions-item>
                <el-descriptions-item label="Port">{{ row.port }}</el-descriptions-item>
                <el-descriptions-item label="Version">{{ row.version }}</el-descriptions-item>
                <el-descriptions-item label="Started At">{{ row.startedAt }}</el-descriptions-item>
                <el-descriptions-item label="Last Heartbeat">{{ row.lastHeartbeat }}</el-descriptions-item>
                <el-descriptions-item label="Request Count">{{ row.requestCount }}</el-descriptions-item>
                <el-descriptions-item label="Error Count">{{ row.errorCount }}</el-descriptions-item>
              </el-descriptions>
              
              <div class="service-actions">
                <el-button @click="viewLogs(row)">View Logs</el-button>
                <el-button @click="viewConfig(row)">View Config</el-button>
                <el-button @click="viewMetrics(row)">View Metrics</el-button>
              </div>
            </div>
          </template>
        </el-table-column>
        
        <el-table-column prop="name" label="Service Name" width="200">
          <template #default="{ row }">
            <div class="service-name">
              <el-icon :color="getStatusColor(row.status)" size="16">
                <CircleFilled />
              </el-icon>
              <span>{{ row.name }}</span>
            </div>
          </template>
        </el-table-column>
        
        <el-table-column prop="description" label="Description" />
        
        <el-table-column prop="status" label="Status" width="120">
          <template #default="{ row }">
            <el-tag :type="getStatusType(row.status)">
              {{ row.status }}
            </el-tag>
          </template>
        </el-table-column>
        
        <el-table-column prop="health" label="Health" width="120">
          <template #default="{ row }">
            <el-progress
              :percentage="row.health"
              :color="getHealthColor(row.health)"
              :stroke-width="6"
            />
          </template>
        </el-table-column>
        
        <el-table-column label="Auto Restart" width="120">
          <template #default="{ row }">
            <el-switch
              v-model="row.autoRestart"
              @change="updateAutoRestart(row)"
            />
          </template>
        </el-table-column>
        
        <el-table-column label="Actions" width="200" fixed="right">
          <template #default="{ row }">
            <el-button
              v-if="row.status === 'stopped'"
              type="success"
              size="small"
              @click="startService(row)"
              :loading="row.loading"
            >
              Start
            </el-button>
            <el-button
              v-else-if="row.status === 'running'"
              type="danger"
              size="small"
              @click="stopService(row)"
              :loading="row.loading"
            >
              Stop
            </el-button>
            <el-button
              type="warning"
              size="small"
              @click="restartService(row)"
              :loading="row.loading"
            >
              Restart
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>
    
    <!-- Resource Usage Charts -->
    <el-row :gutter="20" class="resource-charts">
      <el-col :span="12">
        <el-card>
          <template #header>
            <span>CPU Usage History</span>
          </template>
          <div ref="cpuChartContainer" class="chart-container"></div>
        </el-card>
      </el-col>
      
      <el-col :span="12">
        <el-card>
          <template #header>
            <span>Memory Usage History</span>
          </template>
          <div ref="memoryChartContainer" class="chart-container"></div>
        </el-card>
      </el-col>
    </el-row>
    
    <!-- Log Viewer Dialog -->
    <el-dialog
      v-model="showLogDialog"
      :title="`${selectedService?.name} - Logs`"
      width="80%"
      top="5vh"
    >
      <div class="log-viewer">
        <div class="log-controls">
          <el-select v-model="logLevel" placeholder="Log Level" style="width: 120px">
            <el-option label="All" value="all" />
            <el-option label="Debug" value="debug" />
            <el-option label="Info" value="info" />
            <el-option label="Warning" value="warning" />
            <el-option label="Error" value="error" />
          </el-select>
          
          <el-input
            v-model="logSearch"
            placeholder="Search logs..."
            :prefix-icon="Search"
            clearable
            style="width: 300px"
          />
          
          <el-checkbox v-model="autoScroll">Auto Scroll</el-checkbox>
          
          <el-button @click="clearLogs">Clear</el-button>
          <el-button @click="downloadLogs">Download</el-button>
        </div>
        
        <div ref="logContainer" class="log-content">
          <div
            v-for="(log, index) in filteredLogs"
            :key="index"
            class="log-line"
            :class="`log-${log.level}`"
          >
            <span class="log-timestamp">{{ log.timestamp }}</span>
            <span class="log-level">[{{ log.level.toUpperCase() }}]</span>
            <span class="log-message">{{ log.message }}</span>
          </div>
        </div>
      </div>
    </el-dialog>
    
    <!-- Config Viewer Dialog -->
    <el-dialog
      v-model="showConfigDialog"
      :title="`${selectedService?.name} - Configuration`"
      width="60%"
    >
      <el-form label-width="150px">
        <el-form-item label="Redis URL">
          <el-input v-model="serviceConfig.redisUrl" disabled />
        </el-form-item>
        
        <el-form-item label="Log Level">
          <el-select v-model="serviceConfig.logLevel">
            <el-option label="Debug" value="debug" />
            <el-option label="Info" value="info" />
            <el-option label="Warning" value="warning" />
            <el-option label="Error" value="error" />
          </el-select>
        </el-form-item>
        
        <el-form-item label="Max Workers">
          <el-input-number v-model="serviceConfig.maxWorkers" :min="1" :max="100" />
        </el-form-item>
        
        <el-form-item label="Timeout (ms)">
          <el-input-number v-model="serviceConfig.timeout" :min="1000" :max="60000" :step="1000" />
        </el-form-item>
        
        <el-form-item label="Enable Metrics">
          <el-switch v-model="serviceConfig.enableMetrics" />
        </el-form-item>
        
        <el-form-item label="Custom Settings">
          <el-input
            v-model="serviceConfig.customSettings"
            type="textarea"
            :rows="5"
            placeholder="JSON format"
          />
        </el-form-item>
      </el-form>
      
      <template #footer>
        <el-button @click="showConfigDialog = false">Cancel</el-button>
        <el-button type="primary" @click="saveConfig">Save Configuration</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import {
  Server,
  CircleCheck,
  CircleClose,
  Clock,
  Refresh,
  RefreshRight,
  CircleFilled,
  Search
} from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import * as echarts from 'echarts'
import dayjs from 'dayjs'

// Mock service data
const services = ref([
  {
    name: 'apigateway',
    description: 'REST API Gateway Service',
    status: 'running',
    health: 98,
    autoRestart: true,
    pid: 12345,
    cpuUsage: 12.5,
    memoryUsage: 256,
    port: 8080,
    version: '1.0.0',
    startedAt: dayjs().subtract(2, 'day').format('YYYY-MM-DD HH:mm:ss'),
    lastHeartbeat: dayjs().subtract(5, 'second').format('YYYY-MM-DD HH:mm:ss'),
    requestCount: 125432,
    errorCount: 12,
    loading: false
  },
  {
    name: 'comsrv',
    description: 'Industrial Protocol Gateway',
    status: 'running',
    health: 100,
    autoRestart: true,
    pid: 12346,
    cpuUsage: 8.2,
    memoryUsage: 128,
    port: 8081,
    version: '1.0.0',
    startedAt: dayjs().subtract(2, 'day').format('YYYY-MM-DD HH:mm:ss'),
    lastHeartbeat: dayjs().subtract(3, 'second').format('YYYY-MM-DD HH:mm:ss'),
    requestCount: 0,
    errorCount: 0,
    loading: false
  },
  {
    name: 'modsrv',
    description: 'Computation Engine Service',
    status: 'running',
    health: 95,
    autoRestart: true,
    pid: 12347,
    cpuUsage: 25.8,
    memoryUsage: 512,
    port: 8082,
    version: '1.0.0',
    startedAt: dayjs().subtract(2, 'day').format('YYYY-MM-DD HH:mm:ss'),
    lastHeartbeat: dayjs().subtract(2, 'second').format('YYYY-MM-DD HH:mm:ss'),
    requestCount: 0,
    errorCount: 5,
    loading: false
  },
  {
    name: 'hissrv',
    description: 'Historical Data Service',
    status: 'stopped',
    health: 0,
    autoRestart: false,
    pid: null,
    cpuUsage: 0,
    memoryUsage: 0,
    port: 8083,
    version: '1.0.0',
    startedAt: null,
    lastHeartbeat: null,
    requestCount: 0,
    errorCount: 0,
    loading: false
  },
  {
    name: 'alarmsrv',
    description: 'Alarm Management Service',
    status: 'running',
    health: 100,
    autoRestart: true,
    pid: 12349,
    cpuUsage: 5.2,
    memoryUsage: 96,
    port: 8084,
    version: '1.0.0',
    startedAt: dayjs().subtract(1, 'day').format('YYYY-MM-DD HH:mm:ss'),
    lastHeartbeat: dayjs().subtract(1, 'second').format('YYYY-MM-DD HH:mm:ss'),
    requestCount: 0,
    errorCount: 0,
    loading: false
  },
  {
    name: 'rulesrv',
    description: 'Rule Engine Service',
    status: 'running',
    health: 90,
    autoRestart: true,
    pid: 12350,
    cpuUsage: 15.3,
    memoryUsage: 256,
    port: 8085,
    version: '1.0.0',
    startedAt: dayjs().subtract(1, 'day').format('YYYY-MM-DD HH:mm:ss'),
    lastHeartbeat: dayjs().subtract(4, 'second').format('YYYY-MM-DD HH:mm:ss'),
    requestCount: 0,
    errorCount: 8,
    loading: false
  }
])

// Statistics
const totalServices = computed(() => services.value.length)
const runningServices = computed(() => services.value.filter(s => s.status === 'running').length)
const stoppedServices = computed(() => services.value.filter(s => s.status === 'stopped').length)
const systemUptime = ref(48)

// State
const loading = ref(false)
const refreshing = ref(false)
const showLogDialog = ref(false)
const showConfigDialog = ref(false)
const selectedService = ref<any>(null)

// Log viewer
const logLevel = ref('all')
const logSearch = ref('')
const autoScroll = ref(true)
const logContainer = ref<HTMLElement>()
const logs = ref<any[]>([])

// Config
const serviceConfig = ref({
  redisUrl: 'redis://localhost:6379',
  logLevel: 'info',
  maxWorkers: 10,
  timeout: 30000,
  enableMetrics: true,
  customSettings: ''
})

// Charts
const cpuChartContainer = ref<HTMLElement>()
const memoryChartContainer = ref<HTMLElement>()
const cpuChart = ref<echarts.ECharts>()
const memoryChart = ref<echarts.ECharts>()

// Computed
const filteredLogs = computed(() => {
  let filtered = logs.value
  
  if (logLevel.value !== 'all') {
    filtered = filtered.filter(log => log.level === logLevel.value)
  }
  
  if (logSearch.value) {
    const search = logSearch.value.toLowerCase()
    filtered = filtered.filter(log => 
      log.message.toLowerCase().includes(search)
    )
  }
  
  return filtered
})

// Update interval
let updateInterval: number | null = null

onMounted(() => {
  initCharts()
  generateMockLogs()
  
  // Update data every 5 seconds
  updateInterval = window.setInterval(() => {
    updateServiceData()
    updateCharts()
  }, 5000)
})

onUnmounted(() => {
  if (updateInterval) {
    clearInterval(updateInterval)
  }
  
  if (cpuChart.value) {
    cpuChart.value.dispose()
  }
  
  if (memoryChart.value) {
    memoryChart.value.dispose()
  }
})

// Methods
async function refreshServices() {
  refreshing.value = true
  
  try {
    // TODO: Fetch service status from API
    await new Promise(resolve => setTimeout(resolve, 1000))
    ElMessage.success('Service status refreshed')
  } finally {
    refreshing.value = false
  }
}

async function startService(service: any) {
  service.loading = true
  
  try {
    // TODO: Call API to start service
    await new Promise(resolve => setTimeout(resolve, 2000))
    
    service.status = 'running'
    service.health = 100
    service.pid = Math.floor(Math.random() * 10000) + 10000
    service.startedAt = dayjs().format('YYYY-MM-DD HH:mm:ss')
    
    ElMessage.success(`${service.name} started successfully`)
  } catch (error) {
    ElMessage.error(`Failed to start ${service.name}`)
  } finally {
    service.loading = false
  }
}

async function stopService(service: any) {
  await ElMessageBox.confirm(
    `Are you sure you want to stop ${service.name}?`,
    'Confirm Stop',
    {
      confirmButtonText: 'Stop',
      cancelButtonText: 'Cancel',
      type: 'warning'
    }
  )
  
  service.loading = true
  
  try {
    // TODO: Call API to stop service
    await new Promise(resolve => setTimeout(resolve, 1500))
    
    service.status = 'stopped'
    service.health = 0
    service.pid = null
    service.cpuUsage = 0
    service.memoryUsage = 0
    
    ElMessage.success(`${service.name} stopped`)
  } catch (error) {
    ElMessage.error(`Failed to stop ${service.name}`)
  } finally {
    service.loading = false
  }
}

async function restartService(service: any) {
  service.loading = true
  
  try {
    // TODO: Call API to restart service
    await new Promise(resolve => setTimeout(resolve, 3000))
    
    service.startedAt = dayjs().format('YYYY-MM-DD HH:mm:ss')
    ElMessage.success(`${service.name} restarted successfully`)
  } catch (error) {
    ElMessage.error(`Failed to restart ${service.name}`)
  } finally {
    service.loading = false
  }
}

async function restartAll() {
  await ElMessageBox.confirm(
    'Are you sure you want to restart all services?',
    'Confirm Restart All',
    {
      confirmButtonText: 'Restart All',
      cancelButtonText: 'Cancel',
      type: 'warning'
    }
  )
  
  ElMessage.info('Restarting all services...')
  
  for (const service of services.value) {
    if (service.status === 'running') {
      await restartService(service)
    }
  }
}

function updateAutoRestart(service: any) {
  ElMessage.success(`Auto restart ${service.autoRestart ? 'enabled' : 'disabled'} for ${service.name}`)
}

function viewLogs(service: any) {
  selectedService.value = service
  showLogDialog.value = true
  
  // Generate logs for this service
  generateServiceLogs(service)
}

function viewConfig(service: any) {
  selectedService.value = service
  showConfigDialog.value = true
  
  // Load service config
  // TODO: Fetch from API
}

function viewMetrics(service: any) {
  // TODO: Open metrics dashboard for service
  ElMessage.info(`Opening metrics for ${service.name}`)
}

function saveConfig() {
  ElMessage.success('Configuration saved')
  showConfigDialog.value = false
}

function clearLogs() {
  logs.value = []
}

function downloadLogs() {
  // TODO: Implement log download
  ElMessage.success('Downloading logs...')
}

// Generate mock logs
function generateMockLogs() {
  // Generate initial logs
  for (let i = 0; i < 20; i++) {
    logs.value.push({
      timestamp: dayjs().subtract(i * 10, 'second').format('HH:mm:ss.SSS'),
      level: ['debug', 'info', 'warning', 'error'][Math.floor(Math.random() * 4)],
      message: `Sample log message ${i}: Processing data...`
    })
  }
}

function generateServiceLogs(service: any) {
  logs.value = []
  
  const logMessages = {
    apigateway: [
      'Incoming request: GET /api/channels',
      'Request processed successfully',
      'WebSocket connection established',
      'Authentication successful for user: admin'
    ],
    comsrv: [
      'Modbus connection established: 192.168.1.100:502',
      'Reading registers: 30001-30010',
      'Point update: 10001 = 220.5 V',
      'Channel 1 polling cycle completed'
    ],
    modsrv: [
      'Calculation triggered for rule: CALC_001',
      'DAG execution started',
      'Computed value: 85.5 kW',
      'Publishing results to Redis'
    ],
    alarmsrv: [
      'Alarm condition detected: High Temperature',
      'Alarm acknowledged by user: admin',
      'Alarm cleared: Low Voltage',
      'Notification sent via email'
    ],
    rulesrv: [
      'Rule evaluation: RULE_001',
      'Condition matched: temperature > 80',
      'Action executed: Send alarm',
      'Rule execution completed'
    ]
  }
  
  const messages = logMessages[service.name as keyof typeof logMessages] || []
  
  for (let i = 0; i < 50; i++) {
    logs.value.push({
      timestamp: dayjs().subtract(i * 2, 'second').format('HH:mm:ss.SSS'),
      level: ['debug', 'info', 'warning', 'error'][Math.floor(Math.random() * 4)],
      message: messages[Math.floor(Math.random() * messages.length)]
    })
  }
}

// Update service data
function updateServiceData() {
  services.value.forEach(service => {
    if (service.status === 'running') {
      // Update CPU usage
      service.cpuUsage = Math.max(0, Math.min(100, 
        service.cpuUsage + (Math.random() - 0.5) * 10
      ))
      
      // Update memory usage
      service.memoryUsage = Math.max(50, Math.min(1024, 
        service.memoryUsage + (Math.random() - 0.5) * 50
      ))
      
      // Update last heartbeat
      service.lastHeartbeat = dayjs().format('YYYY-MM-DD HH:mm:ss')
      
      // Update health
      if (service.cpuUsage > 80 || service.memoryUsage > 800) {
        service.health = Math.max(50, service.health - 5)
      } else {
        service.health = Math.min(100, service.health + 2)
      }
    }
  })
}

// Chart methods
function initCharts() {
  if (!cpuChartContainer.value || !memoryChartContainer.value) return
  
  // CPU Chart
  cpuChart.value = echarts.init(cpuChartContainer.value)
  const cpuOption = {
    tooltip: {
      trigger: 'axis',
      axisPointer: {
        type: 'cross'
      }
    },
    legend: {
      data: services.value.map(s => s.name)
    },
    xAxis: {
      type: 'time',
      boundaryGap: false
    },
    yAxis: {
      type: 'value',
      min: 0,
      max: 100,
      axisLabel: {
        formatter: '{value}%'
      }
    },
    series: services.value.map(service => ({
      name: service.name,
      type: 'line',
      smooth: true,
      symbol: 'none',
      data: generateInitialData(service.cpuUsage)
    }))
  }
  cpuChart.value.setOption(cpuOption)
  
  // Memory Chart
  memoryChart.value = echarts.init(memoryChartContainer.value)
  const memoryOption = {
    tooltip: {
      trigger: 'axis',
      axisPointer: {
        type: 'cross'
      }
    },
    legend: {
      data: services.value.map(s => s.name)
    },
    xAxis: {
      type: 'time',
      boundaryGap: false
    },
    yAxis: {
      type: 'value',
      min: 0,
      axisLabel: {
        formatter: '{value} MB'
      }
    },
    series: services.value.map(service => ({
      name: service.name,
      type: 'line',
      smooth: true,
      symbol: 'none',
      data: generateInitialData(service.memoryUsage)
    }))
  }
  memoryChart.value.setOption(memoryOption)
  
  // Handle resize
  window.addEventListener('resize', () => {
    cpuChart.value?.resize()
    memoryChart.value?.resize()
  })
}

function generateInitialData(baseValue: number) {
  const data = []
  const now = Date.now()
  
  for (let i = 59; i >= 0; i--) {
    data.push([
      now - i * 5000,
      baseValue + (Math.random() - 0.5) * 10
    ])
  }
  
  return data
}

function updateCharts() {
  if (!cpuChart.value || !memoryChart.value) return
  
  const now = Date.now()
  
  // Update CPU chart
  services.value.forEach((service, index) => {
    if (service.status === 'running') {
      const series = cpuChart.value!.getOption().series as any[]
      series[index].data.push([now, service.cpuUsage])
      
      // Keep only last 60 points
      if (series[index].data.length > 60) {
        series[index].data.shift()
      }
    }
  })
  
  cpuChart.value.setOption({
    series: cpuChart.value.getOption().series
  })
  
  // Update Memory chart
  services.value.forEach((service, index) => {
    if (service.status === 'running') {
      const series = memoryChart.value!.getOption().series as any[]
      series[index].data.push([now, service.memoryUsage])
      
      // Keep only last 60 points
      if (series[index].data.length > 60) {
        series[index].data.shift()
      }
    }
  })
  
  memoryChart.value.setOption({
    series: memoryChart.value.getOption().series
  })
}

// Utility methods
function getStatusColor(status: string) {
  switch (status) {
    case 'running':
      return '#67C23A'
    case 'stopped':
      return '#F56C6C'
    case 'error':
      return '#E6A23C'
    default:
      return '#909399'
  }
}

function getStatusType(status: string) {
  switch (status) {
    case 'running':
      return 'success'
    case 'stopped':
      return 'danger'
    case 'error':
      return 'warning'
    default:
      return 'info'
  }
}

function getHealthColor(health: number) {
  if (health >= 90) return '#67C23A'
  if (health >= 70) return '#E6A23C'
  return '#F56C6C'
}
</script>

<style lang="scss" scoped>
.service-status {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 20px;
  
  .status-overview {
    :deep(.el-statistic__number) {
      font-size: 28px;
    }
  }
  
  .service-list-card {
    .card-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
    }
    
    .service-name {
      display: flex;
      align-items: center;
      gap: 8px;
    }
    
    .service-details {
      padding: 20px;
      
      .service-actions {
        margin-top: 20px;
        display: flex;
        gap: 10px;
      }
    }
  }
  
  .resource-charts {
    .chart-container {
      height: 300px;
    }
  }
  
  .log-viewer {
    height: 600px;
    display: flex;
    flex-direction: column;
    
    .log-controls {
      display: flex;
      align-items: center;
      gap: 10px;
      padding-bottom: 10px;
      border-bottom: 1px solid #e4e7ed;
      margin-bottom: 10px;
    }
    
    .log-content {
      flex: 1;
      overflow-y: auto;
      font-family: 'Consolas', 'Monaco', monospace;
      font-size: 12px;
      background: #1e1e1e;
      color: #d4d4d4;
      padding: 10px;
      border-radius: 4px;
      
      .log-line {
        margin-bottom: 2px;
        white-space: pre-wrap;
        
        &.log-debug {
          color: #808080;
        }
        
        &.log-info {
          color: #d4d4d4;
        }
        
        &.log-warning {
          color: #dcdcaa;
        }
        
        &.log-error {
          color: #f48771;
        }
        
        .log-timestamp {
          color: #608b4e;
          margin-right: 10px;
        }
        
        .log-level {
          margin-right: 10px;
          font-weight: bold;
        }
      }
    }
  }
}
</style>