<template>
  <div class="realtime-container">
    <div class="page-header">
      <h1>{{ $t('menu.realtime') }}</h1>
      <div class="header-actions">
        <el-select v-model="refreshInterval" size="default" style="width: 140px">
          <el-option 
            v-for="item in refreshOptions" 
            :key="item.value" 
            :label="item.label" 
            :value="item.value"
          />
        </el-select>
        <el-button @click="handleRefresh">
          <el-icon><Refresh /></el-icon>
          {{ $t('common.refresh') }}
        </el-button>
      </div>
    </div>

    <div class="content-wrapper">
      <!-- 左侧设备树 -->
      <el-card class="device-tree-card">
        <template #header>
          <div class="card-header">
            <span>{{ $t('realtime.deviceTree') }}</span>
            <el-input 
              v-model="filterText" 
              :placeholder="$t('common.search')" 
              size="small"
              :prefix-icon="Search"
            />
          </div>
        </template>
        <el-tree
          ref="treeRef"
          :data="deviceTreeData"
          :props="treeProps"
          :filter-node-method="filterNode"
          default-expand-all
          highlight-current
          @node-click="handleNodeClick"
        >
          <template #default="{ data }">
            <span class="tree-node">
              <el-icon v-if="data.type === 'channel'" color="#409eff"><Connection /></el-icon>
              <el-icon v-else-if="data.type === 'device'" color="#67c23a"><Monitor /></el-icon>
              <el-icon v-else color="#e6a23c"><Cpu /></el-icon>
              <span>{{ data.label }}</span>
              <el-tag v-if="data.status" :type="data.status" size="small" style="margin-left: 8px">
                {{ data.status === 'success' ? $t('common.online') : $t('common.offline') }}
              </el-tag>
            </span>
          </template>
        </el-tree>
      </el-card>

      <!-- 右侧内容区 -->
      <div class="content-area">
        <!-- 数据表格 -->
        <el-card class="data-table-card">
          <template #header>
            <div class="card-header">
              <span>{{ selectedNode ? selectedNode.label : $t('realtime.allData') }}</span>
              <div class="header-tools">
                <el-checkbox v-model="autoScroll">{{ $t('realtime.autoScroll') }}</el-checkbox>
                <el-button 
                  v-if="userStore.canControl" 
                  type="primary" 
                  size="small"
                  @click="showControlDialog = true"
                >
                  <el-icon><Setting /></el-icon>
                  {{ $t('realtime.control') }}
                </el-button>
              </div>
            </div>
          </template>
          
          <el-table 
            :data="realtimeData" 
            height="400"
            stripe
            :row-class-name="tableRowClassName"
          >
            <el-table-column prop="name" :label="$t('realtime.pointName')" width="200" />
            <el-table-column prop="value" :label="$t('realtime.currentValue')" width="120">
              <template #default="{ row }">
                <span :class="getValueClass(row)">
                  {{ formatValue(row.value, row.type) }}
                </span>
              </template>
            </el-table-column>
            <el-table-column prop="unit" :label="$t('realtime.unit')" width="80" />
            <el-table-column prop="quality" :label="$t('realtime.quality')" width="100">
              <template #default="{ row }">
                <el-tag :type="getQualityType(row.quality)" size="small">
                  {{ getQualityText(row.quality) }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="timestamp" :label="$t('realtime.updateTime')" width="180">
              <template #default="{ row }">
                {{ formatTime(row.timestamp) }}
              </template>
            </el-table-column>
            <el-table-column prop="description" :label="$t('realtime.description')" />
            <el-table-column 
              v-if="userStore.canControl" 
              :label="$t('common.operation')" 
              width="120"
              fixed="right"
            >
              <template #default="{ row }">
                <el-button 
                  v-if="row.type === 'YK' || row.type === 'YT'" 
                  type="primary" 
                  size="small" 
                  link
                  @click="handleControl(row)"
                >
                  {{ $t('realtime.control') }}
                </el-button>
              </template>
            </el-table-column>
          </el-table>
        </el-card>

        <!-- 趋势图表 -->
        <el-card class="trend-chart-card">
          <template #header>
            <div class="card-header">
              <span>{{ $t('realtime.trendChart') }}</span>
              <el-radio-group v-model="chartRange" size="small">
                <el-radio-button label="1h">{{ $t('realtime.lastHour') }}</el-radio-button>
                <el-radio-button label="6h">{{ $t('realtime.last6Hours') }}</el-radio-button>
                <el-radio-button label="24h">{{ $t('realtime.last24Hours') }}</el-radio-button>
              </el-radio-group>
            </div>
          </template>
          <div ref="chartRef" class="trend-chart"></div>
        </el-card>
      </div>
    </div>

    <!-- 控制对话框 -->
    <el-dialog 
      v-model="showControlDialog" 
      :title="$t('realtime.controlDialog.title')"
      width="500px"
    >
      <el-form :model="controlForm" label-width="100px">
        <el-form-item :label="$t('realtime.controlDialog.point')">
          <el-input v-model="controlForm.pointName" disabled />
        </el-form-item>
        <el-form-item :label="$t('realtime.controlDialog.currentValue')">
          <el-input v-model="controlForm.currentValue" disabled />
        </el-form-item>
        <el-form-item 
          v-if="controlForm.type === 'YK'" 
          :label="$t('realtime.controlDialog.controlValue')"
        >
          <el-radio-group v-model="controlForm.value">
            <el-radio :label="1">{{ $t('common.on') }}</el-radio>
            <el-radio :label="0">{{ $t('common.off') }}</el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item 
          v-else 
          :label="$t('realtime.controlDialog.setValue')"
        >
          <el-input-number v-model="controlForm.value" :precision="2" />
        </el-form-item>
        <el-form-item :label="$t('realtime.controlDialog.remark')">
          <el-input v-model="controlForm.remark" type="textarea" :rows="3" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showControlDialog = false">{{ $t('common.cancel') }}</el-button>
        <el-button type="primary" @click="submitControl">{{ $t('common.confirm') }}</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, watch, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { 
  Refresh, 
  Search, 
  Connection, 
  Monitor, 
  Cpu, 
  Setting 
} from '@element-plus/icons-vue'
import * as echarts from 'echarts'
import dayjs from 'dayjs'
import { useUserStore } from '@/stores/user'

const { t } = useI18n()
const userStore = useUserStore()

// 刷新间隔选项
const refreshInterval = ref(5000)
const refreshOptions = [
  { label: t('realtime.refresh1s'), value: 1000 },
  { label: t('realtime.refresh5s'), value: 5000 },
  { label: t('realtime.refresh10s'), value: 10000 },
  { label: t('realtime.refresh30s'), value: 30000 },
  { label: t('realtime.refreshOff'), value: 0 }
]

// 设备树
const filterText = ref('')
const treeRef = ref()
const selectedNode = ref(null)
const deviceTreeData = ref([
  {
    id: 'ch1',
    label: 'Modbus TCP Channel',
    type: 'channel',
    status: 'success',
    children: [
      {
        id: 'dev1',
        label: 'PLC Device 1',
        type: 'device',
        status: 'success',
        children: [
          { id: 'pt1', label: 'Temperature Sensor', type: 'point' },
          { id: 'pt2', label: 'Pressure Sensor', type: 'point' },
          { id: 'pt3', label: 'Flow Meter', type: 'point' }
        ]
      },
      {
        id: 'dev2',
        label: 'Power Meter',
        type: 'device',
        status: 'danger',
        children: [
          { id: 'pt4', label: 'Voltage', type: 'point' },
          { id: 'pt5', label: 'Current', type: 'point' },
          { id: 'pt6', label: 'Power', type: 'point' }
        ]
      }
    ]
  },
  {
    id: 'ch2',
    label: 'CAN Bus Channel',
    type: 'channel',
    status: 'success',
    children: [
      {
        id: 'dev3',
        label: 'Motor Controller',
        type: 'device',
        status: 'success',
        children: [
          { id: 'pt7', label: 'Motor Speed', type: 'point' },
          { id: 'pt8', label: 'Motor Status', type: 'point' }
        ]
      }
    ]
  }
])

const treeProps = {
  children: 'children',
  label: 'label'
}

// 实时数据
const realtimeData = ref([
  { id: 1, name: 'Temperature_1', value: 25.6, unit: '°C', type: 'YC', quality: 'good', timestamp: Date.now(), description: 'Room temperature' },
  { id: 2, name: 'Pressure_1', value: 101.3, unit: 'kPa', type: 'YC', quality: 'good', timestamp: Date.now(), description: 'System pressure' },
  { id: 3, name: 'Flow_1', value: 45.2, unit: 'm³/h', type: 'YC', quality: 'good', timestamp: Date.now(), description: 'Water flow rate' },
  { id: 4, name: 'Motor_Status', value: 1, unit: '', type: 'YX', quality: 'good', timestamp: Date.now(), description: 'Motor running status' },
  { id: 5, name: 'Valve_Control', value: 0, unit: '', type: 'YK', quality: 'good', timestamp: Date.now(), description: 'Main valve control' },
  { id: 6, name: 'SetPoint_1', value: 50.0, unit: '%', type: 'YT', quality: 'good', timestamp: Date.now(), description: 'Control setpoint' }
])

const autoScroll = ref(true)

// 图表
const chartRef = ref()
const chartInstance = ref(null)
const chartRange = ref('1h')

// 控制对话框
const showControlDialog = ref(false)
const controlForm = ref({
  pointName: '',
  currentValue: '',
  type: '',
  value: 0,
  remark: ''
})

// 定时器
let refreshTimer = null

// 过滤节点
watch(filterText, (val) => {
  treeRef.value?.filter(val)
})

const filterNode = (value, data) => {
  if (!value) return true
  return data.label.toLowerCase().includes(value.toLowerCase())
}

// 节点点击
const handleNodeClick = (data) => {
  selectedNode.value = data
  // 根据选中节点过滤数据
  loadRealtimeData(data)
}

// 刷新数据
const handleRefresh = () => {
  loadRealtimeData(selectedNode.value)
  updateChart()
  ElMessage.success(t('common.refreshSuccess'))
}

// 加载实时数据
const loadRealtimeData = (node) => {
  // 模拟根据节点加载数据
  // 实际应该调用 API
  if (node && node.type === 'device') {
    // 过滤该设备的数据
    realtimeData.value = realtimeData.value.filter(item => {
      return item.name.includes(node.label.split(' ')[0])
    })
  }
  
  // 更新时间戳
  realtimeData.value.forEach(item => {
    item.timestamp = Date.now()
    // 模拟数据变化
    if (item.type === 'YC') {
      item.value = (Math.random() * 100).toFixed(2)
    }
  })
}

// 表格行样式
const tableRowClassName = ({ row }) => {
  if (row.quality !== 'good') return 'warning-row'
  return ''
}

// 值样式
const getValueClass = (row) => {
  if (row.type === 'YX' || row.type === 'YK') {
    return row.value === 1 ? 'value-on' : 'value-off'
  }
  return ''
}

// 格式化值
const formatValue = (value, type) => {
  if (type === 'YX' || type === 'YK') {
    return value === 1 ? t('common.on') : t('common.off')
  }
  return value
}

// 质量类型
const getQualityType = (quality) => {
  const types = {
    good: 'success',
    bad: 'danger',
    uncertain: 'warning'
  }
  return types[quality] || 'info'
}

// 质量文本
const getQualityText = (quality) => {
  const texts = {
    good: t('realtime.qualityGood'),
    bad: t('realtime.qualityBad'),
    uncertain: t('realtime.qualityUncertain')
  }
  return texts[quality] || quality
}

// 格式化时间
const formatTime = (timestamp) => {
  return dayjs(timestamp).format('YYYY-MM-DD HH:mm:ss')
}

// 控制操作
const handleControl = (row) => {
  controlForm.value = {
    pointName: row.name,
    currentValue: formatValue(row.value, row.type),
    type: row.type,
    value: row.type === 'YK' ? row.value : parseFloat(row.value),
    remark: ''
  }
  showControlDialog.value = true
}

// 提交控制
const submitControl = async () => {
  try {
    // 调用控制 API
    // await controlAPI(controlForm.value)
    ElMessage.success(t('realtime.controlSuccess'))
    showControlDialog.value = false
    handleRefresh()
  } catch (error) {
    ElMessage.error(t('realtime.controlFailed'))
  }
}

// 初始化图表
const initChart = () => {
  if (!chartRef.value) return
  
  chartInstance.value = echarts.init(chartRef.value)
  
  const option = {
    title: {
      text: ''
    },
    tooltip: {
      trigger: 'axis',
      axisPointer: {
        type: 'cross'
      }
    },
    legend: {
      data: ['Temperature', 'Pressure', 'Flow']
    },
    grid: {
      left: '3%',
      right: '4%',
      bottom: '3%',
      containLabel: true
    },
    xAxis: {
      type: 'time',
      boundaryGap: false
    },
    yAxis: [
      {
        type: 'value',
        name: 'Temperature (°C)',
        position: 'left'
      },
      {
        type: 'value',
        name: 'Pressure (kPa)',
        position: 'right'
      }
    ],
    series: [
      {
        name: 'Temperature',
        type: 'line',
        smooth: true,
        data: generateMockData('temp'),
        yAxisIndex: 0
      },
      {
        name: 'Pressure',
        type: 'line',
        smooth: true,
        data: generateMockData('pressure'),
        yAxisIndex: 1
      }
    ]
  }
  
  chartInstance.value.setOption(option)
}

// 生成模拟数据
const generateMockData = (type) => {
  const data = []
  const now = Date.now()
  const baseValue = type === 'temp' ? 25 : 100
  
  for (let i = 0; i < 60; i++) {
    data.push([
      now - (60 - i) * 60 * 1000,
      baseValue + Math.random() * 10
    ])
  }
  
  return data
}

// 更新图表
const updateChart = () => {
  if (!chartInstance.value) return
  
  // 添加新数据点
  const option = chartInstance.value.getOption()
  option.series[0].data.push([
    Date.now(),
    25 + Math.random() * 10
  ])
  option.series[1].data.push([
    Date.now(),
    100 + Math.random() * 10
  ])
  
  // 保持数据点数量
  if (option.series[0].data.length > 60) {
    option.series[0].data.shift()
    option.series[1].data.shift()
  }
  
  chartInstance.value.setOption(option)
}

// 启动定时刷新
const startRefresh = () => {
  if (refreshTimer) {
    clearInterval(refreshTimer)
  }
  
  if (refreshInterval.value > 0) {
    refreshTimer = setInterval(() => {
      loadRealtimeData(selectedNode.value)
      updateChart()
    }, refreshInterval.value)
  }
}

// 监听刷新间隔变化
watch(refreshInterval, () => {
  startRefresh()
})

// 监听窗口大小变化
const handleResize = () => {
  chartInstance.value?.resize()
}

onMounted(() => {
  initChart()
  startRefresh()
  window.addEventListener('resize', handleResize)
  
  // 连接 WebSocket
  // realtimeStore.connect()
})

onUnmounted(() => {
  if (refreshTimer) {
    clearInterval(refreshTimer)
  }
  window.removeEventListener('resize', handleResize)
  chartInstance.value?.dispose()
  
  // 断开 WebSocket
  // realtimeStore.disconnect()
})
</script>

<style lang="scss" scoped>
@import '@/styles/design-tokens.scss';

.realtime-container {
  height: 100%;
  display: flex;
  flex-direction: column;
}

// Apple 风格页面头部
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--space-6);
  
  h1 {
    margin: 0;
    font-size: var(--font-size-4xl);
    font-weight: var(--font-weight-bold);
    color: var(--color-text-primary);
    letter-spacing: -0.02em;
  }
  
  .header-actions {
    display: flex;
    gap: var(--space-3);
    
    .el-select {
      :deep(.el-input__wrapper) {
        background: var(--color-background-elevated);
        border: 1px solid var(--color-border-light);
        border-radius: var(--radius-lg);
        box-shadow: none;
        transition: all var(--duration-fast) var(--ease-in-out);
        
        &:hover {
          border-color: var(--color-border);
        }
        
        &.is-focus {
          border-color: var(--color-primary);
          box-shadow: 0 0 0 3px var(--color-primary-light);
        }
      }
    }
    
    .el-button {
      background: var(--color-background-elevated);
      border: 1px solid var(--color-border-light);
      border-radius: var(--radius-lg);
      color: var(--color-primary);
      font-weight: var(--font-weight-medium);
      transition: all var(--duration-fast) var(--ease-in-out);
      
      &:hover {
        background: var(--color-primary);
        color: var(--color-text-inverse);
        border-color: var(--color-primary);
        transform: translateY(-1px);
        box-shadow: var(--shadow-md);
      }
    }
  }
}

.content-wrapper {
  flex: 1;
  display: flex;
  gap: var(--space-6);
  overflow: hidden;
}

// Tesla 风格设备树卡片
.device-tree-card {
  width: 320px;
  flex-shrink: 0;
  background: var(--color-background-elevated);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-sm);
  overflow: hidden;
  
  :deep(.el-card__header) {
    padding: var(--space-5) var(--space-6);
    border-bottom: 1px solid var(--color-border-light);
    background: var(--color-background);
  }
  
  :deep(.el-card__body) {
    height: calc(100vh - 280px);
    overflow-y: auto;
    padding: var(--space-4);
    
    &::-webkit-scrollbar {
      width: 6px;
    }
    
    &::-webkit-scrollbar-track {
      background: transparent;
    }
    
    &::-webkit-scrollbar-thumb {
      background: var(--color-gray-300);
      border-radius: var(--radius-full);
      
      &:hover {
        background: var(--color-gray-400);
      }
    }
  }
  
  .card-header {
    .el-input {
      :deep(.el-input__wrapper) {
        background: var(--color-background);
        border: 1px solid var(--color-border-light);
        border-radius: var(--radius-full);
        padding: 0 var(--space-3);
        height: 32px;
        
        &:hover {
          border-color: var(--color-border);
        }
        
        &.is-focus {
          border-color: var(--color-primary);
          box-shadow: 0 0 0 3px var(--color-primary-light);
        }
      }
    }
  }
  
  // 树节点样式
  :deep(.el-tree) {
    background: transparent;
    
    .el-tree-node__content {
      height: 40px;
      border-radius: var(--radius-lg);
      transition: all var(--duration-fast) var(--ease-in-out);
      margin-bottom: var(--space-1);
      
      &:hover {
        background: var(--color-background);
      }
      
      &.is-current {
        background: var(--color-primary-light);
        color: var(--color-primary);
        font-weight: var(--font-weight-medium);
      }
    }
    
    .tree-node {
      display: flex;
      align-items: center;
      gap: var(--space-2);
      flex: 1;
      
      .el-icon {
        font-size: 18px;
      }
      
      .el-tag {
        height: 20px;
        line-height: 18px;
        padding: 0 var(--space-2);
        font-size: var(--font-size-xs);
        border-radius: var(--radius-md);
      }
    }
  }
}

.content-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  overflow: hidden;
}

// 数据表格卡片
.data-table-card {
  flex: 1;
  background: var(--color-background-elevated);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-sm);
  overflow: hidden;
  
  :deep(.el-card__header) {
    padding: var(--space-5) var(--space-6);
    border-bottom: 1px solid var(--color-border-light);
    background: var(--color-background);
  }
  
  :deep(.el-card__body) {
    padding: 0;
  }
  
  .header-tools {
    .el-checkbox {
      margin-right: var(--space-4);
      
      :deep(.el-checkbox__label) {
        color: var(--color-text-secondary);
        font-weight: var(--font-weight-medium);
      }
    }
    
    .el-button {
      background: var(--color-primary);
      border-color: var(--color-primary);
      color: var(--color-text-inverse);
      border-radius: var(--radius-lg);
      font-weight: var(--font-weight-medium);
      
      &:hover {
        background: var(--color-primary-hover);
        border-color: var(--color-primary-hover);
        transform: translateY(-1px);
        box-shadow: var(--shadow-md);
      }
    }
  }
  
  // 表格样式优化
  :deep(.el-table) {
    font-size: var(--font-size-sm);
    
    .el-table__header {
      th {
        background: var(--color-background);
        color: var(--color-text-secondary);
        font-weight: var(--font-weight-semibold);
        text-transform: uppercase;
        font-size: var(--font-size-xs);
        letter-spacing: 0.05em;
      }
    }
    
    .el-table__row {
      &:hover {
        background: var(--color-background);
      }
      
      &.warning-row {
        background-color: var(--color-danger-light);
        
        &:hover {
          background-color: var(--color-danger-light);
        }
      }
    }
    
    .value-on {
      color: var(--color-success);
      font-weight: var(--font-weight-semibold);
    }
    
    .value-off {
      color: var(--color-text-tertiary);
    }
    
    .el-button--small {
      padding: var(--space-1) var(--space-3);
      height: 28px;
      border-radius: var(--radius-md);
    }
  }
}

// 趋势图表卡片
.trend-chart-card {
  height: 320px;
  background: var(--color-background-elevated);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-sm);
  
  :deep(.el-card__header) {
    padding: var(--space-5) var(--space-6);
    border-bottom: 1px solid var(--color-border-light);
    background: var(--color-background);
  }
  
  :deep(.el-card__body) {
    padding: var(--space-6);
  }
  
  .el-radio-group {
    :deep(.el-radio-button__inner) {
      padding: var(--space-1) var(--space-3);
      border: none;
      background: var(--color-background);
      color: var(--color-text-secondary);
      font-weight: var(--font-weight-medium);
      font-size: var(--font-size-sm);
      height: 28px;
      line-height: 26px;
      
      &:hover {
        color: var(--color-text-primary);
      }
    }
    
    :deep(.el-radio-button__original-radio:checked + .el-radio-button__inner) {
      background: var(--color-primary);
      color: var(--color-text-inverse);
      box-shadow: none;
    }
    
    :deep(.el-radio-button:first-child .el-radio-button__inner) {
      border-radius: var(--radius-md) 0 0 var(--radius-md);
    }
    
    :deep(.el-radio-button:last-child .el-radio-button__inner) {
      border-radius: 0 var(--radius-md) var(--radius-md) 0;
    }
  }
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  
  > span {
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }
  
  .header-tools {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
}

.trend-chart {
  width: 100%;
  height: 240px;
}

// 控制对话框样式
:deep(.el-dialog) {
  border-radius: var(--radius-xl);
  
  .el-dialog__header {
    padding: var(--space-6) var(--space-6) var(--space-4);
    
    .el-dialog__title {
      font-size: var(--font-size-xl);
      font-weight: var(--font-weight-semibold);
      color: var(--color-text-primary);
    }
  }
  
  .el-dialog__body {
    padding: var(--space-4) var(--space-6) var(--space-6);
  }
  
  .el-dialog__footer {
    padding: var(--space-4) var(--space-6) var(--space-6);
  }
}

// 响应式设计
@media (max-width: 1280px) {
  .device-tree-card {
    width: 280px;
  }
}

@media (max-width: 1024px) {
  .content-wrapper {
    flex-direction: column;
  }
  
  .device-tree-card {
    width: 100%;
    height: 300px;
    
    :deep(.el-card__body) {
      height: 220px;
    }
  }
  
  .trend-chart-card {
    height: 280px;
    
    .trend-chart {
      height: 200px;
    }
  }
}

@media (max-width: 768px) {
  .page-header {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-4);
    
    h1 {
      font-size: var(--font-size-3xl);
    }
  }
}
</style>