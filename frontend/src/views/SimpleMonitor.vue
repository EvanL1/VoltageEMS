<template>
  <div class="simple-monitor">
    <h1>实时监控</h1>
    
    <!-- 实时数据卡片 -->
    <div class="data-cards">
      <div class="card" v-for="device in devices" :key="device.id">
        <h3>{{ device.name }}</h3>
        <div class="metric">
          <span class="label">温度:</span>
          <span class="value">{{ device.temperature }}°C</span>
        </div>
        <div class="metric">
          <span class="label">功率:</span>
          <span class="value">{{ device.power }}W</span>
        </div>
        <div class="metric">
          <span class="label">电压:</span>
          <span class="value">{{ device.voltage }}V</span>
        </div>
        <div class="metric">
          <span class="label">电流:</span>
          <span class="value">{{ device.current }}A</span>
        </div>
      </div>
    </div>

    <!-- 简单的图表 -->
    <div class="chart-container">
      <h2>温度趋势（最近10个数据点）</h2>
      <div class="simple-chart">
        <div class="chart-line" v-for="point in temperatureHistory" :key="point">
          <div class="bar" :style="{ height: point + '%' }"></div>
        </div>
      </div>
    </div>

    <!-- Grafana 单个面板嵌入 -->
    <div class="grafana-panel">
      <h2>Grafana 图表（仅显示一个面板）</h2>
      <iframe
        :src="grafanaPanelUrl"
        width="100%"
        height="300px"
        frameborder="0"
      ></iframe>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted } from 'vue'

// 设备数据
const devices = ref([
  { id: 1, name: '设备 1', temperature: 0, power: 0, voltage: 0, current: 0 },
  { id: 2, name: '设备 2', temperature: 0, power: 0, voltage: 0, current: 0 },
  { id: 3, name: '设备 3', temperature: 0, power: 0, voltage: 0, current: 0 }
])

// 温度历史数据
const temperatureHistory = ref([])

// Grafana 单个面板的 URL
const grafanaPanelUrl = ref('http://localhost:3000/d-solo/voltage-realtime?orgId=1&from=now-5m&to=now&panelId=1&refresh=5s')

// 生成随机数据
const updateData = () => {
  devices.value.forEach((device) => {
    device.temperature = (25 + Math.random() * 10).toFixed(1)
    device.power = (2000 + Math.random() * 1000).toFixed(0)
    device.voltage = (210 + Math.random() * 20).toFixed(0)
    device.current = (10 + Math.random() * 5).toFixed(1)
  })
  
  // 更新温度历史
  const avgTemp = devices.value.reduce((sum, d) => sum + parseFloat(d.temperature), 0) / devices.value.length
  temperatureHistory.value.push((avgTemp - 25) * 10) // 归一化到 0-100
  if (temperatureHistory.value.length > 10) {
    temperatureHistory.value.shift()
  }
}

// 定时更新
let interval
onMounted(() => {
  updateData()
  interval = setInterval(updateData, 2000)
})

onUnmounted(() => {
  clearInterval(interval)
})
</script>

<style scoped>
.simple-monitor {
  padding: 20px;
  max-width: 1200px;
  margin: 0 auto;
  font-family: Arial, sans-serif;
}

h1 {
  text-align: center;
  color: #333;
  margin-bottom: 30px;
}

h2 {
  color: #666;
  margin: 30px 0 20px 0;
}

/* 数据卡片 */
.data-cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
  gap: 20px;
  margin-bottom: 40px;
}

.card {
  background: #f5f5f5;
  border-radius: 8px;
  padding: 20px;
  box-shadow: 0 2px 4px rgba(0,0,0,0.1);
}

.card h3 {
  margin: 0 0 15px 0;
  color: #333;
}

.metric {
  display: flex;
  justify-content: space-between;
  margin: 10px 0;
  padding: 5px 0;
  border-bottom: 1px solid #e0e0e0;
}

.label {
  color: #666;
}

.value {
  font-weight: bold;
  color: #2196F3;
}

/* 简单图表 */
.chart-container {
  background: white;
  padding: 20px;
  border-radius: 8px;
  box-shadow: 0 2px 4px rgba(0,0,0,0.1);
  margin-bottom: 40px;
}

.simple-chart {
  display: flex;
  align-items: flex-end;
  height: 200px;
  gap: 5px;
  padding: 20px;
  background: #fafafa;
  border-radius: 4px;
}

.chart-line {
  flex: 1;
  display: flex;
  align-items: flex-end;
}

.bar {
  width: 100%;
  background: #4CAF50;
  border-radius: 4px 4px 0 0;
  transition: height 0.3s ease;
}

/* Grafana 面板 */
.grafana-panel {
  background: white;
  padding: 20px;
  border-radius: 8px;
  box-shadow: 0 2px 4px rgba(0,0,0,0.1);
}
</style>