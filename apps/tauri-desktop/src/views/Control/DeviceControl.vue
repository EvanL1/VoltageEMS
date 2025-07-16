<template>
  <div class="device-control">
    <!-- Control Header -->
    <div class="control-header">
      <el-card>
        <el-row :gutter="20" align="middle">
          <el-col :span="6">
            <el-statistic title="Total Control Points" :value="totalControlPoints">
              <template #prefix>
                <el-icon color="#409EFF"><Operation /></el-icon>
              </template>
            </el-statistic>
          </el-col>
          
          <el-col :span="6">
            <el-statistic title="Active Controls" :value="activeControls">
              <template #prefix>
                <el-icon color="#67C23A"><CircleCheck /></el-icon>
              </template>
            </el-statistic>
          </el-col>
          
          <el-col :span="6">
            <el-statistic title="Pending Commands" :value="pendingCommands">
              <template #prefix>
                <el-icon color="#E6A23C"><Clock /></el-icon>
              </template>
            </el-statistic>
          </el-col>
          
          <el-col :span="6">
            <el-statistic title="Failed Commands" :value="failedCommands">
              <template #prefix>
                <el-icon color="#F56C6C"><CircleClose /></el-icon>
              </template>
            </el-statistic>
          </el-col>
        </el-row>
      </el-card>
    </div>
    
    <!-- Control Panels -->
    <el-row :gutter="20" class="control-panels">
      <!-- Channel Selection -->
      <el-col :span="6">
        <el-card class="channel-list-card">
          <template #header>
            <div class="card-header">
              <span>Channels</span>
              <el-input
                v-model="channelSearch"
                placeholder="Search..."
                :prefix-icon="Search"
                size="small"
                clearable
              />
            </div>
          </template>
          
          <el-scrollbar height="600px">
            <div
              v-for="channel in filteredChannels"
              :key="channel.channel_id"
              class="channel-item"
              :class="{ active: selectedChannel?.channel_id === channel.channel_id }"
              @click="selectChannel(channel)"
            >
              <div class="channel-info">
                <div class="channel-name">{{ channel.name }}</div>
                <div class="channel-meta">
                  <el-tag :type="getStatusType(channel.status)" size="small">
                    {{ channel.status }}
                  </el-tag>
                  <span class="point-count">{{ channel.control_points }} control points</span>
                </div>
              </div>
              <el-icon class="channel-arrow"><ArrowRight /></el-icon>
            </div>
          </el-scrollbar>
        </el-card>
      </el-col>
      
      <!-- Control Points -->
      <el-col :span="18">
        <el-card v-if="selectedChannel" class="control-points-card">
          <template #header>
            <div class="card-header">
              <span>{{ selectedChannel.name }} - Control Points</span>
              <el-space>
                <el-button 
                  type="danger" 
                  size="small"
                  :disabled="!hasSelectedPoints"
                  @click="emergencyStop"
                >
                  <el-icon><VideoPause /></el-icon>
                  Emergency Stop
                </el-button>
                <el-button 
                  type="primary" 
                  size="small"
                  @click="refreshPoints"
                >
                  <el-icon><Refresh /></el-icon>
                  Refresh
                </el-button>
              </el-space>
            </div>
          </template>
          
          <!-- Control Tabs -->
          <el-tabs v-model="activeTab">
            <!-- Binary Controls (YK) -->
            <el-tab-pane label="Binary Controls (YK)" name="YK">
              <div class="control-grid">
                <div
                  v-for="point in ykPoints"
                  :key="point.point_id"
                  class="control-item"
                >
                  <div class="control-header">
                    <span class="control-id">{{ point.point_id }}</span>
                    <el-tag :type="point.value ? 'success' : 'info'" size="small">
                      {{ point.value ? 'ON' : 'OFF' }}
                    </el-tag>
                  </div>
                  
                  <div class="control-name">{{ point.description }}</div>
                  
                  <div class="control-status">
                    <span class="label">Quality:</span>
                    <el-tag :type="getQualityType(point.quality)" size="small">
                      {{ getQualityText(point.quality) }}
                    </el-tag>
                  </div>
                  
                  <div class="control-actions">
                    <el-button-group>
                      <el-button
                        :type="point.value ? 'success' : 'default'"
                        @click="sendControl(point, 1)"
                        :loading="point.loading"
                      >
                        ON
                      </el-button>
                      <el-button
                        :type="!point.value ? 'danger' : 'default'"
                        @click="sendControl(point, 0)"
                        :loading="point.loading"
                      >
                        OFF
                      </el-button>
                    </el-button-group>
                  </div>
                </div>
              </div>
            </el-tab-pane>
            
            <!-- Analog Controls (YT) -->
            <el-tab-pane label="Analog Controls (YT)" name="YT">
              <div class="control-grid">
                <div
                  v-for="point in ytPoints"
                  :key="point.point_id"
                  class="control-item analog"
                >
                  <div class="control-header">
                    <span class="control-id">{{ point.point_id }}</span>
                    <span class="control-value">{{ point.value }} {{ point.unit }}</span>
                  </div>
                  
                  <div class="control-name">{{ point.description }}</div>
                  
                  <div class="control-range">
                    <span>{{ point.min }}</span>
                    <el-slider
                      v-model="point.newValue"
                      :min="point.min"
                      :max="point.max"
                      :step="point.step"
                      :show-tooltip="true"
                    />
                    <span>{{ point.max }}</span>
                  </div>
                  
                  <div class="control-input">
                    <el-input-number
                      v-model="point.newValue"
                      :min="point.min"
                      :max="point.max"
                      :step="point.step"
                      :precision="point.precision"
                      controls-position="right"
                    />
                    <el-button
                      type="primary"
                      @click="sendAdjustment(point)"
                      :loading="point.loading"
                    >
                      Set
                    </el-button>
                  </div>
                </div>
              </div>
            </el-tab-pane>
            
            <!-- Batch Control -->
            <el-tab-pane label="Batch Control" name="batch">
              <div class="batch-control">
                <el-form :model="batchForm" label-width="120px">
                  <el-form-item label="Control Type">
                    <el-radio-group v-model="batchForm.type">
                      <el-radio label="YK">Binary Control</el-radio>
                      <el-radio label="YT">Analog Control</el-radio>
                    </el-radio-group>
                  </el-form-item>
                  
                  <el-form-item label="Select Points">
                    <el-select
                      v-model="batchForm.points"
                      multiple
                      filterable
                      placeholder="Select control points"
                      style="width: 100%"
                    >
                      <el-option
                        v-for="point in availableBatchPoints"
                        :key="point.point_id"
                        :label="`${point.point_id} - ${point.description}`"
                        :value="point.point_id"
                      />
                    </el-select>
                  </el-form-item>
                  
                  <el-form-item v-if="batchForm.type === 'YK'" label="Control Value">
                    <el-radio-group v-model="batchForm.value">
                      <el-radio :label="1">ON</el-radio>
                      <el-radio :label="0">OFF</el-radio>
                    </el-radio-group>
                  </el-form-item>
                  
                  <el-form-item v-else label="Set Value">
                    <el-input-number
                      v-model="batchForm.value"
                      :precision="2"
                      style="width: 200px"
                    />
                  </el-form-item>
                  
                  <el-form-item label="Execution">
                    <el-radio-group v-model="batchForm.execution">
                      <el-radio label="simultaneous">Simultaneous</el-radio>
                      <el-radio label="sequential">Sequential</el-radio>
                    </el-radio-group>
                  </el-form-item>
                  
                  <el-form-item v-if="batchForm.execution === 'sequential'" label="Delay (ms)">
                    <el-input-number
                      v-model="batchForm.delay"
                      :min="0"
                      :max="10000"
                      :step="100"
                      style="width: 200px"
                    />
                  </el-form-item>
                  
                  <el-form-item>
                    <el-button
                      type="primary"
                      @click="executeBatchControl"
                      :disabled="batchForm.points.length === 0"
                    >
                      Execute Batch Control
                    </el-button>
                    <el-button @click="resetBatchForm">Reset</el-button>
                  </el-form-item>
                </el-form>
              </div>
            </el-tab-pane>
            
            <!-- Control History -->
            <el-tab-pane label="Control History" name="history">
              <el-table :data="controlHistory" style="width: 100%">
                <el-table-column prop="timestamp" label="Timestamp" width="180" />
                <el-table-column prop="pointId" label="Point ID" width="100" />
                <el-table-column prop="description" label="Description" />
                <el-table-column prop="action" label="Action" width="120">
                  <template #default="{ row }">
                    <el-tag>{{ row.action }}</el-tag>
                  </template>
                </el-table-column>
                <el-table-column prop="value" label="Value" width="100" />
                <el-table-column prop="status" label="Status" width="100">
                  <template #default="{ row }">
                    <el-tag :type="row.status === 'success' ? 'success' : 'danger'">
                      {{ row.status }}
                    </el-tag>
                  </template>
                </el-table-column>
                <el-table-column prop="operator" label="Operator" width="120" />
              </el-table>
            </el-tab-pane>
          </el-tabs>
        </el-card>
        
        <!-- No Channel Selected -->
        <el-empty v-else description="Select a channel to view control points" />
      </el-col>
    </el-row>
    
    <!-- Control Confirmation Dialog -->
    <el-dialog
      v-model="showConfirmDialog"
      title="Confirm Control Command"
      width="400px"
    >
      <div class="confirm-content">
        <el-alert
          title="Please confirm the control command"
          type="warning"
          :closable="false"
          show-icon
        />
        
        <div class="confirm-details">
          <div class="detail-item">
            <span class="label">Channel:</span>
            <span class="value">{{ confirmData.channel }}</span>
          </div>
          <div class="detail-item">
            <span class="label">Point:</span>
            <span class="value">{{ confirmData.point }}</span>
          </div>
          <div class="detail-item">
            <span class="label">Action:</span>
            <span class="value">{{ confirmData.action }}</span>
          </div>
          <div class="detail-item">
            <span class="label">Value:</span>
            <span class="value">{{ confirmData.value }}</span>
          </div>
        </div>
        
        <el-input
          v-model="confirmReason"
          type="textarea"
          placeholder="Enter reason for this control action (optional)"
          :rows="3"
          style="margin-top: 20px"
        />
      </div>
      
      <template #footer>
        <el-button @click="cancelControl">Cancel</el-button>
        <el-button type="primary" @click="confirmControl" :loading="confirmLoading">
          Confirm
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { 
  Operation,
  CircleCheck,
  Clock,
  CircleClose,
  Search,
  ArrowRight,
  VideoPause,
  Refresh
} from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import dayjs from 'dayjs'

// Mock data
const channels = ref([
  { 
    channel_id: 1, 
    name: 'Main Power System', 
    status: 'online',
    control_points: 12
  },
  { 
    channel_id: 2, 
    name: 'Solar Panel Control', 
    status: 'online',
    control_points: 8
  },
  { 
    channel_id: 3, 
    name: 'Energy Storage System', 
    status: 'offline',
    control_points: 15
  },
  { 
    channel_id: 4, 
    name: 'Diesel Generator', 
    status: 'online',
    control_points: 10
  }
])

// Statistics
const totalControlPoints = ref(45)
const activeControls = ref(30)
const pendingCommands = ref(2)
const failedCommands = ref(1)

// Channel selection
const channelSearch = ref('')
const selectedChannel = ref<any>(null)

const filteredChannels = computed(() => {
  if (!channelSearch.value) return channels.value
  
  const search = channelSearch.value.toLowerCase()
  return channels.value.filter(ch => 
    ch.name.toLowerCase().includes(search) ||
    ch.channel_id.toString().includes(search)
  )
})

// Control points
const activeTab = ref('YK')
const ykPoints = ref<any[]>([])
const ytPoints = ref<any[]>([])
const hasSelectedPoints = ref(false)

// Batch control
const batchForm = ref({
  type: 'YK',
  points: [] as number[],
  value: 1,
  execution: 'simultaneous',
  delay: 500
})

const availableBatchPoints = computed(() => {
  if (batchForm.value.type === 'YK') {
    return ykPoints.value
  } else {
    return ytPoints.value
  }
})

// Control history
const controlHistory = ref([
  {
    timestamp: dayjs().subtract(5, 'minute').format('YYYY-MM-DD HH:mm:ss'),
    pointId: 30001,
    description: 'Main Circuit Breaker',
    action: 'CONTROL',
    value: 'ON',
    status: 'success',
    operator: 'Admin'
  },
  {
    timestamp: dayjs().subtract(10, 'minute').format('YYYY-MM-DD HH:mm:ss'),
    pointId: 40001,
    description: 'Power Setpoint',
    action: 'ADJUST',
    value: '85.5 kW',
    status: 'success',
    operator: 'Admin'
  }
])

// Confirmation dialog
const showConfirmDialog = ref(false)
const confirmData = ref<any>({})
const confirmReason = ref('')
const confirmLoading = ref(false)
const pendingAction = ref<any>(null)

// Methods
function selectChannel(channel: any) {
  selectedChannel.value = channel
  loadControlPoints()
}

function loadControlPoints() {
  // Mock YK points
  ykPoints.value = [
    {
      point_id: 30001,
      description: 'Main Circuit Breaker',
      value: 1,
      quality: 192,
      loading: false
    },
    {
      point_id: 30002,
      description: 'Auxiliary Power Switch',
      value: 0,
      quality: 192,
      loading: false
    },
    {
      point_id: 30003,
      description: 'Emergency Stop',
      value: 0,
      quality: 192,
      loading: false
    },
    {
      point_id: 30004,
      description: 'Cooling System',
      value: 1,
      quality: 192,
      loading: false
    }
  ]
  
  // Mock YT points
  ytPoints.value = [
    {
      point_id: 40001,
      description: 'Power Setpoint',
      value: 75.5,
      newValue: 75.5,
      unit: 'kW',
      min: 0,
      max: 100,
      step: 0.5,
      precision: 1,
      loading: false
    },
    {
      point_id: 40002,
      description: 'Voltage Setpoint',
      value: 220,
      newValue: 220,
      unit: 'V',
      min: 200,
      max: 240,
      step: 1,
      precision: 0,
      loading: false
    },
    {
      point_id: 40003,
      description: 'Power Factor Target',
      value: 0.95,
      newValue: 0.95,
      unit: '',
      min: 0.8,
      max: 1.0,
      step: 0.01,
      precision: 2,
      loading: false
    }
  ]
}

function refreshPoints() {
  ElMessage.success('Control points refreshed')
  loadControlPoints()
}

function sendControl(point: any, value: number) {
  confirmData.value = {
    channel: selectedChannel.value.name,
    point: `${point.point_id} - ${point.description}`,
    action: 'CONTROL',
    value: value === 1 ? 'ON' : 'OFF'
  }
  
  pendingAction.value = () => {
    point.loading = true
    
    // Simulate control command
    setTimeout(() => {
      point.value = value
      point.loading = false
      
      // Add to history
      controlHistory.value.unshift({
        timestamp: dayjs().format('YYYY-MM-DD HH:mm:ss'),
        pointId: point.point_id,
        description: point.description,
        action: 'CONTROL',
        value: value === 1 ? 'ON' : 'OFF',
        status: 'success',
        operator: 'Admin'
      })
      
      ElMessage.success('Control command sent successfully')
    }, 1000)
  }
  
  showConfirmDialog.value = true
}

function sendAdjustment(point: any) {
  confirmData.value = {
    channel: selectedChannel.value.name,
    point: `${point.point_id} - ${point.description}`,
    action: 'ADJUST',
    value: `${point.newValue} ${point.unit}`
  }
  
  pendingAction.value = () => {
    point.loading = true
    
    // Simulate adjustment command
    setTimeout(() => {
      point.value = point.newValue
      point.loading = false
      
      // Add to history
      controlHistory.value.unshift({
        timestamp: dayjs().format('YYYY-MM-DD HH:mm:ss'),
        pointId: point.point_id,
        description: point.description,
        action: 'ADJUST',
        value: `${point.newValue} ${point.unit}`,
        status: 'success',
        operator: 'Admin'
      })
      
      ElMessage.success('Adjustment command sent successfully')
    }, 1000)
  }
  
  showConfirmDialog.value = true
}

function confirmControl() {
  confirmLoading.value = true
  
  setTimeout(() => {
    confirmLoading.value = false
    showConfirmDialog.value = false
    
    if (pendingAction.value) {
      pendingAction.value()
      pendingAction.value = null
    }
    
    confirmReason.value = ''
  }, 500)
}

function cancelControl() {
  showConfirmDialog.value = false
  pendingAction.value = null
  confirmReason.value = ''
}

async function emergencyStop() {
  await ElMessageBox.confirm(
    'This will send STOP command to all control points. Continue?',
    'Emergency Stop',
    {
      confirmButtonText: 'Execute',
      cancelButtonText: 'Cancel',
      type: 'warning'
    }
  )
  
  ElMessage.warning('Emergency stop executed')
}

function executeBatchControl() {
  ElMessageBox.confirm(
    `Execute batch control for ${batchForm.value.points.length} points?`,
    'Batch Control',
    {
      confirmButtonText: 'Execute',
      cancelButtonText: 'Cancel',
      type: 'warning'
    }
  ).then(() => {
    ElMessage.success(`Batch control executed for ${batchForm.value.points.length} points`)
    resetBatchForm()
  })
}

function resetBatchForm() {
  batchForm.value = {
    type: 'YK',
    points: [],
    value: 1,
    execution: 'simultaneous',
    delay: 500
  }
}

// Utility methods
function getStatusType(status: string) {
  switch (status) {
    case 'online':
      return 'success'
    case 'offline':
      return 'warning'
    case 'error':
      return 'danger'
    default:
      return 'info'
  }
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

onMounted(() => {
  // Select first channel by default
  if (channels.value.length > 0) {
    selectChannel(channels.value[0])
  }
})
</script>

<style lang="scss" scoped>
.device-control {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 20px;
  
  .control-header {
    :deep(.el-statistic__number) {
      font-size: 24px;
    }
  }
  
  .control-panels {
    flex: 1;
    
    .channel-list-card {
      height: 100%;
      
      .card-header {
        display: flex;
        flex-direction: column;
        gap: 10px;
      }
      
      .channel-item {
        padding: 12px;
        margin-bottom: 8px;
        border: 1px solid #e4e7ed;
        border-radius: 4px;
        cursor: pointer;
        display: flex;
        justify-content: space-between;
        align-items: center;
        transition: all 0.3s;
        
        &:hover {
          background-color: #f5f7fa;
          border-color: #409EFF;
        }
        
        &.active {
          background-color: #ecf5ff;
          border-color: #409EFF;
        }
        
        .channel-info {
          flex: 1;
          
          .channel-name {
            font-weight: 500;
            margin-bottom: 4px;
          }
          
          .channel-meta {
            display: flex;
            align-items: center;
            gap: 10px;
            font-size: 12px;
            color: #909399;
          }
        }
      }
    }
    
    .control-points-card {
      height: 100%;
      
      .card-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
      }
      
      .control-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
        gap: 20px;
      }
      
      .control-item {
        border: 1px solid #e4e7ed;
        border-radius: 4px;
        padding: 16px;
        
        .control-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 8px;
          
          .control-id {
            font-weight: 600;
            color: #606266;
          }
        }
        
        .control-name {
          font-size: 16px;
          margin-bottom: 12px;
        }
        
        .control-status {
          display: flex;
          align-items: center;
          gap: 8px;
          margin-bottom: 12px;
          font-size: 14px;
          
          .label {
            color: #909399;
          }
        }
        
        .control-value {
          font-size: 18px;
          font-weight: 600;
          color: #409EFF;
        }
        
        .control-range {
          display: flex;
          align-items: center;
          gap: 10px;
          margin: 16px 0;
          
          .el-slider {
            flex: 1;
          }
        }
        
        .control-input {
          display: flex;
          gap: 10px;
          
          .el-input-number {
            flex: 1;
          }
        }
        
        &.analog {
          .control-actions {
            margin-top: 16px;
          }
        }
      }
      
      .batch-control {
        max-width: 600px;
      }
    }
  }
  
  .confirm-content {
    .confirm-details {
      margin-top: 20px;
      
      .detail-item {
        display: flex;
        padding: 8px 0;
        
        .label {
          width: 80px;
          color: #909399;
        }
        
        .value {
          flex: 1;
          font-weight: 500;
        }
      }
    }
  }
}
</style>