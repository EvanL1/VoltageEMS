<template>
  <div class="grafana-live">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>Grafana 实时数据展示</span>
          <el-space>
            <el-button type="primary" size="small" @click="refreshDashboard">
              <el-icon><Refresh /></el-icon>
              刷新
            </el-button>
            <el-button size="small" @click="openInGrafana">
              在 Grafana 中打开
            </el-button>
          </el-space>
        </div>
      </template>

      <el-alert
        type="success"
        :closable="false"
        show-icon
        style="margin-bottom: 20px"
      >
        <template #title>
          数据源已连接
        </template>
        <template #default>
          <p>模拟数据服务器运行在 http://localhost:3001</p>
          <p>正在展示 3 个设备的实时数据：温度、电压、电流、功率</p>
        </template>
      </el-alert>

      <!-- 设备选择和时间范围 -->
      <div class="controls">
        <el-select v-model="selectedPanel" @change="updatePanel" style="width: 300px">
          <el-option label="完整仪表板" value="full" />
          <el-option label="温度监控" value="temperature" />
          <el-option label="功率分析" value="power" />
          <el-option label="电压电流" value="voltage-current" />
        </el-select>
        
        <el-radio-group v-model="timeRange" @change="updateTimeRange">
          <el-radio-button label="5m">5分钟</el-radio-button>
          <el-radio-button label="15m">15分钟</el-radio-button>
          <el-radio-button label="1h">1小时</el-radio-button>
          <el-radio-button label="6h">6小时</el-radio-button>
        </el-radio-group>
      </div>

      <!-- Grafana 仪表板 -->
      <div class="dashboard-container">
        <iframe
          ref="grafanaFrame"
          :src="dashboardUrl"
          width="100%"
          height="800px"
          frameborder="0"
        ></iframe>
      </div>

      <!-- 数据说明 -->
      <el-collapse style="margin-top: 20px">
        <el-collapse-item title="数据说明" name="1">
          <div class="data-info">
            <h4>模拟数据说明：</h4>
            <ul>
              <li><strong>温度</strong>：25-35°C 范围内随机波动</li>
              <li><strong>电压</strong>：220V ± 20V 范围内波动</li>
              <li><strong>电流</strong>：10-15A 范围内变化</li>
              <li><strong>功率</strong>：2000-3000W，白天（8:00-18:00）功率更高</li>
            </ul>
            <h4>可用设备：</h4>
            <ul>
              <li>device_001 - 变压器 #1</li>
              <li>device_002 - 变压器 #2</li>
              <li>device_003 - 配电柜 #1</li>
            </ul>
          </div>
        </el-collapse-item>
      </el-collapse>
    </el-card>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue'
import { Refresh } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'

const grafanaFrame = ref(null)
const selectedPanel = ref('full')
const timeRange = ref('1h')

// 计算 Dashboard URL
const dashboardUrl = computed(() => {
  const baseUrl = 'http://localhost:3000'
  const params = new URLSearchParams({
    orgId: '1',
    from: `now-${timeRange.value}`,
    to: 'now',
    refresh: '5s',
    kiosk: 'tv' // 隐藏 Grafana UI
  })
  
  // 根据选择显示不同的面板
  let panelParam = ''
  if (selectedPanel.value !== 'full') {
    const panelMap = {
      'temperature': '&viewPanel=1',
      'power': '&viewPanel=2',
      'voltage-current': '&viewPanel=3'
    }
    panelParam = panelMap[selectedPanel.value] || ''
  }
  
  return `${baseUrl}/d/voltage-realtime?${params.toString()}${panelParam}`
})

const refreshDashboard = () => {
  if (grafanaFrame.value) {
    grafanaFrame.value.src = dashboardUrl.value
    ElMessage.success('仪表板已刷新')
  }
}

const openInGrafana = () => {
  window.open('http://localhost:3000/d/voltage-ems', '_blank')
}

const updatePanel = () => {
  refreshDashboard()
}

const updateTimeRange = () => {
  refreshDashboard()
}
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

.controls {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
  padding: 16px;
  background-color: #f5f7fa;
  border-radius: 4px;
}

.dashboard-container {
  border: 1px solid #dcdfe6;
  border-radius: 4px;
  overflow: hidden;
  background-color: #fff;
}

.data-info {
  color: #606266;
  line-height: 1.8;
}

.data-info h4 {
  color: #303133;
  margin: 16px 0 8px 0;
}

.data-info h4:first-child {
  margin-top: 0;
}

.data-info ul {
  margin: 0;
  padding-left: 20px;
}

.data-info li {
  margin: 4px 0;
}
</style>