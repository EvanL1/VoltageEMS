<template>
  <div class="grafana-realtime">
    <el-row :gutter="20">
      <!-- 左侧：实时数据 -->
      <el-col :span="8">
        <el-card>
          <template #header>
            <span>实时数据流</span>
          </template>
          
          <div class="data-stream">
            <div v-for="(data, index) in realtimeData" :key="index" class="data-item">
              <el-tag :type="getTagType(data.device)">{{ data.device }}</el-tag>
              <span class="metric">{{ data.metric }}:</span>
              <span class="value">{{ data.value }} {{ data.unit }}</span>
              <span class="time">{{ data.time }}</span>
            </div>
          </div>
          
          <el-divider />
          
          <div class="stats">
            <h4>当前状态</h4>
            <el-row :gutter="10">
              <el-col :span="12">
                <el-statistic title="平均温度" :value="avgTemp" suffix="°C" />
              </el-col>
              <el-col :span="12">
                <el-statistic title="总功率" :value="totalPower" suffix="W" />
              </el-col>
            </el-row>
          </div>
        </el-card>
      </el-col>
      
      <!-- 右侧：Grafana 仪表板 -->
      <el-col :span="16">
        <el-card>
          <template #header>
            <div class="card-header">
              <span>Grafana 实时监控</span>
              <el-button-group>
                <el-button size="small" @click="timeRange = '5m'" :type="timeRange === '5m' ? 'primary' : ''">5分钟</el-button>
                <el-button size="small" @click="timeRange = '30m'" :type="timeRange === '30m' ? 'primary' : ''">30分钟</el-button>
                <el-button size="small" @click="timeRange = '1h'" :type="timeRange === '1h' ? 'primary' : ''">1小时</el-button>
              </el-button-group>
            </div>
          </template>
          
          <div class="grafana-container">
            <iframe
              :src="grafanaUrl"
              width="100%"
              height="600px"
              frameborder="0"
            ></iframe>
          </div>
        </el-card>
      </el-col>
    </el-row>
    
    <!-- 说明信息 -->
    <el-card style="margin-top: 20px">
      <template #header>
        <span>系统架构说明</span>
      </template>
      
      <div class="architecture">
        <el-steps :active="3" finish-status="success">
          <el-step title="数据生成" description="模拟服务器生成实时数据" />
          <el-step title="数据传输" description="通过 SimpleJSON API 传输" />
          <el-step title="Grafana 可视化" description="实时图表展示" />
          <el-step title="前端集成" description="iframe 嵌入展示" />
        </el-steps>
        
        <el-alert
          type="info"
          style="margin-top: 20px"
          :closable="false"
        >
          <template #title>
            运行状态
          </template>
          <ul style="margin: 0; padding-left: 20px">
            <li>Grafana 服务：http://localhost:3000 ✅</li>
            <li>模拟数据服务：http://localhost:3001 ✅</li>
            <li>前端开发服务：http://localhost:8080 ✅</li>
            <li>数据更新频率：每分钟一个数据点</li>
            <li>图表刷新间隔：5秒</li>
          </ul>
        </el-alert>
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'

const realtimeData = ref([])
const timeRange = ref('30m')
const avgTemp = ref(0)
const totalPower = ref(0)

const grafanaUrl = computed(() => {
  return `http://localhost:3000/d/voltage-realtime?orgId=1&from=now-${timeRange.value}&to=now&refresh=5s&kiosk=tv`
})

const getTagType = (device) => {
  const types = {
    'device_001': 'success',
    'device_002': 'warning',
    'device_003': 'info'
  }
  return types[device] || 'info'
}

// 模拟实时数据流
let dataInterval
const generateRealtimeData = () => {
  const devices = ['device_001', 'device_002', 'device_003']
  const metrics = [
    { name: 'temperature', unit: '°C', min: 25, max: 35 },
    { name: 'voltage', unit: 'V', min: 210, max: 230 },
    { name: 'current', unit: 'A', min: 10, max: 15 },
    { name: 'power', unit: 'W', min: 2000, max: 3000 }
  ]
  
  const device = devices[Math.floor(Math.random() * devices.length)]
  const metric = metrics[Math.floor(Math.random() * metrics.length)]
  const value = (metric.min + Math.random() * (metric.max - metric.min)).toFixed(2)
  
  const newData = {
    device,
    metric: metric.name,
    value: parseFloat(value),
    unit: metric.unit,
    time: new Date().toLocaleTimeString('zh-CN')
  }
  
  realtimeData.value.unshift(newData)
  if (realtimeData.value.length > 10) {
    realtimeData.value.pop()
  }
  
  // 更新统计数据
  updateStats()
}

const updateStats = () => {
  const tempData = realtimeData.value.filter(d => d.metric === 'temperature')
  if (tempData.length > 0) {
    avgTemp.value = (tempData.reduce((sum, d) => sum + d.value, 0) / tempData.length).toFixed(1)
  }
  
  const powerData = realtimeData.value.filter(d => d.metric === 'power')
  if (powerData.length > 0) {
    totalPower.value = Math.round(powerData.reduce((sum, d) => sum + d.value, 0))
  }
}

onMounted(() => {
  // 生成初始数据
  for (let i = 0; i < 5; i++) {
    generateRealtimeData()
  }
  
  // 定时更新数据
  dataInterval = setInterval(generateRealtimeData, 2000)
})

onUnmounted(() => {
  clearInterval(dataInterval)
})
</script>

<style scoped>
.grafana-realtime {
  padding: 20px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.data-stream {
  max-height: 400px;
  overflow-y: auto;
}

.data-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px;
  border-bottom: 1px solid #ebeef5;
  font-size: 14px;
}

.data-item:last-child {
  border-bottom: none;
}

.metric {
  color: #606266;
  min-width: 80px;
}

.value {
  font-weight: bold;
  color: #303133;
  min-width: 80px;
}

.time {
  color: #909399;
  font-size: 12px;
  margin-left: auto;
}

.stats {
  margin-top: 20px;
}

.stats h4 {
  margin: 0 0 16px 0;
  color: #303133;
}

.grafana-container {
  background-color: #f5f7fa;
  border-radius: 4px;
  overflow: hidden;
}

.architecture ul {
  line-height: 1.8;
  color: #606266;
}

/* 自定义滚动条 */
.data-stream::-webkit-scrollbar {
  width: 6px;
}

.data-stream::-webkit-scrollbar-thumb {
  background-color: #dcdfe6;
  border-radius: 3px;
}

.data-stream::-webkit-scrollbar-thumb:hover {
  background-color: #c0c4cc;
}
</style>