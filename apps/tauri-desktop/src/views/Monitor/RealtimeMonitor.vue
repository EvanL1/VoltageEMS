<template>
  <div class="realtime-monitor">
    <!-- Top Toolbar -->
    <div class="monitor-toolbar">
      <div class="toolbar-left">
        <el-button-group>
          <el-button :type="viewMode === 'grid' ? 'primary' : ''" @click="viewMode = 'grid'">
            <el-icon><Grid /></el-icon>
            Grid View
          </el-button>
          <el-button :type="viewMode === 'table' ? 'primary' : ''" @click="viewMode = 'table'">
            <el-icon><List /></el-icon>
            Table View
          </el-button>
          <el-button :type="viewMode === 'chart' ? 'primary' : ''" @click="viewMode = 'chart'">
            <el-icon><DataLine /></el-icon>
            Chart View
          </el-button>
        </el-button-group>
      </div>
      
      <div class="toolbar-center">
        <el-input
          v-model="searchQuery"
          placeholder="Search channels..."
          :prefix-icon="Search"
          clearable
          style="width: 300px"
        />
      </div>
      
      <div class="toolbar-right">
        <el-button @click="refreshData" :loading="loading">
          <el-icon><Refresh /></el-icon>
          Refresh
        </el-button>
        <el-button type="primary" @click="showSubscriptionDialog = true">
          <el-icon><Connection /></el-icon>
          Subscriptions
        </el-button>
      </div>
    </div>
    
    <!-- Statistics Overview -->
    <el-row :gutter="20" class="statistics-row">
      <el-col :xs="12" :sm="6" :md="6" :lg="6">
        <el-card class="stat-card" shadow="hover">
          <el-statistic title="Total Channels" :value="statistics.total_channels">
            <template #prefix>
              <el-icon color="#409EFF"><Monitor /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :xs="12" :sm="6" :md="6" :lg="6">
        <el-card class="stat-card online" shadow="hover">
          <el-statistic title="Online Channels" :value="statistics.online_channels">
            <template #prefix>
              <el-icon color="#67C23A"><CircleCheck /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :xs="12" :sm="6" :md="6" :lg="6">
        <el-card class="stat-card offline" shadow="hover">
          <el-statistic title="Offline Channels" :value="statistics.offline_channels">
            <template #prefix>
              <el-icon color="#E6A23C"><CircleClose /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :xs="12" :sm="6" :md="6" :lg="6">
        <el-card class="stat-card" shadow="hover">
          <el-statistic title="Total Points" :value="statistics.total_points">
            <template #prefix>
              <el-icon color="#909399"><DataLine /></el-icon>
            </template>
            <template #suffix>
              <span v-if="statistics.total_errors > 0" class="error-count">
                ({{ statistics.total_errors }} errors)
              </span>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
    </el-row>
    
    <!-- Main Content Area -->
    <el-card class="content-card">
      <!-- Grid View -->
      <div v-if="viewMode === 'grid'" class="grid-view">
        <el-row :gutter="20">
          <el-col
            v-for="channel in filteredChannels"
            :key="channel.channel_id"
            :xs="24" :sm="12" :md="8" :lg="6"
          >
            <el-card
              class="channel-card"
              :class="'status-' + channel.status"
              shadow="hover"
              @click="selectChannel(channel)"
            >
              <template #header>
                <div class="channel-header">
                  <span class="channel-name">{{ channel.name }}</span>
                  <el-tag :type="getStatusType(channel.status)" size="small">
                    {{ channel.status }}
                  </el-tag>
                </div>
              </template>
              
              <div class="channel-info">
                <div class="info-item">
                  <span class="label">Channel ID:</span>
                  <span class="value">{{ channel.channel_id }}</span>
                </div>
                <div class="info-item">
                  <span class="label">Points:</span>
                  <span class="value">{{ channel.point_count }}</span>
                </div>
                <div class="info-item">
                  <span class="label">Errors:</span>
                  <span class="value" :class="{ error: channel.error_count > 0 }">
                    {{ channel.error_count }}
                  </span>
                </div>
                <div class="info-item">
                  <span class="label">Last Update:</span>
                  <span class="value">{{ formatTime(channel.last_update) }}</span>
                </div>
              </div>
              
              <div class="channel-actions">
                <el-button type="primary" size="small" text @click.stop="viewDetails(channel)">
                  View Details
                </el-button>
                <el-button type="warning" size="small" text @click.stop="viewAlarms(channel)">
                  Alarms ({{ channel.error_count }})
                </el-button>
              </div>
            </el-card>
          </el-col>
        </el-row>
      </div>
      
      <!-- Table View -->
      <div v-else-if="viewMode === 'table'" class="table-view">
        <el-table
          :data="filteredChannels"
          style="width: 100%"
          @row-click="selectChannel"
          :row-class-name="tableRowClassName"
          stripe
        >
          <el-table-column type="selection" width="55" />
          <el-table-column prop="channel_id" label="ID" width="80" sortable />
          <el-table-column prop="name" label="Channel Name" min-width="200" />
          <el-table-column prop="status" label="Status" width="120" sortable>
            <template #default="{ row }">
              <el-tag :type="getStatusType(row.status)">
                {{ row.status }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="point_count" label="Points" width="100" sortable />
          <el-table-column prop="error_count" label="Errors" width="100" sortable>
            <template #default="{ row }">
              <span :class="{ 'error-text': row.error_count > 0 }">
                {{ row.error_count }}
              </span>
            </template>
          </el-table-column>
          <el-table-column prop="last_update" label="Last Update" width="180">
            <template #default="{ row }">
              {{ formatTime(row.last_update) }}
            </template>
          </el-table-column>
          <el-table-column label="Actions" width="200" fixed="right">
            <template #default="{ row }">
              <el-button type="primary" size="small" text @click.stop="viewDetails(row)">
                Details
              </el-button>
              <el-button type="info" size="small" text @click.stop="viewChart(row)">
                Chart
              </el-button>
              <el-button type="danger" size="small" text @click.stop="viewAlarms(row)">
                Alarms
              </el-button>
            </template>
          </el-table-column>
        </el-table>
      </div>
      
      <!-- Chart View -->
      <div v-else-if="viewMode === 'chart'" class="chart-view">
        <div class="chart-controls">
          <el-select
            v-model="chartChannels"
            multiple
            placeholder="Select channels to display"
            style="width: 400px"
          >
            <el-option
              v-for="channel in channels"
              :key="channel.channel_id"
              :label="channel.name"
              :value="channel.channel_id"
            />
          </el-select>
          
          <el-radio-group v-model="chartTimeRange">
            <el-radio-button label="1m">1 Min</el-radio-button>
            <el-radio-button label="5m">5 Min</el-radio-button>
            <el-radio-button label="10m">10 Min</el-radio-button>
            <el-radio-button label="30m">30 Min</el-radio-button>
            <el-radio-button label="1h">1 Hour</el-radio-button>
          </el-radio-group>
        </div>
        
        <div ref="chartContainer" class="chart-container"></div>
      </div>
    </el-card>
    
    <!-- Channel Details Dialog -->
    <el-dialog
      v-model="showDetailsDialog"
      :title="`Channel ${selectedChannel?.name} - Real-time Data`"
      width="90%"
      destroy-on-close
    >
      <ChannelDetails
        v-if="selectedChannel"
        :channel-id="selectedChannel.channel_id"
        :channel-name="selectedChannel.name"
      />
    </el-dialog>
    
    <!-- Subscription Management Dialog -->
    <el-dialog
      v-model="showSubscriptionDialog"
      title="Subscription Management"
      width="600px"
    >
      <el-form>
        <el-form-item label="Active Subscriptions">
          <el-tag
            v-for="sub in activeSubscriptions"
            :key="sub"
            closable
            @close="unsubscribe(sub)"
            style="margin-right: 10px; margin-bottom: 10px"
          >
            {{ sub }}
          </el-tag>
        </el-form-item>
        
        <el-form-item label="Add Subscription">
          <el-input
            v-model="newSubscription"
            placeholder="Enter channel pattern (e.g., channel:*:status)"
            @keyup.enter="addSubscription"
          >
            <template #append>
              <el-button @click="addSubscription">Add</el-button>
            </template>
          </el-input>
        </el-form-item>
      </el-form>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { 
  Grid, 
  List, 
  DataLine, 
  Search, 
  Refresh, 
  Connection,
  Monitor,
  CircleCheck,
  CircleClose
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import { useRealtimeStore } from '@/stores/realtime'
import { wsManager } from '@/api/websocket'
import ChannelDetails from '@/components/ChannelDetails.vue'
import * as echarts from 'echarts'
import type { ChannelStatus } from '@/types/realtime'
import dayjs from 'dayjs'

const realtimeStore = useRealtimeStore()

// View mode
const viewMode = ref<'grid' | 'table' | 'chart'>('grid')

// Search
const searchQuery = ref('')

// Data
const channels = computed(() => realtimeStore.channels)
const statistics = computed(() => realtimeStore.statistics)
const loading = computed(() => realtimeStore.loading)

// Filtered channels
const filteredChannels = computed(() => {
  if (!searchQuery.value) return channels.value
  
  const query = searchQuery.value.toLowerCase()
  return channels.value.filter(channel => 
    channel.name.toLowerCase().includes(query) ||
    channel.channel_id.toString().includes(query)
  )
})

// Selected channel
const selectedChannel = ref<ChannelStatus | null>(null)
const showDetailsDialog = ref(false)

// Subscriptions
const showSubscriptionDialog = ref(false)
const activeSubscriptions = ref<string[]>(['channel:*:status', 'modsrv:outputs:*'])
const newSubscription = ref('')

// Chart
const chartContainer = ref<HTMLElement>()
const chart = ref<echarts.ECharts>()
const chartChannels = ref<number[]>([])
const chartTimeRange = ref('5m')

// Auto refresh
const refreshTimer = ref<number | null>(null)

onMounted(async () => {
  // Connect WebSocket
  try {
    await wsManager.connect()
    
    // Subscribe to default channels
    wsManager.subscribe(activeSubscriptions.value)
    
    // Set up data update callback
    wsManager.onDataUpdateCallback((channel, data) => {
      realtimeStore.updateChannelData(channel, data)
    })
  } catch (error) {
    console.error('Failed to connect WebSocket:', error)
  }
  
  // Load initial data
  await refreshData()
  
  // Set up auto refresh
  refreshTimer.value = window.setInterval(refreshData, 30000)
  
  // Initialize chart if in chart view
  if (viewMode.value === 'chart' && chartContainer.value) {
    initChart()
  }
})

onUnmounted(() => {
  if (refreshTimer.value) {
    clearInterval(refreshTimer.value)
  }
  
  if (chart.value) {
    chart.value.dispose()
  }
  
  wsManager.disconnect()
})

// Watch view mode changes
watch(viewMode, (newMode) => {
  if (newMode === 'chart') {
    // Wait for DOM update
    setTimeout(() => {
      if (chartContainer.value && !chart.value) {
        initChart()
      }
    }, 100)
  }
})

// Methods
async function refreshData() {
  await realtimeStore.fetchChannels()
  await realtimeStore.fetchStatistics()
}

function selectChannel(channel: ChannelStatus) {
  selectedChannel.value = channel
  showDetailsDialog.value = true
}

function viewDetails(channel: ChannelStatus) {
  selectChannel(channel)
}

function viewChart(channel: ChannelStatus) {
  viewMode.value = 'chart'
  chartChannels.value = [channel.channel_id]
}

function viewAlarms(channel: ChannelStatus) {
  // TODO: Navigate to alarm view with channel filter
  ElMessage.info(`View alarms for ${channel.name}`)
}

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

function tableRowClassName({ row }: { row: ChannelStatus }) {
  return `row-status-${row.status}`
}

function formatTime(time?: Date) {
  if (!time) return '-'
  return dayjs(time).format('YYYY-MM-DD HH:mm:ss')
}

// Subscription management
function addSubscription() {
  if (!newSubscription.value) return
  
  if (!activeSubscriptions.value.includes(newSubscription.value)) {
    activeSubscriptions.value.push(newSubscription.value)
    wsManager.subscribe([newSubscription.value])
    ElMessage.success(`Subscribed to ${newSubscription.value}`)
  }
  
  newSubscription.value = ''
}

function unsubscribe(pattern: string) {
  const index = activeSubscriptions.value.indexOf(pattern)
  if (index > -1) {
    activeSubscriptions.value.splice(index, 1)
    wsManager.unsubscribe([pattern])
    ElMessage.success(`Unsubscribed from ${pattern}`)
  }
}

// Chart initialization
function initChart() {
  if (!chartContainer.value) return
  
  chart.value = echarts.init(chartContainer.value)
  
  const option = {
    title: {
      text: 'Channel Status Overview',
      left: 'center'
    },
    tooltip: {
      trigger: 'axis'
    },
    legend: {
      bottom: 0,
      data: []
    },
    grid: {
      left: '3%',
      right: '4%',
      bottom: '10%',
      containLabel: true
    },
    xAxis: {
      type: 'time',
      boundaryGap: false
    },
    yAxis: {
      type: 'value'
    },
    series: []
  }
  
  chart.value.setOption(option)
  
  // Handle resize
  window.addEventListener('resize', () => {
    chart.value?.resize()
  })
}
</script>

<style lang="scss" scoped>
.realtime-monitor {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 20px;
  
  .monitor-toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
    
    .toolbar-left,
    .toolbar-right {
      display: flex;
      align-items: center;
      gap: 10px;
    }
  }
  
  .statistics-row {
    .stat-card {
      transition: all 0.3s;
      
      &:hover {
        transform: translateY(-2px);
      }
      
      :deep(.el-statistic__number) {
        font-size: 28px;
      }
      
      &.online :deep(.el-statistic__number) {
        color: #67C23A;
      }
      
      &.offline :deep(.el-statistic__number) {
        color: #E6A23C;
      }
      
      .error-count {
        color: #F56C6C;
        font-size: 14px;
      }
    }
  }
  
  .content-card {
    flex: 1;
    display: flex;
    flex-direction: column;
    
    :deep(.el-card__body) {
      flex: 1;
      display: flex;
      flex-direction: column;
      padding: 20px;
    }
  }
  
  // Grid View
  .grid-view {
    .channel-card {
      margin-bottom: 20px;
      cursor: pointer;
      transition: all 0.3s;
      
      &:hover {
        transform: translateY(-2px);
      }
      
      &.status-online {
        border-left: 4px solid #67C23A;
      }
      
      &.status-offline {
        border-left: 4px solid #E6A23C;
      }
      
      &.status-error {
        border-left: 4px solid #F56C6C;
      }
      
      .channel-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        
        .channel-name {
          font-weight: 600;
          font-size: 16px;
        }
      }
      
      .channel-info {
        .info-item {
          display: flex;
          justify-content: space-between;
          margin-bottom: 8px;
          
          .label {
            color: #909399;
            font-size: 14px;
          }
          
          .value {
            font-weight: 500;
            
            &.error {
              color: #F56C6C;
            }
          }
        }
      }
      
      .channel-actions {
        margin-top: 12px;
        display: flex;
        justify-content: space-between;
      }
    }
  }
  
  // Table View
  .table-view {
    :deep(.el-table) {
      .row-status-offline {
        background-color: #fef0f0;
      }
      
      .row-status-error {
        background-color: #fef0f0;
      }
      
      .error-text {
        color: #F56C6C;
        font-weight: 600;
      }
    }
  }
  
  // Chart View
  .chart-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    
    .chart-controls {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 20px;
    }
    
    .chart-container {
      flex: 1;
      min-height: 400px;
    }
  }
}
</style>