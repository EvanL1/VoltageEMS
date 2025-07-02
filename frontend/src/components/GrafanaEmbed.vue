<template>
  <div class="grafana-container">
    <div v-if="loading" class="loading-container">
      <el-icon class="is-loading"><Loading /></el-icon>
      <p>正在加载 Grafana...</p>
    </div>
    <iframe
      v-else
      :key="iframeKey"
      :src="grafanaUrl"
      :style="{ height: height }"
      class="grafana-iframe"
      frameborder="0"
      allowfullscreen
    ></iframe>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { Loading } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import { useGrafanaAuth } from '@/composables/useGrafanaAuth'

const props = defineProps({
  dashboardUid: {
    type: String,
    required: true
  },
  height: {
    type: String,
    default: '600px'
  },
  timeRange: {
    type: Object,
    default: null
  },
  variables: {
    type: Object,
    default: () => ({})
  },
  theme: {
    type: String,
    default: 'light'
  },
  refresh: {
    type: String,
    default: '10s'
  }
})

const loading = ref(true)
const iframeKey = ref(0)
const { ensureGrafanaAuth } = useGrafanaAuth()

// 构建 Grafana URL
const grafanaUrl = computed(() => {
  const params = new URLSearchParams({
    orgId: '1',
    theme: props.theme,
    refresh: props.refresh,
    kiosk: 'tv' // 隐藏 Grafana UI
  })

  // 添加时间范围
  if (props.timeRange) {
    params.append('from', props.timeRange.from)
    params.append('to', props.timeRange.to)
  }

  // 添加变量
  Object.entries(props.variables).forEach(([key, value]) => {
    params.append(`var-${key}`, value)
  })

  return `/grafana/d/${props.dashboardUid}?${params.toString()}`
})

// 监听属性变化，刷新 iframe
watch(() => [props.timeRange, props.variables], () => {
  iframeKey.value++
})

onMounted(async () => {
  try {
    await ensureGrafanaAuth()
    loading.value = false
  } catch (error) {
    ElMessage.error('Grafana 认证失败，请刷新页面重试')
    console.error('Grafana auth error:', error)
  }
})
</script>

<style scoped>
.grafana-container {
  width: 100%;
  position: relative;
  background-color: #f0f0f0;
  border-radius: 4px;
  overflow: hidden;
}

.loading-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 400px;
  background-color: #fafafa;
}

.loading-container p {
  margin-top: 16px;
  color: #666;
}

.grafana-iframe {
  width: 100%;
  border: none;
  display: block;
}
</style>