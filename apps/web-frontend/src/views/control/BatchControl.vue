<template>
  <div class="batch-control-container">
    <div class="page-header">
      <h1>{{ $t('menu.batchControl') }}</h1>
      <div class="header-actions">
        <el-button @click="refreshData">
          <el-icon><Refresh /></el-icon>
          {{ $t('common.refresh') }}
        </el-button>
      </div>
    </div>

    <!-- 批量操作配置 -->
    <el-row :gutter="20">
      <!-- 设备选择 -->
      <el-col :span="12">
        <el-card>
          <template #header>
            <div class="card-header">
              <span>{{ $t('batchControl.deviceSelection') }}</span>
              <el-tag type="info">{{ selectedDevices.length }} {{ $t('batchControl.selected') }}</el-tag>
            </div>
          </template>
          
          <div class="device-selection">
            <!-- 快速选择 -->
            <div class="quick-select">
              <el-button @click="selectAll">{{ $t('batchControl.selectAll') }}</el-button>
              <el-button @click="clearSelection">{{ $t('batchControl.clearSelection') }}</el-button>
              <el-button @click="selectByType">{{ $t('batchControl.selectByType') }}</el-button>
              <el-button @click="selectByStatus">{{ $t('batchControl.selectByStatus') }}</el-button>
            </div>

            <!-- 搜索框 -->
            <el-input
              v-model="searchText"
              :placeholder="$t('batchControl.searchDevices')"
              prefix-icon="Search"
              clearable
              class="search-input"
            />

            <!-- 设备列表 -->
            <div class="device-list">
              <el-checkbox-group v-model="selectedDevices">
                <div v-for="device in filteredDevices" :key="device.id" class="device-item">
                  <el-checkbox :value="device.id">
                    <div class="device-info">
                      <span class="device-name">{{ device.name }}</span>
                      <el-tag size="small" :type="getDeviceStatusType(device.status)">
                        {{ device.status }}
                      </el-tag>
                      <span class="device-type">{{ device.type }}</span>
                    </div>
                  </el-checkbox>
                </div>
              </el-checkbox-group>
            </div>
          </div>
        </el-card>
      </el-col>

      <!-- 操作配置 -->
      <el-col :span="12">
        <el-card>
          <template #header>
            <span>{{ $t('batchControl.operationConfig') }}</span>
          </template>

          <el-form :model="batchForm" label-width="120px">
            <el-form-item :label="$t('batchControl.operationType')">
              <el-radio-group v-model="batchForm.operationType">
                <el-radio value="switch">{{ $t('batchControl.switchControl') }}</el-radio>
                <el-radio value="setValue">{{ $t('batchControl.setValues') }}</el-radio>
                <el-radio value="command">{{ $t('batchControl.sendCommand') }}</el-radio>
              </el-radio-group>
            </el-form-item>

            <!-- 开关控制 -->
            <template v-if="batchForm.operationType === 'switch'">
              <el-form-item :label="$t('batchControl.switchAction')">
                <el-radio-group v-model="batchForm.switchAction">
                  <el-radio value="on">{{ $t('common.on') }}</el-radio>
                  <el-radio value="off">{{ $t('common.off') }}</el-radio>
                  <el-radio value="toggle">{{ $t('batchControl.toggle') }}</el-radio>
                </el-radio-group>
              </el-form-item>
            </template>

            <!-- 设置值 -->
            <template v-if="batchForm.operationType === 'setValue'">
              <el-form-item :label="$t('batchControl.pointType')">
                <el-select v-model="batchForm.pointType" @change="loadCommonPoints">
                  <el-option value="YT" :label="$t('batchControl.analogControl')" />
                  <el-option value="YK" :label="$t('batchControl.digitalControl')" />
                </el-select>
              </el-form-item>
              
              <el-form-item :label="$t('batchControl.commonPoints')">
                <el-select v-model="batchForm.selectedPoint" :placeholder="$t('batchControl.selectPoint')">
                  <el-option
                    v-for="point in commonPoints"
                    :key="point.name"
                    :value="point.name"
                    :label="point.label"
                  />
                </el-select>
              </el-form-item>

              <el-form-item :label="$t('batchControl.setValue')">
                <el-input-number
                  v-model="batchForm.setValue"
                  :min="0"
                  :max="100"
                  :step="1"
                />
              </el-form-item>
            </template>

            <!-- 发送命令 -->
            <template v-if="batchForm.operationType === 'command'">
              <el-form-item :label="$t('batchControl.commandType')">
                <el-select v-model="batchForm.commandType">
                  <el-option value="reset" :label="$t('batchControl.reset')" />
                  <el-option value="calibrate" :label="$t('batchControl.calibrate')" />
                  <el-option value="restart" :label="$t('batchControl.restart')" />
                  <el-option value="custom" :label="$t('batchControl.custom')" />
                </el-select>
              </el-form-item>

              <el-form-item v-if="batchForm.commandType === 'custom'" :label="$t('batchControl.customCommand')">
                <el-input v-model="batchForm.customCommand" />
              </el-form-item>
            </template>

            <el-form-item :label="$t('batchControl.executionMode')">
              <el-radio-group v-model="batchForm.executionMode">
                <el-radio value="immediate">{{ $t('batchControl.immediate') }}</el-radio>
                <el-radio value="scheduled">{{ $t('batchControl.scheduled') }}</el-radio>
                <el-radio value="sequential">{{ $t('batchControl.sequential') }}</el-radio>
              </el-radio-group>
            </el-form-item>

            <el-form-item v-if="batchForm.executionMode === 'scheduled'" :label="$t('batchControl.scheduleTime')">
              <el-date-picker
                v-model="batchForm.scheduleTime"
                type="datetime"
                :placeholder="$t('batchControl.selectTime')"
              />
            </el-form-item>

            <el-form-item v-if="batchForm.executionMode === 'sequential'" :label="$t('batchControl.interval')">
              <el-input-number
                v-model="batchForm.interval"
                :min="1"
                :max="60"
                :step="1"
              />
              <span class="interval-unit">{{ $t('batchControl.seconds') }}</span>
            </el-form-item>

            <el-form-item :label="$t('common.remark')">
              <el-input
                v-model="batchForm.remark"
                type="textarea"
                :rows="3"
                :placeholder="$t('batchControl.remarkPlaceholder')"
              />
            </el-form-item>
          </el-form>

          <!-- 操作按钮 -->
          <div class="operation-buttons">
            <el-button
              type="primary"
              size="large"
              @click="previewOperation"
              :disabled="selectedDevices.length === 0"
            >
              <el-icon><View /></el-icon>
              {{ $t('batchControl.preview') }}
            </el-button>
            <el-button
              type="success"
              size="large"
              @click="executeOperation"
              :disabled="selectedDevices.length === 0 || !userStore.canControl"
            >
              <el-icon><VideoPlay /></el-icon>
              {{ $t('batchControl.execute') }}
            </el-button>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 执行历史 -->
    <el-card class="history-card">
      <template #header>
        <div class="card-header">
          <span>{{ $t('batchControl.executionHistory') }}</span>
          <el-button link @click="clearHistory">{{ $t('batchControl.clearHistory') }}</el-button>
        </div>
      </template>

      <el-table :data="executionHistory" style="width: 100%">
        <el-table-column prop="id" label="ID" width="80" />
        <el-table-column prop="time" :label="$t('common.time')" width="180">
          <template #default="{ row }">
            {{ formatDateTime(row.time) }}
          </template>
        </el-table-column>
        <el-table-column prop="deviceCount" :label="$t('batchControl.deviceCount')" width="120" />
        <el-table-column prop="operationType" :label="$t('batchControl.operationType')" width="120">
          <template #default="{ row }">
            <el-tag>{{ getOperationTypeLabel(row.operationType) }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="details" :label="$t('batchControl.operationDetails')" />
        <el-table-column prop="status" :label="$t('common.status')" width="100">
          <template #default="{ row }">
            <el-tag :type="getStatusType(row.status)">
              {{ getStatusLabel(row.status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="operator" :label="$t('batchControl.operator')" width="120" />
        <el-table-column :label="$t('common.actions')" width="150" fixed="right">
          <template #default="{ row }">
            <el-button link @click="viewDetails(row)">{{ $t('common.details') }}</el-button>
            <el-button link @click="retryOperation(row)" v-if="row.status === 'failed'">
              {{ $t('batchControl.retry') }}
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 预览对话框 -->
    <el-dialog
      v-model="showPreviewDialog"
      :title="$t('batchControl.operationPreview')"
      width="700px"
    >
      <div class="preview-content">
        <el-descriptions :column="2" border>
          <el-descriptions-item :label="$t('batchControl.selectedDevices')">
            {{ selectedDevices.length }} {{ $t('batchControl.devices') }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('batchControl.operationType')">
            {{ getOperationTypeLabel(batchForm.operationType) }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('batchControl.operationDetails')" :span="2">
            {{ getOperationDetails() }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('batchControl.executionMode')">
            {{ getExecutionModeLabel(batchForm.executionMode) }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('batchControl.estimatedTime')">
            {{ estimatedTime }} {{ $t('batchControl.seconds') }}
          </el-descriptions-item>
        </el-descriptions>

        <div class="affected-devices">
          <h4>{{ $t('batchControl.affectedDevices') }}</h4>
          <el-table :data="affectedDevices" max-height="300">
            <el-table-column prop="name" :label="$t('common.name')" />
            <el-table-column prop="type" :label="$t('common.type')" />
            <el-table-column prop="status" :label="$t('common.status')">
              <template #default="{ row }">
                <el-tag size="small" :type="getDeviceStatusType(row.status)">
                  {{ row.status }}
                </el-tag>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </div>

      <template #footer>
        <el-button @click="showPreviewDialog = false">{{ $t('common.cancel') }}</el-button>
        <el-button 
          type="primary" 
          @click="confirmExecute"
          :disabled="!userStore.canControl"
        >
          {{ $t('common.confirm') }}
        </el-button>
      </template>
    </el-dialog>

    <!-- 详情对话框 -->
    <el-dialog
      v-model="showDetailsDialog"
      :title="$t('batchControl.executionDetails')"
      width="800px"
    >
      <div v-if="selectedHistory">
        <el-descriptions :column="2" border>
          <el-descriptions-item :label="$t('batchControl.executionId')">
            {{ selectedHistory.id }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('batchControl.executionTime')">
            {{ formatDateTime(selectedHistory.time) }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('batchControl.operator')">
            {{ selectedHistory.operator }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('common.status')">
            <el-tag :type="getStatusType(selectedHistory.status)">
              {{ getStatusLabel(selectedHistory.status) }}
            </el-tag>
          </el-descriptions-item>
          <el-descriptions-item :label="$t('batchControl.operationType')">
            {{ getOperationTypeLabel(selectedHistory.operationType) }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('batchControl.operationDetails')">
            {{ selectedHistory.details }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('common.remark')" :span="2">
            {{ selectedHistory.remark || '-' }}
          </el-descriptions-item>
        </el-descriptions>

        <div class="device-results">
          <h4>{{ $t('batchControl.deviceResults') }}</h4>
          <el-table :data="selectedHistory.deviceResults" style="width: 100%">
            <el-table-column prop="deviceName" :label="$t('batchControl.deviceName')" />
            <el-table-column prop="result" :label="$t('batchControl.result')">
              <template #default="{ row }">
                <el-tag :type="row.success ? 'success' : 'danger'" size="small">
                  {{ row.success ? $t('common.success') : $t('common.failed') }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="message" :label="$t('batchControl.message')" />
            <el-table-column prop="executionTime" :label="$t('batchControl.executionTime')">
              <template #default="{ row }">
                {{ row.executionTime }}ms
              </template>
            </el-table-column>
          </el-table>
        </div>
      </div>

      <template #footer>
        <el-button @click="showDetailsDialog = false">{{ $t('common.close') }}</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { 
  Refresh, 
  View, 
  VideoPlay
} from '@element-plus/icons-vue'
import dayjs from 'dayjs'
import { useUserStore } from '@/stores/user'

const { t } = useI18n()
const userStore = useUserStore()

// 设备数据
const devices = ref([])
const selectedDevices = ref([])
const searchText = ref('')

// 批量操作表单
const batchForm = ref({
  operationType: 'switch',
  switchAction: 'on',
  pointType: 'YT',
  selectedPoint: '',
  setValue: 0,
  commandType: 'reset',
  customCommand: '',
  executionMode: 'immediate',
  scheduleTime: null,
  interval: 5,
  remark: ''
})

// 常用控制点
const commonPoints = ref([])

// 执行历史
const executionHistory = ref([])

// 对话框
const showPreviewDialog = ref(false)
const showDetailsDialog = ref(false)
const selectedHistory = ref(null)

// 计算属性
const filteredDevices = computed(() => {
  if (!searchText.value) return devices.value
  
  const search = searchText.value.toLowerCase()
  return devices.value.filter(device => 
    device.name.toLowerCase().includes(search) ||
    device.type.toLowerCase().includes(search)
  )
})

const affectedDevices = computed(() => {
  return devices.value.filter(d => selectedDevices.value.includes(d.id))
})

const estimatedTime = computed(() => {
  if (batchForm.value.executionMode === 'sequential') {
    return selectedDevices.value.length * batchForm.value.interval
  }
  return selectedDevices.value.length * 0.1 // 估计每个设备0.1秒
})

// 方法
const refreshData = () => {
  loadDevices()
  ElMessage.success(t('common.refreshSuccess'))
}

const loadDevices = () => {
  // 模拟加载设备数据
  const mockDevices = []
  for (let i = 1; i <= 50; i++) {
    mockDevices.push({
      id: `DEV${String(i).padStart(3, '0')}`,
      name: `Device_${i}`,
      type: ['PLC', 'Sensor', 'Meter', 'Actuator'][Math.floor(Math.random() * 4)],
      status: ['online', 'offline', 'error'][Math.floor(Math.random() * 3)],
      controlPoints: Math.floor(Math.random() * 10) + 1
    })
  }
  devices.value = mockDevices
}

const selectAll = () => {
  selectedDevices.value = devices.value.map(d => d.id)
}

const clearSelection = () => {
  selectedDevices.value = []
}

const selectByType = async () => {
  const types = ['PLC', 'Sensor', 'Meter', 'Actuator']
  try {
    const { value } = await ElMessageBox.prompt(
      t('batchControl.selectDeviceType'),
      t('batchControl.selectByType'),
      {
        inputPattern: new RegExp(`^(${types.join('|')})$`),
        inputErrorMessage: t('batchControl.invalidDeviceType'),
        inputPlaceholder: types.join(' / ')
      }
    )
    
    selectedDevices.value = devices.value
      .filter(d => d.type === value)
      .map(d => d.id)
  } catch {
    // 用户取消
  }
}

const selectByStatus = async () => {
  try {
    await ElMessageBox({
      title: t('batchControl.selectByStatus'),
      showCancelButton: true,
      confirmButtonText: t('common.ok'),
      cancelButtonText: t('common.cancel'),
      showInput: false,
      customClass: 'status-select-dialog',
      beforeClose: (action, instance, done) => {
        if (action === 'confirm') {
          const checkboxes = document.querySelectorAll('.status-select-dialog input[type="checkbox"]:checked')
          const selectedStatuses = Array.from(checkboxes).map(cb => cb.value)
          selectedDevices.value = devices.value
            .filter(d => selectedStatuses.includes(d.status))
            .map(d => d.id)
        }
        done()
      },
      dangerouslyUseHTMLString: true,
      message: `
        <div>
          <p>${t('batchControl.selectDeviceStatus')}</p>
          <label><input type="checkbox" value="online" checked> ${t('common.online')}</label><br>
          <label><input type="checkbox" value="offline"> ${t('common.offline')}</label><br>
          <label><input type="checkbox" value="error"> ${t('common.error')}</label>
        </div>
      `
    })
  } catch {
    // 用户取消
  }
}

const loadCommonPoints = () => {
  // 根据点类型加载常用控制点
  if (batchForm.value.pointType === 'YT') {
    commonPoints.value = [
      { name: 'setpoint_temp', label: 'Temperature Setpoint' },
      { name: 'setpoint_power', label: 'Power Setpoint' },
      { name: 'setpoint_speed', label: 'Speed Setpoint' },
      { name: 'setpoint_voltage', label: 'Voltage Setpoint' }
    ]
  } else {
    commonPoints.value = [
      { name: 'switch_main', label: 'Main Switch' },
      { name: 'switch_backup', label: 'Backup Switch' },
      { name: 'enable_alarm', label: 'Enable Alarm' },
      { name: 'reset_fault', label: 'Reset Fault' }
    ]
  }
}

const getOperationTypeLabel = (type) => {
  const labels = {
    switch: t('batchControl.switchControl'),
    setValue: t('batchControl.setValues'),
    command: t('batchControl.sendCommand')
  }
  return labels[type] || type
}

const getExecutionModeLabel = (mode) => {
  const labels = {
    immediate: t('batchControl.immediate'),
    scheduled: t('batchControl.scheduled'),
    sequential: t('batchControl.sequential')
  }
  return labels[mode] || mode
}

const getOperationDetails = () => {
  switch (batchForm.value.operationType) {
    case 'switch':
      return `${t('batchControl.switchAction')}: ${batchForm.value.switchAction}`
    case 'setValue':
      return `${batchForm.value.selectedPoint} = ${batchForm.value.setValue}`
    case 'command':
      return batchForm.value.commandType === 'custom' 
        ? batchForm.value.customCommand 
        : batchForm.value.commandType
    default:
      return '-'
  }
}

const getDeviceStatusType = (status) => {
  const types = {
    online: 'success',
    offline: 'info',
    error: 'danger'
  }
  return types[status] || 'info'
}

const getStatusType = (status) => {
  const types = {
    executing: 'warning',
    success: 'success',
    failed: 'danger',
    partial: 'warning'
  }
  return types[status] || 'info'
}

const getStatusLabel = (status) => {
  const labels = {
    executing: t('batchControl.executing'),
    success: t('common.success'),
    failed: t('common.failed'),
    partial: t('batchControl.partial')
  }
  return labels[status] || status
}

const formatDateTime = (timestamp) => {
  return dayjs(timestamp).format('YYYY-MM-DD HH:mm:ss')
}

const previewOperation = () => {
  if (selectedDevices.value.length === 0) {
    ElMessage.warning(t('batchControl.selectDevicesFirst'))
    return
  }
  showPreviewDialog.value = true
}

const executeOperation = () => {
  previewOperation()
}

const confirmExecute = async () => {
  showPreviewDialog.value = false
  
  // 模拟执行批量操作
  const historyItem = {
    id: `BATCH${Date.now()}`,
    time: Date.now(),
    deviceCount: selectedDevices.value.length,
    operationType: batchForm.value.operationType,
    details: getOperationDetails(),
    status: 'executing',
    operator: userStore.userInfo.name,
    remark: batchForm.value.remark,
    deviceResults: []
  }
  
  executionHistory.value.unshift(historyItem)
  
  // 模拟执行过程
  let successCount = 0
  for (const deviceId of selectedDevices.value) {
    const device = devices.value.find(d => d.id === deviceId)
    const success = Math.random() > 0.1 // 90%成功率
    
    historyItem.deviceResults.push({
      deviceId,
      deviceName: device.name,
      success,
      message: success ? 'Operation completed' : 'Device not responding',
      executionTime: Math.floor(Math.random() * 100) + 50
    })
    
    if (success) successCount++
    
    // 如果是顺序执行，等待间隔时间
    if (batchForm.value.executionMode === 'sequential' && batchForm.value.interval > 0) {
      await new Promise(resolve => setTimeout(resolve, batchForm.value.interval * 1000))
    }
  }
  
  // 更新状态
  historyItem.status = successCount === selectedDevices.value.length 
    ? 'success' 
    : successCount > 0 
      ? 'partial' 
      : 'failed'
  
  ElMessage({
    type: historyItem.status === 'success' ? 'success' : 'warning',
    message: t('batchControl.executionComplete', { 
      success: successCount, 
      total: selectedDevices.value.length 
    })
  })
  
  // 清空选择
  selectedDevices.value = []
}

const viewDetails = (row) => {
  selectedHistory.value = row
  showDetailsDialog.value = true
}

const retryOperation = async (row) => {
  try {
    await ElMessageBox.confirm(
      t('batchControl.retryConfirm'),
      t('common.confirm'),
      {
        confirmButtonText: t('common.ok'),
        cancelButtonText: t('common.cancel'),
        type: 'warning'
      }
    )
    
    // 重新执行失败的设备
    const failedDevices = row.deviceResults
      .filter(r => !r.success)
      .map(r => r.deviceId)
    
    selectedDevices.value = failedDevices
    batchForm.value.operationType = row.operationType
    executeOperation()
  } catch {
    // 用户取消
  }
}

const clearHistory = async () => {
  try {
    await ElMessageBox.confirm(
      t('batchControl.clearHistoryConfirm'),
      t('common.warning'),
      {
        confirmButtonText: t('common.ok'),
        cancelButtonText: t('common.cancel'),
        type: 'warning'
      }
    )
    
    executionHistory.value = []
    ElMessage.success(t('batchControl.historyCleared'))
  } catch {
    // 用户取消
  }
}

// 生命周期
onMounted(() => {
  loadDevices()
  loadCommonPoints()
  
  // 模拟一些历史记录
  for (let i = 1; i <= 5; i++) {
    executionHistory.value.push({
      id: `BATCH2025010${i}`,
      time: Date.now() - i * 3600000,
      deviceCount: Math.floor(Math.random() * 20) + 5,
      operationType: ['switch', 'setValue', 'command'][Math.floor(Math.random() * 3)],
      details: 'Historical operation',
      status: ['success', 'failed', 'partial'][Math.floor(Math.random() * 3)],
      operator: ['admin', 'engineer'][Math.floor(Math.random() * 2)],
      deviceResults: []
    })
  }
})
</script>

<style lang="scss" scoped>
@import '@/styles/design-tokens.scss';

.batch-control-container {
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
      background: var(--color-background-elevated);
      border: 1px solid var(--color-border-light);
      color: var(--color-text-secondary);
      border-radius: var(--radius-lg);
      font-weight: var(--font-weight-medium);
      transition: all var(--duration-fast) var(--ease-in-out);
      
      &:hover {
        background: var(--color-background-secondary);
        border-color: var(--color-border);
        color: var(--color-text-primary);
      }
    }
  }
}

// Tesla 风格卡片
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
      
      .el-tag {
        background: rgba(var(--color-info-rgb), 0.1);
        color: var(--color-info);
        border: none;
        border-radius: var(--radius-md);
        font-weight: var(--font-weight-medium);
      }
    }
  }
  
  :deep(.el-card__body) {
    padding: var(--space-5);
  }
}

// 设备选择区域
.device-selection {
  .quick-select {
    display: flex;
    gap: var(--space-2);
    margin-bottom: var(--space-4);
    flex-wrap: wrap;
    
    .el-button {
      height: 36px;
      background: var(--color-background-secondary);
      border: 1px solid var(--color-border-light);
      color: var(--color-text-secondary);
      border-radius: var(--radius-lg);
      font-weight: var(--font-weight-medium);
      transition: all var(--duration-fast) var(--ease-in-out);
      
      &:hover {
        background: var(--color-primary);
        border-color: var(--color-primary);
        color: var(--color-text-inverse);
      }
    }
  }
  
  .search-input {
    margin-bottom: var(--space-4);
    
    :deep(.el-input__wrapper) {
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
  
  .device-list {
    height: 400px;
    overflow-y: auto;
    border: 1px solid var(--color-border-light);
    border-radius: var(--radius-lg);
    padding: var(--space-3);
    background: var(--color-background-secondary);
    
    &::-webkit-scrollbar {
      width: 8px;
    }
    
    &::-webkit-scrollbar-track {
      background: var(--color-background-secondary);
      border-radius: var(--radius-md);
    }
    
    &::-webkit-scrollbar-thumb {
      background: var(--color-gray-300);
      border-radius: var(--radius-md);
      
      &:hover {
        background: var(--color-gray-400);
      }
    }
    
    .device-item {
      margin-bottom: var(--space-2);
      padding: var(--space-2);
      border-radius: var(--radius-md);
      transition: all var(--duration-fast) var(--ease-in-out);
      
      &:hover {
        background: var(--color-background-elevated);
      }
      
      :deep(.el-checkbox) {
        width: 100%;
        
        .el-checkbox__label {
          width: calc(100% - 24px);
        }
      }
      
      .device-info {
        display: flex;
        align-items: center;
        gap: var(--space-2);
        
        .device-name {
          font-weight: var(--font-weight-semibold);
          color: var(--color-text-primary);
          flex: 1;
        }
        
        .el-tag {
          border-radius: var(--radius-md);
          font-weight: var(--font-weight-medium);
          border: none;
          font-size: var(--font-size-xs);
        }
        
        .device-type {
          color: var(--color-text-tertiary);
          font-size: var(--font-size-sm);
          font-weight: var(--font-weight-medium);
        }
      }
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
  
  .el-radio-group {
    display: flex;
    gap: var(--space-4);
    
    .el-radio {
      margin-right: 0;
      
      .el-radio__label {
        font-weight: var(--font-weight-medium);
        color: var(--color-text-primary);
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
  
  .el-input-number {
    width: 200px;
    
    .el-input__wrapper {
      border-radius: var(--radius-lg);
      box-shadow: none;
      border: 1px solid var(--color-border-light);
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

// 操作按钮
.operation-buttons {
  margin-top: var(--space-6);
  display: flex;
  justify-content: center;
  gap: var(--space-4);
  padding-top: var(--space-5);
  border-top: 1px solid var(--color-border-light);
  
  .el-button {
    min-width: 140px;
    height: 44px;
    border-radius: var(--radius-lg);
    font-weight: var(--font-weight-semibold);
    font-size: var(--font-size-base);
    transition: all var(--duration-normal) var(--ease-in-out);
    
    &[type="primary"] {
      background: var(--color-primary);
      border-color: var(--color-primary);
      
      &:hover:not(:disabled) {
        background: var(--color-primary-hover);
        transform: translateY(-1px);
        box-shadow: var(--shadow-lg);
      }
    }
    
    &[type="success"] {
      background: var(--color-success);
      border-color: var(--color-success);
      
      &:hover:not(:disabled) {
        transform: translateY(-1px);
        box-shadow: var(--shadow-lg);
      }
    }
    
    .el-icon {
      margin-right: var(--space-2);
    }
  }
}

// 历史记录卡片
.history-card {
  margin-top: var(--space-6);
  
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
    
    .el-tag {
      border-radius: var(--radius-md);
      font-weight: var(--font-weight-medium);
      border: none;
    }
    
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

// 预览内容
.preview-content {
  .affected-devices {
    margin-top: var(--space-5);
    
    h4 {
      margin-bottom: var(--space-3);
      font-size: var(--font-size-lg);
      font-weight: var(--font-weight-semibold);
      color: var(--color-text-primary);
    }
  }
}

// 设备结果
.device-results {
  margin-top: var(--space-5);
  
  h4 {
    margin-bottom: var(--space-3);
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }
}

// 间隔单位
.interval-unit {
  margin-left: var(--space-2);
  color: var(--color-text-tertiary);
  font-weight: var(--font-weight-medium);
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
@media (max-width: 1200px) {
  .el-row {
    .el-col {
      margin-bottom: var(--space-5);
    }
  }
  
  .device-list {
    height: 300px;
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
  
  .operation-buttons {
    flex-direction: column;
    
    .el-button {
      width: 100%;
    }
  }
}
</style>