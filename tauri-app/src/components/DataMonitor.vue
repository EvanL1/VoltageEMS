<template>
  <div class="data-monitor">
    <el-row :gutter="20">
      <!-- Statistics Cards -->
      <el-col :span="6">
        <el-card class="stat-card">
          <el-statistic title="Total Channels" :value="statistics.total_channels">
            <template #prefix>
              <el-icon><Monitor /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card class="stat-card online">
          <el-statistic title="Online Channels" :value="statistics.online_channels">
            <template #prefix>
              <el-icon><CircleCheck /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card class="stat-card offline">
          <el-statistic title="Offline Channels" :value="statistics.offline_channels">
            <template #prefix>
              <el-icon><CircleClose /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card class="stat-card">
          <el-statistic title="Total Points" :value="statistics.total_points">
            <template #prefix>
              <el-icon><DataLine /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
    </el-row>
    
    <!-- Connection Status -->
    <el-card class="connection-status" :class="{ connected: wsConnected }">
      <div class="status-content">
        <el-icon :size="20" :color="wsConnected ? '#67C23A' : '#F56C6C'">
          <Connection />
        </el-icon>
        <span>{{ wsConnected ? 'Connected' : 'Disconnected' }}</span>
        <el-button 
          v-if="!wsConnected" 
          type="primary" 
          size="small" 
          @click="reconnect"
        >
          Reconnect
        </el-button>
      </div>
    </el-card>
    
    <!-- Channel List -->
    <el-card class="channel-list">
      <template #header>
        <div class="card-header">
          <span>Channels</span>
          <el-button type="primary" size="small" @click="refreshChannels">
            <el-icon class="el-icon--left"><Refresh /></el-icon>
            Refresh
          </el-button>
        </div>
      </template>
      
      <el-table 
        :data="channels" 
        style="width: 100%"
        :row-class-name="tableRowClassName"
        @row-click="selectChannel"
      >
        <el-table-column prop="channel_id" label="ID" width="80" />
        <el-table-column prop="name" label="Name" />
        <el-table-column prop="status" label="Status" width="100">
          <template #default="{ row }">
            <el-tag 
              :type="getStatusType(row.status)"
              size="small"
            >
              {{ row.status }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="point_count" label="Points" width="100" />
        <el-table-column prop="error_count" label="Errors" width="100" />
        <el-table-column prop="last_update" label="Last Update" width="180">
          <template #default="{ row }">
            {{ formatTime(row.last_update) }}
          </template>
        </el-table-column>
      </el-table>
    </el-card>
    
    <!-- Selected Channel Details -->
    <el-dialog 
      v-model="showChannelDetails" 
      :title="`Channel ${selectedChannel?.name} - Real-time Data`"
      width="80%"
      destroy-on-close
    >
      <ChannelDetails 
        v-if="selectedChannel"
        :channel-id="selectedChannel.channel_id"
        :channel-name="selectedChannel.name"
      />
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { 
  Monitor, 
  CircleCheck, 
  CircleClose, 
  DataLine, 
  Connection,
  Refresh 
} from '@element-plus/icons-vue'
import { useRealtimeStore } from '@/stores/realtime'
import { wsManager } from '@/api/websocket'
import ChannelDetails from './ChannelDetails.vue'
import type { ChannelStatus } from '@/types/realtime'
import dayjs from 'dayjs'

const realtimeStore = useRealtimeStore()

// WebSocket connection status
const wsConnected = computed(() => wsManager.connected.value)

// Data from store
const channels = computed(() => realtimeStore.channels)
const statistics = computed(() => realtimeStore.statistics)

// Local state
const selectedChannel = ref<ChannelStatus | null>(null)
const showChannelDetails = ref(false)
const refreshTimer = ref<number | null>(null)

// Initialize WebSocket connection
onMounted(async () => {
  try {
    await wsManager.connect()
    
    // Subscribe to channel updates
    wsManager.subscribe(['channel:*:status', 'modsrv:outputs:*'])
    
    // Set up data update callback
    wsManager.onDataUpdateCallback((channel, data) => {
      realtimeStore.updateChannelData(channel, data)
    })
    
    // Initial data fetch
    await refreshData()
    
    // Set up auto-refresh
    refreshTimer.value = window.setInterval(refreshData, 30000) // 30 seconds
    
  } catch (error) {
    console.error('Failed to initialize WebSocket:', error)
  }
})

onUnmounted(() => {
  if (refreshTimer.value) {
    clearInterval(refreshTimer.value)
  }
  wsManager.disconnect()
})

async function refreshData() {
  await realtimeStore.fetchChannels()
  await realtimeStore.fetchStatistics()
}

async function refreshChannels() {
  await realtimeStore.fetchChannels()
}

async function reconnect() {
  try {
    await wsManager.connect()
    wsManager.subscribe(['channel:*:status', 'modsrv:outputs:*'])
  } catch (error) {
    console.error('Failed to reconnect:', error)
  }
}

function selectChannel(row: ChannelStatus) {
  selectedChannel.value = row
  showChannelDetails.value = true
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
</script>

<style scoped lang="scss">
.data-monitor {
  padding: 20px;
}

.stat-card {
  margin-bottom: 20px;
  
  &.online {
    .el-statistic__number {
      color: #67C23A;
    }
  }
  
  &.offline {
    .el-statistic__number {
      color: #E6A23C;
    }
  }
}

.connection-status {
  margin-bottom: 20px;
  
  .status-content {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  
  &.connected {
    border-color: #67C23A;
  }
}

.channel-list {
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  
  :deep(.el-table) {
    .row-status-online {
      cursor: pointer;
      
      &:hover {
        background-color: #f0f9ff;
      }
    }
    
    .row-status-offline {
      cursor: pointer;
      background-color: #fef0f0;
      
      &:hover {
        background-color: #fde2e2;
      }
    }
    
    .row-status-error {
      cursor: pointer;
      background-color: #fef0f0;
      
      &:hover {
        background-color: #fde2e2;
      }
    }
  }
}
</style>