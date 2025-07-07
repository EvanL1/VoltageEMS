<template>
  <div class="alarm-management-container">
    <div class="page-header">
      <h1>{{ $t('menu.alarmManagement') }}</h1>
      <div class="header-actions">
        <el-button @click="refreshAlarms">
          <el-icon><Refresh /></el-icon>
          {{ $t('common.refresh') }}
        </el-button>
        <el-button 
          type="success" 
          @click="confirmSelected"
          :disabled="selectedAlarms.length === 0 || !checkPermission(PERMISSIONS.CONTROL.ALARM_CONFIRM)"
        >
          <el-icon><CircleCheck /></el-icon>
          {{ $t('alarms.confirmSelected') }}
        </el-button>
        <el-button 
          type="danger" 
          @click="clearSelected"
          :disabled="selectedAlarms.length === 0 || !checkPermission(PERMISSIONS.CONTROL.ALARM_DELETE)"
        >
          <el-icon><Delete /></el-icon>
          {{ $t('alarms.clearSelected') }}
        </el-button>
      </div>
    </div>

    <!-- 统计卡片 -->
    <el-row :gutter="20" class="stats-row">
      <el-col :xs="12" :sm="6">
        <el-card class="stat-card">
          <div class="stat-icon critical">
            <el-icon :size="24"><Warning /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ stats.critical }}</div>
            <div class="stat-label">{{ $t('alarms.levels.critical') }}</div>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="6">
        <el-card class="stat-card">
          <div class="stat-icon high">
            <el-icon :size="24"><WarningFilled /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ stats.high }}</div>
            <div class="stat-label">{{ $t('alarms.levels.high') }}</div>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="6">
        <el-card class="stat-card">
          <div class="stat-icon medium">
            <el-icon :size="24"><InfoFilled /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ stats.medium }}</div>
            <div class="stat-label">{{ $t('alarms.levels.medium') }}</div>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="6">
        <el-card class="stat-card">
          <div class="stat-icon total">
            <el-icon :size="24"><Bell /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ stats.total }}</div>
            <div class="stat-label">{{ $t('alarms.totalActive') }}</div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 过滤器 -->
    <el-card class="filter-card">
      <el-form :inline="true" :model="filterForm" class="filter-form">
        <el-form-item :label="$t('alarms.alarmLevel')">
          <el-select 
            v-model="filterForm.level" 
            :placeholder="$t('common.all')"
            clearable
            @change="handleFilter"
          >
            <el-option 
              v-for="level in alarmLevels" 
              :key="level.value" 
              :label="level.label" 
              :value="level.value"
            />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('alarms.alarmSource')">
          <el-input 
            v-model="filterForm.source" 
            :placeholder="$t('alarms.enterSource')"
            clearable
            @clear="handleFilter"
            @keyup.enter="handleFilter"
          />
        </el-form-item>
        <el-form-item :label="$t('alarms.status.label')">
          <el-select 
            v-model="filterForm.status" 
            :placeholder="$t('common.all')"
            clearable
            @change="handleFilter"
          >
            <el-option 
              v-for="status in statusOptions" 
              :key="status.value" 
              :label="status.label" 
              :value="status.value"
            />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('common.timeRange')">
          <el-date-picker
            v-model="filterForm.timeRange"
            type="datetimerange"
            :range-separator="$t('common.to')"
            :start-placeholder="$t('common.startTime')"
            :end-placeholder="$t('common.endTime')"
            @change="handleFilter"
          />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="handleFilter">
            {{ $t('common.search') }}
          </el-button>
          <el-button @click="resetFilter">
            {{ $t('common.reset') }}
          </el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <!-- 告警列表 -->
    <el-card>
      <el-table 
        :data="filteredAlarms" 
        style="width: 100%"
        @selection-change="handleSelectionChange"
        :row-class-name="tableRowClassName"
      >
        <el-table-column 
          type="selection" 
          width="55"
          :selectable="row => checkAnyPermission([PERMISSIONS.CONTROL.ALARM_CONFIRM, PERMISSIONS.CONTROL.ALARM_DELETE])"
        />
        <el-table-column 
          prop="id" 
          label="ID" 
          width="80"
        />
        <el-table-column 
          prop="level" 
          :label="$t('alarms.alarmLevel')" 
          width="100"
        >
          <template #default="{ row }">
            <el-tag 
              :type="getLevelType(row.level)" 
              disable-transitions
            >
              {{ $t(`alarms.levels.${row.level}`) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column 
          prop="source" 
          :label="$t('alarms.alarmSource')" 
          width="150"
        />
        <el-table-column 
          prop="message" 
          :label="$t('alarms.alarmMessage')"
          show-overflow-tooltip
        />
        <el-table-column 
          prop="time" 
          :label="$t('alarms.alarmTime')" 
          width="180"
        >
          <template #default="{ row }">
            {{ formatDateTime(row.time) }}
          </template>
        </el-table-column>
        <el-table-column 
          prop="status" 
          :label="$t('alarms.status.label')" 
          width="100"
        >
          <template #default="{ row }">
            <el-tag :type="getStatusType(row.status)" size="small">
              {{ $t(`alarms.status.${row.status}`) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column 
          prop="confirmedBy" 
          :label="$t('alarms.confirmedBy')" 
          width="120"
        >
          <template #default="{ row }">
            {{ row.confirmedBy || '-' }}
          </template>
        </el-table-column>
        <el-table-column 
          :label="$t('common.operation')" 
          width="200"
          fixed="right"
        >
          <template #default="{ row }">
            <el-button 
              type="primary" 
              size="small" 
              link
              @click="viewDetails(row)"
            >
              {{ $t('common.details') }}
            </el-button>
            <el-button 
              v-if="row.status === 'active' && checkPermission(PERMISSIONS.CONTROL.ALARM_CONFIRM)" 
              type="success" 
              size="small" 
              link
              @click="confirmAlarm(row)"
            >
              {{ $t('alarms.acknowledgeAlarm') }}
            </el-button>
            <el-button 
              v-if="row.status !== 'cleared' && checkPermission(PERMISSIONS.CONTROL.ALARM_HANDLE)" 
              type="warning" 
              size="small" 
              link
              @click="handleAlarm(row)"
            >
              {{ $t('alarms.handleAlarm') }}
            </el-button>
            <el-button 
              v-if="row.status !== 'cleared' && checkPermission(PERMISSIONS.CONTROL.ALARM_DELETE)" 
              type="danger" 
              size="small" 
              link
              @click="clearAlarm(row)"
            >
              {{ $t('alarms.clearAlarm') }}
            </el-button>
          </template>
        </el-table-column>
      </el-table>

      <!-- 分页 -->
      <el-pagination
        v-model:current-page="currentPage"
        v-model:page-size="pageSize"
        :page-sizes="[10, 20, 50, 100]"
        :total="totalAlarms"
        layout="total, sizes, prev, pager, next, jumper"
        class="pagination"
        @size-change="handleSizeChange"
        @current-change="handleCurrentChange"
      />
    </el-card>

    <!-- 告警详情对话框 -->
    <el-dialog 
      v-model="showDetailsDialog" 
      :title="$t('alarms.alarmDetails')"
      width="700px"
    >
      <el-descriptions v-if="selectedAlarm" :column="2" border>
        <el-descriptions-item :label="$t('alarms.alarmId')">
          {{ selectedAlarm.id }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('alarms.alarmLevel')">
          <el-tag :type="getLevelType(selectedAlarm.level)">
            {{ $t(`alarms.levels.${selectedAlarm.level}`) }}
          </el-tag>
        </el-descriptions-item>
        <el-descriptions-item :label="$t('alarms.alarmSource')">
          {{ selectedAlarm.source }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('alarms.alarmTime')">
          {{ formatDateTime(selectedAlarm.time) }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('alarms.status.label')" :span="2">
          <el-tag :type="getStatusType(selectedAlarm.status)">
            {{ $t(`alarms.status.${selectedAlarm.status}`) }}
          </el-tag>
        </el-descriptions-item>
        <el-descriptions-item :label="$t('alarms.alarmMessage')" :span="2">
          {{ selectedAlarm.message }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('alarms.deviceInfo')" :span="2">
          <div>{{ $t('alarms.deviceName') }}: {{ selectedAlarm.deviceName }}</div>
          <div>{{ $t('alarms.deviceType') }}: {{ selectedAlarm.deviceType }}</div>
          <div>{{ $t('alarms.location') }}: {{ selectedAlarm.location }}</div>
        </el-descriptions-item>
        <el-descriptions-item :label="$t('alarms.handleInfo')" :span="2">
          <div v-if="selectedAlarm.confirmedBy">
            {{ $t('alarms.confirmedBy') }}: {{ selectedAlarm.confirmedBy }}
            <br>
            {{ $t('alarms.confirmedTime') }}: {{ formatDateTime(selectedAlarm.confirmedTime) }}
          </div>
          <div v-if="selectedAlarm.handledBy">
            {{ $t('alarms.handledBy') }}: {{ selectedAlarm.handledBy }}
            <br>
            {{ $t('alarms.handledTime') }}: {{ formatDateTime(selectedAlarm.handledTime) }}
          </div>
          <div v-if="selectedAlarm.clearedBy">
            {{ $t('alarms.clearedBy') }}: {{ selectedAlarm.clearedBy }}
            <br>
            {{ $t('alarms.clearedTime') }}: {{ formatDateTime(selectedAlarm.clearedTime) }}
          </div>
          <div v-if="!selectedAlarm.confirmedBy && !selectedAlarm.handledBy && !selectedAlarm.clearedBy">
            {{ $t('alarms.notHandled') }}
          </div>
        </el-descriptions-item>
        <el-descriptions-item :label="$t('alarms.suggestion')" :span="2">
          {{ selectedAlarm.suggestion || $t('common.none') }}
        </el-descriptions-item>
      </el-descriptions>

      <!-- 处理历史 -->
      <div class="handle-history" v-if="selectedAlarm?.history?.length > 0">
        <h4>{{ $t('alarms.handleHistory') }}</h4>
        <el-timeline>
          <el-timeline-item 
            v-for="(item, index) in selectedAlarm.history" 
            :key="index"
            :timestamp="formatDateTime(item.time)"
            :type="item.type"
          >
            {{ item.action }} - {{ item.operator }}
            <div v-if="item.remark" class="history-remark">
              {{ $t('common.remark') }}: {{ item.remark }}
            </div>
          </el-timeline-item>
        </el-timeline>
      </div>

      <template #footer>
        <el-button @click="showDetailsDialog = false">{{ $t('common.close') }}</el-button>
        <el-button 
          v-if="selectedAlarm?.status === 'active' && checkPermission(PERMISSIONS.CONTROL.ALARM_CONFIRM)" 
          type="success" 
          @click="confirmAlarm(selectedAlarm)"
        >
          {{ $t('alarms.acknowledgeAlarm') }}
        </el-button>
        <el-button 
          v-if="selectedAlarm?.status !== 'cleared' && checkPermission(PERMISSIONS.CONTROL.ALARM_HANDLE)" 
          type="warning" 
          @click="handleAlarm(selectedAlarm)"
        >
          {{ $t('alarms.handleAlarm') }}
        </el-button>
        <el-button 
          v-if="selectedAlarm?.status !== 'cleared' && checkPermission(PERMISSIONS.CONTROL.ALARM_DELETE)" 
          type="danger" 
          @click="clearAlarm(selectedAlarm)"
        >
          {{ $t('alarms.clearAlarm') }}
        </el-button>
      </template>
    </el-dialog>

    <!-- 确认/清除对话框 -->
    <el-dialog 
      v-model="showHandleDialog" 
      :title="handleType === 'confirm' ? $t('alarms.confirmAlarmTitle') : handleType === 'handle' ? $t('alarms.handleAlarmTitle') : $t('alarms.clearAlarmTitle')"
      width="500px"
    >
      <el-form :model="handleForm" label-width="100px">
        <el-form-item :label="$t('alarms.alarmInfo')">
          <div class="alarm-info">
            <div>ID: {{ handleForm.alarm?.id }}</div>
            <div>{{ $t('alarms.alarmSource') }}: {{ handleForm.alarm?.source }}</div>
            <div>{{ $t('alarms.alarmMessage') }}: {{ handleForm.alarm?.message }}</div>
          </div>
        </el-form-item>
        <el-form-item :label="$t('common.remark')">
          <el-input 
            v-model="handleForm.remark" 
            type="textarea" 
            :rows="3"
            :placeholder="$t('alarms.handleRemarkPlaceholder')"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showHandleDialog = false">{{ $t('common.cancel') }}</el-button>
        <el-button 
          :type="handleType === 'confirm' ? 'success' : handleType === 'handle' ? 'warning' : 'danger'" 
          @click="submitHandle"
          :loading="handleLoading"
        >
          {{ $t('common.confirm') }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, watchEffect } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { 
  Refresh, 
  CircleCheck, 
  Delete, 
  Warning, 
  WarningFilled, 
  InfoFilled, 
  Bell 
} from '@element-plus/icons-vue'
import dayjs from 'dayjs'
import { useUserStore } from '@/stores/user'
import { usePermission } from '@/composables/usePermission'
// import { useAlarmStore } from '@/stores/alarm'

const { t } = useI18n()
const userStore = useUserStore()
const { PERMISSIONS, checkPermission, checkAnyPermission } = usePermission()
// const alarmStore = useAlarmStore() // Commented out as it's not used

// 统计数据
const stats = ref({
  critical: 0,
  high: 0,
  medium: 0,
  total: 0
})

// 计算属性 - 检查用户权限
// 这些权限检查已经改为在具体操作时使用 checkPermission 函数

// 过滤表单
const filterForm = ref({
  level: '',
  source: '',
  status: '',
  timeRange: null
})

// 告警级别选项
const alarmLevels = [
  { value: 'critical', label: t('alarms.levels.critical') },
  { value: 'high', label: t('alarms.levels.high') },
  { value: 'medium', label: t('alarms.levels.medium') },
  { value: 'low', label: t('alarms.levels.low') },
  { value: 'info', label: t('alarms.levels.info') }
]

// 状态选项
const statusOptions = [
  { value: 'active', label: t('alarms.status.active') },
  { value: 'acknowledged', label: t('alarms.status.acknowledged') },
  { value: 'handled', label: t('alarms.status.handled') },
  { value: 'cleared', label: t('alarms.status.cleared') }
]

// 告警列表
const alarms = ref([])
const selectedAlarms = ref([])
const currentPage = ref(1)
const pageSize = ref(20)
const totalAlarms = ref(0)

// 对话框
const showDetailsDialog = ref(false)
const selectedAlarm = ref(null)
const showHandleDialog = ref(false)
const handleType = ref('confirm')
const handleForm = ref({
  alarm: null,
  remark: ''
})
const handleLoading = ref(false)

// 定时器
let refreshTimer = null

// 过滤后的全部告警（不分页）
const filteredAllAlarms = computed(() => {
  let result = alarms.value

  // 按级别过滤
  if (filterForm.value.level) {
    result = result.filter(a => a.level === filterForm.value.level)
  }

  // 按来源过滤
  if (filterForm.value.source) {
    result = result.filter(a => 
      a.source.toLowerCase().includes(filterForm.value.source.toLowerCase())
    )
  }

  // 按状态过滤
  if (filterForm.value.status) {
    result = result.filter(a => a.status === filterForm.value.status)
  }

  // 按时间范围过滤
  if (filterForm.value.timeRange) {
    const [start, end] = filterForm.value.timeRange
    result = result.filter(a => {
      const time = new Date(a.time).getTime()
      return time >= start.getTime() && time <= end.getTime()
    })
  }

  return result
})

// 计算属性 - 分页后的告警
const filteredAlarms = computed(() => {
  const result = filteredAllAlarms.value
  
  const start = (currentPage.value - 1) * pageSize.value
  const end = start + pageSize.value
  
  return result.slice(start, end)
})

// 监听过滤结果更新总数
watchEffect(() => {
  totalAlarms.value = filteredAllAlarms.value.length
})

// 方法
const refreshAlarms = async () => {
  try {
    await loadAlarms()
    updateStats()
    ElMessage.success(t('common.refreshSuccess'))
  } catch (error) {
    ElMessage.error(t('common.refreshFailed'))
  }
}

const loadAlarms = async () => {
  // 模拟加载告警数据
  const mockAlarms = []
  const levels = ['critical', 'high', 'medium', 'low', 'info']
  const sources = ['1F-PLC-01', '2F-SENSOR-01', 'OUT-METER-01', 'MOTOR-01']
  const statuses = ['active', 'acknowledged', 'handled', 'cleared']
  
  for (let i = 1; i <= 100; i++) {
    const level = levels[Math.floor(Math.random() * levels.length)]
    const status = statuses[Math.floor(Math.random() * statuses.length)]
    
    mockAlarms.push({
      id: `ALM${String(i).padStart(5, '0')}`,
      level,
      source: sources[Math.floor(Math.random() * sources.length)],
      message: getAlarmMessage(level, i),
      time: Date.now() - Math.random() * 86400000 * 7, // 最近7天
      status,
      confirmedBy: status !== 'active' ? 'engineer' : null,
      confirmedTime: status !== 'active' ? Date.now() - Math.random() * 3600000 : null,
      handledBy: (status === 'handled' || status === 'cleared') ? 'engineer' : null,
      handledTime: (status === 'handled' || status === 'cleared') ? Date.now() - Math.random() * 2400000 : null,
      clearedBy: status === 'cleared' ? 'admin' : null,
      clearedTime: status === 'cleared' ? Date.now() - Math.random() * 1800000 : null,
      deviceName: `Device_${Math.floor(Math.random() * 10) + 1}`,
      deviceType: ['PLC', 'Sensor', 'Meter'][Math.floor(Math.random() * 3)],
      location: `Building ${Math.floor(Math.random() * 3) + 1}`,
      suggestion: getSuggestion(level),
      history: status !== 'active' ? generateHistory(status) : []
    })
  }
  
  alarms.value = mockAlarms.sort((a, b) => b.time - a.time)
}

const getAlarmMessage = (level, index) => {
  const messages = {
    critical: ['System Emergency Shutdown', 'Device Critical Failure', 'Communication Completely Interrupted'],
    high: ['Temperature Limit Exceeded', 'Pressure Abnormal', 'Voltage Fluctuation'],
    medium: ['Device Offline', 'Data Anomaly', 'Performance Degradation'],
    low: ['Parameter Deviation', 'Minor Fault', 'Maintenance Required'],
    info: ['System Startup', 'Parameter Changed', 'Routine Check']
  }
  return messages[level][index % 3]
}

const getSuggestion = (level) => {
  const suggestions = {
    critical: 'Immediate action required, contact technical support',
    high: 'Check device status ASAP, shutdown for maintenance if necessary',
    medium: 'Schedule inspection to prevent issue escalation',
    low: 'Log issue for planned maintenance',
    info: 'No action required'
  }
  return suggestions[level]
}

const generateHistory = (status) => {
  const history = [
    {
      time: Date.now() - 7200000,
      action: 'Alarm Triggered',
      operator: 'System',
      type: 'danger'
    }
  ]
  
  if (status !== 'active') {
    history.push({
      time: Date.now() - 3600000,
      action: 'Alarm Acknowledged',
      operator: 'engineer',
      type: 'success',
      remark: 'Inspection scheduled'
    })
  }
  
  if (status === 'handled' || status === 'cleared') {
    history.push({
      time: Date.now() - 2400000,
      action: 'Alarm Handled',
      operator: 'engineer',
      type: 'warning',
      remark: 'Maintenance performed'
    })
  }
  
  if (status === 'cleared') {
    history.push({
      time: Date.now() - 1800000,
      action: 'Alarm Cleared',
      operator: 'admin',
      type: 'primary',
      remark: 'Issue resolved'
    })
  }
  
  return history
}

const updateStats = () => {
  const activeAlarms = alarms.value.filter(a => a.status === 'active')
  stats.value = {
    critical: activeAlarms.filter(a => a.level === 'critical').length,
    high: activeAlarms.filter(a => a.level === 'high').length,
    medium: activeAlarms.filter(a => a.level === 'medium').length,
    total: activeAlarms.length
  }
}

const handleFilter = () => {
  currentPage.value = 1
}

const resetFilter = () => {
  filterForm.value = {
    level: '',
    source: '',
    status: '',
    timeRange: null
  }
  handleFilter()
}

const handleSelectionChange = (selection) => {
  selectedAlarms.value = selection
}

const handleSizeChange = () => {
  currentPage.value = 1
}

const handleCurrentChange = () => {
  // 页码变化处理
}

const tableRowClassName = ({ row }) => {
  if (row.status === 'active' && row.level === 'critical') {
    return 'critical-row'
  }
  if (row.status === 'active' && row.level === 'high') {
    return 'high-row'
  }
  return ''
}

const getLevelType = (level) => {
  const types = {
    critical: 'danger',
    high: 'warning',
    medium: 'primary',
    low: 'info',
    info: 'success'
  }
  return types[level] || 'info'
}

const getStatusType = (status) => {
  const types = {
    active: 'danger',
    acknowledged: 'warning',
    handled: 'info',
    cleared: 'success'
  }
  return types[status] || 'info'
}

const formatDateTime = (timestamp) => {
  if (!timestamp) return '-'
  return dayjs(timestamp).format('YYYY-MM-DD HH:mm:ss')
}

const viewDetails = (alarm) => {
  selectedAlarm.value = alarm
  showDetailsDialog.value = true
}

const confirmAlarm = (alarm) => {
  handleType.value = 'confirm'
  handleForm.value = {
    alarm,
    remark: ''
  }
  showHandleDialog.value = true
}

const handleAlarm = (alarm) => {
  handleType.value = 'handle'
  handleForm.value = {
    alarm,
    remark: ''
  }
  showHandleDialog.value = true
}

const clearAlarm = (alarm) => {
  handleType.value = 'clear'
  handleForm.value = {
    alarm,
    remark: ''
  }
  showHandleDialog.value = true
}

const confirmSelected = async () => {
  if (selectedAlarms.value.length === 0) return
  
  // 检查权限
  if (!checkPermission(PERMISSIONS.CONTROL.ALARM_CONFIRM)) {
    ElMessage.error(t('common.noPermission'))
    return
  }
  
  try {
    await ElMessageBox.confirm(
      t('alarms.confirmSelectedMessage', { count: selectedAlarms.value.length }),
      t('common.confirm'),
      {
        confirmButtonText: t('common.ok'),
        cancelButtonText: t('common.cancel'),
        type: 'warning'
      }
    )
    
    // 批量确认
    for (const alarm of selectedAlarms.value) {
      if (alarm.status === 'active') {
        alarm.status = 'acknowledged'
        alarm.confirmedBy = userStore.userInfo.name
        alarm.confirmedTime = Date.now()
      }
    }
    
    updateStats()
    ElMessage.success(t('alarms.batchConfirmSuccess'))
  } catch {
    // 用户取消
  }
}

const clearSelected = async () => {
  if (selectedAlarms.value.length === 0) return
  
  // 检查权限 - 只有管理员可以删除告警
  if (!checkPermission(PERMISSIONS.CONTROL.ALARM_DELETE)) {
    ElMessage.error(t('common.noPermission'))
    return
  }
  
  try {
    await ElMessageBox.confirm(
      t('alarms.clearSelectedMessage', { count: selectedAlarms.value.length }),
      t('common.warning'),
      {
        confirmButtonText: t('common.ok'),
        cancelButtonText: t('common.cancel'),
        type: 'warning'
      }
    )
    
    // 批量清除
    for (const alarm of selectedAlarms.value) {
      if (alarm.status !== 'cleared') {
        alarm.status = 'cleared'
        alarm.clearedBy = userStore.userInfo.name
        alarm.clearedTime = Date.now()
      }
    }
    
    updateStats()
    ElMessage.success(t('alarms.batchClearSuccess'))
  } catch {
    // 用户取消
  }
}

const submitHandle = async () => {
  handleLoading.value = true
  try {
    // 模拟处理请求
    await new Promise(resolve => setTimeout(resolve, 1000))
    
    const { alarm, remark } = handleForm.value
    
    if (handleType.value === 'confirm') {
      alarm.status = 'acknowledged'
      alarm.confirmedBy = userStore.userInfo.name
      alarm.confirmedTime = Date.now()
    } else if (handleType.value === 'handle') {
      alarm.status = 'handled'
      alarm.handledBy = userStore.userInfo.name
      alarm.handledTime = Date.now()
    } else {
      alarm.status = 'cleared'
      alarm.clearedBy = userStore.userInfo.name
      alarm.clearedTime = Date.now()
    }
    
    // 添加历史记录
    if (!alarm.history) alarm.history = []
    const actionMap = {
      confirm: 'Alarm Acknowledged',
      handle: 'Alarm Handled',
      clear: 'Alarm Cleared'
    }
    const typeMap = {
      confirm: 'success',
      handle: 'warning',
      clear: 'primary'
    }
    alarm.history.push({
      time: Date.now(),
      action: actionMap[handleType.value],
      operator: userStore.userInfo.name,
      type: typeMap[handleType.value],
      remark
    })
    
    updateStats()
    showHandleDialog.value = false
    if (showDetailsDialog.value) {
      showDetailsDialog.value = false
    }
    
    const messageMap = {
      confirm: t('alarms.confirmSuccess'),
      handle: t('alarms.handleSuccess'),
      clear: t('alarms.clearSuccess')
    }
    ElMessage.success(messageMap[handleType.value])
  } catch (error) {
    ElMessage.error(t('common.operationFailed'))
  } finally {
    handleLoading.value = false
  }
}

// 启动自动刷新
const startAutoRefresh = () => {
  refreshTimer = setInterval(() => {
    loadAlarms()
    updateStats()
  }, 30000) // 30秒刷新一次
}

onMounted(() => {
  loadAlarms()
  updateStats()
  startAutoRefresh()
})

onUnmounted(() => {
  if (refreshTimer) {
    clearInterval(refreshTimer)
  }
})
</script>

<style lang="scss" scoped>
@import '@/styles/design-tokens.scss';

.alarm-management-container {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: var(--space-5);
}

// Apple 风格页面头部
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--space-6);
  
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
    
    .el-button {
      border-radius: var(--radius-lg);
      font-weight: var(--font-weight-medium);
      transition: all var(--duration-fast) var(--ease-in-out);
      
      &:not([type="primary"]):not([type="success"]):not([type="danger"]) {
        background: var(--color-background-elevated);
        border: 1px solid var(--color-border-light);
        color: var(--color-text-secondary);
        
        &:hover {
          background: var(--color-background-secondary);
          border-color: var(--color-border);
          color: var(--color-text-primary);
        }
      }
      
      &[type="primary"] {
        background: var(--color-primary);
        border-color: var(--color-primary);
        
        &:hover {
          background: var(--color-primary-hover);
          border-color: var(--color-primary-hover);
          transform: translateY(-1px);
          box-shadow: var(--shadow-md);
        }
      }
      
      &[type="success"] {
        background: var(--color-success);
        border-color: var(--color-success);
        
        &:hover:not(:disabled) {
          transform: translateY(-1px);
          box-shadow: var(--shadow-md);
        }
      }
      
      &[type="danger"] {
        background: var(--color-danger);
        border-color: var(--color-danger);
        
        &:hover:not(:disabled) {
          transform: translateY(-1px);
          box-shadow: var(--shadow-md);
        }
      }
    }
  }
}

// Tesla 风格统计卡片
.stats-row {
  margin-bottom: var(--space-6);
}

.stat-card {
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
    height: 4px;
    opacity: 0;
    transition: opacity var(--duration-normal) var(--ease-in-out);
  }
  
  &:hover {
    transform: translateY(-2px);
    box-shadow: var(--shadow-md);
    
    &::before {
      opacity: 1;
    }
  }
  
  :deep(.el-card__body) {
    display: flex;
    align-items: center;
    padding: var(--space-6);
  }
  
  .stat-icon {
    width: 56px;
    height: 56px;
    border-radius: var(--radius-lg);
    display: flex;
    align-items: center;
    justify-content: center;
    margin-right: var(--space-5);
    transition: all var(--duration-normal) var(--ease-in-out);
    
    .el-icon {
      transition: transform var(--duration-normal) var(--ease-in-out);
    }
    
    &.critical {
      background: rgba(var(--color-danger-rgb), 0.1);
      color: var(--color-danger);
      
      ~ .stat-content .stat-value {
        color: var(--color-danger);
      }
    }
    
    &.high {
      background: rgba(var(--color-warning-rgb), 0.1);
      color: var(--color-warning);
      
      ~ .stat-content .stat-value {
        color: var(--color-warning);
      }
    }
    
    &.medium {
      background: rgba(var(--color-primary-rgb), 0.1);
      color: var(--color-primary);
      
      ~ .stat-content .stat-value {
        color: var(--color-primary);
      }
    }
    
    &.total {
      background: rgba(var(--color-info-rgb), 0.1);
      color: var(--color-info);
      
      ~ .stat-content .stat-value {
        color: var(--color-info);
      }
    }
  }
  
  &:hover .stat-icon .el-icon {
    transform: scale(1.1);
  }
  
  .stat-content {
    flex: 1;
    
    .stat-value {
      font-size: var(--font-size-3xl);
      font-weight: var(--font-weight-bold);
      line-height: 1;
      margin-bottom: var(--space-2);
      letter-spacing: -0.02em;
    }
    
    .stat-label {
      color: var(--color-text-tertiary);
      font-size: var(--font-size-sm);
      font-weight: var(--font-weight-medium);
      text-transform: uppercase;
      letter-spacing: 0.05em;
    }
  }
  
  // 顶部渐变条颜色
  &:has(.stat-icon.critical)::before {
    background: linear-gradient(90deg, var(--color-danger) 0%, #ff6b6b 100%);
  }
  
  &:has(.stat-icon.high)::before {
    background: linear-gradient(90deg, var(--color-warning) 0%, #ffa940 100%);
  }
  
  &:has(.stat-icon.medium)::before {
    background: linear-gradient(90deg, var(--color-primary) 0%, #40a9ff 100%);
  }
  
  &:has(.stat-icon.total)::before {
    background: linear-gradient(90deg, var(--color-info) 0%, #73d13d 100%);
  }
}

// 过滤卡片
.filter-card {
  margin-bottom: var(--space-6);
  background: var(--color-background-elevated);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-xs);
  
  :deep(.el-card__body) {
    padding: var(--space-5);
  }
  
  .filter-form {
    :deep(.el-form-item) {
      margin-bottom: 0;
      margin-right: var(--space-4);
      
      &:last-child {
        margin-right: 0;
      }
      
      .el-form-item__label {
        color: var(--color-text-secondary);
        font-weight: var(--font-weight-medium);
      }
    }
    
    :deep(.el-input) {
      .el-input__wrapper {
        border-radius: var(--radius-lg);
        box-shadow: none;
        border: 1px solid var(--color-border-light);
        transition: all var(--duration-fast) var(--ease-in-out);
        
        &:hover {
          border-color: var(--color-border);
        }
        
        &.is-focus {
          border-color: var(--color-primary);
          box-shadow: 0 0 0 2px rgba(var(--color-primary-rgb), 0.1);
        }
      }
    }
    
    :deep(.el-select) {
      .el-select__wrapper {
        border-radius: var(--radius-lg);
        box-shadow: none;
        border: 1px solid var(--color-border-light);
        transition: all var(--duration-fast) var(--ease-in-out);
        
        &:hover {
          border-color: var(--color-border);
        }
        
        &.is-focus {
          border-color: var(--color-primary);
          box-shadow: 0 0 0 2px rgba(var(--color-primary-rgb), 0.1);
        }
      }
    }
    
    :deep(.el-date-editor) {
      .el-range-input {
        font-weight: var(--font-weight-medium);
      }
    }
  }
}

// 主要内容卡片
.el-card {
  background: var(--color-background-elevated);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-xs);
  
  :deep(.el-card__body) {
    padding: var(--space-5);
  }
}

// 表格样式
:deep(.el-table) {
  font-size: var(--font-size-base);
  
  .el-table__header-wrapper {
    .el-table__cell {
      background: var(--color-background-secondary);
      color: var(--color-text-secondary);
      font-weight: var(--font-weight-semibold);
      border-bottom: 1px solid var(--color-border-light);
    }
  }
  
  .el-table__row {
    &:hover {
      > td.el-table__cell {
        background: var(--color-background-secondary);
      }
    }
    
    &.critical-row {
      background-color: rgba(var(--color-danger-rgb), 0.05);
      
      &:hover > td.el-table__cell {
        background-color: rgba(var(--color-danger-rgb), 0.08);
      }
    }
    
    &.high-row {
      background-color: rgba(var(--color-warning-rgb), 0.05);
      
      &:hover > td.el-table__cell {
        background-color: rgba(var(--color-warning-rgb), 0.08);
      }
    }
  }
  
  .el-table__cell {
    border-bottom: 1px solid var(--color-border-light);
  }
  
  .cell {
    font-weight: var(--font-weight-medium);
  }
  
  // 标签样式
  .el-tag {
    border-radius: var(--radius-md);
    font-weight: var(--font-weight-medium);
    border: none;
    
    &--danger {
      background: rgba(var(--color-danger-rgb), 0.1);
      color: var(--color-danger);
    }
    
    &--warning {
      background: rgba(var(--color-warning-rgb), 0.1);
      color: var(--color-warning);
    }
    
    &--primary {
      background: rgba(var(--color-primary-rgb), 0.1);
      color: var(--color-primary);
    }
    
    &--success {
      background: rgba(var(--color-success-rgb), 0.1);
      color: var(--color-success);
    }
    
    &--info {
      background: rgba(var(--color-info-rgb), 0.1);
      color: var(--color-info);
    }
  }
  
  // 操作按钮
  .el-button--small {
    padding: 0;
    font-weight: var(--font-weight-medium);
    
    &.is-link {
      &:hover {
        opacity: 0.8;
        text-decoration: underline;
      }
    }
  }
}

// 分页
.pagination {
  margin-top: var(--space-5);
  display: flex;
  justify-content: flex-end;
  
  :deep(.el-pagination) {
    .el-pager li,
    .btn-prev,
    .btn-next {
      border-radius: var(--radius-md);
      font-weight: var(--font-weight-medium);
      transition: all var(--duration-fast) var(--ease-in-out);
      
      &:hover {
        background: var(--color-background-secondary);
      }
      
      &.is-active {
        background: var(--color-primary);
        color: var(--color-text-inverse);
      }
    }
  }
}

// 对话框样式
:deep(.el-dialog) {
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-2xl);
  
  .el-dialog__header {
    padding: var(--space-6);
    border-bottom: 1px solid var(--color-border-light);
    
    .el-dialog__title {
      font-size: var(--font-size-xl);
      font-weight: var(--font-weight-semibold);
      color: var(--color-text-primary);
    }
  }
  
  .el-dialog__body {
    padding: var(--space-6);
  }
  
  .el-dialog__footer {
    padding: var(--space-5) var(--space-6);
    border-top: 1px solid var(--color-border-light);
    
    .el-button {
      border-radius: var(--radius-lg);
      font-weight: var(--font-weight-medium);
      min-width: 100px;
    }
  }
}

// 告警详情
.alarm-info {
  line-height: 1.8;
  color: var(--color-text-secondary);
  font-weight: var(--font-weight-medium);
  
  div {
    padding: var(--space-2) 0;
    border-bottom: 1px solid var(--color-border-light);
    
    &:last-child {
      border-bottom: none;
    }
  }
}

.handle-history {
  margin-top: var(--space-6);
  padding-top: var(--space-6);
  border-top: 1px solid var(--color-border-light);
  
  h4 {
    margin-bottom: var(--space-4);
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }
  
  :deep(.el-timeline) {
    padding-left: var(--space-3);
    
    .el-timeline-item__wrapper {
      padding-left: var(--space-5);
    }
    
    .el-timeline-item__timestamp {
      color: var(--color-text-tertiary);
      font-weight: var(--font-weight-medium);
    }
    
    .el-timeline-item__content {
      color: var(--color-text-primary);
      font-weight: var(--font-weight-medium);
    }
  }
  
  .history-remark {
    margin-top: var(--space-2);
    padding: var(--space-2) var(--space-3);
    background: var(--color-background-secondary);
    border-radius: var(--radius-md);
    color: var(--color-text-secondary);
    font-size: var(--font-size-sm);
  }
}

// 描述列表样式
:deep(.el-descriptions) {
  .el-descriptions__label {
    color: var(--color-text-tertiary);
    font-weight: var(--font-weight-medium);
    background: var(--color-background-secondary);
  }
  
  .el-descriptions__content {
    font-weight: var(--font-weight-medium);
  }
}

// 响应式
@media (max-width: 768px) {
  .page-header {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-4);
    
    h1 {
      font-size: var(--font-size-3xl);
    }
    
    .header-actions {
      width: 100%;
      flex-wrap: wrap;
    }
  }
  
  .filter-form {
    :deep(.el-form-item) {
      display: block;
      margin-bottom: var(--space-3) !important;
    }
  }
  
  .stats-row {
    .el-col {
      margin-bottom: var(--space-3);
    }
  }
}
</style>