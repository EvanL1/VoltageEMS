<template>
  <div class="grafana-embed-container">
    <!-- 控制栏 -->
    <div v-if="showControls" class="grafana-controls">
      <div class="controls-left">
        <el-select 
          v-if="showDashboardSelector" 
          v-model="selectedDashboard" 
          :placeholder="t('selectDashboard')"
          @change="handleDashboardChange"
        >
          <el-option
            v-for="dashboard in availableDashboards"
            :key="dashboard.uid"
            :label="dashboard.name[locale]"
            :value="dashboard.uid"
          />
        </el-select>
        
        <el-button-group v-if="showTimeRangeSelector">
          <el-button
            v-for="range in timeRanges"
            :key="range.value"
            :type="currentTimeRange === range.value ? 'primary' : ''"
            @click="setTimeRange(range.value)"
          >
            {{ range.label[locale] }}
          </el-button>
        </el-button-group>
      </div>
      
      <div class="controls-right">
        <el-button 
          v-if="showRefreshButton"
          :icon="Refresh" 
          circle 
          @click="refreshDashboard"
          :loading="isRefreshing"
        />
        <el-button 
          v-if="showFullscreenButton"
          :icon="isFullscreen ? Close : FullScreen" 
          circle 
          @click="toggleFullscreen"
        />
      </div>
    </div>
    
    <!-- 加载状态 -->
    <div v-if="loading" class="loading-overlay">
      <el-icon class="is-loading" :size="40"><Loading /></el-icon>
      <p>{{ grafanaService.getMessage('loading', locale) }}</p>
    </div>
    
    <!-- 错误状态 -->
    <div v-else-if="error" class="error-overlay">
      <el-icon :size="40" color="#f56c6c"><WarningFilled /></el-icon>
      <p>{{ error }}</p>
      <el-button type="primary" @click="retry" :loading="isRetrying">
        {{ grafanaService.getMessage('retry', locale) }}
      </el-button>
    </div>
    
    <!-- Grafana iframe -->
    <div 
      v-show="!loading && !error" 
      class="iframe-wrapper" 
      :class="{ 'fullscreen': isFullscreen }"
    >
      <iframe
        ref="grafanaIframe"
        :key="iframeKey"
        :src="embedUrl"
        :style="{ height: computedHeight }"
        class="grafana-iframe"
        frameborder="0"
        allowfullscreen
        @load="handleIframeLoad"
        @error="handleIframeError"
      ></iframe>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { 
  Loading, 
  Refresh, 
  FullScreen, 
  Close, 
  WarningFilled 
} from '@element-plus/icons-vue'
import grafanaService from '@/services/UnifiedGrafanaService'
import grafanaConfig from '@/config/grafana'

// Props
// eslint-disable-next-line no-undef
const props = defineProps({
  // 仪表板配置
  dashboardUid: {
    type: String,
    default: ''
  },
  height: {
    type: String,
    default: '600px'
  },
  
  // 时间范围
  timeRange: {
    type: Object,
    default: null
  },
  
  // 变量
  variables: {
    type: Object,
    default: () => ({})
  },
  
  // UI 控制
  showControls: {
    type: Boolean,
    default: true
  },
  showDashboardSelector: {
    type: Boolean,
    default: false
  },
  showTimeRangeSelector: {
    type: Boolean,
    default: true
  },
  showRefreshButton: {
    type: Boolean,
    default: true
  },
  showFullscreenButton: {
    type: Boolean,
    default: true
  },
  
  // 其他选项
  theme: {
    type: String,
    default: 'light'
  },
  refresh: {
    type: String,
    default: ''
  },
  autoRetry: {
    type: Boolean,
    default: true
  }
})

// 响应式状态
const loading = ref(true)
const error = ref('')
const isFullscreen = ref(false)
const isRefreshing = ref(false)
const isRetrying = ref(false)
const iframeKey = ref(0)
const grafanaIframe = ref(null)
const selectedDashboard = ref(props.dashboardUid)
const currentTimeRange = ref('6h')

// 国际化
const { locale } = useI18n()
const t = (key) => grafanaService.getMessage(key, locale.value)

// 可用的仪表板
const availableDashboards = computed(() => {
  return Object.values(grafanaConfig.dashboards)
})

// 时间范围选项
const timeRanges = [
  { value: '5m', label: { zh: '最近5分钟', en: 'Last 5 minutes' } },
  { value: '15m', label: { zh: '最近15分钟', en: 'Last 15 minutes' } },
  { value: '1h', label: { zh: '最近1小时', en: 'Last 1 hour' } },
  { value: '6h', label: { zh: '最近6小时', en: 'Last 6 hours' } },
  { value: '24h', label: { zh: '最近24小时', en: 'Last 24 hours' } },
  { value: '7d', label: { zh: '最近7天', en: 'Last 7 days' } }
]

// 计算高度
const computedHeight = computed(() => {
  return isFullscreen.value ? '100vh' : props.height
})

// 构建嵌入 URL
const embedUrl = computed(() => {
  if (!selectedDashboard.value) return ''
  
  const options = {
    theme: props.theme,
    refresh: props.refresh || getDefaultRefresh(),
    vars: props.variables,
    timeRange: props.timeRange || {
      from: `now-${currentTimeRange.value}`,
      to: 'now'
    }
  }
  
  const url = grafanaService.buildEmbedUrl(selectedDashboard.value, options)
  
  // 添加 auth_token 参数以支持认证（如果有 API Key）
  const apiKey = grafanaService.getApiKey()
  if (apiKey) {
    const separator = url.includes('?') ? '&' : '?'
    return `${url}${separator}auth_token=${apiKey}`
  }
  
  return url
})

// 获取默认刷新间隔
function getDefaultRefresh() {
  const dashboard = Object.values(grafanaConfig.dashboards)
    .find(d => d.uid === selectedDashboard.value)
  return dashboard?.refreshInterval || '10s'
}

// 处理仪表板切换
function handleDashboardChange(uid) {
  selectedDashboard.value = uid
  refreshDashboard()
}

// 设置时间范围
function setTimeRange(range) {
  currentTimeRange.value = range
  refreshDashboard()
}

// 刷新仪表板
async function refreshDashboard() {
  isRefreshing.value = true
  iframeKey.value++
  
  setTimeout(() => {
    isRefreshing.value = false
  }, 1000)
}

// 切换全屏
function toggleFullscreen() {
  isFullscreen.value = !isFullscreen.value
  
  if (isFullscreen.value) {
    document.body.style.overflow = 'hidden'
  } else {
    document.body.style.overflow = ''
  }
}

// 处理 iframe 加载完成
function handleIframeLoad() {
  loading.value = false
  error.value = ''
  
  // 尝试与 iframe 通信
  try {
    const iframeWindow = grafanaIframe.value?.contentWindow
    if (iframeWindow) {
      // 发送认证信息
      const apiKey = grafanaService.getApiKey()
      if (apiKey) {
        iframeWindow.postMessage({
          type: 'grafana-auth',
          apiKey: apiKey
        }, '*')
      }
    }
  } catch (e) {
    console.warn('Cannot communicate with iframe:', e)
  }
}

// 处理 iframe 错误
function handleIframeError(event) {
  console.error('Iframe error:', event)
  error.value = grafanaService.getMessage('error', locale.value)
  loading.value = false
  
  if (props.autoRetry) {
    retry()
  }
}

// 重试加载
async function retry() {
  isRetrying.value = true
  error.value = ''
  
  try {
    // 检查认证
    if (!grafanaService.isAuthenticated()) {
      await authenticate()
    }
    
    // 刷新 iframe
    await refreshDashboard()
    
  } catch (err) {
    error.value = err.message || grafanaService.getMessage('error', locale.value)
  } finally {
    isRetrying.value = false
  }
}

// 认证
async function authenticate() {
  try {
    // 这里应该调用实际的认证逻辑
    // 暂时使用默认凭据
    const result = await grafanaService.authenticate('admin', 'admin')
    
    if (!result.success) {
      throw new Error(result.error || 'Authentication failed')
    }
    
  } catch (err) {
    console.error('Authentication error:', err)
    throw err
  }
}

// 监听认证失败事件
function handleAuthFailed() {
  error.value = grafanaService.getMessage('authError', locale.value)
  retry()
}

// 监听属性变化
watch([
  () => props.dashboardUid,
  () => props.timeRange,
  () => props.variables,
  () => props.theme
], () => {
  if (!loading.value) {
    refreshDashboard()
  }
})

// 生命周期
onMounted(async () => {
  // 监听认证失败事件
  window.addEventListener('grafana-auth-failed', handleAuthFailed)
  
  // 初始化
  try {
    if (!selectedDashboard.value && availableDashboards.value.length > 0) {
      selectedDashboard.value = availableDashboards.value[0].uid
    }
    
    // 检查认证
    if (!grafanaService.isAuthenticated()) {
      await authenticate()
    }
    
    loading.value = false
    
  } catch (err) {
    error.value = err.message || grafanaService.getMessage('error', locale.value)
    loading.value = false
  }
})

onUnmounted(() => {
  // 清理
  window.removeEventListener('grafana-auth-failed', handleAuthFailed)
  
  if (isFullscreen.value) {
    document.body.style.overflow = ''
  }
})
</script>

<style scoped>
.grafana-embed-container {
  width: 100%;
  position: relative;
  background-color: #f5f7fa;
  border-radius: 4px;
  overflow: hidden;
}

/* 控制栏 */
.grafana-controls {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  background-color: #fff;
  border-bottom: 1px solid #e4e7ed;
}

.controls-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.controls-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* 加载状态 */
.loading-overlay,
.error-overlay {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 400px;
  background-color: var(--color-background-elevated);
}

.loading-overlay p,
.error-overlay p {
  margin-top: 16px;
  color: #606266;
  font-size: 14px;
}

.error-overlay p {
  color: #f56c6c;
  margin-bottom: 16px;
}

/* iframe 容器 */
.iframe-wrapper {
  position: relative;
  transition: all 0.3s;
}

.iframe-wrapper.fullscreen {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  z-index: 2000;
  background-color: #000;
}

.grafana-iframe {
  width: 100%;
  border: none;
  display: block;
  background-color: #fff;
}

/* 响应式 */
@media (max-width: 768px) {
  .grafana-controls {
    flex-direction: column;
    gap: 12px;
    align-items: stretch;
  }
  
  .controls-left,
  .controls-right {
    width: 100%;
    justify-content: space-between;
  }
  
  .el-button-group {
    display: flex;
    flex-wrap: wrap;
  }
}

/* 动画 */
.el-icon.is-loading {
  animation: rotating 2s linear infinite;
}

@keyframes rotating {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}
</style>