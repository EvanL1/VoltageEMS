<template>
  <div class="channel-details">
    <!-- Point Type Tabs -->
    <el-tabs v-model="activeTab" @tab-click="handleTabClick">
      <el-tab-pane label="Measurements (YC)" name="YC">
        <PointTable :points="ycPoints" point-type="YC" />
      </el-tab-pane>
      
      <el-tab-pane label="Signals (YX)" name="YX">
        <PointTable :points="yxPoints" point-type="YX" />
      </el-tab-pane>
      
      <el-tab-pane label="Controls (YK)" name="YK">
        <PointTable :points="ykPoints" point-type="YK" />
      </el-tab-pane>
      
      <el-tab-pane label="Adjustments (YT)" name="YT">
        <PointTable :points="ytPoints" point-type="YT" />
      </el-tab-pane>
      
      <el-tab-pane label="Real-time Chart" name="chart">
        <RealtimeChart 
          :channel-id="channelId"
          :points="selectedPoints"
        />
      </el-tab-pane>
    </el-tabs>
    
    <!-- Auto-refresh toggle -->
    <div class="refresh-control">
      <el-switch 
        v-model="autoRefresh" 
        active-text="Auto Refresh"
        @change="toggleAutoRefresh"
      />
      <el-button 
        v-if="!autoRefresh" 
        type="primary" 
        size="small" 
        @click="refreshPoints"
      >
        <el-icon class="el-icon--left"><Refresh /></el-icon>
        Refresh
      </el-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { Refresh } from '@element-plus/icons-vue'
import { useRealtimeStore } from '@/stores/realtime'
import PointTable from './PointTable.vue'
import RealtimeChart from './RealtimeChart.vue'
import type { PointData } from '@/types/realtime'

const props = defineProps<{
  channelId: number
  channelName: string
}>()

const realtimeStore = useRealtimeStore()

// Local state
const activeTab = ref('YC')
const autoRefresh = ref(true)
const refreshTimer = ref<number | null>(null)
const selectedPoints = ref<PointData[]>([])

// Computed points by type
const points = computed(() => realtimeStore.getChannelPoints.value(props.channelId))

const ycPoints = computed(() => 
  points.value.filter(p => p.point_type === 'YC')
)

const yxPoints = computed(() => 
  points.value.filter(p => p.point_type === 'YX')
)

const ykPoints = computed(() => 
  points.value.filter(p => p.point_type === 'YK')
)

const ytPoints = computed(() => 
  points.value.filter(p => p.point_type === 'YT')
)

// Initialize
onMounted(async () => {
  await refreshPoints()
  
  if (autoRefresh.value) {
    startAutoRefresh()
  }
})

onUnmounted(() => {
  stopAutoRefresh()
})

// Watch for tab changes
watch(activeTab, (newTab) => {
  if (newTab === 'chart') {
    // Select some default points for chart
    selectedPoints.value = ycPoints.value.slice(0, 5)
  }
})

async function refreshPoints() {
  const typeMap: Record<string, string> = {
    'YC': 'm',
    'YX': 's',
    'YK': 'c',
    'YT': 'a'
  }
  
  await realtimeStore.fetchChannelPoints(
    props.channelId, 
    typeMap[activeTab.value] || undefined
  )
}

function handleTabClick() {
  if (activeTab.value !== 'chart') {
    refreshPoints()
  }
}

function toggleAutoRefresh(value: boolean) {
  if (value) {
    startAutoRefresh()
  } else {
    stopAutoRefresh()
  }
}

function startAutoRefresh() {
  refreshTimer.value = window.setInterval(() => {
    refreshPoints()
  }, 5000) // 5 seconds
}

function stopAutoRefresh() {
  if (refreshTimer.value) {
    clearInterval(refreshTimer.value)
    refreshTimer.value = null
  }
}
</script>

<style scoped lang="scss">
.channel-details {
  position: relative;
  min-height: 500px;
  
  .refresh-control {
    position: absolute;
    top: 10px;
    right: 10px;
    display: flex;
    align-items: center;
    gap: 10px;
  }
  
  :deep(.el-tabs__header) {
    margin-bottom: 20px;
  }
}
</style>