<template>
  <div class="grafana-live">
    <el-card>
      <template #header>
        <div class="card-header">
          <h2>{{ $t('grafana.title') }}</h2>
          <el-button 
            type="primary" 
            @click="openInGrafana"
            :icon="Link"
          >
            {{ $t('grafana.openInGrafana') || '在 Grafana 中打开' }}
          </el-button>
        </div>
      </template>

      <!-- 状态指示器 -->
      <el-row :gutter="20" class="status-row">
        <el-col :span="6">
          <div class="status-card">
            <el-icon :size="24" :color="isConnected ? '#67c23a' : '#f56c6c'">
              <Connection />
            </el-icon>
            <div class="status-info">
              <div class="status-label">{{ $t('grafana.status.dataSource') }}</div>
              <div class="status-value">
                {{ isConnected ? $t('grafana.status.connected') : $t('grafana.status.disconnected') }}
              </div>
            </div>
          </div>
        </el-col>
        <el-col :span="6">
          <div class="status-card">
            <el-icon :size="24" color="#409eff">
              <Timer />
            </el-icon>
            <div class="status-info">
              <div class="status-label">{{ $t('grafana.status.autoRefresh') }}</div>
              <div class="status-value">{{ currentRefreshInterval }}</div>
            </div>
          </div>
        </el-col>
        <el-col :span="6">
          <div class="status-card">
            <el-icon :size="24" color="#e6a23c">
              <DataAnalysis />
            </el-icon>
            <div class="status-info">
              <div class="status-label">{{ $t('devices.title') }}</div>
              <div class="status-value">3 {{ $t('common.active') || '活动' }}</div>
            </div>
          </div>
        </el-col>
        <el-col :span="6">
          <div class="status-card">
            <el-icon :size="24" color="#909399">
              <Clock />
            </el-icon>
            <div class="status-info">
              <div class="status-label">{{ $t('grafana.status.lastUpdate') }}</div>
              <div class="status-value">{{ lastUpdateTime }}</div>
            </div>
          </div>
        </el-col>
      </el-row>

      <!-- Grafana 增强嵌入组件 -->
      <GrafanaEmbedEnhanced
        :dashboard-uid="selectedDashboard"
        :height="dashboardHeight"
        :show-controls="true"
        :show-dashboard-selector="true"
        :show-time-range-selector="true"
        :variables="dashboardVariables"
        :refresh="refreshInterval"
        @dashboard-loaded="handleDashboardLoaded"
      />

      <!-- 数据信息面板 -->
      <el-collapse v-model="activeCollapse" class="info-collapse">
        <el-collapse-item :title="$t('grafana.dataInfo') || '数据信息'" name="dataInfo">
          <el-tabs v-model="activeTab">
            <el-tab-pane :label="$t('grafana.tabs.overview') || '概览'" name="overview">
              <div class="info-content">
                <h4>{{ $t('grafana.info.simulatedData') || '模拟数据说明' }}：</h4>
                <el-descriptions :column="2" border>
                  <el-descriptions-item :label="$t('history.dataTypes.temperature')">
                    25-35°C {{ $t('grafana.info.randomFluctuation') || '范围内随机波动' }}
                  </el-descriptions-item>
                  <el-descriptions-item :label="$t('history.dataTypes.voltage')">
                    220V ± 20V {{ $t('grafana.info.fluctuation') || '范围内波动' }}
                  </el-descriptions-item>
                  <el-descriptions-item :label="$t('history.dataTypes.current')">
                    10-15A {{ $t('grafana.info.variation') || '范围内变化' }}
                  </el-descriptions-item>
                  <el-descriptions-item :label="$t('history.dataTypes.power')">
                    2000-3000W，{{ $t('grafana.info.higherDaytime') || '白天功率更高' }}
                  </el-descriptions-item>
                </el-descriptions>
              </div>
            </el-tab-pane>
            
            <el-tab-pane :label="$t('devices.title')" name="devices">
              <div class="info-content">
                <el-table :data="deviceList" stripe>
                  <el-table-column prop="id" :label="$t('devices.deviceId')" width="120" />
                  <el-table-column prop="name" :label="$t('devices.deviceName')" />
                  <el-table-column prop="type" :label="$t('devices.deviceType')" />
                  <el-table-column :label="$t('devices.connectionStatus')" width="120">
                    <template #default>
                      <el-tag type="success" size="small">
                        {{ $t('devices.status.online') }}
                      </el-tag>
                    </template>
                  </el-table-column>
                </el-table>
              </div>
            </el-tab-pane>
            
            <el-tab-pane :label="$t('grafana.tabs.metrics') || '指标'" name="metrics">
              <div class="info-content">
                <el-space wrap>
                  <el-tag v-for="metric in availableMetrics" :key="metric">
                    {{ metric }}
                  </el-tag>
                </el-space>
              </div>
            </el-tab-pane>
          </el-tabs>
        </el-collapse-item>
      </el-collapse>
    </el-card>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { 
  Link, 
  Connection, 
  Timer, 
  DataAnalysis, 
  Clock 
} from '@element-plus/icons-vue'
import GrafanaEmbedEnhanced from '@/components/GrafanaEmbedEnhanced.vue'
import grafanaConfig from '@/config/grafana'

const { locale } = useI18n()

// 响应式数据
const selectedDashboard = ref('voltage-realtime')
const isConnected = ref(true)
const lastUpdateTime = ref('')
const activeCollapse = ref(['dataInfo'])
const activeTab = ref('overview')
const dashboardHeight = ref('700px')
const refreshInterval = ref('5s')

// 仪表板变量
const dashboardVariables = ref({
  device: 'all',
  metric: 'all'
})

// 设备列表
const deviceList = ref([
  { id: 'device_001', name: '变压器 #1', type: '电力变压器' },
  { id: 'device_002', name: '变压器 #2', type: '电力变压器' },
  { id: 'device_003', name: '配电柜 #1', type: '配电设备' }
])

// 可用指标
const availableMetrics = ref([
  'voltage_rms', 'current_rms', 'power_active', 
  'power_reactive', 'power_factor', 'frequency',
  'temperature', 'humidity', 'energy_total'
])

// 计算属性
const currentRefreshInterval = computed(() => {
  const intervals = {
    '5s': locale.value === 'zh' ? '5秒' : '5 seconds',
    '10s': locale.value === 'zh' ? '10秒' : '10 seconds',
    '30s': locale.value === 'zh' ? '30秒' : '30 seconds',
    '1m': locale.value === 'zh' ? '1分钟' : '1 minute',
    '5m': locale.value === 'zh' ? '5分钟' : '5 minutes'
  }
  return intervals[refreshInterval.value] || refreshInterval.value
})

// 方法
const updateLastTime = () => {
  const now = new Date()
  lastUpdateTime.value = now.toLocaleTimeString(locale.value === 'zh' ? 'zh-CN' : 'en-US')
}

const openInGrafana = () => {
  const url = `${grafanaConfig.baseUrl}/d/${selectedDashboard.value}`
  window.open(url, '_blank')
}

const handleDashboardLoaded = () => {
  updateLastTime()
}

// 定时器
let updateTimer = null

// 生命周期
onMounted(() => {
  updateLastTime()
  // 每5秒更新一次时间
  updateTimer = setInterval(updateLastTime, 5000)
})

onUnmounted(() => {
  if (updateTimer) {
    clearInterval(updateTimer)
  }
})
</script>

<style scoped>
.grafana-live {
  height: 100%;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.card-header h2 {
  margin: 0;
  font-size: 20px;
  font-weight: 500;
  color: #303133;
}

/* 状态卡片 */
.status-row {
  margin-bottom: 24px;
}

.status-card {
  display: flex;
  align-items: center;
  padding: 16px;
  background-color: #f5f7fa;
  border-radius: 8px;
  transition: all 0.3s;
}

.status-card:hover {
  background-color: #e9ebf0;
  transform: translateY(-2px);
}

.status-info {
  margin-left: 12px;
  flex: 1;
}

.status-label {
  font-size: 12px;
  color: #909399;
  margin-bottom: 4px;
}

.status-value {
  font-size: 16px;
  font-weight: 500;
  color: #303133;
}

/* 信息面板 */
.info-collapse {
  margin-top: 24px;
}

.info-content {
  padding: 16px;
}

.info-content h4 {
  margin: 0 0 16px 0;
  color: #303133;
  font-size: 16px;
}

/* 响应式 */
@media (max-width: 1200px) {
  .status-row .el-col {
    margin-bottom: 12px;
  }
  
  .status-row .el-col:nth-child(odd) {
    padding-right: 10px !important;
  }
  
  .status-row .el-col:nth-child(even) {
    padding-left: 10px !important;
  }
}

@media (max-width: 768px) {
  .card-header {
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }
  
  .status-row .el-col {
    margin-bottom: 8px;
  }
  
  .dashboardHeight {
    height: 500px;
  }
}

/* 深色模式支持 */
@media (prefers-color-scheme: dark) {
  .status-card {
    background-color: #1a1a1a;
  }
  
  .status-card:hover {
    background-color: #2a2a2a;
  }
  
  .status-value {
    color: #e4e7ed;
  }
}
</style>