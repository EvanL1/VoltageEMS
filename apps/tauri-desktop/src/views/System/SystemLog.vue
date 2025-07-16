<template>
  <div class="system-log">
    <!-- Log Controls -->
    <el-card class="log-controls-card">
      <div class="log-controls">
        <el-select v-model="selectedService" placeholder="All Services" clearable style="width: 200px">
          <el-option label="All Services" value="" />
          <el-option label="API Gateway" value="apigateway" />
          <el-option label="Communication Service" value="comsrv" />
          <el-option label="Computation Service" value="modsrv" />
          <el-option label="Historical Service" value="hissrv" />
          <el-option label="Alarm Service" value="alarmsrv" />
          <el-option label="Rule Service" value="rulesrv" />
        </el-select>
        
        <el-select v-model="logLevel" placeholder="Log Level" clearable style="width: 120px">
          <el-option label="All Levels" value="" />
          <el-option label="Debug" value="debug" />
          <el-option label="Info" value="info" />
          <el-option label="Warning" value="warning" />
          <el-option label="Error" value="error" />
          <el-option label="Critical" value="critical" />
        </el-select>
        
        <el-date-picker
          v-model="timeRange"
          type="datetimerange"
          range-separator="to"
          start-placeholder="Start time"
          end-placeholder="End time"
          format="YYYY-MM-DD HH:mm:ss"
          value-format="YYYY-MM-DD HH:mm:ss"
          :shortcuts="timeShortcuts"
        />
        
        <el-input
          v-model="searchQuery"
          placeholder="Search logs..."
          :prefix-icon="Search"
          clearable
          style="width: 300px"
        />
        
        <div style="flex: 1"></div>
        
        <el-button-group>
          <el-button @click="toggleLiveMode" :type="liveMode ? 'primary' : 'default'">
            <el-icon><VideoPause v-if="liveMode" /><VideoPlay v-else /></el-icon>
            {{ liveMode ? 'Pause' : 'Live' }}
          </el-button>
          <el-button @click="clearLogs">
            <el-icon><Delete /></el-icon>
            Clear
          </el-button>
          <el-button @click="downloadLogs">
            <el-icon><Download /></el-icon>
            Download
          </el-button>
        </el-button-group>
      </div>
    </el-card>
    
    <!-- Log Statistics -->
    <el-row :gutter="20" class="log-stats">
      <el-col :span="6">
        <el-card>
          <el-statistic title="Total Logs" :value="totalLogs">
            <template #suffix>
              <el-text type="info" size="small">in view</el-text>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card>
          <el-statistic title="Error Rate" :value="errorRate" :precision="1">
            <template #suffix>%</template>
            <template #prefix>
              <el-icon :color="getErrorRateColor(errorRate)"><CircleClose /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card>
          <el-statistic title="Log Rate" :value="logRate">
            <template #suffix>logs/min</template>
            <template #prefix>
              <el-icon color="#409EFF"><Clock /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card>
          <el-statistic title="Size" :value="logSize" :precision="2">
            <template #suffix>MB</template>
            <template #prefix>
              <el-icon color="#E6A23C"><Files /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
    </el-row>
    
    <!-- Log Viewer -->
    <el-card class="log-viewer-card">
      <div class="log-viewer">
        <div class="log-header">
          <el-checkbox v-model="showTimestamp">Timestamp</el-checkbox>
          <el-checkbox v-model="showService">Service</el-checkbox>
          <el-checkbox v-model="showLevel">Level</el-checkbox>
          <el-checkbox v-model="showContext">Context</el-checkbox>
          <el-checkbox v-model="wrapLines">Wrap Lines</el-checkbox>
          
          <div style="flex: 1"></div>
          
          <el-text type="info">
            {{ filteredLogs.length }} logs
          </el-text>
        </div>
        
        <div 
          ref="logContainer" 
          class="log-container"
          :class="{ 'wrap-lines': wrapLines }"
          @scroll="handleScroll"
        >
          <virtual-list
            :data-key="'id'"
            :data-sources="filteredLogs"
            :estimate-size="24"
            :keeps="50"
          >
            <template #default="{ source }">
              <div 
                class="log-line"
                :class="`log-${source.level}`"
                @click="selectLog(source)"
              >
                <span v-if="showTimestamp" class="log-timestamp">
                  {{ formatTimestamp(source.timestamp) }}
                </span>
                <span v-if="showService" class="log-service">
                  [{{ source.service }}]
                </span>
                <span v-if="showLevel" class="log-level">
                  {{ source.level.toUpperCase() }}
                </span>
                <span class="log-message">{{ source.message }}</span>
                <span v-if="showContext && source.context" class="log-context">
                  {{ JSON.stringify(source.context) }}
                </span>
              </div>
            </template>
          </virtual-list>
        </div>
        
        <div class="log-footer">
          <el-text type="info" size="small">
            {{ liveMode ? 'Live mode - Auto scrolling' : 'Paused - Manual scrolling' }}
          </el-text>
          
          <el-button 
            v-if="!autoScroll && !liveMode" 
            type="primary"
            size="small"
            @click="scrollToBottom"
          >
            Jump to Bottom
          </el-button>
        </div>
      </div>
    </el-card>
    
    <!-- Log Detail Dialog -->
    <el-dialog
      v-model="showDetailDialog"
      title="Log Details"
      width="60%"
    >
      <el-descriptions :column="2" border v-if="selectedLog">
        <el-descriptions-item label="Timestamp">
          {{ formatTimestamp(selectedLog.timestamp) }}
        </el-descriptions-item>
        <el-descriptions-item label="Service">
          {{ selectedLog.service }}
        </el-descriptions-item>
        <el-descriptions-item label="Level">
          <el-tag :type="getLevelType(selectedLog.level)">
            {{ selectedLog.level }}
          </el-tag>
        </el-descriptions-item>
        <el-descriptions-item label="Logger">
          {{ selectedLog.logger || 'default' }}
        </el-descriptions-item>
        <el-descriptions-item label="Thread">
          {{ selectedLog.thread || 'main' }}
        </el-descriptions-item>
        <el-descriptions-item label="File">
          {{ selectedLog.file || 'N/A' }}
        </el-descriptions-item>
        <el-descriptions-item label="Message" :span="2">
          <pre class="log-detail-message">{{ selectedLog.message }}</pre>
        </el-descriptions-item>
        <el-descriptions-item label="Context" :span="2" v-if="selectedLog.context">
          <pre class="log-detail-context">{{ JSON.stringify(selectedLog.context, null, 2) }}</pre>
        </el-descriptions-item>
        <el-descriptions-item label="Stack Trace" :span="2" v-if="selectedLog.stackTrace">
          <pre class="log-detail-stack">{{ selectedLog.stackTrace }}</pre>
        </el-descriptions-item>
      </el-descriptions>
      
      <template #footer>
        <el-button @click="copyLogDetail">Copy</el-button>
        <el-button @click="showDetailDialog = false">Close</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import {
  Search,
  VideoPause,
  VideoPlay,
  Delete,
  Download,
  CircleClose,
  Clock,
  Files
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
// @ts-ignore
import VirtualList from 'vue-virtual-scroll-list'
import dayjs from 'dayjs'

// Log data
const logs = ref<any[]>([])
const selectedService = ref('')
const logLevel = ref('')
const timeRange = ref<string[]>([])
const searchQuery = ref('')

// View settings
const showTimestamp = ref(true)
const showService = ref(true)
const showLevel = ref(true)
const showContext = ref(false)
const wrapLines = ref(false)
const liveMode = ref(true)
const autoScroll = ref(true)

// Statistics
const totalLogs = computed(() => filteredLogs.value.length)
const errorRate = computed(() => {
  const errors = filteredLogs.value.filter(log => log.level === 'error' || log.level === 'critical').length
  return totalLogs.value > 0 ? (errors / totalLogs.value) * 100 : 0
})
const logRate = ref(120)
const logSize = ref(15.6)

// Log viewer
const logContainer = ref<HTMLElement>()
const selectedLog = ref<any>(null)
const showDetailDialog = ref(false)

// Time shortcuts
const timeShortcuts = [
  {
    text: 'Last 15 minutes',
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setTime(start.getTime() - 15 * 60 * 1000)
      return [start, end]
    }
  },
  {
    text: 'Last hour',
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
  }
]

// Computed
const filteredLogs = computed(() => {
  let filtered = logs.value
  
  if (selectedService.value) {
    filtered = filtered.filter(log => log.service === selectedService.value)
  }
  
  if (logLevel.value) {
    filtered = filtered.filter(log => log.level === logLevel.value)
  }
  
  if (timeRange.value?.length === 2) {
    const [start, end] = timeRange.value
    filtered = filtered.filter(log => {
      const logTime = dayjs(log.timestamp)
      return logTime.isAfter(start) && logTime.isBefore(end)
    })
  }
  
  if (searchQuery.value) {
    const search = searchQuery.value.toLowerCase()
    filtered = filtered.filter(log => 
      log.message.toLowerCase().includes(search) ||
      (log.context && JSON.stringify(log.context).toLowerCase().includes(search))
    )
  }
  
  return filtered
})

// Log generation
let logInterval: number | null = null

onMounted(() => {
  // Generate initial logs
  generateInitialLogs()
  
  // Start live log generation
  if (liveMode.value) {
    startLiveMode()
  }
  
  // Set default time range
  const end = new Date()
  const start = new Date()
  start.setTime(start.getTime() - 15 * 60 * 1000)
  timeRange.value = [
    dayjs(start).format('YYYY-MM-DD HH:mm:ss'),
    dayjs(end).format('YYYY-MM-DD HH:mm:ss')
  ]
})

onUnmounted(() => {
  stopLiveMode()
})

// Watch for auto scroll
watch(filteredLogs, () => {
  if (autoScroll.value && liveMode.value) {
    nextTick(() => {
      scrollToBottom()
    })
  }
})

// Methods
function generateInitialLogs() {
  const services = ['apigateway', 'comsrv', 'modsrv', 'hissrv', 'alarmsrv', 'rulesrv']
  const levels = ['debug', 'info', 'warning', 'error', 'critical']
  const messages = {
    debug: [
      'Entering function processData()',
      'Redis connection pool status: active=5, idle=10',
      'Memory usage: 256MB / 1024MB',
      'Cache hit rate: 85%'
    ],
    info: [
      'Service started successfully',
      'Connected to Redis at localhost:6379',
      'WebSocket client connected from 192.168.1.100',
      'Configuration loaded from /config/default.yml',
      'Scheduled task completed: data cleanup'
    ],
    warning: [
      'Connection timeout, retrying...',
      'Memory usage above 80%',
      'Slow query detected: 2.5s',
      'Failed to load optional module'
    ],
    error: [
      'Failed to connect to database',
      'Invalid configuration parameter',
      'Request failed: 500 Internal Server Error',
      'Unhandled exception in worker thread'
    ],
    critical: [
      'System out of memory',
      'Service crash detected',
      'Data corruption detected',
      'Security breach attempt detected'
    ]
  }
  
  // Generate 500 initial logs
  for (let i = 0; i < 500; i++) {
    const service = services[Math.floor(Math.random() * services.length)]
    const level = levels[Math.floor(Math.random() * levels.length)]
    const messageList = messages[level as keyof typeof messages]
    const message = messageList[Math.floor(Math.random() * messageList.length)]
    
    const log = {
      id: Date.now() + Math.random(),
      timestamp: dayjs().subtract(Math.floor(Math.random() * 900), 'second').toISOString(),
      service,
      level,
      message,
      logger: `${service}.${['main', 'worker', 'handler', 'processor'][Math.floor(Math.random() * 4)]}`,
      thread: `thread-${Math.floor(Math.random() * 10) + 1}`,
      file: `src/${service}/${['main', 'handler', 'utils', 'service'][Math.floor(Math.random() * 4)]}.rs`
    }
    
    // Add context for some logs
    if (Math.random() > 0.7) {
      log.context = {
        requestId: `req-${Math.random().toString(36).substr(2, 9)}`,
        userId: Math.floor(Math.random() * 100) + 1,
        channelId: Math.floor(Math.random() * 10) + 1
      }
    }
    
    // Add stack trace for errors
    if (level === 'error' || level === 'critical') {
      log.stackTrace = generateStackTrace(service)
    }
    
    logs.value.push(log)
  }
  
  // Sort by timestamp
  logs.value.sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime())
}

function generateStackTrace(service: string) {
  return `Error: Test error
    at ${service}::handler::process_request (src/${service}/handler.rs:45:10)
    at ${service}::main::{{closure}} (src/${service}/main.rs:123:5)
    at tokio::runtime::Runtime::block_on (tokio/src/runtime/mod.rs:456:15)
    at ${service}::main (src/${service}/main.rs:120:3)`
}

function startLiveMode() {
  logInterval = window.setInterval(() => {
    // Generate 1-3 new logs
    const count = Math.floor(Math.random() * 3) + 1
    for (let i = 0; i < count; i++) {
      generateNewLog()
    }
    
    // Update log rate
    logRate.value = Math.floor(Math.random() * 50) + 100
    
    // Remove old logs to prevent memory issues
    if (logs.value.length > 1000) {
      logs.value = logs.value.slice(-1000)
    }
  }, 1000)
}

function stopLiveMode() {
  if (logInterval) {
    clearInterval(logInterval)
    logInterval = null
  }
}

function generateNewLog() {
  const services = ['apigateway', 'comsrv', 'modsrv', 'hissrv', 'alarmsrv', 'rulesrv']
  const service = services[Math.floor(Math.random() * services.length)]
  
  // 70% info, 20% debug, 8% warning, 2% error/critical
  const rand = Math.random()
  let level = 'info'
  if (rand < 0.02) level = Math.random() > 0.5 ? 'error' : 'critical'
  else if (rand < 0.1) level = 'warning'
  else if (rand < 0.3) level = 'debug'
  
  const messages = {
    apigateway: [
      'GET /api/channels - 200 OK (15ms)',
      'WebSocket message received: subscribe',
      'JWT token validated successfully',
      'Rate limit check passed for user: admin'
    ],
    comsrv: [
      'Modbus read completed: 30001-30010',
      'Channel 1 polling cycle started',
      'Point update published: 10001 = 220.5',
      'Connection established to 192.168.1.100:502'
    ],
    modsrv: [
      'Calculation triggered for rule: CALC_001',
      'DAG execution completed in 12ms',
      'Published computed value: 85.5 kW',
      'Cache updated for channel: 1'
    ],
    hissrv: [
      'Batch write to InfluxDB: 1000 points',
      'Data retention policy applied',
      'Query executed: last 24 hours',
      'Compression completed: 15.2MB -> 3.1MB'
    ],
    alarmsrv: [
      'Alarm condition evaluated: temperature > 80',
      'New alarm triggered: ALM-001',
      'Alarm acknowledged by user: admin',
      'Notification sent via email'
    ],
    rulesrv: [
      'Rule engine cycle started',
      'Evaluating rule: RULE_001',
      'Condition matched, executing actions',
      'Rule execution completed: 3 actions'
    ]
  }
  
  const serviceMessages = messages[service as keyof typeof messages]
  const message = serviceMessages[Math.floor(Math.random() * serviceMessages.length)]
  
  const log = {
    id: Date.now() + Math.random(),
    timestamp: dayjs().toISOString(),
    service,
    level,
    message,
    logger: `${service}.${['main', 'worker', 'handler', 'processor'][Math.floor(Math.random() * 4)]}`,
    thread: `thread-${Math.floor(Math.random() * 10) + 1}`,
    file: `src/${service}/${['main', 'handler', 'utils', 'service'][Math.floor(Math.random() * 4)]}.rs`
  }
  
  // Add context occasionally
  if (Math.random() > 0.8) {
    log.context = {
      duration: `${Math.floor(Math.random() * 100)}ms`,
      count: Math.floor(Math.random() * 1000)
    }
  }
  
  logs.value.push(log)
}

function toggleLiveMode() {
  liveMode.value = !liveMode.value
  
  if (liveMode.value) {
    startLiveMode()
    autoScroll.value = true
  } else {
    stopLiveMode()
    autoScroll.value = false
  }
}

function clearLogs() {
  logs.value = []
  ElMessage.success('Logs cleared')
}

function downloadLogs() {
  const logText = filteredLogs.value
    .map(log => `${log.timestamp} [${log.service}] ${log.level.toUpperCase()} ${log.message}`)
    .join('\n')
  
  const blob = new Blob([logText], { type: 'text/plain' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `system_logs_${dayjs().format('YYYYMMDD_HHmmss')}.log`
  a.click()
  URL.revokeObjectURL(url)
  
  ElMessage.success('Logs downloaded')
}

function handleScroll() {
  if (!logContainer.value) return
  
  const { scrollTop, scrollHeight, clientHeight } = logContainer.value
  autoScroll.value = scrollHeight - scrollTop - clientHeight < 100
}

function scrollToBottom() {
  if (!logContainer.value) return
  logContainer.value.scrollTop = logContainer.value.scrollHeight
}

function selectLog(log: any) {
  selectedLog.value = log
  showDetailDialog.value = true
}

function copyLogDetail() {
  if (!selectedLog.value) return
  
  const text = JSON.stringify(selectedLog.value, null, 2)
  navigator.clipboard.writeText(text)
  ElMessage.success('Log copied to clipboard')
}

// Utility
function formatTimestamp(timestamp: string) {
  return dayjs(timestamp).format('HH:mm:ss.SSS')
}

function getLevelType(level: string) {
  switch (level) {
    case 'debug':
      return 'info'
    case 'info':
      return ''
    case 'warning':
      return 'warning'
    case 'error':
      return 'danger'
    case 'critical':
      return 'danger'
    default:
      return ''
  }
}

function getErrorRateColor(rate: number) {
  if (rate < 1) return '#67C23A'
  if (rate < 5) return '#E6A23C'
  return '#F56C6C'
}
</script>

<style lang="scss" scoped>
.system-log {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 20px;
  
  .log-controls-card {
    .log-controls {
      display: flex;
      align-items: center;
      gap: 10px;
      flex-wrap: wrap;
    }
  }
  
  .log-stats {
    :deep(.el-statistic__number) {
      font-size: 24px;
    }
  }
  
  .log-viewer-card {
    flex: 1;
    display: flex;
    flex-direction: column;
    
    :deep(.el-card__body) {
      flex: 1;
      display: flex;
      flex-direction: column;
      padding: 0;
    }
  }
  
  .log-viewer {
    flex: 1;
    display: flex;
    flex-direction: column;
    
    .log-header {
      display: flex;
      align-items: center;
      gap: 20px;
      padding: 15px 20px;
      border-bottom: 1px solid #e4e7ed;
      
      .el-checkbox {
        margin-right: 0;
      }
    }
    
    .log-container {
      flex: 1;
      overflow-y: auto;
      background: #1e1e1e;
      color: #d4d4d4;
      font-family: 'Consolas', 'Monaco', monospace;
      font-size: 13px;
      line-height: 1.6;
      
      &.wrap-lines {
        .log-line {
          white-space: pre-wrap;
          word-break: break-all;
        }
      }
      
      .log-line {
        padding: 2px 20px;
        white-space: nowrap;
        cursor: pointer;
        transition: background-color 0.2s;
        
        &:hover {
          background-color: #2a2a2a;
        }
        
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
        
        &.log-critical {
          color: #f48771;
          background-color: #5a1d1d;
        }
        
        .log-timestamp {
          color: #608b4e;
          margin-right: 10px;
        }
        
        .log-service {
          color: #569cd6;
          margin-right: 10px;
        }
        
        .log-level {
          margin-right: 10px;
          font-weight: bold;
          min-width: 60px;
          display: inline-block;
        }
        
        .log-context {
          color: #9cdcfe;
          margin-left: 10px;
          opacity: 0.8;
        }
      }
    }
    
    .log-footer {
      display: flex;
      justify-content: space-between;
      align-items: center;
      padding: 10px 20px;
      border-top: 1px solid #e4e7ed;
    }
  }
  
  .log-detail-message,
  .log-detail-context,
  .log-detail-stack {
    background: #f5f7fa;
    padding: 10px;
    border-radius: 4px;
    white-space: pre-wrap;
    word-break: break-all;
    font-family: 'Consolas', 'Monaco', monospace;
    font-size: 12px;
    margin: 0;
  }
}
</style>