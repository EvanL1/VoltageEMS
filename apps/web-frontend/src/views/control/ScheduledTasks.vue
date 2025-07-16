<template>
  <div class="scheduled-tasks-container">
    <div class="page-header">
      <h1>{{ $t('menu.scheduledTasks') }}</h1>
      <div class="header-actions">
        <el-button @click="refreshTasks">
          <el-icon><Refresh /></el-icon>
          {{ $t('common.refresh') }}
        </el-button>
        <el-button 
          type="primary" 
          @click="createTask"
          :disabled="!userStore.canControl"
        >
          <el-icon><Plus /></el-icon>
          {{ $t('scheduledTasks.createTask') }}
        </el-button>
      </div>
    </div>

    <!-- 任务统计 -->
    <el-row :gutter="20" class="stats-row">
      <el-col :xs="12" :sm="6">
        <el-card class="stat-card">
          <div class="stat-icon active">
            <el-icon :size="24"><VideoPlay /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ stats.active }}</div>
            <div class="stat-label">{{ $t('scheduledTasks.activeTasks') }}</div>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="6">
        <el-card class="stat-card">
          <div class="stat-icon scheduled">
            <el-icon :size="24"><Clock /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ stats.scheduled }}</div>
            <div class="stat-label">{{ $t('scheduledTasks.scheduledTasks') }}</div>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="6">
        <el-card class="stat-card">
          <div class="stat-icon completed">
            <el-icon :size="24"><CircleCheck /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ stats.completed }}</div>
            <div class="stat-label">{{ $t('scheduledTasks.completedToday') }}</div>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="12" :sm="6">
        <el-card class="stat-card">
          <div class="stat-icon failed">
            <el-icon :size="24"><CircleClose /></el-icon>
          </div>
          <div class="stat-content">
            <div class="stat-value">{{ stats.failed }}</div>
            <div class="stat-label">{{ $t('scheduledTasks.failedToday') }}</div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 任务列表 -->
    <el-card>
      <template #header>
        <div class="card-header">
          <span>{{ $t('scheduledTasks.taskList') }}</span>
          <div class="filter-controls">
            <el-select 
              v-model="filterStatus" 
              :placeholder="$t('scheduledTasks.filterByStatus')"
              clearable
              @change="filterTasks"
            >
              <el-option 
                v-for="status in statusOptions" 
                :key="status.value" 
                :label="status.label" 
                :value="status.value"
              />
            </el-select>
            <el-select 
              v-model="filterType" 
              :placeholder="$t('scheduledTasks.filterByType')"
              clearable
              @change="filterTasks"
            >
              <el-option 
                v-for="type in typeOptions" 
                :key="type.value" 
                :label="type.label" 
                :value="type.value"
              />
            </el-select>
          </div>
        </div>
      </template>

      <el-table :data="filteredTasks" style="width: 100%">
        <el-table-column prop="id" label="ID" width="80" />
        <el-table-column prop="name" :label="$t('scheduledTasks.taskName')" min-width="200">
          <template #default="{ row }">
            <el-link type="primary" @click="viewDetails(row)">{{ row.name }}</el-link>
          </template>
        </el-table-column>
        <el-table-column prop="type" :label="$t('scheduledTasks.taskType')" width="120">
          <template #default="{ row }">
            <el-tag :type="getTypeTagType(row.type)">
              {{ getTypeLabel(row.type) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="schedule" :label="$t('scheduledTasks.schedule')" width="150">
          <template #default="{ row }">
            <div class="schedule-info">
              <el-icon><Clock /></el-icon>
              <span>{{ formatSchedule(row) }}</span>
            </div>
          </template>
        </el-table-column>
        <el-table-column prop="nextRun" :label="$t('scheduledTasks.nextRun')" width="180">
          <template #default="{ row }">
            {{ formatNextRun(row.nextRun) }}
          </template>
        </el-table-column>
        <el-table-column prop="lastRun" :label="$t('scheduledTasks.lastRun')" width="180">
          <template #default="{ row }">
            <div v-if="row.lastRun">
              <div>{{ formatDateTime(row.lastRun.time) }}</div>
              <el-tag 
                :type="row.lastRun.success ? 'success' : 'danger'" 
                size="small"
              >
                {{ row.lastRun.success ? $t('common.success') : $t('common.failed') }}
              </el-tag>
            </div>
            <span v-else>-</span>
          </template>
        </el-table-column>
        <el-table-column prop="status" :label="$t('common.status')" width="100">
          <template #default="{ row }">
            <el-tag :type="getStatusType(row.status)">
              {{ getStatusLabel(row.status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="creator" :label="$t('scheduledTasks.creator')" width="120" />
        <el-table-column :label="$t('common.actions')" width="200" fixed="right">
          <template #default="{ row }">
            <el-button 
              v-if="row.status === 'paused'" 
              link 
              @click="resumeTask(row)"
              :disabled="!userStore.canControl"
            >
              {{ $t('scheduledTasks.resume') }}
            </el-button>
            <el-button 
              v-else-if="row.status === 'active'" 
              link 
              @click="pauseTask(row)"
              :disabled="!userStore.canControl"
            >
              {{ $t('scheduledTasks.pause') }}
            </el-button>
            <el-button 
              link 
              @click="executeNow(row)"
              :disabled="!userStore.canControl"
            >
              {{ $t('scheduledTasks.executeNow') }}
            </el-button>
            <el-button 
              link 
              @click="editTask(row)"
              :disabled="!userStore.canConfig"
            >
              {{ $t('common.edit') }}
            </el-button>
            <el-button 
              link 
              type="danger" 
              @click="deleteTask(row)"
              :disabled="!userStore.canConfig"
            >
              {{ $t('common.delete') }}
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 任务详情对话框 -->
    <el-dialog
      v-model="showDetailsDialog"
      :title="$t('scheduledTasks.taskDetails')"
      width="800px"
    >
      <div v-if="selectedTask">
        <el-tabs v-model="activeTab">
          <el-tab-pane :label="$t('scheduledTasks.basicInfo')" name="basic">
            <el-descriptions :column="2" border>
              <el-descriptions-item :label="$t('scheduledTasks.taskId')">
                {{ selectedTask.id }}
              </el-descriptions-item>
              <el-descriptions-item :label="$t('scheduledTasks.taskName')">
                {{ selectedTask.name }}
              </el-descriptions-item>
              <el-descriptions-item :label="$t('scheduledTasks.taskType')">
                <el-tag :type="getTypeTagType(selectedTask.type)">
                  {{ getTypeLabel(selectedTask.type) }}
                </el-tag>
              </el-descriptions-item>
              <el-descriptions-item :label="$t('common.status')">
                <el-tag :type="getStatusType(selectedTask.status)">
                  {{ getStatusLabel(selectedTask.status) }}
                </el-tag>
              </el-descriptions-item>
              <el-descriptions-item :label="$t('scheduledTasks.schedule')">
                {{ formatSchedule(selectedTask) }}
              </el-descriptions-item>
              <el-descriptions-item :label="$t('scheduledTasks.nextRun')">
                {{ formatNextRun(selectedTask.nextRun) }}
              </el-descriptions-item>
              <el-descriptions-item :label="$t('scheduledTasks.creator')">
                {{ selectedTask.creator }}
              </el-descriptions-item>
              <el-descriptions-item :label="$t('scheduledTasks.createTime')">
                {{ formatDateTime(selectedTask.createTime) }}
              </el-descriptions-item>
              <el-descriptions-item :label="$t('common.description')" :span="2">
                {{ selectedTask.description || '-' }}
              </el-descriptions-item>
            </el-descriptions>
          </el-tab-pane>

          <el-tab-pane :label="$t('scheduledTasks.taskConfig')" name="config">
            <el-descriptions :column="1" border>
              <el-descriptions-item :label="$t('scheduledTasks.targetDevices')">
                <el-tag v-for="device in selectedTask.config.devices" :key="device" class="device-tag">
                  {{ device }}
                </el-tag>
              </el-descriptions-item>
              <el-descriptions-item :label="$t('scheduledTasks.operation')">
                {{ selectedTask.config.operation }}
              </el-descriptions-item>
              <el-descriptions-item :label="$t('scheduledTasks.parameters')">
                <pre>{{ JSON.stringify(selectedTask.config.params, null, 2) }}</pre>
              </el-descriptions-item>
              <el-descriptions-item :label="$t('scheduledTasks.retryPolicy')">
                {{ $t('scheduledTasks.retryTimes', { times: selectedTask.config.retryTimes }) }}
              </el-descriptions-item>
              <el-descriptions-item :label="$t('scheduledTasks.timeout')">
                {{ selectedTask.config.timeout }}s
              </el-descriptions-item>
            </el-descriptions>
          </el-tab-pane>

          <el-tab-pane :label="$t('scheduledTasks.executionHistory')" name="history">
            <el-table :data="selectedTask.history" style="width: 100%" max-height="400">
              <el-table-column prop="time" :label="$t('common.time')" width="180">
                <template #default="{ row }">
                  {{ formatDateTime(row.time) }}
                </template>
              </el-table-column>
              <el-table-column prop="duration" :label="$t('scheduledTasks.duration')" width="120">
                <template #default="{ row }">
                  {{ row.duration }}ms
                </template>
              </el-table-column>
              <el-table-column prop="success" :label="$t('scheduledTasks.result')" width="100">
                <template #default="{ row }">
                  <el-tag :type="row.success ? 'success' : 'danger'" size="small">
                    {{ row.success ? $t('common.success') : $t('common.failed') }}
                  </el-tag>
                </template>
              </el-table-column>
              <el-table-column prop="message" :label="$t('scheduledTasks.message')" />
              <el-table-column prop="operator" :label="$t('scheduledTasks.triggeredBy')" width="120" />
            </el-table>
          </el-tab-pane>
        </el-tabs>
      </div>

      <template #footer>
        <el-button @click="showDetailsDialog = false">{{ $t('common.close') }}</el-button>
      </template>
    </el-dialog>

    <!-- 创建/编辑任务对话框 -->
    <el-dialog
      v-model="showTaskDialog"
      :title="editingTask ? $t('scheduledTasks.editTask') : $t('scheduledTasks.createTask')"
      width="700px"
    >
      <el-form :model="taskForm" :rules="taskRules" ref="taskFormRef" label-width="120px">
        <el-form-item :label="$t('scheduledTasks.taskName')" prop="name">
          <el-input v-model="taskForm.name" :placeholder="$t('scheduledTasks.enterTaskName')" />
        </el-form-item>

        <el-form-item :label="$t('scheduledTasks.taskType')" prop="type">
          <el-select v-model="taskForm.type" :placeholder="$t('scheduledTasks.selectTaskType')">
            <el-option 
              v-for="type in typeOptions" 
              :key="type.value" 
              :label="type.label" 
              :value="type.value"
            />
          </el-select>
        </el-form-item>

        <el-form-item :label="$t('scheduledTasks.scheduleType')" prop="scheduleType">
          <el-radio-group v-model="taskForm.scheduleType">
            <el-radio value="once">{{ $t('scheduledTasks.once') }}</el-radio>
            <el-radio value="interval">{{ $t('scheduledTasks.interval') }}</el-radio>
            <el-radio value="cron">{{ $t('scheduledTasks.cron') }}</el-radio>
          </el-radio-group>
        </el-form-item>

        <!-- 一次性执行 -->
        <el-form-item 
          v-if="taskForm.scheduleType === 'once'" 
          :label="$t('scheduledTasks.executeTime')" 
          prop="executeTime"
        >
          <el-date-picker
            v-model="taskForm.executeTime"
            type="datetime"
            :placeholder="$t('scheduledTasks.selectExecuteTime')"
          />
        </el-form-item>

        <!-- 间隔执行 -->
        <template v-if="taskForm.scheduleType === 'interval'">
          <el-form-item :label="$t('scheduledTasks.intervalValue')" prop="intervalValue">
            <el-input-number 
              v-model="taskForm.intervalValue" 
              :min="1" 
              :max="999"
            />
            <el-select v-model="taskForm.intervalUnit" style="margin-left: 10px; width: 100px;">
              <el-option value="minutes" :label="$t('scheduledTasks.minutes')" />
              <el-option value="hours" :label="$t('scheduledTasks.hours')" />
              <el-option value="days" :label="$t('scheduledTasks.days')" />
            </el-select>
          </el-form-item>
          <el-form-item :label="$t('scheduledTasks.startTime')" prop="startTime">
            <el-date-picker
              v-model="taskForm.startTime"
              type="datetime"
              :placeholder="$t('scheduledTasks.selectStartTime')"
            />
          </el-form-item>
        </template>

        <!-- Cron表达式 -->
        <el-form-item 
          v-if="taskForm.scheduleType === 'cron'" 
          :label="$t('scheduledTasks.cronExpression')" 
          prop="cronExpression"
        >
          <el-input v-model="taskForm.cronExpression" :placeholder="$t('scheduledTasks.cronPlaceholder')" />
          <div class="cron-examples">
            <el-link @click="setCronExample('0 0 * * *')">{{ $t('scheduledTasks.everyDayMidnight') }}</el-link>
            <el-link @click="setCronExample('0 */2 * * *')">{{ $t('scheduledTasks.everyTwoHours') }}</el-link>
            <el-link @click="setCronExample('0 8 * * 1-5')">{{ $t('scheduledTasks.weekdays8am') }}</el-link>
          </div>
        </el-form-item>

        <el-form-item :label="$t('scheduledTasks.targetDevices')" prop="devices">
          <el-select 
            v-model="taskForm.devices" 
            multiple 
            :placeholder="$t('scheduledTasks.selectDevices')"
          >
            <el-option 
              v-for="device in availableDevices" 
              :key="device.id" 
              :label="device.name" 
              :value="device.id"
            />
          </el-select>
        </el-form-item>

        <el-form-item :label="$t('scheduledTasks.operation')" prop="operation">
          <el-input 
            v-model="taskForm.operation" 
            :placeholder="$t('scheduledTasks.enterOperation')"
          />
        </el-form-item>

        <el-form-item :label="$t('scheduledTasks.parameters')">
          <el-input 
            v-model="taskForm.params" 
            type="textarea" 
            :rows="4"
            :placeholder="$t('scheduledTasks.jsonFormat')"
          />
        </el-form-item>

        <el-form-item :label="$t('common.description')">
          <el-input 
            v-model="taskForm.description" 
            type="textarea" 
            :rows="2"
            :placeholder="$t('scheduledTasks.enterDescription')"
          />
        </el-form-item>

        <el-form-item :label="$t('scheduledTasks.advancedSettings')">
          <el-checkbox v-model="showAdvanced">{{ $t('scheduledTasks.showAdvanced') }}</el-checkbox>
        </el-form-item>

        <template v-if="showAdvanced">
          <el-form-item :label="$t('scheduledTasks.retryTimes')">
            <el-input-number v-model="taskForm.retryTimes" :min="0" :max="5" />
          </el-form-item>
          <el-form-item :label="$t('scheduledTasks.timeout')">
            <el-input-number v-model="taskForm.timeout" :min="10" :max="300" />
            <span style="margin-left: 10px;">{{ $t('scheduledTasks.seconds') }}</span>
          </el-form-item>
          <el-form-item :label="$t('scheduledTasks.notifyOnFailure')">
            <el-switch v-model="taskForm.notifyOnFailure" />
          </el-form-item>
        </template>
      </el-form>

      <template #footer>
        <el-button @click="showTaskDialog = false">{{ $t('common.cancel') }}</el-button>
        <el-button type="primary" @click="submitTask" :loading="taskSubmitting">
          {{ $t('common.confirm') }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { 
  Refresh, 
  Plus,
  VideoPlay,
  Clock,
  CircleCheck,
  CircleClose
} from '@element-plus/icons-vue'
import dayjs from 'dayjs'
import { useUserStore } from '@/stores/user'

const { t } = useI18n()
const userStore = useUserStore()

// 统计数据
const stats = ref({
  active: 0,
  scheduled: 0,
  completed: 0,
  failed: 0
})

// 任务列表
const tasks = ref([])
const filterStatus = ref('')
const filterType = ref('')

// 状态选项
const statusOptions = [
  { value: 'active', label: t('scheduledTasks.active') },
  { value: 'paused', label: t('scheduledTasks.paused') },
  { value: 'completed', label: t('scheduledTasks.completed') },
  { value: 'failed', label: t('scheduledTasks.failed') }
]

// 类型选项
const typeOptions = [
  { value: 'control', label: t('scheduledTasks.control') },
  { value: 'data_collect', label: t('scheduledTasks.dataCollect') },
  { value: 'maintenance', label: t('scheduledTasks.maintenance') },
  { value: 'report', label: t('scheduledTasks.report') }
]

// 对话框
const showDetailsDialog = ref(false)
const selectedTask = ref(null)
const activeTab = ref('basic')
const showTaskDialog = ref(false)
const editingTask = ref(null)
const taskFormRef = ref()
const taskSubmitting = ref(false)
const showAdvanced = ref(false)

// 任务表单
const taskForm = ref({
  name: '',
  type: '',
  scheduleType: 'once',
  executeTime: null,
  intervalValue: 1,
  intervalUnit: 'hours',
  startTime: null,
  cronExpression: '',
  devices: [],
  operation: '',
  params: '',
  description: '',
  retryTimes: 3,
  timeout: 60,
  notifyOnFailure: true
})

// 验证规则
const taskRules = {
  name: [
    { required: true, message: t('scheduledTasks.nameRequired'), trigger: 'blur' }
  ],
  type: [
    { required: true, message: t('scheduledTasks.typeRequired'), trigger: 'change' }
  ],
  executeTime: [
    { required: true, message: t('scheduledTasks.executeTimeRequired'), trigger: 'change' }
  ],
  intervalValue: [
    { required: true, message: t('scheduledTasks.intervalRequired'), trigger: 'blur' }
  ],
  cronExpression: [
    { required: true, message: t('scheduledTasks.cronRequired'), trigger: 'blur' }
  ],
  devices: [
    { required: true, message: t('scheduledTasks.devicesRequired'), trigger: 'change', type: 'array', min: 1 }
  ],
  operation: [
    { required: true, message: t('scheduledTasks.operationRequired'), trigger: 'blur' }
  ]
}

// 可用设备
const availableDevices = ref([])

// 定时器
let refreshTimer = null

// 计算属性
const filteredTasks = computed(() => {
  let result = tasks.value
  
  if (filterStatus.value) {
    result = result.filter(task => task.status === filterStatus.value)
  }
  
  if (filterType.value) {
    result = result.filter(task => task.type === filterType.value)
  }
  
  return result
})

// 方法
const refreshTasks = () => {
  loadTasks()
  updateStats()
  ElMessage.success(t('common.refreshSuccess'))
}

const loadTasks = () => {
  // 模拟加载任务数据
  const mockTasks = []
  const statuses = ['active', 'paused', 'completed', 'failed']
  const types = ['control', 'data_collect', 'maintenance', 'report']
  
  for (let i = 1; i <= 20; i++) {
    const status = statuses[Math.floor(Math.random() * statuses.length)]
    const type = types[Math.floor(Math.random() * types.length)]
    
    mockTasks.push({
      id: `TASK${String(i).padStart(4, '0')}`,
      name: `${getTypeLabel(type)} Task ${i}`,
      type,
      status,
      scheduleType: ['once', 'interval', 'cron'][Math.floor(Math.random() * 3)],
      interval: Math.floor(Math.random() * 24) + 1,
      intervalUnit: 'hours',
      cronExpression: '0 0 * * *',
      nextRun: status === 'active' ? Date.now() + Math.random() * 86400000 : null,
      lastRun: Math.random() > 0.3 ? {
        time: Date.now() - Math.random() * 86400000,
        success: Math.random() > 0.2,
        duration: Math.floor(Math.random() * 5000) + 500
      } : null,
      creator: ['admin', 'engineer'][Math.floor(Math.random() * 2)],
      createTime: Date.now() - Math.random() * 86400000 * 30,
      description: `Automated ${type} task for system maintenance`,
      config: {
        devices: [`DEV${String(Math.floor(Math.random() * 10) + 1).padStart(3, '0')}`],
        operation: type === 'control' ? 'switch_on' : 'collect_data',
        params: { value: Math.floor(Math.random() * 100) },
        retryTimes: 3,
        timeout: 60
      },
      history: generateHistory(status)
    })
  }
  
  tasks.value = mockTasks
}

const generateHistory = (status) => {
  if (status === 'active' || status === 'paused') return []
  
  const history = []
  for (let i = 0; i < 5; i++) {
    history.push({
      time: Date.now() - (i + 1) * 86400000,
      duration: Math.floor(Math.random() * 5000) + 500,
      success: Math.random() > 0.2,
      message: Math.random() > 0.2 ? 'Task executed successfully' : 'Device not responding',
      operator: 'System'
    })
  }
  return history
}

const updateStats = () => {
  const today = new Date()
  today.setHours(0, 0, 0, 0)
  
  stats.value = {
    active: tasks.value.filter(t => t.status === 'active').length,
    scheduled: tasks.value.filter(t => t.status === 'active' && t.nextRun).length,
    completed: tasks.value.filter(t => 
      t.status === 'completed' && 
      t.lastRun && 
      new Date(t.lastRun.time) >= today
    ).length,
    failed: tasks.value.filter(t => 
      t.lastRun && 
      !t.lastRun.success && 
      new Date(t.lastRun.time) >= today
    ).length
  }
}

const filterTasks = () => {
  // 过滤触发时自动更新
}

const getTypeLabel = (type) => {
  const typeOption = typeOptions.find(t => t.value === type)
  return typeOption ? typeOption.label : type
}

const getTypeTagType = (type) => {
  const types = {
    control: 'primary',
    data_collect: 'success',
    maintenance: 'warning',
    report: 'info'
  }
  return types[type] || 'info'
}

const getStatusType = (status) => {
  const types = {
    active: 'success',
    paused: 'warning',
    completed: 'info',
    failed: 'danger'
  }
  return types[status] || 'info'
}

const getStatusLabel = (status) => {
  const statusOption = statusOptions.find(s => s.value === status)
  return statusOption ? statusOption.label : status
}

const formatSchedule = (task) => {
  if (task.scheduleType === 'once') {
    return t('scheduledTasks.once')
  } else if (task.scheduleType === 'interval') {
    return `${task.interval} ${task.intervalUnit}`
  } else if (task.scheduleType === 'cron') {
    return task.cronExpression
  }
  return '-'
}

const formatDateTime = (timestamp) => {
  if (!timestamp) return '-'
  return dayjs(timestamp).format('YYYY-MM-DD HH:mm:ss')
}

const formatNextRun = (timestamp) => {
  if (!timestamp) return '-'
  const now = Date.now()
  const diff = timestamp - now
  
  if (diff < 0) return t('scheduledTasks.overdue')
  if (diff < 60000) return t('scheduledTasks.lessThanMinute')
  if (diff < 3600000) return t('scheduledTasks.minutesLater', { minutes: Math.floor(diff / 60000) })
  if (diff < 86400000) return t('scheduledTasks.hoursLater', { hours: Math.floor(diff / 3600000) })
  return formatDateTime(timestamp)
}

const loadAvailableDevices = () => {
  // 模拟加载可用设备
  const devices = []
  for (let i = 1; i <= 20; i++) {
    devices.push({
      id: `DEV${String(i).padStart(3, '0')}`,
      name: `Device_${i}`
    })
  }
  availableDevices.value = devices
}

const viewDetails = (task) => {
  selectedTask.value = task
  activeTab.value = 'basic'
  showDetailsDialog.value = true
}

const createTask = () => {
  editingTask.value = null
  taskForm.value = {
    name: '',
    type: '',
    scheduleType: 'once',
    executeTime: null,
    intervalValue: 1,
    intervalUnit: 'hours',
    startTime: null,
    cronExpression: '',
    devices: [],
    operation: '',
    params: '',
    description: '',
    retryTimes: 3,
    timeout: 60,
    notifyOnFailure: true
  }
  showAdvanced.value = false
  showTaskDialog.value = true
}

const editTask = (task) => {
  editingTask.value = task
  taskForm.value = {
    name: task.name,
    type: task.type,
    scheduleType: task.scheduleType,
    executeTime: task.scheduleType === 'once' ? task.nextRun : null,
    intervalValue: task.interval || 1,
    intervalUnit: task.intervalUnit || 'hours',
    startTime: null,
    cronExpression: task.cronExpression || '',
    devices: task.config.devices,
    operation: task.config.operation,
    params: JSON.stringify(task.config.params, null, 2),
    description: task.description,
    retryTimes: task.config.retryTimes,
    timeout: task.config.timeout,
    notifyOnFailure: true
  }
  showAdvanced.value = true
  showTaskDialog.value = true
}

const setCronExample = (expression) => {
  taskForm.value.cronExpression = expression
}

const submitTask = async () => {
  const valid = await taskFormRef.value.validate()
  if (!valid) return
  
  taskSubmitting.value = true
  try {
    // 模拟提交任务
    await new Promise(resolve => setTimeout(resolve, 1000))
    
    if (editingTask.value) {
      ElMessage.success(t('scheduledTasks.updateSuccess'))
    } else {
      ElMessage.success(t('scheduledTasks.createSuccess'))
    }
    
    showTaskDialog.value = false
    loadTasks()
    updateStats()
  } catch (error) {
    ElMessage.error(t('common.operationFailed'))
  } finally {
    taskSubmitting.value = false
  }
}

const pauseTask = async (task) => {
  try {
    await ElMessageBox.confirm(
      t('scheduledTasks.pauseConfirm'),
      t('common.confirm'),
      {
        confirmButtonText: t('common.ok'),
        cancelButtonText: t('common.cancel'),
        type: 'warning'
      }
    )
    
    task.status = 'paused'
    updateStats()
    ElMessage.success(t('scheduledTasks.pauseSuccess'))
  } catch {
    // 用户取消
  }
}

const resumeTask = async (task) => {
  task.status = 'active'
  task.nextRun = Date.now() + 60000 // 1分钟后执行
  updateStats()
  ElMessage.success(t('scheduledTasks.resumeSuccess'))
}

const executeNow = async (task) => {
  try {
    await ElMessageBox.confirm(
      t('scheduledTasks.executeNowConfirm'),
      t('common.confirm'),
      {
        confirmButtonText: t('common.ok'),
        cancelButtonText: t('common.cancel'),
        type: 'info'
      }
    )
    
    ElMessage.info(t('scheduledTasks.executionStarted'))
    
    // 模拟执行
    setTimeout(() => {
      task.lastRun = {
        time: Date.now(),
        success: Math.random() > 0.2,
        duration: Math.floor(Math.random() * 5000) + 500
      }
      
      if (task.lastRun.success) {
        ElMessage.success(t('scheduledTasks.executionSuccess'))
      } else {
        ElMessage.error(t('scheduledTasks.executionFailed'))
      }
      
      updateStats()
    }, 2000)
  } catch {
    // 用户取消
  }
}

const deleteTask = async (task) => {
  try {
    await ElMessageBox.confirm(
      t('scheduledTasks.deleteConfirm'),
      t('common.warning'),
      {
        confirmButtonText: t('common.ok'),
        cancelButtonText: t('common.cancel'),
        type: 'warning'
      }
    )
    
    const index = tasks.value.findIndex(t => t.id === task.id)
    if (index > -1) {
      tasks.value.splice(index, 1)
    }
    
    updateStats()
    ElMessage.success(t('common.deleteSuccess'))
  } catch {
    // 用户取消
  }
}

// 自动刷新
const startAutoRefresh = () => {
  refreshTimer = setInterval(() => {
    loadTasks()
    updateStats()
  }, 30000) // 30秒刷新一次
}

// 生命周期
onMounted(() => {
  loadTasks()
  updateStats()
  loadAvailableDevices()
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

.scheduled-tasks-container {
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
      
      &:not([type="primary"]) {
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
        
        &:hover:not(:disabled) {
          background: var(--color-primary-hover);
          border-color: var(--color-primary-hover);
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
    
    &.active {
      background: rgba(var(--color-primary-rgb), 0.1);
      color: var(--color-primary);
      
      ~ .stat-content .stat-value {
        color: var(--color-primary);
      }
    }
    
    &.scheduled {
      background: rgba(var(--color-warning-rgb), 0.1);
      color: var(--color-warning);
      
      ~ .stat-content .stat-value {
        color: var(--color-warning);
      }
    }
    
    &.completed {
      background: rgba(var(--color-success-rgb), 0.1);
      color: var(--color-success);
      
      ~ .stat-content .stat-value {
        color: var(--color-success);
      }
    }
    
    &.failed {
      background: rgba(var(--color-danger-rgb), 0.1);
      color: var(--color-danger);
      
      ~ .stat-content .stat-value {
        color: var(--color-danger);
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
  &:has(.stat-icon.active)::before {
    background: linear-gradient(90deg, var(--color-primary) 0%, #40a9ff 100%);
  }
  
  &:has(.stat-icon.scheduled)::before {
    background: linear-gradient(90deg, var(--color-warning) 0%, #ffa940 100%);
  }
  
  &:has(.stat-icon.completed)::before {
    background: linear-gradient(90deg, var(--color-success) 0%, #73d13d 100%);
  }
  
  &:has(.stat-icon.failed)::before {
    background: linear-gradient(90deg, var(--color-danger) 0%, #ff6b6b 100%);
  }
}

// 主卡片样式
.el-card {
  background: var(--color-background-elevated);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-sm);
  transition: all var(--duration-normal) var(--ease-in-out);
  
  &:hover {
    box-shadow: var(--shadow-md);
  }
  
  :deep(.el-card__header) {
    padding: var(--space-5);
    border-bottom: 1px solid var(--color-border-light);
    background: var(--color-background-secondary);
    
    .card-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      font-size: var(--font-size-lg);
      font-weight: var(--font-weight-semibold);
      color: var(--color-text-primary);
      
      .filter-controls {
        display: flex;
        gap: var(--space-3);
        
        .el-select {
          width: 180px;
          
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
      }
    }
  }
  
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
  }
  
  .el-table__cell {
    border-bottom: 1px solid var(--color-border-light);
    
    .cell {
      font-weight: var(--font-weight-medium);
    }
  }
  
  // 链接样式
  .el-link {
    font-weight: var(--font-weight-semibold);
    transition: all var(--duration-fast) var(--ease-in-out);
    
    &:hover {
      text-decoration: underline;
    }
  }
  
  // 标签样式
  .el-tag {
    border-radius: var(--radius-md);
    font-weight: var(--font-weight-medium);
    border: none;
    
    &--primary {
      background: rgba(var(--color-primary-rgb), 0.1);
      color: var(--color-primary);
    }
    
    &--success {
      background: rgba(var(--color-success-rgb), 0.1);
      color: var(--color-success);
    }
    
    &--warning {
      background: rgba(var(--color-warning-rgb), 0.1);
      color: var(--color-warning);
    }
    
    &--danger {
      background: rgba(var(--color-danger-rgb), 0.1);
      color: var(--color-danger);
    }
    
    &--info {
      background: rgba(var(--color-info-rgb), 0.1);
      color: var(--color-info);
    }
  }
  
  // 操作按钮
  .el-button {
    padding: 0;
    font-weight: var(--font-weight-medium);
    
    &.is-link {
      &:hover {
        opacity: 0.8;
      }
    }
  }
}

// 日程信息
.schedule-info {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  color: var(--color-text-secondary);
  font-weight: var(--font-weight-medium);
  
  .el-icon {
    color: var(--color-primary);
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

// 表单样式
:deep(.el-form) {
  .el-form-item {
    margin-bottom: var(--space-5);
    
    .el-form-item__label {
      color: var(--color-text-secondary);
      font-weight: var(--font-weight-semibold);
    }
  }
  
  .el-input {
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
  
  .el-select {
    width: 100%;
    
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
  
  .el-textarea {
    .el-textarea__inner {
      border-radius: var(--radius-lg);
      box-shadow: none;
      border: 1px solid var(--color-border-light);
      font-weight: var(--font-weight-medium);
      
      &:hover {
        border-color: var(--color-border);
      }
      
      &:focus {
        border-color: var(--color-primary);
        box-shadow: 0 0 0 2px rgba(var(--color-primary-rgb), 0.1);
      }
    }
  }
}

// Cron示例
.cron-examples {
  margin-top: var(--space-2);
  display: flex;
  gap: var(--space-4);
  flex-wrap: wrap;
  
  .el-link {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
  }
}

// 设备标签
.device-tag {
  margin-right: var(--space-2);
  margin-bottom: var(--space-1);
  background: var(--color-background-secondary);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-md);
  font-weight: var(--font-weight-medium);
}

// 预格式化文本
pre {
  margin: 0;
  font-family: 'SF Mono', Monaco, 'Courier New', monospace;
  font-size: var(--font-size-sm);
  background-color: var(--color-background-secondary);
  padding: var(--space-3);
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border-light);
  overflow: auto;
  font-weight: var(--font-weight-medium);
  color: var(--color-text-secondary);
}

// Tabs样式
:deep(.el-tabs) {
  .el-tabs__nav-wrap {
    &::after {
      height: 1px;
      background-color: var(--color-border-light);
    }
  }
  
  .el-tabs__item {
    font-weight: var(--font-weight-medium);
    color: var(--color-text-secondary);
    
    &:hover {
      color: var(--color-text-primary);
    }
    
    &.is-active {
      color: var(--color-primary);
      font-weight: var(--font-weight-semibold);
    }
  }
  
  .el-tabs__active-bar {
    background-color: var(--color-primary);
    height: 2px;
  }
}

// 时间线样式
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
  
  .filter-controls {
    flex-direction: column;
    gap: var(--space-2) !important;
    
    .el-select {
      width: 100% !important;
    }
  }
  
  .stats-row {
    .el-col {
      margin-bottom: var(--space-3);
    }
  }
}
</style>