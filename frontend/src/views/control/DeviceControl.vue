<template>
  <div class="device-control-container">
    <div class="page-header">
      <h1>{{ $t('menu.deviceControl') }}</h1>
      <div class="header-actions">
        <el-button @click="refreshDevices">
          <el-icon><Refresh /></el-icon>
          {{ $t('common.refresh') }}
        </el-button>
        <el-button 
          type="primary" 
          @click="handleShowBatchControl"
          v-if="hasPermission(PERMISSIONS.CONTROL.BATCH_CONTROL)"
        >
          <el-icon><Operation /></el-icon>
          {{ $t('deviceControl.batchControl') }}
        </el-button>
      </div>
    </div>

    <div class="content-wrapper">
      <!-- 左侧设备分组 -->
      <el-card class="device-groups">
        <template #header>
          <div class="card-header">
            <span>{{ $t('deviceControl.deviceGroups') }}</span>
            <el-button type="primary" size="small" text @click="showAddGroupDialog = true">
              <el-icon><Plus /></el-icon>
            </el-button>
          </div>
        </template>
        
        <el-menu
          :default-active="activeGroup"
          @select="handleGroupSelect"
        >
          <el-menu-item index="all">
            <el-icon><Grid /></el-icon>
            <span>{{ $t('deviceControl.allDevices') }}</span>
            <el-tag size="small" style="margin-left: auto">{{ allDevicesCount }}</el-tag>
          </el-menu-item>
          <el-menu-item 
            v-for="group in deviceGroups" 
            :key="group.id" 
            :index="group.id"
          >
            <el-icon><Folder /></el-icon>
            <span>{{ group.name }}</span>
            <el-tag size="small" style="margin-left: auto">{{ group.deviceCount }}</el-tag>
          </el-menu-item>
        </el-menu>
      </el-card>

      <!-- 右侧设备列表 -->
      <div class="device-list-area">
        <!-- 搜索栏 -->
        <el-card class="search-bar">
          <el-row :gutter="20">
            <el-col :span="8">
              <el-input 
                v-model="searchForm.keyword" 
                :placeholder="$t('deviceControl.searchPlaceholder')"
                clearable
                @clear="handleSearch"
                @keyup.enter="handleSearch"
              >
                <template #prefix>
                  <el-icon><Search /></el-icon>
                </template>
              </el-input>
            </el-col>
            <el-col :span="6">
              <el-select 
                v-model="searchForm.type" 
                :placeholder="$t('deviceControl.deviceType')"
                clearable
                @change="handleSearch"
              >
                <el-option 
                  v-for="type in deviceTypes" 
                  :key="type.value" 
                  :label="type.label" 
                  :value="type.value" 
                />
              </el-select>
            </el-col>
            <el-col :span="6">
              <el-select 
                v-model="searchForm.status" 
                :placeholder="$t('common.status')"
                clearable
                @change="handleSearch"
              >
                <el-option 
                  v-for="status in statusOptions" 
                  :key="status.value" 
                  :label="status.label" 
                  :value="status.value" 
                />
              </el-select>
            </el-col>
            <el-col :span="4">
              <el-button type="primary" @click="handleSearch">
                {{ $t('common.search') }}
              </el-button>
            </el-col>
          </el-row>
        </el-card>

        <!-- 设备卡片列表 -->
        <div class="device-cards">
          <el-row :gutter="20">
            <el-col 
              v-for="device in filteredDevices" 
              :key="device.id" 
              :xs="24" 
              :sm="12" 
              :md="8" 
              :lg="6"
            >
              <el-card 
                class="device-card" 
                :class="{ 'device-offline': device.status === 'offline' }"
                @click="selectDevice(device)"
              >
                <div class="device-icon">
                  <el-icon :size="40" :color="getDeviceIconColor(device.status)">
                    <Monitor v-if="device.type === 'plc'" />
                    <Connection v-else-if="device.type === 'sensor'" />
                    <Cpu v-else />
                  </el-icon>
                </div>
                <div class="device-info">
                  <h3>{{ device.name }}</h3>
                  <p class="device-id">ID: {{ device.id }}</p>
                  <el-tag 
                    :type="getStatusType(device.status)" 
                    size="small"
                  >
                    {{ getStatusText(device.status) }}
                  </el-tag>
                </div>
                <div class="device-stats">
                  <div class="stat-item">
                    <span class="stat-label">{{ $t('deviceControl.controlPoints') }}</span>
                    <span class="stat-value">{{ device.controlPoints }}</span>
                  </div>
                  <div class="stat-item">
                    <span class="stat-label">{{ $t('deviceControl.lastControl') }}</span>
                    <span class="stat-value">{{ formatTime(device.lastControl) }}</span>
                  </div>
                </div>
                <div class="device-actions">
                  <el-button 
                    type="primary" 
                    size="small" 
                    @click.stop="showControlPanel(device)"
                    :disabled="device.status === 'offline' || !hasPermission(PERMISSIONS.CONTROL.DEVICE_CONTROL)"
                  >
                    {{ $t('deviceControl.control') }}
                  </el-button>
                  <el-button 
                    size="small" 
                    @click.stop="showDeviceDetails(device)"
                  >
                    {{ $t('deviceControl.details') }}
                  </el-button>
                </div>
              </el-card>
            </el-col>
          </el-row>

          <!-- 空状态 -->
          <el-empty 
            v-if="filteredDevices.length === 0" 
            :description="$t('deviceControl.noDevices')"
          />
        </div>
      </div>
    </div>

    <!-- 设备控制面板 -->
    <el-drawer
      v-model="showControlDrawer"
      :title="controlPanelTitle"
      direction="rtl"
      size="50%"
    >
      <div v-if="selectedDevice" class="control-panel">
        <div class="panel-header">
          <h3>{{ selectedDevice.name }}</h3>
          <el-tag :type="getStatusType(selectedDevice.status)">
            {{ getStatusText(selectedDevice.status) }}
          </el-tag>
        </div>

        <!-- 权限不足提示 -->
        <el-alert 
          v-if="!hasPermission(PERMISSIONS.CONTROL.DEVICE_CONTROL)"
          :title="$t('deviceControl.noControlPermissionTip')"
          type="info"
          :closable="false"
          show-icon
          style="margin: 16px 24px;"
        />

        <el-tabs v-model="activeControlTab">
          <el-tab-pane :label="$t('deviceControl.quickControl')" name="quick">
            <div class="quick-controls">
              <el-row :gutter="20">
                <el-col 
                  v-for="point in quickControlPoints" 
                  :key="point.id" 
                  :span="12"
                >
                  <div class="control-point">
                    <div class="point-info">
                      <span class="point-name">{{ point.name }}</span>
                      <span class="point-value">{{ formatPointValue(point) }}</span>
                    </div>
                    <div class="point-control">
                      <el-switch 
                        v-if="point.type === 'YK'"
                        v-model="point.value"
                        :disabled="!hasPermission(PERMISSIONS.CONTROL.DEVICE_CONTROL)"
                        @change="handleQuickControl(point)"
                      />
                      <el-button 
                        v-else
                        size="small"
                        :disabled="!hasPermission(PERMISSIONS.CONTROL.DEVICE_CONTROL)"
                        @click="showSetValueDialog(point)"
                      >
                        {{ $t('deviceControl.setValue') }}
                      </el-button>
                    </div>
                  </div>
                </el-col>
              </el-row>
            </div>
          </el-tab-pane>

          <el-tab-pane :label="$t('deviceControl.allPoints')" name="all">
            <el-table :data="controlPoints" style="width: 100%">
              <el-table-column prop="name" :label="$t('deviceControl.pointName')" />
              <el-table-column prop="type" :label="$t('common.type')" width="80" />
              <el-table-column :label="$t('deviceControl.currentValue')" width="120">
                <template #default="{ row }">
                  {{ formatPointValue(row) }}
                </template>
              </el-table-column>
              <el-table-column prop="description" :label="$t('common.description')" />
              <el-table-column :label="$t('common.operation')" width="150" fixed="right">
                <template #default="{ row }">
                  <el-button 
                    v-if="row.type === 'YK' || row.type === 'YT'"
                    type="primary" 
                    size="small" 
                    link
                    :disabled="!hasPermission(PERMISSIONS.CONTROL.DEVICE_CONTROL)"
                    @click="showControlDialog(row)"
                  >
                    {{ $t('deviceControl.control') }}
                  </el-button>
                </template>
              </el-table-column>
            </el-table>
          </el-tab-pane>

          <el-tab-pane :label="$t('deviceControl.controlHistory')" name="history">
            <el-table :data="controlHistory" style="width: 100%">
              <el-table-column prop="time" :label="$t('common.time')" width="180">
                <template #default="{ row }">
                  {{ formatDateTime(row.time) }}
                </template>
              </el-table-column>
              <el-table-column prop="point" :label="$t('deviceControl.pointName')" />
              <el-table-column prop="action" :label="$t('deviceControl.action')" />
              <el-table-column prop="operator" :label="$t('deviceControl.operator')" />
              <el-table-column prop="result" :label="$t('deviceControl.result')">
                <template #default="{ row }">
                  <el-tag :type="row.success ? 'success' : 'danger'" size="small">
                    {{ row.success ? $t('common.success') : $t('common.failed') }}
                  </el-tag>
                </template>
              </el-table-column>
            </el-table>
          </el-tab-pane>
        </el-tabs>
      </div>
    </el-drawer>

    <!-- 单点控制对话框 -->
    <el-dialog 
      v-model="showSingleControlDialog" 
      :title="$t('deviceControl.controlOperation')"
      width="500px"
    >
      <el-form :model="controlForm" label-width="100px">
        <el-form-item :label="$t('deviceControl.device')">
          <el-input v-model="controlForm.deviceName" disabled />
        </el-form-item>
        <el-form-item :label="$t('deviceControl.point')">
          <el-input v-model="controlForm.pointName" disabled />
        </el-form-item>
        <el-form-item :label="$t('deviceControl.currentValue')">
          <el-input v-model="controlForm.currentValue" disabled />
        </el-form-item>
        <el-form-item 
          v-if="controlForm.type === 'YK'" 
          :label="$t('deviceControl.controlValue')"
        >
          <el-radio-group v-model="controlForm.value">
            <el-radio :label="1">{{ $t('common.on') }}</el-radio>
            <el-radio :label="0">{{ $t('common.off') }}</el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item 
          v-else 
          :label="$t('deviceControl.setValue')"
        >
          <el-input-number 
            v-model="controlForm.value" 
            :precision="2"
            :min="controlForm.min"
            :max="controlForm.max"
          />
          <span style="margin-left: 10px">{{ controlForm.unit }}</span>
        </el-form-item>
        <el-form-item :label="$t('deviceControl.controlMode')">
          <el-radio-group v-model="controlForm.mode">
            <el-radio label="direct">{{ $t('deviceControl.directControl') }}</el-radio>
            <el-radio label="confirm">{{ $t('deviceControl.confirmControl') }}</el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item :label="$t('common.remark')">
          <el-input 
            v-model="controlForm.remark" 
            type="textarea" 
            :rows="3" 
            :placeholder="$t('deviceControl.remarkPlaceholder')"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showSingleControlDialog = false">{{ $t('common.cancel') }}</el-button>
        <el-button 
          type="primary" 
          @click="submitControl"
          :loading="controlLoading"
        >
          {{ $t('common.confirm') }}
        </el-button>
      </template>
    </el-dialog>

    <!-- 批量控制对话框 -->
    <el-dialog 
      v-model="showBatchControlDialog" 
      :title="$t('deviceControl.batchControl')"
      width="800px"
    >
      <div class="batch-control">
        <el-alert 
          :title="$t('deviceControl.batchControlTip')" 
          type="warning" 
          show-icon 
          :closable="false"
          style="margin-bottom: 20px"
        />
        
        <el-form :model="batchControlForm" label-width="120px">
          <el-form-item :label="$t('deviceControl.selectDevices')">
            <el-select 
              v-model="batchControlForm.devices" 
              multiple 
              :placeholder="$t('deviceControl.selectDevicesPlaceholder')"
              style="width: 100%"
            >
              <el-option 
                v-for="device in onlineDevices" 
                :key="device.id" 
                :label="device.name" 
                :value="device.id"
              />
            </el-select>
          </el-form-item>
          <el-form-item :label="$t('deviceControl.controlType')">
            <el-radio-group v-model="batchControlForm.type">
              <el-radio label="on">{{ $t('deviceControl.allOn') }}</el-radio>
              <el-radio label="off">{{ $t('deviceControl.allOff') }}</el-radio>
              <el-radio label="custom">{{ $t('deviceControl.customControl') }}</el-radio>
            </el-radio-group>
          </el-form-item>
          <el-form-item 
            v-if="batchControlForm.type === 'custom'" 
            :label="$t('deviceControl.controlPoints')"
          >
            <el-table :data="batchControlPoints" style="width: 100%">
              <el-table-column prop="device" :label="$t('deviceControl.device')" />
              <el-table-column prop="point" :label="$t('deviceControl.point')" />
              <el-table-column :label="$t('deviceControl.setValue')" width="200">
                <template #default="{ row }">
                  <el-switch 
                    v-if="row.type === 'YK'"
                    v-model="row.value"
                  />
                  <el-input-number 
                    v-else
                    v-model="row.value" 
                    size="small"
                    :precision="2"
                  />
                </template>
              </el-table-column>
            </el-table>
          </el-form-item>
        </el-form>
      </div>
      <template #footer>
        <el-button @click="showBatchControlDialog = false">{{ $t('common.cancel') }}</el-button>
        <el-button 
          type="primary" 
          @click="submitBatchControl"
          :loading="batchControlLoading"
        >
          {{ $t('deviceControl.executeBatch') }}
        </el-button>
      </template>
    </el-dialog>

    <!-- 添加分组对话框 -->
    <el-dialog 
      v-model="showAddGroupDialog" 
      :title="$t('deviceControl.addGroup')"
      width="500px"
    >
      <el-form :model="groupForm" label-width="100px">
        <el-form-item :label="$t('deviceControl.groupName')">
          <el-input 
            v-model="groupForm.name" 
            :placeholder="$t('deviceControl.groupNamePlaceholder')"
          />
        </el-form-item>
        <el-form-item :label="$t('common.description')">
          <el-input 
            v-model="groupForm.description" 
            type="textarea" 
            :rows="3"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showAddGroupDialog = false">{{ $t('common.cancel') }}</el-button>
        <el-button type="primary" @click="addGroup">{{ $t('common.confirm') }}</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { 
  Plus, 
  Refresh, 
  Search, 
  Grid, 
  Folder, 
  Monitor, 
  Connection, 
  Cpu,
  Operation,
} from '@element-plus/icons-vue'
import dayjs from 'dayjs'
import { usePermission } from '@/composables/usePermission'
import { PERMISSIONS } from '@/utils/permission'

const { t } = useI18n()
const { hasPermission } = usePermission()

// 设备分组
const activeGroup = ref('all')
const deviceGroups = ref([
  { id: 'group1', name: '一楼设备', deviceCount: 12 },
  { id: 'group2', name: '二楼设备', deviceCount: 8 },
  { id: 'group3', name: '室外设备', deviceCount: 5 }
])

// 搜索表单
const searchForm = ref({
  keyword: '',
  type: '',
  status: ''
})

// 设备类型选项
const deviceTypes = [
  { value: 'plc', label: t('deviceControl.types.plc') },
  { value: 'sensor', label: t('deviceControl.types.sensor') },
  { value: 'meter', label: t('deviceControl.types.meter') },
  { value: 'actuator', label: t('deviceControl.types.actuator') }
]

// 状态选项
const statusOptions = [
  { value: 'online', label: t('common.online') },
  { value: 'offline', label: t('common.offline') },
  { value: 'error', label: t('common.error') }
]

// 设备列表
const devices = ref([
  {
    id: 'DEV001',
    name: '1F-PLC-01',
    type: 'plc',
    status: 'online',
    controlPoints: 24,
    lastControl: Date.now() - 3600000,
    group: 'group1'
  },
  {
    id: 'DEV002',
    name: '1F-SENSOR-01',
    type: 'sensor',
    status: 'online',
    controlPoints: 8,
    lastControl: Date.now() - 7200000,
    group: 'group1'
  },
  {
    id: 'DEV003',
    name: '2F-PLC-01',
    type: 'plc',
    status: 'offline',
    controlPoints: 16,
    lastControl: Date.now() - 86400000,
    group: 'group2'
  },
  {
    id: 'DEV004',
    name: 'OUT-METER-01',
    type: 'meter',
    status: 'online',
    controlPoints: 4,
    lastControl: Date.now() - 1800000,
    group: 'group3'
  }
])

// 控制面板
const showControlDrawer = ref(false)
const selectedDevice = ref(null)
const activeControlTab = ref('quick')

// 控制点数据
const quickControlPoints = ref([])
const controlPoints = ref([])
const controlHistory = ref([])

// 控制对话框
const showSingleControlDialog = ref(false)
const controlForm = ref({
  deviceName: '',
  pointName: '',
  currentValue: '',
  type: '',
  value: 0,
  mode: 'direct',
  remark: '',
  min: 0,
  max: 100,
  unit: ''
})
const controlLoading = ref(false)

// 批量控制
const showBatchControlDialog = ref(false)
const batchControlForm = ref({
  devices: [],
  type: 'on'
})
const batchControlPoints = ref([])
const batchControlLoading = ref(false)

// 添加分组
const showAddGroupDialog = ref(false)
const groupForm = ref({
  name: '',
  description: ''
})

// 计算属性
const allDevicesCount = computed(() => devices.value.length)

const controlPanelTitle = computed(() => {
  if (!hasPermission(PERMISSIONS.CONTROL.DEVICE_CONTROL)) {
    return `${t('deviceControl.controlPanel')} (${t('common.viewOnly')})`
  }
  return t('deviceControl.controlPanel')
})

const filteredDevices = computed(() => {
  let result = devices.value

  // 按分组筛选
  if (activeGroup.value !== 'all') {
    result = result.filter(d => d.group === activeGroup.value)
  }

  // 按搜索条件筛选
  if (searchForm.value.keyword) {
    const keyword = searchForm.value.keyword.toLowerCase()
    result = result.filter(d => 
      d.name.toLowerCase().includes(keyword) ||
      d.id.toLowerCase().includes(keyword)
    )
  }

  if (searchForm.value.type) {
    result = result.filter(d => d.type === searchForm.value.type)
  }

  if (searchForm.value.status) {
    result = result.filter(d => d.status === searchForm.value.status)
  }

  return result
})

const onlineDevices = computed(() => 
  devices.value.filter(d => d.status === 'online')
)

// 方法
const handleGroupSelect = (index) => {
  activeGroup.value = index
}

const handleSearch = () => {
  // 搜索逻辑
}

const refreshDevices = () => {
  ElMessage.success(t('common.refreshSuccess'))
}

const getDeviceIconColor = (status) => {
  const colors = {
    online: '#67c23a',
    offline: '#909399',
    error: '#f56c6c'
  }
  return colors[status] || '#409eff'
}

const getStatusType = (status) => {
  const types = {
    online: 'success',
    offline: 'info',
    error: 'danger'
  }
  return types[status] || 'info'
}

const getStatusText = (status) => {
  const texts = {
    online: t('common.online'),
    offline: t('common.offline'),
    error: t('common.error')
  }
  return texts[status] || status
}

const formatTime = (timestamp) => {
  if (!timestamp) return '-'
  const diff = Date.now() - timestamp
  if (diff < 3600000) {
    return `${Math.floor(diff / 60000)} ${t('common.minutesAgo')}`
  } else if (diff < 86400000) {
    return `${Math.floor(diff / 3600000)} ${t('common.hoursAgo')}`
  } else {
    return dayjs(timestamp).format('MM-DD HH:mm')
  }
}

const formatDateTime = (timestamp) => {
  return dayjs(timestamp).format('YYYY-MM-DD HH:mm:ss')
}

const formatPointValue = (point) => {
  if (point.type === 'YX' || point.type === 'YK') {
    return point.value ? t('common.on') : t('common.off')
  }
  return `${point.value} ${point.unit || ''}`
}

const selectDevice = (device) => {
  selectedDevice.value = device
}

const showControlPanel = (device) => {
  // 允许所有用户查看控制面板，但无权限者只能查看
  selectedDevice.value = device
  showControlDrawer.value = true
  loadControlPoints(device)
  
  // 给无权限用户友好提示
  if (!hasPermission(PERMISSIONS.CONTROL.DEVICE_CONTROL)) {
    ElMessage.info(t('deviceControl.viewOnlyTip'))
  }
}

const showDeviceDetails = (device) => {
  // 显示设备详情
  ElMessage.info(`查看设备 ${device.name} 的详情`)
}

const loadControlPoints = () => {
  // 模拟加载控制点数据
  quickControlPoints.value = [
    { id: 1, name: '主电源开关', type: 'YK', value: 1, unit: '' },
    { id: 2, name: '风机启停', type: 'YK', value: 0, unit: '' },
    { id: 3, name: '温度设定', type: 'YT', value: 25.0, unit: '°C' },
    { id: 4, name: '压力设定', type: 'YT', value: 101.3, unit: 'kPa' }
  ]

  controlPoints.value = [
    ...quickControlPoints.value,
    { id: 5, name: '阀门开度', type: 'YT', value: 50.0, unit: '%', description: '主管道阀门开度控制' },
    { id: 6, name: '备用电源', type: 'YK', value: 0, unit: '', description: '备用电源切换开关' },
    { id: 7, name: '报警复位', type: 'YK', value: 0, unit: '', description: '系统报警复位按钮' },
    { id: 8, name: '运行模式', type: 'YT', value: 1, unit: '', description: '0-手动 1-自动 2-远程' }
  ]

  controlHistory.value = [
    { time: Date.now() - 300000, point: '主电源开关', action: '开启', operator: 'engineer', success: true },
    { time: Date.now() - 600000, point: '温度设定', action: '设置为 25.0°C', operator: 'engineer', success: true },
    { time: Date.now() - 900000, point: '风机启停', action: '关闭', operator: 'admin', success: true },
    { time: Date.now() - 1200000, point: '压力设定', action: '设置为 101.3kPa', operator: 'engineer', success: false }
  ]
}

const handleQuickControl = async (point) => {
  if (!hasPermission(PERMISSIONS.CONTROL.DEVICE_CONTROL)) {
    ElMessage.warning(t('common.noPermission'))
    // 恢复原值
    point.value = !point.value
    return
  }

  try {
    await ElMessageBox.confirm(
      t('deviceControl.confirmControlMessage', { 
        point: point.name, 
        value: point.value ? t('common.on') : t('common.off') 
      }),
      t('common.confirm'),
      {
        confirmButtonText: t('common.ok'),
        cancelButtonText: t('common.cancel'),
        type: 'warning'
      }
    )

    // 执行控制
    ElMessage.success(t('deviceControl.controlSuccess'))
  } catch {
    // 用户取消，恢复原值
    point.value = !point.value
  }
}

const showSetValueDialog = (point) => {
  if (!hasPermission(PERMISSIONS.CONTROL.DEVICE_CONTROL)) {
    ElMessage.warning(t('common.noPermission'))
    return
  }
  
  controlForm.value = {
    deviceName: selectedDevice.value.name,
    pointName: point.name,
    currentValue: formatPointValue(point),
    type: point.type,
    value: point.value,
    mode: 'direct',
    remark: '',
    min: 0,
    max: 100,
    unit: point.unit
  }
  showSingleControlDialog.value = true
}

const showControlDialog = (point) => {
  if (!hasPermission(PERMISSIONS.CONTROL.DEVICE_CONTROL)) {
    ElMessage.warning(t('common.noPermission'))
    return
  }
  showSetValueDialog(point)
}

const submitControl = async () => {
  // 最终执行前再次检查权限
  if (!hasPermission(PERMISSIONS.CONTROL.DEVICE_CONTROL)) {
    ElMessage.warning(t('common.noPermission'))
    showSingleControlDialog.value = false
    return
  }
  
  controlLoading.value = true
  try {
    // 模拟控制请求
    await new Promise(resolve => setTimeout(resolve, 1000))
    
    ElMessage.success(t('deviceControl.controlSuccess'))
    showSingleControlDialog.value = false
    
    // 刷新控制历史
    loadControlPoints()
  } catch (error) {
    ElMessage.error(t('deviceControl.controlFailed'))
  } finally {
    controlLoading.value = false
  }
}

const submitBatchControl = async () => {
  // 检查批量控制权限
  if (!hasPermission(PERMISSIONS.CONTROL.BATCH_CONTROL)) {
    ElMessage.warning(t('common.noPermission'))
    showBatchControlDialog.value = false
    return
  }
  
  if (batchControlForm.value.devices.length === 0) {
    ElMessage.warning(t('deviceControl.selectDevicesFirst'))
    return
  }

  batchControlLoading.value = true
  try {
    // 模拟批量控制请求
    await new Promise(resolve => setTimeout(resolve, 2000))
    
    ElMessage.success(t('deviceControl.batchControlSuccess'))
    showBatchControlDialog.value = false
  } catch (error) {
    ElMessage.error(t('deviceControl.batchControlFailed'))
  } finally {
    batchControlLoading.value = false
  }
}

const addGroup = () => {
  if (!groupForm.value.name) {
    ElMessage.warning(t('deviceControl.groupNameRequired'))
    return
  }

  deviceGroups.value.push({
    id: `group${Date.now()}`,
    name: groupForm.value.name,
    deviceCount: 0
  })

  ElMessage.success(t('deviceControl.addGroupSuccess'))
  showAddGroupDialog.value = false
  groupForm.value = { name: '', description: '' }
}

const handleShowBatchControl = () => {
  if (!hasPermission(PERMISSIONS.CONTROL.BATCH_CONTROL)) {
    ElMessage.warning(t('common.noPermission'))
    return
  }
  showBatchControlDialog.value = true
}

onMounted(() => {
  // 初始化数据
})
</script>

<style lang="scss" scoped>
@import '@/styles/design-tokens.scss';

.device-control-container {
  height: 100%;
  display: flex;
  flex-direction: column;
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
      
      &.el-button--primary {
        background: var(--color-primary);
        border-color: var(--color-primary);
        color: var(--color-text-inverse);
        
        &:hover {
          background: var(--color-primary-hover);
          border-color: var(--color-primary-hover);
        }
      }
    }
  }
}

.content-wrapper {
  flex: 1;
  display: flex;
  gap: var(--space-6);
  overflow: hidden;
}

// Tesla 风格设备分组卡片
.device-groups {
  width: 280px;
  flex-shrink: 0;
  background: var(--color-background-elevated);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-sm);
  overflow: hidden;
  
  :deep(.el-card__header) {
    padding: var(--space-5) var(--space-6);
    border-bottom: 1px solid var(--color-border-light);
    background: var(--color-background-secondary);
  }
  
  :deep(.el-card__body) {
    padding: var(--space-4);
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
  
  :deep(.el-menu) {
    border: none;
    background: transparent;
    
    .el-menu-item {
      height: 44px;
      line-height: 44px;
      margin: 0 0 var(--space-2) 0;
      padding: 0 var(--space-4);
      border-radius: var(--radius-lg);
      color: var(--color-text-secondary);
      font-weight: var(--font-weight-medium);
      transition: all var(--duration-fast) var(--ease-in-out);
      
      &:hover {
        background: var(--color-background-secondary);
        color: var(--color-text-primary);
      }
      
      &.is-active {
        background: var(--color-primary);
        color: var(--color-text-inverse);
        box-shadow: var(--shadow-sm);
      }
      
      span {
        display: flex;
        justify-content: space-between;
        align-items: center;
      }
    }
  }
}

.device-list-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  overflow: hidden;
}

// 搜索栏
.search-bar {
  flex-shrink: 0;
  background: var(--color-background-elevated);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-sm);
  
  :deep(.el-card__body) {
    padding: var(--space-5);
  }
  
  .el-form {
    :deep(.el-form-item) {
      margin-bottom: 0;
      margin-right: var(--space-4);
      
      .el-form-item__label {
        color: var(--color-text-secondary);
        font-weight: var(--font-weight-medium);
      }
    }
    
    :deep(.el-select .el-input__wrapper),
    :deep(.el-input__wrapper) {
      background: var(--color-background);
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
}

// 设备卡片区域
.device-cards {
  flex: 1;
  overflow-y: auto;
  padding-right: var(--space-2);
  
  &::-webkit-scrollbar {
    width: 6px;
  }
  
  &::-webkit-scrollbar-track {
    background: transparent;
  }
  
  &::-webkit-scrollbar-thumb {
    background: var(--color-gray-300);
    border-radius: var(--radius-full);
    
    &:hover {
      background: var(--color-gray-400);
    }
  }
  
  .device-card {
    margin-bottom: var(--space-5);
    cursor: pointer;
    background: var(--color-background-elevated);
    border: 1px solid var(--color-border-light);
    border-radius: var(--radius-xl);
    box-shadow: var(--shadow-sm);
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
      background: var(--color-success);
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
    
    &.device-offline {
      opacity: 0.7;
      
      &::before {
        background: var(--color-gray-400);
      }
      
      .device-icon {
        filter: grayscale(1);
      }
    }
    
    :deep(.el-card__body) {
      padding: var(--space-5);
    }
    
    .device-icon {
      text-align: center;
      margin-bottom: var(--space-4);
      font-size: 48px;
      color: var(--color-primary);
    }
    
    .device-info {
      text-align: center;
      margin-bottom: var(--space-4);
      
      h3 {
        margin: 0 0 var(--space-2) 0;
        font-size: var(--font-size-lg);
        font-weight: var(--font-weight-semibold);
        color: var(--color-text-primary);
      }
      
      .device-id {
        color: var(--color-text-tertiary);
        font-size: var(--font-size-sm);
        margin-bottom: var(--space-2);
        font-family: var(--font-family-mono);
      }
      
      .el-tag {
        margin-top: var(--space-2);
      }
    }
    
    .device-stats {
      border-top: 1px solid var(--color-border-light);
      padding-top: var(--space-3);
      margin-bottom: var(--space-4);
      
      .stat-item {
        display: flex;
        justify-content: space-between;
        margin-bottom: var(--space-2);
        font-size: var(--font-size-sm);
        
        .stat-label {
          color: var(--color-text-tertiary);
          font-weight: var(--font-weight-medium);
        }
        
        .stat-value {
          color: var(--color-text-primary);
          font-weight: var(--font-weight-semibold);
        }
      }
    }
    
    .device-actions {
      display: flex;
      gap: var(--space-2);
      
      .el-button {
        flex: 1;
        height: 36px;
        border-radius: var(--radius-lg);
        font-weight: var(--font-weight-medium);
      }
    }
  }
}

// 控制面板抽屉
:deep(.el-drawer) {
  .el-drawer__header {
    padding: var(--space-6);
    border-bottom: 1px solid var(--color-border-light);
    margin-bottom: 0;
    
    .el-drawer__title {
      font-size: var(--font-size-xl);
      font-weight: var(--font-weight-semibold);
      color: var(--color-text-primary);
    }
  }
  
  .el-drawer__body {
    padding: 0;
  }
}

.control-panel {
  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--space-5) var(--space-6);
    background: var(--color-background-secondary);
    border-bottom: 1px solid var(--color-border-light);
    
    h3 {
      margin: 0;
      font-size: var(--font-size-lg);
      font-weight: var(--font-weight-semibold);
      color: var(--color-text-primary);
    }
  }
  
  .quick-controls {
    padding: var(--space-6);
    
    .control-point {
      display: flex;
      justify-content: space-between;
      align-items: center;
      padding: var(--space-4);
      background-color: var(--color-background-secondary);
      border-radius: var(--radius-lg);
      margin-bottom: var(--space-3);
      transition: all var(--duration-fast) var(--ease-in-out);
      
      &:hover {
        background-color: var(--color-background-tertiary);
        transform: translateX(4px);
      }
      
      .point-info {
        display: flex;
        align-items: center;
        gap: var(--space-3);
        
        .point-name {
          font-weight: var(--font-weight-medium);
          color: var(--color-text-primary);
        }
        
        .point-value {
          color: var(--color-primary);
          font-weight: var(--font-weight-semibold);
        }
      }
    }
  }
  
  :deep(.el-tabs) {
    .el-tabs__header {
      margin: 0;
      padding: 0 var(--space-6);
      background: var(--color-background);
      border-bottom: 1px solid var(--color-border-light);
    }
    
    .el-tabs__content {
      padding: var(--space-6);
    }
  }
}

// 批量控制对话框
.batch-control {
  :deep(.el-table) {
    margin-top: var(--space-4);
    font-size: var(--font-size-sm);
    
    .el-table__header {
      th {
        background: var(--color-background-secondary);
        color: var(--color-text-secondary);
        font-weight: var(--font-weight-semibold);
      }
    }
  }
}

// 单点控制对话框
:deep(.el-dialog) {
  border-radius: var(--radius-xl);
  
  .el-dialog__header {
    padding: var(--space-6) var(--space-6) var(--space-4);
    
    .el-dialog__title {
      font-size: var(--font-size-xl);
      font-weight: var(--font-weight-semibold);
      color: var(--color-text-primary);
    }
  }
  
  .el-dialog__body {
    padding: var(--space-4) var(--space-6) var(--space-6);
  }
}

// 响应式
@media (max-width: 1024px) {
  .content-wrapper {
    flex-direction: column;
  }
  
  .device-groups {
    width: 100%;
    
    :deep(.el-menu) {
      display: flex;
      overflow-x: auto;
      
      .el-menu-item {
        flex-shrink: 0;
      }
    }
  }
}

@media (max-width: 768px) {
  .page-header {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-4);
    
    h1 {
      font-size: var(--font-size-3xl);
    }
  }
  
  .device-cards {
    .el-col {
      margin-bottom: var(--space-3);
    }
  }
}
</style>