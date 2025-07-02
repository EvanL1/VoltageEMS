<template>
  <div class="ultra-simple">
    <h1>🌡️ 实时温度监控</h1>
    
    <!-- 大数字显示 -->
    <div class="big-display">
      <div class="device-box" v-for="device in devices" :key="device.id">
        <h2>{{ device.name }}</h2>
        <div class="temperature" :class="getTempClass(device.temp)">
          {{ device.temp }}°C
        </div>
        <div class="status">{{ getStatus(device.temp) }}</div>
      </div>
    </div>

    <!-- 极简历史记录 -->
    <div class="history">
      <h3>最近更新</h3>
      <div class="log-item" v-for="(log, index) in logs" :key="index">
        {{ log }}
      </div>
    </div>

    <!-- 刷新状态 -->
    <div class="footer">
      <span :class="['status-dot', isUpdating ? 'active' : '']"></span>
      {{ isUpdating ? '更新中...' : '等待更新' }}
      <span class="time">{{ currentTime }}</span>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted } from 'vue'

const devices = ref([
  { id: 1, name: '设备 1', temp: 25.0 },
  { id: 2, name: '设备 2', temp: 25.0 },
  { id: 3, name: '设备 3', temp: 25.0 }
])

const logs = ref([])
const isUpdating = ref(false)
const currentTime = ref('')

// 获取温度样式类
const getTempClass = (temp) => {
  if (temp < 28) return 'normal'
  if (temp < 32) return 'warning'
  return 'danger'
}

// 获取状态文字
const getStatus = (temp) => {
  if (temp < 28) return '正常'
  if (temp < 32) return '偏高'
  return '过热'
}

// 更新数据
const updateData = async () => {
  isUpdating.value = true
  
  // 模拟从服务器获取数据
  try {
    const response = await fetch('http://localhost:3001/query', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        targets: [
          { target: 'device_001.temperature' },
          { target: 'device_002.temperature' },
          { target: 'device_003.temperature' }
        ],
        range: {
          from: new Date(Date.now() - 60000).toISOString(),
          to: new Date().toISOString()
        }
      })
    })
    
    if (response.ok) {
      const data = await response.json()
      
      // 更新设备温度
      data.forEach((series, index) => {
        if (series.datapoints && series.datapoints.length > 0) {
          const lastPoint = series.datapoints[series.datapoints.length - 1]
          const temp = lastPoint[0].toFixed(1)
          devices.value[index].temp = parseFloat(temp)
          
          // 添加到日志
          const logEntry = `${new Date().toLocaleTimeString()} - ${devices.value[index].name}: ${temp}°C`
          logs.value.unshift(logEntry)
          if (logs.value.length > 5) logs.value.pop()
        }
      })
    }
  } catch (error) {
    console.error('获取数据失败:', error)
  }
  
  isUpdating.value = false
  currentTime.value = new Date().toLocaleTimeString()
}

// 定时更新
let interval
onMounted(() => {
  updateData()
  interval = setInterval(updateData, 5000) // 每5秒更新
  
  // 更新时间
  setInterval(() => {
    currentTime.value = new Date().toLocaleTimeString()
  }, 1000)
})

onUnmounted(() => {
  clearInterval(interval)
})
</script>

<style scoped>
.ultra-simple {
  max-width: 800px;
  margin: 0 auto;
  padding: 40px 20px;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  background: #f5f5f5;
  min-height: 100vh;
}

h1 {
  text-align: center;
  color: #333;
  margin-bottom: 40px;
  font-size: 32px;
}

/* 大数字显示 */
.big-display {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 20px;
  margin-bottom: 40px;
}

.device-box {
  background: white;
  border-radius: 16px;
  padding: 30px;
  text-align: center;
  box-shadow: 0 4px 6px rgba(0,0,0,0.07);
  transition: transform 0.2s;
}

.device-box:hover {
  transform: translateY(-2px);
  box-shadow: 0 6px 12px rgba(0,0,0,0.1);
}

.device-box h2 {
  margin: 0 0 20px 0;
  color: #666;
  font-size: 18px;
  font-weight: normal;
}

.temperature {
  font-size: 48px;
  font-weight: bold;
  margin: 10px 0;
  transition: color 0.3s;
}

.temperature.normal {
  color: #4CAF50;
}

.temperature.warning {
  color: #FF9800;
}

.temperature.danger {
  color: #F44336;
}

.status {
  font-size: 14px;
  color: #999;
}

/* 历史记录 */
.history {
  background: white;
  border-radius: 12px;
  padding: 20px;
  margin-bottom: 30px;
}

.history h3 {
  margin: 0 0 15px 0;
  color: #666;
  font-size: 16px;
}

.log-item {
  padding: 8px 0;
  border-bottom: 1px solid #f0f0f0;
  font-size: 14px;
  color: #666;
  font-family: monospace;
}

.log-item:last-child {
  border-bottom: none;
}

/* 底部状态 */
.footer {
  text-align: center;
  color: #999;
  font-size: 14px;
}

.status-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #ccc;
  margin-right: 8px;
  transition: background 0.3s;
}

.status-dot.active {
  background: #4CAF50;
  animation: pulse 1s infinite;
}

@keyframes pulse {
  0% { opacity: 1; }
  50% { opacity: 0.5; }
  100% { opacity: 1; }
}

.time {
  margin-left: 20px;
  font-family: monospace;
}
</style>