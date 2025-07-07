<template>
  <div class="history-analysis">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>历史数据分析</span>
          <el-space>
            <el-button @click="createDashboard">创建仪表板</el-button>
            <el-button @click="exportDashboard">导出</el-button>
          </el-space>
        </div>
      </template>

      <!-- 工具栏 -->
      <div class="toolbar">
        <el-space size="large">
          <el-select v-model="selectedDevice" placeholder="选择设备" style="width: 200px">
            <el-option label="所有设备" value="all" />
            <el-option label="变压器 #1" value="device_001" />
            <el-option label="变压器 #2" value="device_002" />
            <el-option label="配电柜 #1" value="device_003" />
          </el-select>

          <el-date-picker
            v-model="timeRange"
            type="datetimerange"
            range-separator="至"
            start-placeholder="开始时间"
            end-placeholder="结束时间"
            format="YYYY-MM-DD HH:mm"
            :shortcuts="timeShortcuts"
            @change="handleTimeRangeChange"
          />
        </el-space>
      </div>

      <!-- 仪表板标签页 -->
      <el-tabs v-model="activeTab" @tab-change="handleTabChange">
        <el-tab-pane
          v-for="dashboard in dashboards"
          :key="dashboard.uid"
          :label="dashboard.title"
          :name="dashboard.uid"
        >
          <div class="dashboard-container">
            <grafana-embed
              :dashboard-uid="dashboard.uid"
              height="calc(100vh - 320px)"
              :time-range="grafanaTimeRange"
              :variables="{ device: selectedDevice }"
              theme="light"
              refresh="10s"
            />
          </div>
        </el-tab-pane>

        <!-- 自定义仪表板 -->
        <el-tab-pane v-if="customDashboards.length > 0" label="自定义仪表板" name="custom">
          <div class="custom-dashboards">
            <el-card
              v-for="dashboard in customDashboards"
              :key="dashboard.uid"
              class="dashboard-card"
            >
              <template #header>
                <span>{{ dashboard.title }}</span>
              </template>
              <grafana-embed
                :dashboard-uid="dashboard.uid"
                height="400px"
                :time-range="grafanaTimeRange"
                :variables="{ device: selectedDevice }"
              />
            </el-card>
          </div>
        </el-tab-pane>
      </el-tabs>
    </el-card>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import GrafanaEmbed from '@/components/GrafanaEmbed.vue'

const route = useRoute()
const router = useRouter()

// 预定义的仪表板
const dashboards = [
  {
    uid: 'voltage-ems-overview',
    title: '系统总览',
    description: '系统整体运行状态和关键指标'
  },
  {
    uid: 'device-analysis',
    title: '设备分析',
    description: '设备运行数据详细分析'
  },
  {
    uid: 'energy-consumption',
    title: '能耗分析',
    description: '能源消耗趋势和效率分析'
  },
  {
    uid: 'alarm-history',
    title: '告警历史',
    description: '历史告警记录和统计分析'
  }
]

const activeTab = ref(route.query.tab || 'voltage-ems-overview')
const selectedDevice = ref('all')
const timeRange = ref([
  new Date(Date.now() - 24 * 60 * 60 * 1000),
  new Date()
])
const customDashboards = ref([])

// 时间快捷选项
const timeShortcuts = [
  {
    text: '最近1小时',
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setTime(start.getTime() - 3600 * 1000)
      return [start, end]
    }
  },
  {
    text: '最近6小时',
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setTime(start.getTime() - 3600 * 1000 * 6)
      return [start, end]
    }
  },
  {
    text: '最近24小时',
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setTime(start.getTime() - 3600 * 1000 * 24)
      return [start, end]
    }
  },
  {
    text: '最近7天',
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setTime(start.getTime() - 3600 * 1000 * 24 * 7)
      return [start, end]
    }
  },
  {
    text: '最近30天',
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setTime(start.getTime() - 3600 * 1000 * 24 * 30)
      return [start, end]
    }
  }
]

// 计算 Grafana 时间范围格式
const grafanaTimeRange = computed(() => {
  if (!timeRange.value || timeRange.value.length !== 2) {
    return { from: 'now-24h', to: 'now' }
  }
  return {
    from: timeRange.value[0].toISOString(),
    to: timeRange.value[1].toISOString()
  }
})

// 处理标签页切换
const handleTabChange = (tab) => {
  router.push({ query: { ...route.query, tab } })
}

// 处理时间范围变化
const handleTimeRangeChange = () => {
  // 时间范围变化会自动触发 grafana-embed 组件更新
}

// 创建新仪表板
const createDashboard = () => {
  // 在新窗口打开 Grafana 创建页面
  window.open('/grafana/dashboard/new', '_blank')
}

// 导出仪表板
const exportDashboard = async () => {
  try {
    const dashboard = dashboards.find(d => d.uid === activeTab.value)
    if (!dashboard) {
      ElMessage.warning('请选择要导出的仪表板')
      return
    }

    // 这里应该调用实际的导出 API
    ElMessage.success('导出功能开发中...')
  } catch (error) {
    console.error('Export failed:', error)
    ElMessage.error('导出失败')
  }
}

// 加载自定义仪表板
const loadCustomDashboards = async () => {
  try {
    // 这里应该调用实际的 API 获取自定义仪表板
    // const dashboards = await grafanaService.getDashboards()
    // customDashboards.value = dashboards.filter(d => d.tags?.includes('custom'))
  } catch (error) {
    console.error('Failed to load custom dashboards:', error)
  }
}

onMounted(() => {
  loadCustomDashboards()
})
</script>

<style scoped>
.history-analysis {
  height: 100%;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.toolbar {
  margin-bottom: 16px;
  padding: 16px;
  background-color: var(--color-background-elevated);
  border-radius: 4px;
}

.dashboard-container {
  min-height: 600px;
  background-color: var(--color-gray-50);
  border-radius: 4px;
  padding: 1px;
}

.custom-dashboards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(600px, 1fr));
  gap: 16px;
}

.dashboard-card {
  border-radius: 4px;
  overflow: hidden;
}
</style>