<template>
  <div class="grafana-embedded">
    <!-- 顶部工具栏 -->
    <div class="toolbar">
      <h2>{{ $t('grafana.title') }}</h2>
      <div class="controls">
        <el-select v-model="currentDashboard" @change="switchDashboard" :placeholder="$t('grafana.selectDashboard')">
          <el-option
            v-for="dash in dashboards"
            :key="dash.uid"
            :label="dash.title"
            :value="dash.uid"
          />
        </el-select>
        
        <el-radio-group v-model="timeRange" @change="updateTimeRange">
          <el-radio-button label="now-5m">{{ $t('grafana.timeRange.5min') }}</el-radio-button>
          <el-radio-button label="now-15m">{{ $t('grafana.timeRange.15min') }}</el-radio-button>
          <el-radio-button label="now-30m">{{ $t('grafana.timeRange.30min') }}</el-radio-button>
          <el-radio-button label="now-1h">{{ $t('grafana.timeRange.1hour') }}</el-radio-button>
        </el-radio-group>
        
        <el-button @click="refreshDashboard" :icon="Refresh">{{ $t('common.refresh') }}</el-button>
      </div>
    </div>

    <!-- Grafana 嵌入区域 -->
    <div class="grafana-container">
      <iframe
        ref="grafanaFrame"
        :src="grafanaUrl"
        width="100%"
        height="100%"
        frameborder="0"
        allowfullscreen
        sandbox="allow-same-origin allow-scripts allow-popups allow-forms allow-top-navigation"
        loading="lazy"
        allow="fullscreen"
        referrerpolicy="no-referrer-when-downgrade"
      ></iframe>
    </div>

    <!-- 底部状态栏 -->
    <div class="status-bar">
      <span class="status-item">
        <i class="el-icon-success status-icon"></i>
        {{ $t('grafana.status.dataSource') }}: {{ $t('grafana.status.connected') }}
      </span>
      <span class="status-item">
        <i class="el-icon-time"></i>
        {{ $t('grafana.status.autoRefresh') }}: 5{{ $t('grafana.status.seconds') }}
      </span>
      <span class="status-item">
        <i class="el-icon-data-line"></i>
        {{ $t('grafana.status.lastUpdate') }}: {{ lastUpdate }}
      </span>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { Refresh } from '@element-plus/icons-vue'

// 可用的仪表板
const { t: $t } = useI18n()

const dashboards = computed(() => [
  { 
    uid: 'simple-view', 
    title: $t('grafana.dashboards.temperatureMonitoring'),
    panelId: '2' // 指定面板ID
  },
  { 
    uid: 'voltage-realtime', 
    title: $t('grafana.dashboards.comprehensiveMonitoring'),
    panelId: '1' // 指定面板ID
  }
])

const currentDashboard = ref('simple-view')
const timeRange = ref('now-30m')
const lastUpdate = ref(new Date().toLocaleTimeString())

// 构建 Grafana URL
const grafanaUrl = computed(() => {
  const dashboard = dashboards.value.find(d => d.uid === currentDashboard.value)
  
  const params = new URLSearchParams({
    orgId: '1',
    from: timeRange.value,
    to: 'now',
    refresh: '5s',
    theme: 'light',
    panelId: dashboard?.panelId || '1'
  })
  
  // 使用 d-solo 模式显示单个面板，完全隐藏 Grafana UI
  return `/grafana/d-solo/${currentDashboard.value}?${params.toString()}`
})

// 刷新仪表板
const refreshDashboard = () => {
  if (grafanaFrame.value) {
    grafanaFrame.value.src = grafanaUrl.value
  }
  lastUpdate.value = new Date().toLocaleTimeString()
}

// 切换仪表板
const switchDashboard = () => {
  refreshDashboard()
}

// 更新时间范围
const updateTimeRange = () => {
  refreshDashboard()
}

// 定时更新最后更新时间
let updateTimer
onMounted(() => {
  updateTimer = setInterval(() => {
    lastUpdate.value = new Date().toLocaleTimeString()
  }, 5000)
})

onUnmounted(() => {
  clearInterval(updateTimer)
})

const grafanaFrame = ref(null)
</script>

<style scoped>
.grafana-embedded {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background-color: var(--color-gray-50);
}

/* 顶部工具栏 */
.toolbar {
  background: white;
  padding: 16px 24px;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.08);
  display: flex;
  justify-content: space-between;
  align-items: center;
  z-index: 10;
}

.toolbar h2 {
  margin: 0;
  font-size: 20px;
  color: #303133;
}

.controls {
  display: flex;
  gap: 16px;
  align-items: center;
}

/* Grafana 容器 */
.grafana-container {
  flex: 1;
  background: white;
  margin: 16px;
  border-radius: 8px;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.08);
  overflow: hidden;
}

/* 底部状态栏 */
.status-bar {
  background: white;
  padding: 12px 24px;
  border-top: 1px solid #e4e7ed;
  display: flex;
  gap: 32px;
  font-size: 14px;
  color: #606266;
}

.status-item {
  display: flex;
  align-items: center;
  gap: 8px;
}

.status-icon {
  color: #67c23a;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .toolbar {
    flex-direction: column;
    gap: 16px;
  }
  
  .controls {
    flex-wrap: wrap;
    justify-content: center;
  }
  
  .status-bar {
    flex-wrap: wrap;
    gap: 16px;
  }
}
</style>