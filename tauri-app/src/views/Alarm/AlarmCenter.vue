<template>
  <div class="alarm-center">
    <!-- Alarm Statistics -->
    <el-row :gutter="20" class="alarm-stats">
      <el-col :span="6">
        <el-card>
          <el-statistic title="Active Alarms" :value="activeAlarms">
            <template #prefix>
              <el-icon color="#F56C6C"><Warning /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card>
          <el-statistic title="Acknowledged" :value="acknowledgedAlarms">
            <template #prefix>
              <el-icon color="#E6A23C"><Check /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card>
          <el-statistic title="Today's Alarms" :value="todayAlarms">
            <template #prefix>
              <el-icon color="#409EFF"><Calendar /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card>
          <el-statistic title="Critical Alarms" :value="criticalAlarms">
            <template #prefix>
              <el-icon color="#F56C6C"><WarnTriangleFilled /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
    </el-row>
    
    <!-- Alarm Toolbar -->
    <el-card class="alarm-toolbar-card">
      <div class="alarm-toolbar">
        <el-space>
          <el-select v-model="filterLevel" placeholder="Alarm Level" clearable>
            <el-option label="Critical" value="critical" />
            <el-option label="Major" value="major" />
            <el-option label="Minor" value="minor" />
            <el-option label="Warning" value="warning" />
            <el-option label="Info" value="info" />
          </el-select>
          
          <el-select v-model="filterStatus" placeholder="Status" clearable>
            <el-option label="Active" value="active" />
            <el-option label="Acknowledged" value="acknowledged" />
            <el-option label="Cleared" value="cleared" />
          </el-select>
          
          <el-date-picker
            v-model="filterTimeRange"
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
            placeholder="Search alarms..."
            :prefix-icon="Search"
            clearable
            style="width: 300px"
          />
        </el-space>
        
        <el-space>
          <el-button @click="refreshAlarms" :loading="refreshing">
            <el-icon><Refresh /></el-icon>
            Refresh
          </el-button>
          
          <el-button type="primary" @click="acknowledgeSelected" :disabled="selectedAlarms.length === 0">
            <el-icon><Check /></el-icon>
            Acknowledge Selected
          </el-button>
          
          <el-button @click="exportAlarms">
            <el-icon><Download /></el-icon>
            Export
          </el-button>
          
          <el-button @click="showSettings = true">
            <el-icon><Setting /></el-icon>
            Settings
          </el-button>
        </el-space>
      </div>
    </el-card>
    
    <!-- Alarm Table -->
    <el-card class="alarm-table-card">
      <el-table
        :data="filteredAlarms"
        v-loading="loading"
        @selection-change="handleSelectionChange"
        :row-class-name="getRowClassName"
        style="width: 100%"
      >
        <el-table-column type="selection" width="55" />
        
        <el-table-column label="Severity" width="100">
          <template #default="{ row }">
            <div class="severity-indicator">
              <el-icon :size="20" :color="getSeverityColor(row.level)">
                <WarnTriangleFilled v-if="row.level === 'critical'" />
                <WarningFilled v-else-if="row.level === 'major'" />
                <Warning v-else />
              </el-icon>
              <span>{{ row.level }}</span>
            </div>
          </template>
        </el-table-column>
        
        <el-table-column prop="timestamp" label="Time" width="180">
          <template #default="{ row }">
            {{ formatTime(row.timestamp) }}
          </template>
        </el-table-column>
        
        <el-table-column prop="source" label="Source" width="150" />
        
        <el-table-column prop="description" label="Description" min-width="300" show-overflow-tooltip />
        
        <el-table-column prop="value" label="Value" width="120">
          <template #default="{ row }">
            <span class="alarm-value">{{ row.value }} {{ row.unit }}</span>
          </template>
        </el-table-column>
        
        <el-table-column prop="status" label="Status" width="120">
          <template #default="{ row }">
            <el-tag :type="getStatusType(row.status)">
              {{ row.status }}
            </el-tag>
          </template>
        </el-table-column>
        
        <el-table-column prop="acknowledgedBy" label="Ack By" width="120" />
        
        <el-table-column label="Actions" width="180" fixed="right">
          <template #default="{ row }">
            <el-button
              v-if="row.status === 'active'"
              type="primary"
              size="small"
              text
              @click="acknowledgeAlarm(row)"
            >
              Acknowledge
            </el-button>
            <el-button
              type="info"
              size="small"
              text
              @click="viewDetails(row)"
            >
              Details
            </el-button>
            <el-button
              type="danger"
              size="small"
              text
              @click="deleteAlarm(row)"
            >
              Delete
            </el-button>
          </template>
        </el-table-column>
      </el-table>
      
      <el-pagination
        v-model:current-page="currentPage"
        v-model:page-size="pageSize"
        :page-sizes="[20, 50, 100, 200]"
        :total="totalAlarms"
        layout="total, sizes, prev, pager, next, jumper"
        style="margin-top: 20px"
      />
    </el-card>
    
    <!-- Alarm Details Dialog -->
    <el-dialog
      v-model="showDetailsDialog"
      :title="`Alarm Details - ${selectedAlarm?.description}`"
      width="60%"
    >
      <el-descriptions :column="2" border v-if="selectedAlarm">
        <el-descriptions-item label="Alarm ID">{{ selectedAlarm.id }}</el-descriptions-item>
        <el-descriptions-item label="Severity">
          <el-tag :type="getLevelType(selectedAlarm.level)">{{ selectedAlarm.level }}</el-tag>
        </el-descriptions-item>
        <el-descriptions-item label="Source">{{ selectedAlarm.source }}</el-descriptions-item>
        <el-descriptions-item label="Channel">{{ selectedAlarm.channel }}</el-descriptions-item>
        <el-descriptions-item label="Point ID">{{ selectedAlarm.pointId }}</el-descriptions-item>
        <el-descriptions-item label="Status">
          <el-tag :type="getStatusType(selectedAlarm.status)">{{ selectedAlarm.status }}</el-tag>
        </el-descriptions-item>
        <el-descriptions-item label="Triggered At">{{ formatTime(selectedAlarm.timestamp) }}</el-descriptions-item>
        <el-descriptions-item label="Duration">{{ calculateDuration(selectedAlarm.timestamp) }}</el-descriptions-item>
        <el-descriptions-item label="Value">{{ selectedAlarm.value }} {{ selectedAlarm.unit }}</el-descriptions-item>
        <el-descriptions-item label="Threshold">{{ selectedAlarm.threshold }} {{ selectedAlarm.unit }}</el-descriptions-item>
        <el-descriptions-item label="Acknowledged By">{{ selectedAlarm.acknowledgedBy || 'N/A' }}</el-descriptions-item>
        <el-descriptions-item label="Acknowledged At">{{ selectedAlarm.acknowledgedAt ? formatTime(selectedAlarm.acknowledgedAt) : 'N/A' }}</el-descriptions-item>
        <el-descriptions-item label="Description" :span="2">{{ selectedAlarm.description }}</el-descriptions-item>
        <el-descriptions-item label="Additional Info" :span="2">
          <pre>{{ JSON.stringify(selectedAlarm.metadata, null, 2) }}</pre>
        </el-descriptions-item>
      </el-descriptions>
      
      <div class="alarm-actions" style="margin-top: 20px">
        <el-input
          v-model="acknowledgeNote"
          type="textarea"
          placeholder="Add note (optional)"
          :rows="3"
          style="margin-bottom: 10px"
        />
        <el-button type="primary" @click="acknowledgeWithNote" v-if="selectedAlarm?.status === 'active'">
          Acknowledge with Note
        </el-button>
      </div>
      
      <h4 style="margin-top: 30px">Alarm History</h4>
      <el-timeline>
        <el-timeline-item
          v-for="(event, index) in alarmHistory"
          :key="index"
          :timestamp="event.timestamp"
          :type="event.type"
        >
          {{ event.description }}
        </el-timeline-item>
      </el-timeline>
    </el-dialog>
    
    <!-- Settings Dialog -->
    <el-dialog
      v-model="showSettings"
      title="Alarm Settings"
      width="600px"
    >
      <el-form label-width="150px">
        <el-form-item label="Sound Alerts">
          <el-switch v-model="settings.soundEnabled" />
        </el-form-item>
        
        <el-form-item label="Critical Alarm Sound">
          <el-select v-model="settings.criticalSound">
            <el-option label="Siren" value="siren" />
            <el-option label="Bell" value="bell" />
            <el-option label="Buzzer" value="buzzer" />
          </el-select>
          <el-button @click="testSound('critical')" style="margin-left: 10px">Test</el-button>
        </el-form-item>
        
        <el-form-item label="Desktop Notifications">
          <el-switch v-model="settings.notificationsEnabled" />
        </el-form-item>
        
        <el-form-item label="Auto Refresh">
          <el-switch v-model="settings.autoRefresh" />
        </el-form-item>
        
        <el-form-item label="Refresh Interval">
          <el-input-number
            v-model="settings.refreshInterval"
            :min="5"
            :max="300"
            :step="5"
            :disabled="!settings.autoRefresh"
          />
          <span style="margin-left: 10px">seconds</span>
        </el-form-item>
        
        <el-form-item label="Alarm Retention">
          <el-input-number
            v-model="settings.retentionDays"
            :min="1"
            :max="365"
          />
          <span style="margin-left: 10px">days</span>
        </el-form-item>
        
        <el-form-item label="Priority Levels">
          <el-checkbox-group v-model="settings.priorityLevels">
            <el-checkbox label="critical">Critical</el-checkbox>
            <el-checkbox label="major">Major</el-checkbox>
            <el-checkbox label="minor">Minor</el-checkbox>
            <el-checkbox label="warning">Warning</el-checkbox>
            <el-checkbox label="info">Info</el-checkbox>
          </el-checkbox-group>
        </el-form-item>
      </el-form>
      
      <template #footer>
        <el-button @click="showSettings = false">Cancel</el-button>
        <el-button type="primary" @click="saveSettings">Save Settings</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import {
  Warning,
  Check,
  Calendar,
  WarnTriangleFilled,
  WarningFilled,
  Search,
  Refresh,
  Download,
  Setting
} from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox, ElNotification } from 'element-plus'
import dayjs from 'dayjs'
import relativeTime from 'dayjs/plugin/relativeTime'

dayjs.extend(relativeTime)

// Mock alarm data
const alarms = ref([
  {
    id: 'ALM-001',
    level: 'critical',
    timestamp: dayjs().subtract(5, 'minute').toISOString(),
    source: 'Main Power System',
    channel: 'Channel 1',
    pointId: 10001,
    description: 'Voltage exceeds upper limit',
    value: 265,
    unit: 'V',
    threshold: 250,
    status: 'active',
    acknowledgedBy: null,
    acknowledgedAt: null,
    metadata: {
      ruleId: 'RULE-001',
      tags: ['power', 'voltage', 'critical']
    }
  },
  {
    id: 'ALM-002',
    level: 'major',
    timestamp: dayjs().subtract(10, 'minute').toISOString(),
    source: 'Solar Panel',
    channel: 'Channel 2',
    pointId: 20001,
    description: 'Low power generation efficiency',
    value: 45,
    unit: '%',
    threshold: 60,
    status: 'acknowledged',
    acknowledgedBy: 'Admin',
    acknowledgedAt: dayjs().subtract(8, 'minute').toISOString(),
    metadata: {
      ruleId: 'RULE-002',
      tags: ['solar', 'efficiency', 'warning']
    }
  },
  {
    id: 'ALM-003',
    level: 'minor',
    timestamp: dayjs().subtract(30, 'minute').toISOString(),
    source: 'Energy Storage',
    channel: 'Channel 3',
    pointId: 30001,
    description: 'Battery temperature slightly elevated',
    value: 38,
    unit: '°C',
    threshold: 35,
    status: 'active',
    acknowledgedBy: null,
    acknowledgedAt: null,
    metadata: {
      ruleId: 'RULE-003',
      tags: ['battery', 'temperature', 'minor']
    }
  },
  {
    id: 'ALM-004',
    level: 'warning',
    timestamp: dayjs().subtract(1, 'hour').toISOString(),
    source: 'Diesel Generator',
    channel: 'Channel 4',
    pointId: 40001,
    description: 'Maintenance due in 50 hours',
    value: 950,
    unit: 'hours',
    threshold: 1000,
    status: 'cleared',
    acknowledgedBy: 'Operator',
    acknowledgedAt: dayjs().subtract(45, 'minute').toISOString(),
    metadata: {
      ruleId: 'RULE-004',
      tags: ['maintenance', 'generator', 'info']
    }
  }
])

// Statistics
const activeAlarms = computed(() => alarms.value.filter(a => a.status === 'active').length)
const acknowledgedAlarms = computed(() => alarms.value.filter(a => a.status === 'acknowledged').length)
const todayAlarms = computed(() => {
  const today = dayjs().startOf('day')
  return alarms.value.filter(a => dayjs(a.timestamp).isAfter(today)).length
})
const criticalAlarms = computed(() => alarms.value.filter(a => a.level === 'critical' && a.status === 'active').length)

// Filter state
const filterLevel = ref('')
const filterStatus = ref('')
const filterTimeRange = ref<string[]>([])
const searchQuery = ref('')

// Table state
const loading = ref(false)
const refreshing = ref(false)
const selectedAlarms = ref<any[]>([])
const currentPage = ref(1)
const pageSize = ref(20)
const totalAlarms = ref(150)

// Dialog state
const showDetailsDialog = ref(false)
const selectedAlarm = ref<any>(null)
const acknowledgeNote = ref('')
const alarmHistory = ref<any[]>([])

// Settings
const showSettings = ref(false)
const settings = ref({
  soundEnabled: true,
  criticalSound: 'siren',
  notificationsEnabled: true,
  autoRefresh: true,
  refreshInterval: 30,
  retentionDays: 30,
  priorityLevels: ['critical', 'major', 'minor', 'warning']
})

// Time shortcuts
const timeShortcuts = [
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
  },
  {
    text: 'Last 7 days',
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setTime(start.getTime() - 3600 * 1000 * 24 * 7)
      return [start, end]
    }
  }
]

// Auto refresh
let refreshInterval: number | null = null

// Computed
const filteredAlarms = computed(() => {
  let filtered = alarms.value
  
  if (filterLevel.value) {
    filtered = filtered.filter(a => a.level === filterLevel.value)
  }
  
  if (filterStatus.value) {
    filtered = filtered.filter(a => a.status === filterStatus.value)
  }
  
  if (filterTimeRange.value?.length === 2) {
    const [start, end] = filterTimeRange.value
    filtered = filtered.filter(a => {
      const alarmTime = dayjs(a.timestamp)
      return alarmTime.isAfter(start) && alarmTime.isBefore(end)
    })
  }
  
  if (searchQuery.value) {
    const search = searchQuery.value.toLowerCase()
    filtered = filtered.filter(a => 
      a.description.toLowerCase().includes(search) ||
      a.source.toLowerCase().includes(search) ||
      a.pointId.toString().includes(search)
    )
  }
  
  return filtered
})

// Watch for new critical alarms
watch(() => alarms.value.filter(a => a.level === 'critical' && a.status === 'active').length, (newCount, oldCount) => {
  if (newCount > oldCount && settings.value.soundEnabled) {
    playAlarmSound('critical')
  }
  
  if (newCount > oldCount && settings.value.notificationsEnabled) {
    showNotification('Critical Alarm', 'New critical alarm detected!')
  }
})

onMounted(() => {
  startAutoRefresh()
  
  // Simulate real-time alarm updates
  setInterval(() => {
    if (Math.random() > 0.8) {
      addNewAlarm()
    }
  }, 10000)
})

onUnmounted(() => {
  stopAutoRefresh()
})

// Methods
async function refreshAlarms() {
  refreshing.value = true
  
  try {
    // TODO: Fetch alarms from API
    await new Promise(resolve => setTimeout(resolve, 1000))
    ElMessage.success('Alarms refreshed')
  } finally {
    refreshing.value = false
  }
}

function handleSelectionChange(selection: any[]) {
  selectedAlarms.value = selection
}

async function acknowledgeAlarm(alarm: any) {
  alarm.status = 'acknowledged'
  alarm.acknowledgedBy = 'Admin'
  alarm.acknowledgedAt = dayjs().toISOString()
  
  ElMessage.success('Alarm acknowledged')
}

async function acknowledgeSelected() {
  const activeSelected = selectedAlarms.value.filter(a => a.status === 'active')
  
  if (activeSelected.length === 0) {
    ElMessage.warning('No active alarms selected')
    return
  }
  
  await ElMessageBox.confirm(
    `Acknowledge ${activeSelected.length} selected alarms?`,
    'Confirm Acknowledge',
    {
      confirmButtonText: 'Acknowledge',
      cancelButtonText: 'Cancel',
      type: 'warning'
    }
  )
  
  activeSelected.forEach(alarm => {
    alarm.status = 'acknowledged'
    alarm.acknowledgedBy = 'Admin'
    alarm.acknowledgedAt = dayjs().toISOString()
  })
  
  ElMessage.success(`${activeSelected.length} alarms acknowledged`)
}

function viewDetails(alarm: any) {
  selectedAlarm.value = alarm
  showDetailsDialog.value = true
  
  // Load alarm history
  alarmHistory.value = [
    {
      timestamp: alarm.timestamp,
      type: 'primary',
      description: 'Alarm triggered'
    }
  ]
  
  if (alarm.acknowledgedAt) {
    alarmHistory.value.push({
      timestamp: alarm.acknowledgedAt,
      type: 'success',
      description: `Acknowledged by ${alarm.acknowledgedBy}`
    })
  }
  
  if (alarm.status === 'cleared') {
    alarmHistory.value.push({
      timestamp: dayjs().subtract(30, 'minute').toISOString(),
      type: 'info',
      description: 'Alarm condition cleared'
    })
  }
}

async function deleteAlarm(alarm: any) {
  await ElMessageBox.confirm(
    'Delete this alarm record?',
    'Confirm Delete',
    {
      confirmButtonText: 'Delete',
      cancelButtonText: 'Cancel',
      type: 'warning'
    }
  )
  
  const index = alarms.value.findIndex(a => a.id === alarm.id)
  if (index > -1) {
    alarms.value.splice(index, 1)
    ElMessage.success('Alarm deleted')
  }
}

function acknowledgeWithNote() {
  if (selectedAlarm.value) {
    selectedAlarm.value.status = 'acknowledged'
    selectedAlarm.value.acknowledgedBy = 'Admin'
    selectedAlarm.value.acknowledgedAt = dayjs().toISOString()
    
    if (acknowledgeNote.value) {
      selectedAlarm.value.metadata.acknowledgeNote = acknowledgeNote.value
    }
    
    showDetailsDialog.value = false
    acknowledgeNote.value = ''
    ElMessage.success('Alarm acknowledged with note')
  }
}

function exportAlarms() {
  // TODO: Implement export functionality
  ElMessage.success('Exporting alarms...')
}

function saveSettings() {
  ElMessage.success('Settings saved')
  showSettings.value = false
  
  // Restart auto refresh if needed
  stopAutoRefresh()
  if (settings.value.autoRefresh) {
    startAutoRefresh()
  }
}

function testSound(level: string) {
  playAlarmSound(level)
}

function playAlarmSound(level: string) {
  // TODO: Implement actual sound playback
  console.log(`Playing ${level} alarm sound: ${settings.value.criticalSound}`)
}

function showNotification(title: string, message: string) {
  ElNotification({
    title,
    message,
    type: 'warning',
    duration: 0
  })
}

function startAutoRefresh() {
  if (settings.value.autoRefresh) {
    refreshInterval = window.setInterval(() => {
      refreshAlarms()
    }, settings.value.refreshInterval * 1000)
  }
}

function stopAutoRefresh() {
  if (refreshInterval) {
    clearInterval(refreshInterval)
    refreshInterval = null
  }
}

function addNewAlarm() {
  const newAlarm = {
    id: `ALM-${Date.now()}`,
    level: ['critical', 'major', 'minor', 'warning'][Math.floor(Math.random() * 4)],
    timestamp: dayjs().toISOString(),
    source: ['Main Power System', 'Solar Panel', 'Energy Storage', 'Diesel Generator'][Math.floor(Math.random() * 4)],
    channel: `Channel ${Math.floor(Math.random() * 4) + 1}`,
    pointId: Math.floor(Math.random() * 40000) + 10000,
    description: 'New alarm condition detected',
    value: Math.floor(Math.random() * 100),
    unit: 'units',
    threshold: 80,
    status: 'active',
    acknowledgedBy: null,
    acknowledgedAt: null,
    metadata: {
      ruleId: `RULE-${Math.floor(Math.random() * 100)}`,
      tags: ['auto', 'generated']
    }
  }
  
  alarms.value.unshift(newAlarm)
}

// Utility methods
function formatTime(timestamp: string) {
  return dayjs(timestamp).format('YYYY-MM-DD HH:mm:ss')
}

function calculateDuration(timestamp: string) {
  return dayjs(timestamp).fromNow(true)
}

function getRowClassName({ row }: { row: any }) {
  if (row.status === 'active' && row.level === 'critical') {
    return 'critical-row'
  }
  return ''
}

function getSeverityColor(level: string) {
  switch (level) {
    case 'critical':
      return '#F56C6C'
    case 'major':
      return '#E6A23C'
    case 'minor':
      return '#F6CA57'
    case 'warning':
      return '#909399'
    case 'info':
      return '#409EFF'
    default:
      return '#909399'
  }
}

function getLevelType(level: string) {
  switch (level) {
    case 'critical':
      return 'danger'
    case 'major':
      return 'warning'
    case 'minor':
      return 'warning'
    case 'warning':
      return 'info'
    case 'info':
      return ''
    default:
      return 'info'
  }
}

function getStatusType(status: string) {
  switch (status) {
    case 'active':
      return 'danger'
    case 'acknowledged':
      return 'warning'
    case 'cleared':
      return 'success'
    default:
      return 'info'
  }
}
</script>

<style lang="scss" scoped>
.alarm-center {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 20px;
  
  .alarm-stats {
    :deep(.el-statistic__number) {
      font-size: 28px;
    }
  }
  
  .alarm-toolbar-card {
    .alarm-toolbar {
      display: flex;
      justify-content: space-between;
      align-items: center;
      flex-wrap: wrap;
      gap: 10px;
    }
  }
  
  .alarm-table-card {
    flex: 1;
    
    :deep(.critical-row) {
      background-color: #fef0f0;
      
      &:hover {
        background-color: #fee !important;
      }
    }
    
    .severity-indicator {
      display: flex;
      align-items: center;
      gap: 5px;
      text-transform: uppercase;
      font-weight: 500;
      font-size: 12px;
    }
    
    .alarm-value {
      font-weight: 600;
      color: #606266;
    }
  }
  
  .alarm-actions {
    padding: 20px 0;
  }
}
</style>