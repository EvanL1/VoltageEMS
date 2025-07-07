<template>
  <div class="device-status-container">
    <!-- 页面头部 -->
    <div class="page-header">
      <h1>{{ $t('deviceStatus.title') }}</h1>
      <div class="header-actions">
        <el-radio-group v-model="viewMode" size="default">
          <el-radio-button label="card">{{ $t('deviceStatus.cardView') }}</el-radio-button>
          <el-radio-button label="list">{{ $t('deviceStatus.listView') }}</el-radio-button>
        </el-radio-group>
        <el-button @click="handleRefreshAll">
          <el-icon><Refresh /></el-icon>
          {{ $t('deviceStatus.refreshAll') }}
        </el-button>
      </div>
    </div>

    <!-- 统计卡片 -->
    <div class="stats-cards">
      <el-row :gutter="20">
        <el-col :xs="24" :sm="12" :md="6">
          <el-card class="stat-card">
            <div class="stat-content">
              <div class="stat-info">
                <div class="stat-value">{{ stats.total }}</div>
                <div class="stat-label">{{ $t('deviceStatus.totalDevices') }}</div>
              </div>
              <el-icon class="stat-icon" :size="40" color="#409EFF">
                <Monitor />
              </el-icon>
            </div>
          </el-card>
        </el-col>
        <el-col :xs="24" :sm="12" :md="6">
          <el-card class="stat-card">
            <div class="stat-content">
              <div class="stat-info">
                <div class="stat-value online">{{ stats.online }}</div>
                <div class="stat-label">{{ $t('deviceStatus.onlineDevices') }}</div>
              </div>
              <el-icon class="stat-icon" :size="40" color="#67C23A">
                <CircleCheck />
              </el-icon>
            </div>
          </el-card>
        </el-col>
        <el-col :xs="24" :sm="12" :md="6">
          <el-card class="stat-card">
            <div class="stat-content">
              <div class="stat-info">
                <div class="stat-value offline">{{ stats.offline }}</div>
                <div class="stat-label">{{ $t('deviceStatus.offlineDevices') }}</div>
              </div>
              <el-icon class="stat-icon" :size="40" color="#F56C6C">
                <CircleClose />
              </el-icon>
            </div>
          </el-card>
        </el-col>
        <el-col :xs="24" :sm="12" :md="6">
          <el-card class="stat-card">
            <div class="stat-content">
              <div class="stat-info">
                <div class="stat-value warning">{{ stats.warning }}</div>
                <div class="stat-label">{{ $t('deviceStatus.warningDevices') }}</div>
              </div>
              <el-icon class="stat-icon" :size="40" color="#E6A23C">
                <Warning />
              </el-icon>
            </div>
          </el-card>
        </el-col>
      </el-row>
    </div>

    <!-- 筛选条件 -->
    <el-card class="filter-card">
      <el-form :inline="true" :model="filterForm">
        <el-form-item :label="$t('deviceStatus.deviceType')">
          <el-select 
            v-model="filterForm.type" 
            :placeholder="$t('deviceStatus.selectType')"
            clearable
            @change="handleFilter"
          >
            <el-option label="PLC" value="plc" />
            <el-option label="Sensor" value="sensor" />
            <el-option label="Meter" value="meter" />
            <el-option label="Actuator" value="actuator" />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('deviceStatus.status')">
          <el-select 
            v-model="filterForm.status" 
            :placeholder="$t('deviceStatus.selectStatus')"
            clearable
            @change="handleFilter"
          >
            <el-option :label="$t('common.online')" value="online" />
            <el-option :label="$t('common.offline')" value="offline" />
            <el-option :label="$t('deviceStatus.warning')" value="warning" />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('deviceStatus.channel')">
          <el-select 
            v-model="filterForm.channel" 
            :placeholder="$t('deviceStatus.selectChannel')"
            clearable
            @change="handleFilter"
          >
            <el-option label="Channel 1 - Modbus TCP" value="1" />
            <el-option label="Channel 2 - IEC 104" value="2" />
            <el-option label="Channel 3 - CAN" value="3" />
          </el-select>
        </el-form-item>
        <el-form-item>
          <el-input 
            v-model="filterForm.keyword" 
            :placeholder="$t('deviceStatus.searchPlaceholder')"
            clearable
            @input="handleSearch"
          >
            <template #prefix>
              <el-icon><Search /></el-icon>
            </template>
          </el-input>
        </el-form-item>
      </el-form>
    </el-card>

    <!-- 设备列表 -->
    <el-card class="device-list-card">
      <!-- 卡片视图 -->
      <div v-if="viewMode === 'card'" class="device-cards">
        <el-row :gutter="20">
          <el-col 
            v-for="device in filteredDevices" 
            :key="device.id"
            :xs="24" 
            :sm="12" 
            :md="8" 
            :lg="6"
          >
            <div class="device-card" :class="getStatusClass(device.status)">
              <div class="device-header">
                <el-icon class="device-icon" :size="24">
                  <component :is="getDeviceIcon(device.type)" />
                </el-icon>
                <el-tag :type="getStatusType(device.status)" size="small">
                  {{ getStatusLabel(device.status) }}
                </el-tag>
              </div>
              <div class="device-info">
                <h3>{{ device.name }}</h3>
                <p class="device-id">ID: {{ device.id }}</p>
                <p class="device-type">{{ device.type }}</p>
              </div>
              <div class="device-stats">
                <div class="stat-item">
                  <span class="label">{{ $t('deviceStatus.quality') }}:</span>
                  <el-progress 
                    :percentage="device.quality" 
                    :color="getQualityColor(device.quality)"
                    :stroke-width="6"
                  />
                </div>
                <div class="stat-item">
                  <span class="label">{{ $t('deviceStatus.lastUpdate') }}:</span>
                  <span class="value">{{ formatTime(device.lastUpdate) }}</span>
                </div>
              </div>
              <div class="device-actions">
                <el-button 
                  type="primary" 
                  size="small" 
                  @click="showDeviceDetails(device)"
                >
                  {{ $t('common.details') }}
                </el-button>
                <el-button 
                  size="small" 
                  @click="refreshDevice(device)"
                >
                  {{ $t('common.refresh') }}
                </el-button>
              </div>
            </div>
          </el-col>
        </el-row>
      </div>

      <!-- 列表视图 -->
      <el-table 
        v-else 
        :data="filteredDevices" 
        stripe
        v-loading="loading"
      >
        <el-table-column prop="id" :label="$t('deviceStatus.deviceId')" width="120" />
        <el-table-column prop="name" :label="$t('deviceStatus.deviceName')" min-width="150" />
        <el-table-column prop="type" :label="$t('deviceStatus.deviceType')" width="120" />
        <el-table-column prop="channel" :label="$t('deviceStatus.channel')" width="150">
          <template #default="{ row }">
            Channel {{ row.channel }}
          </template>
        </el-table-column>
        <el-table-column prop="status" :label="$t('deviceStatus.status')" width="100">
          <template #default="{ row }">
            <el-tag :type="getStatusType(row.status)">
              {{ getStatusLabel(row.status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="quality" :label="$t('deviceStatus.quality')" width="120">
          <template #default="{ row }">
            <el-progress 
              :percentage="row.quality" 
              :color="getQualityColor(row.quality)"
            />
          </template>
        </el-table-column>
        <el-table-column prop="lastUpdate" :label="$t('deviceStatus.lastUpdate')" width="180">
          <template #default="{ row }">
            {{ formatTime(row.lastUpdate) }}
          </template>
        </el-table-column>
        <el-table-column :label="$t('common.actions')" width="180" fixed="right">
          <template #default="{ row }">
            <el-button type="primary" link size="small" @click="showDeviceDetails(row)">
              {{ $t('common.details') }}
            </el-button>
            <el-button type="primary" link size="small" @click="refreshDevice(row)">
              {{ $t('common.refresh') }}
            </el-button>
          </template>
        </el-table-column>
      </el-table>

      <!-- 分页 -->
      <div class="pagination-container">
        <el-pagination
          v-model:current-page="currentPage"
          v-model:page-size="pageSize"
          :page-sizes="[20, 50, 100, 200]"
          :total="totalDevices"
          layout="total, sizes, prev, pager, next, jumper"
          @size-change="handleSizeChange"
          @current-change="handlePageChange"
        />
      </div>
    </el-card>

    <!-- 设备详情对话框 -->
    <el-dialog 
      v-model="detailsDialog.visible" 
      :title="$t('deviceStatus.deviceDetails')"
      width="800px"
    >
      <div v-if="detailsDialog.device" class="device-details">
        <el-descriptions :column="2" border>
          <el-descriptions-item :label="$t('deviceStatus.deviceId')">
            {{ detailsDialog.device.id }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('deviceStatus.deviceName')">
            {{ detailsDialog.device.name }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('deviceStatus.deviceType')">
            {{ detailsDialog.device.type }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('deviceStatus.channel')">
            Channel {{ detailsDialog.device.channel }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('deviceStatus.status')">
            <el-tag :type="getStatusType(detailsDialog.device.status)">
              {{ getStatusLabel(detailsDialog.device.status) }}
            </el-tag>
          </el-descriptions-item>
          <el-descriptions-item :label="$t('deviceStatus.quality')">
            <el-progress 
              :percentage="detailsDialog.device.quality" 
              :color="getQualityColor(detailsDialog.device.quality)"
            />
          </el-descriptions-item>
          <el-descriptions-item :label="$t('deviceStatus.ipAddress')">
            {{ detailsDialog.device.ipAddress }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('deviceStatus.port')">
            {{ detailsDialog.device.port }}
          </el-descriptions-item>
          <el-descriptions-item :label="$t('deviceStatus.lastUpdate')" :span="2">
            {{ formatTime(detailsDialog.device.lastUpdate) }}
          </el-descriptions-item>
        </el-descriptions>

        <!-- 通信统计 -->
        <div class="comm-stats">
          <h4>{{ $t('deviceStatus.commStats') }}</h4>
          <el-row :gutter="20">
            <el-col :span="6">
              <div class="stat-box">
                <div class="stat-number">{{ detailsDialog.device.totalMessages }}</div>
                <div class="stat-title">{{ $t('deviceStatus.totalMessages') }}</div>
              </div>
            </el-col>
            <el-col :span="6">
              <div class="stat-box">
                <div class="stat-number success">{{ detailsDialog.device.successMessages }}</div>
                <div class="stat-title">{{ $t('deviceStatus.successMessages') }}</div>
              </div>
            </el-col>
            <el-col :span="6">
              <div class="stat-box">
                <div class="stat-number error">{{ detailsDialog.device.errorMessages }}</div>
                <div class="stat-title">{{ $t('deviceStatus.errorMessages') }}</div>
              </div>
            </el-col>
            <el-col :span="6">
              <div class="stat-box">
                <div class="stat-number">{{ detailsDialog.device.avgResponseTime }}ms</div>
                <div class="stat-title">{{ $t('deviceStatus.avgResponseTime') }}</div>
              </div>
            </el-col>
          </el-row>
        </div>
      </div>
      <template #footer>
        <el-button @click="detailsDialog.visible = false">{{ $t('common.close') }}</el-button>
        <el-button type="primary" @click="refreshDevice(detailsDialog.device)">
          {{ $t('common.refresh') }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { 
  Refresh, Monitor, CircleCheck, CircleClose, 
  Warning, Search, Cpu, Connection, Timer, Odometer
} from '@element-plus/icons-vue'
import dayjs from 'dayjs'

const { t } = useI18n()

// 视图模式
const viewMode = ref('card')

// 统计数据
const stats = ref({
  total: 0,
  online: 0,
  offline: 0,
  warning: 0
})

// 筛选表单
const filterForm = ref({
  type: '',
  status: '',
  channel: '',
  keyword: ''
})

// 分页
const currentPage = ref(1)
const pageSize = ref(20)
const totalDevices = ref(0)

// 加载状态
const loading = ref(false)

// 设备列表
const devices = ref([])

// 设备详情对话框
const detailsDialog = ref({
  visible: false,
  device: null
})

// 自动刷新定时器
let refreshTimer = null

// 计算过滤后的设备列表
const filteredDevices = computed(() => {
  let result = devices.value

  // 类型过滤
  if (filterForm.value.type) {
    result = result.filter(d => d.type === filterForm.value.type)
  }

  // 状态过滤
  if (filterForm.value.status) {
    result = result.filter(d => d.status === filterForm.value.status)
  }

  // 通道过滤
  if (filterForm.value.channel) {
    result = result.filter(d => d.channel === filterForm.value.channel)
  }

  // 关键字搜索
  if (filterForm.value.keyword) {
    const keyword = filterForm.value.keyword.toLowerCase()
    result = result.filter(d => 
      d.name.toLowerCase().includes(keyword) ||
      d.id.toLowerCase().includes(keyword)
    )
  }

  // 分页
  const start = (currentPage.value - 1) * pageSize.value
  const end = start + pageSize.value
  return result.slice(start, end)
})

// 获取设备图标
const getDeviceIcon = (type) => {
  const iconMap = {
    plc: Cpu,
    sensor: Timer,
    meter: Odometer,
    actuator: Connection
  }
  return iconMap[type] || Monitor
}

// 获取状态类名
const getStatusClass = (status) => {
  return `status-${status}`
}

// 获取状态类型
const getStatusType = (status) => {
  const typeMap = {
    online: 'success',
    offline: 'danger',
    warning: 'warning'
  }
  return typeMap[status] || 'info'
}

// 获取状态标签
const getStatusLabel = (status) => {
  const labelMap = {
    online: t('common.online'),
    offline: t('common.offline'),
    warning: t('deviceStatus.warning')
  }
  return labelMap[status] || status
}

// 获取质量颜色
const getQualityColor = (quality) => {
  if (quality >= 90) return '#67C23A'
  if (quality >= 70) return '#E6A23C'
  return '#F56C6C'
}

// 格式化时间
const formatTime = (time) => {
  return dayjs(time).format('YYYY-MM-DD HH:mm:ss')
}

// 加载设备数据
const loadDevices = async () => {
  loading.value = true
  try {
    // 模拟API调用
    await new Promise(resolve => setTimeout(resolve, 500))
    
    // 生成模拟数据
    const types = ['plc', 'sensor', 'meter', 'actuator']
    const statuses = ['online', 'online', 'online', 'offline', 'warning']
    
    devices.value = Array.from({ length: 150 }, (_, i) => {
      const type = types[Math.floor(Math.random() * types.length)]
      const status = statuses[Math.floor(Math.random() * statuses.length)]
      const quality = status === 'online' ? 85 + Math.floor(Math.random() * 15) :
                     status === 'warning' ? 60 + Math.floor(Math.random() * 20) : 0
      
      return {
        id: `DEV${String(i + 1).padStart(5, '0')}`,
        name: `${type.toUpperCase()} Device ${i + 1}`,
        type,
        channel: Math.floor(Math.random() * 3) + 1,
        status,
        quality,
        lastUpdate: new Date(Date.now() - Math.random() * 3600000),
        ipAddress: `192.168.1.${100 + i}`,
        port: 502 + Math.floor(i / 10),
        totalMessages: Math.floor(Math.random() * 10000),
        successMessages: Math.floor(Math.random() * 9000),
        errorMessages: Math.floor(Math.random() * 1000),
        avgResponseTime: Math.floor(Math.random() * 100) + 10
      }
    })

    // 更新统计
    updateStats()
  } catch (error) {
    ElMessage.error(t('deviceStatus.loadFailed'))
  } finally {
    loading.value = false
  }
}

// 更新统计数据
const updateStats = () => {
  stats.value.total = devices.value.length
  stats.value.online = devices.value.filter(d => d.status === 'online').length
  stats.value.offline = devices.value.filter(d => d.status === 'offline').length
  stats.value.warning = devices.value.filter(d => d.status === 'warning').length
}

// 处理筛选
const handleFilter = () => {
  currentPage.value = 1
}

// 处理搜索
const handleSearch = () => {
  currentPage.value = 1
}

// 处理分页大小变化
const handleSizeChange = () => {
  currentPage.value = 1
}

// 处理页码变化
const handlePageChange = () => {
  // 页码变化时自动处理
}

// 刷新所有设备
const handleRefreshAll = async () => {
  ElMessage.info(t('deviceStatus.refreshing'))
  await loadDevices()
  ElMessage.success(t('deviceStatus.refreshSuccess'))
}

// 刷新单个设备
const refreshDevice = async (device) => {
  ElMessage.info(t('deviceStatus.refreshingDevice', { name: device.name }))
  
  // 模拟刷新
  await new Promise(resolve => setTimeout(resolve, 1000))
  
  // 更新设备状态
  device.lastUpdate = new Date()
  if (device.status === 'offline') {
    device.status = Math.random() > 0.7 ? 'online' : 'offline'
  }
  if (device.status === 'online') {
    device.quality = 85 + Math.floor(Math.random() * 15)
  }
  
  updateStats()
  ElMessage.success(t('deviceStatus.refreshDeviceSuccess'))
}

// 显示设备详情
const showDeviceDetails = (device) => {
  detailsDialog.value.device = device
  detailsDialog.value.visible = true
}

// 启动自动刷新
const startAutoRefresh = () => {
  refreshTimer = setInterval(() => {
    // 随机更新一些设备状态
    devices.value.forEach(device => {
      if (Math.random() < 0.1) {
        device.lastUpdate = new Date()
        if (device.status === 'online' && Math.random() < 0.05) {
          device.status = 'warning'
        } else if (device.status === 'warning' && Math.random() < 0.3) {
          device.status = 'online'
        }
        device.quality = device.status === 'online' ? 85 + Math.floor(Math.random() * 15) :
                        device.status === 'warning' ? 60 + Math.floor(Math.random() * 20) : 0
      }
    })
    updateStats()
  }, 5000)
}

onMounted(() => {
  loadDevices()
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

.device-status-container {
  min-height: 100%;
}

// Apple 风格页面头部
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--space-8);
  
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
    align-items: center;
    
    .el-radio-group {
      :deep(.el-radio-button__inner) {
        padding: var(--space-2) var(--space-4);
        border: none;
        background: var(--color-background);
        color: var(--color-text-secondary);
        font-weight: var(--font-weight-medium);
        border-radius: var(--radius-lg);
        transition: all var(--duration-fast) var(--ease-in-out);
        
        &:hover {
          background: var(--color-gray-200);
          color: var(--color-text-primary);
        }
      }
      
      :deep(.el-radio-button__original-radio:checked + .el-radio-button__inner) {
        background: var(--color-primary);
        color: var(--color-text-inverse);
        box-shadow: var(--shadow-sm);
      }
      
      :deep(.el-radio-button:first-child .el-radio-button__inner) {
        border-radius: var(--radius-lg) 0 0 var(--radius-lg);
      }
      
      :deep(.el-radio-button:last-child .el-radio-button__inner) {
        border-radius: 0 var(--radius-lg) var(--radius-lg) 0;
      }
    }
    
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
    }
  }
}

// Tesla 风格统计卡片
.stats-cards {
  margin-bottom: var(--space-8);
  
  .stat-card {
    height: 120px;
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
      height: 3px;
      background: linear-gradient(90deg, var(--color-primary), var(--color-primary-hover));
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
    
    :deep(.el-card__body) {
      height: 100%;
      padding: var(--space-5);
    }
    
    .stat-content {
      display: flex;
      justify-content: space-between;
      align-items: center;
      height: 100%;
      
      .stat-info {
        flex: 1;
        
        .stat-value {
          font-size: var(--font-size-4xl);
          font-weight: var(--font-weight-bold);
          margin-bottom: var(--space-1);
          line-height: 1;
          letter-spacing: -0.02em;
          
          &.online {
            color: var(--color-success);
          }
          
          &.offline {
            color: var(--color-danger);
          }
          
          &.warning {
            color: var(--color-warning);
          }
        }
        
        .stat-label {
          font-size: var(--font-size-sm);
          color: var(--color-text-tertiary);
          font-weight: var(--font-weight-medium);
        }
      }
      
      .stat-icon {
        opacity: 0.2;
        transition: opacity var(--duration-fast) var(--ease-in-out);
      }
    }
    
    &:hover .stat-icon {
      opacity: 0.3;
    }
  }
}

// 筛选卡片
.filter-card {
  margin-bottom: var(--space-6);
  background: var(--color-background-elevated);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-xs);
  
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

// 设备列表卡片
.device-list-card {
  background: var(--color-background-elevated);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-xs);
  
  :deep(.el-card__body) {
    padding: var(--space-6);
  }
  
  .device-cards {
    .device-card {
      border: 1px solid var(--color-border-light);
      border-radius: var(--radius-xl);
      padding: var(--space-5);
      margin-bottom: var(--space-5);
      background: var(--color-background);
      transition: all var(--duration-normal) var(--ease-in-out);
      position: relative;
      overflow: hidden;
      
      &::after {
        content: '';
        position: absolute;
        top: 0;
        left: 0;
        bottom: 0;
        width: 4px;
        transition: opacity var(--duration-fast) var(--ease-in-out);
      }
      
      &:hover {
        box-shadow: var(--shadow-lg);
        transform: translateY(-4px);
        border-color: transparent;
      }
      
      &.status-online::after {
        background: var(--color-success);
      }
      
      &.status-offline::after {
        background: var(--color-danger);
      }
      
      &.status-warning::after {
        background: var(--color-warning);
      }
      
      .device-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: var(--space-4);
        
        .device-icon {
          color: var(--color-primary);
        }
        
        .el-tag {
          height: 24px;
          line-height: 22px;
          padding: 0 var(--space-3);
          border-radius: var(--radius-full);
          font-weight: var(--font-weight-medium);
          font-size: var(--font-size-xs);
        }
      }
      
      .device-info {
        margin-bottom: var(--space-5);
        
        h3 {
          margin: 0 0 var(--space-2) 0;
          font-size: var(--font-size-lg);
          font-weight: var(--font-weight-semibold);
          color: var(--color-text-primary);
        }
        
        p {
          margin: 0;
          font-size: var(--font-size-sm);
          color: var(--color-text-tertiary);
          line-height: 1.5;
          
          &.device-id {
            font-family: var(--font-family-mono);
          }
        }
      }
      
      .device-stats {
        margin-bottom: var(--space-5);
        
        .stat-item {
          margin-bottom: var(--space-3);
          
          .label {
            font-size: var(--font-size-sm);
            color: var(--color-text-secondary);
            margin-right: var(--space-2);
            font-weight: var(--font-weight-medium);
          }
          
          .value {
            font-size: var(--font-size-sm);
            color: var(--color-text-tertiary);
          }
          
          :deep(.el-progress) {
            margin-top: var(--space-2);
            
            .el-progress__text {
              font-size: var(--font-size-xs);
              font-weight: var(--font-weight-medium);
            }
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
          
          &--primary {
            background: var(--color-primary);
            border-color: var(--color-primary);
            
            &:hover {
              background: var(--color-primary-hover);
              border-color: var(--color-primary-hover);
            }
          }
        }
      }
    }
  }
  
  // 表格视图样式
  :deep(.el-table) {
    font-size: var(--font-size-sm);
    
    .el-table__header {
      th {
        background: var(--color-background);
        color: var(--color-text-secondary);
        font-weight: var(--font-weight-semibold);
        text-transform: uppercase;
        font-size: var(--font-size-xs);
        letter-spacing: 0.05em;
      }
    }
    
    .el-table__row {
      &:hover {
        background: var(--color-background);
      }
    }
    
    .el-button--small {
      padding: var(--space-1) var(--space-3);
      height: 28px;
      border-radius: var(--radius-md);
    }
  }
  
  .pagination-container {
    margin-top: var(--space-6);
    display: flex;
    justify-content: flex-end;
    
    :deep(.el-pagination) {
      .el-pager li,
      .btn-prev,
      .btn-next {
        background: var(--color-background);
        border: 1px solid var(--color-border-light);
        border-radius: var(--radius-md);
        font-weight: var(--font-weight-medium);
        
        &:hover {
          color: var(--color-primary);
          border-color: var(--color-primary);
        }
        
        &.active {
          background: var(--color-primary);
          border-color: var(--color-primary);
          color: var(--color-text-inverse);
        }
      }
    }
  }
}

// 设备详情对话框
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

.device-details {
  .comm-stats {
    margin-top: var(--space-6);
    
    h4 {
      margin: 0 0 var(--space-4) 0;
      font-size: var(--font-size-lg);
      font-weight: var(--font-weight-semibold);
      color: var(--color-text-primary);
    }
    
    .stat-box {
      text-align: center;
      padding: var(--space-5);
      background: var(--color-background);
      border-radius: var(--radius-lg);
      border: 1px solid var(--color-border-light);
      transition: all var(--duration-fast) var(--ease-in-out);
      
      &:hover {
        transform: translateY(-2px);
        box-shadow: var(--shadow-sm);
      }
      
      .stat-number {
        font-size: var(--font-size-3xl);
        font-weight: var(--font-weight-bold);
        margin-bottom: var(--space-2);
        letter-spacing: -0.02em;
        
        &.success {
          color: var(--color-success);
        }
        
        &.error {
          color: var(--color-danger);
        }
      }
      
      .stat-title {
        font-size: var(--font-size-sm);
        color: var(--color-text-tertiary);
        font-weight: var(--font-weight-medium);
      }
    }
  }
}

// 响应式布局
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
      justify-content: space-between;
    }
  }
  
  .filter-card {
    .el-form {
      :deep(.el-form-item) {
        display: block;
        margin-bottom: var(--space-4);
        margin-right: 0;
      }
    }
  }
  
  .stats-cards {
    .el-col {
      margin-bottom: var(--space-4);
    }
  }
}
</style>