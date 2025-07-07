<template>
  <div class="topology-container">
    <!-- 页面头部 -->
    <div class="page-header">
      <h1>{{ $t('topology.title') }}</h1>
      <div class="header-actions">
        <el-button-group>
          <el-button @click="zoomIn">
            <el-icon><ZoomIn /></el-icon>
          </el-button>
          <el-button @click="zoomOut">
            <el-icon><ZoomOut /></el-icon>
          </el-button>
          <el-button @click="resetZoom">
            <el-icon><FullScreen /></el-icon>
          </el-button>
        </el-button-group>
        <el-button @click="toggleAutoLayout">
          <el-icon><Grid /></el-icon>
          {{ autoLayout ? $t('topology.freeLayout') : $t('topology.autoLayout') }}
        </el-button>
        <el-button @click="handleRefresh">
          <el-icon><Refresh /></el-icon>
          {{ $t('common.refresh') }}
        </el-button>
      </div>
    </div>

    <!-- 拓扑图区域 -->
    <el-row :gutter="20">
      <el-col :span="18">
        <el-card class="topology-card">
          <div ref="topologyRef" class="topology-graph"></div>
          <!-- 图例 -->
          <div class="topology-legend">
            <div class="legend-item">
              <span class="legend-icon service"></span>
              <span>{{ $t('topology.service') }}</span>
            </div>
            <div class="legend-item">
              <span class="legend-icon device-online"></span>
              <span>{{ $t('topology.deviceOnline') }}</span>
            </div>
            <div class="legend-item">
              <span class="legend-icon device-offline"></span>
              <span>{{ $t('topology.deviceOffline') }}</span>
            </div>
            <div class="legend-item">
              <span class="legend-icon device-warning"></span>
              <span>{{ $t('topology.deviceWarning') }}</span>
            </div>
            <div class="legend-item">
              <span class="legend-line normal"></span>
              <span>{{ $t('topology.normalConnection') }}</span>
            </div>
            <div class="legend-item">
              <span class="legend-line warning"></span>
              <span>{{ $t('topology.warningConnection') }}</span>
            </div>
            <div class="legend-item">
              <span class="legend-line error"></span>
              <span>{{ $t('topology.errorConnection') }}</span>
            </div>
          </div>
        </el-card>
      </el-col>
      
      <!-- 右侧信息面板 -->
      <el-col :span="6">
        <!-- 统计信息 -->
        <el-card class="info-card">
          <template #header>
            <span>{{ $t('topology.statistics') }}</span>
          </template>
          <div class="stat-item">
            <span class="stat-label">{{ $t('topology.totalServices') }}:</span>
            <span class="stat-value">{{ stats.totalServices }}</span>
          </div>
          <div class="stat-item">
            <span class="stat-label">{{ $t('topology.totalDevices') }}:</span>
            <span class="stat-value">{{ stats.totalDevices }}</span>
          </div>
          <div class="stat-item">
            <span class="stat-label">{{ $t('topology.onlineDevices') }}:</span>
            <span class="stat-value success">{{ stats.onlineDevices }}</span>
          </div>
          <div class="stat-item">
            <span class="stat-label">{{ $t('topology.offlineDevices') }}:</span>
            <span class="stat-value danger">{{ stats.offlineDevices }}</span>
          </div>
          <div class="stat-item">
            <span class="stat-label">{{ $t('topology.warningDevices') }}:</span>
            <span class="stat-value warning">{{ stats.warningDevices }}</span>
          </div>
          <div class="stat-item">
            <span class="stat-label">{{ $t('topology.totalConnections') }}:</span>
            <span class="stat-value">{{ stats.totalConnections }}</span>
          </div>
        </el-card>

        <!-- 选中节点信息 -->
        <el-card v-if="selectedNode" class="info-card">
          <template #header>
            <span>{{ $t('topology.nodeInfo') }}</span>
          </template>
          <el-descriptions :column="1" size="small">
            <el-descriptions-item :label="$t('common.name')">
              {{ selectedNode.name }}
            </el-descriptions-item>
            <el-descriptions-item :label="$t('common.type')">
              <el-tag size="small" :type="getNodeTypeTag(selectedNode.nodeType)">
                {{ selectedNode.nodeType }}
              </el-tag>
            </el-descriptions-item>
            <el-descriptions-item v-if="selectedNode.nodeType === 'device'" :label="$t('common.status')">
              <el-tag size="small" :type="getStatusType(selectedNode.status)">
                {{ getStatusLabel(selectedNode.status) }}
              </el-tag>
            </el-descriptions-item>
            <el-descriptions-item v-if="selectedNode.id" label="ID">
              {{ selectedNode.id }}
            </el-descriptions-item>
            <el-descriptions-item v-if="selectedNode.ip" :label="$t('topology.ipAddress')">
              {{ selectedNode.ip }}
            </el-descriptions-item>
            <el-descriptions-item v-if="selectedNode.protocol" :label="$t('topology.protocol')">
              {{ selectedNode.protocol }}
            </el-descriptions-item>
            <el-descriptions-item v-if="selectedNode.channel" :label="$t('topology.channel')">
              {{ selectedNode.channel }}
            </el-descriptions-item>
            <el-descriptions-item v-if="selectedNode.connections" :label="$t('topology.connections')">
              {{ selectedNode.connections }}
            </el-descriptions-item>
          </el-descriptions>
          <div v-if="selectedNode.nodeType === 'device'" class="node-actions">
            <el-button type="primary" size="small" @click="viewDeviceDetails">
              {{ $t('common.details') }}
            </el-button>
            <el-button size="small" @click="refreshNode">
              {{ $t('common.refresh') }}
            </el-button>
          </div>
        </el-card>

        <!-- 过滤器 -->
        <el-card class="info-card">
          <template #header>
            <span>{{ $t('topology.filter') }}</span>
          </template>
          <el-form label-position="top">
            <el-form-item :label="$t('topology.showServices')">
              <el-switch v-model="filter.showServices" @change="updateTopology" />
            </el-form-item>
            <el-form-item :label="$t('topology.showDevices')">
              <el-switch v-model="filter.showDevices" @change="updateTopology" />
            </el-form-item>
            <el-form-item :label="$t('topology.deviceStatus')">
              <el-checkbox-group v-model="filter.deviceStatus" @change="updateTopology">
                <el-checkbox label="online">{{ $t('common.online') }}</el-checkbox>
                <el-checkbox label="offline">{{ $t('common.offline') }}</el-checkbox>
                <el-checkbox label="warning">{{ $t('topology.warning') }}</el-checkbox>
              </el-checkbox-group>
            </el-form-item>
          </el-form>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { 
  ZoomIn, ZoomOut, FullScreen, Grid, Refresh
} from '@element-plus/icons-vue'
import * as echarts from 'echarts'

const { t } = useI18n()
const router = useRouter()

// DOM引用
const topologyRef = ref(null)

// 图表实例
let topologyChart = null

// 自动布局
const autoLayout = ref(true)

// 统计数据
const stats = ref({
  totalServices: 5,
  totalDevices: 48,
  onlineDevices: 42,
  offlineDevices: 4,
  warningDevices: 2,
  totalConnections: 65
})

// 选中的节点
const selectedNode = ref(null)

// 过滤器
const filter = ref({
  showServices: true,
  showDevices: true,
  deviceStatus: ['online', 'offline', 'warning']
})

// 缩放级别
let currentZoom = 1

// 获取节点类型标签
const getNodeTypeTag = (type) => {
  return type === 'service' ? 'primary' : 'info'
}

// 获取状态类型
const getStatusType = (status) => {
  const typeMap = {
    online: 'success',
    offline: 'danger',
    warning: 'warning'
  }
  return typeMap[status] || 'info'
}

// 获取状态标签
const getStatusLabel = (status) => {
  const labelMap = {
    online: t('common.online'),
    offline: t('common.offline'),
    warning: t('topology.warning')
  }
  return labelMap[status] || status
}

// 放大
const zoomIn = () => {
  currentZoom = Math.min(currentZoom * 1.2, 3)
  if (topologyChart) {
    topologyChart.setOption({
      series: [{
        zoom: currentZoom
      }]
    })
  }
}

// 缩小
const zoomOut = () => {
  currentZoom = Math.max(currentZoom / 1.2, 0.5)
  if (topologyChart) {
    topologyChart.setOption({
      series: [{
        zoom: currentZoom
      }]
    })
  }
}

// 重置缩放
const resetZoom = () => {
  currentZoom = 1
  if (topologyChart) {
    topologyChart.setOption({
      series: [{
        zoom: 1,
        center: null
      }]
    })
  }
}

// 切换自动布局
const toggleAutoLayout = () => {
  autoLayout.value = !autoLayout.value
  updateTopology()
}

// 刷新
const handleRefresh = () => {
  ElMessage.info(t('topology.refreshing'))
  loadData()
}

// 查看设备详情
const viewDeviceDetails = () => {
  if (selectedNode.value && selectedNode.value.nodeType === 'device') {
    router.push({
      name: 'DeviceStatus',
      query: { deviceId: selectedNode.value.id }
    })
  }
}

// 刷新节点
const refreshNode = () => {
  if (selectedNode.value) {
    ElMessage.info(t('topology.refreshingNode', { name: selectedNode.value.name }))
    setTimeout(() => {
      ElMessage.success(t('common.refreshSuccess'))
    }, 1000)
  }
}

// 生成拓扑数据
const generateTopologyData = () => {
  const nodes = []
  const links = []
  
  // 服务节点
  const services = [
    { id: 'redis', name: 'Redis', x: 400, y: 300 },
    { id: 'comsrv', name: 'ComSrv', x: 200, y: 200 },
    { id: 'modsrv', name: 'ModSrv', x: 600, y: 200 },
    { id: 'hissrv', name: 'HisSrv', x: 200, y: 400 },
    { id: 'netsrv', name: 'NetSrv', x: 600, y: 400 },
    { id: 'alarmsrv', name: 'AlarmSrv', x: 400, y: 500 }
  ]
  
  if (filter.value.showServices) {
    services.forEach(service => {
      nodes.push({
        id: service.id,
        name: service.name,
        value: 60,
        x: autoLayout.value ? undefined : service.x,
        y: autoLayout.value ? undefined : service.y,
        fixed: !autoLayout.value,
        nodeType: 'service',
        category: 0,
        symbol: 'rect',
        symbolSize: [80, 60],
        itemStyle: {
          color: '#409EFF'
        }
      })
    })
    
    // 服务之间的连接
    links.push(
      { source: 'comsrv', target: 'redis', value: 10 },
      { source: 'redis', target: 'modsrv', value: 8 },
      { source: 'redis', target: 'hissrv', value: 6 },
      { source: 'redis', target: 'netsrv', value: 7 },
      { source: 'redis', target: 'alarmsrv', value: 5 }
    )
  }
  
  // 设备节点
  if (filter.value.showDevices) {
    const devices = []
    const statuses = ['online', 'online', 'online', 'offline', 'warning']
    const protocols = ['Modbus TCP', 'IEC 104', 'Modbus RTU', 'CAN']
    const channels = ['Channel 1', 'Channel 2', 'Channel 3']
    
    // 生成设备节点
    for (let i = 0; i < 48; i++) {
      const status = statuses[Math.floor(Math.random() * statuses.length)]
      const protocol = protocols[Math.floor(Math.random() * protocols.length)]
      const channel = channels[Math.floor(Math.random() * channels.length)]
      
      if (filter.value.deviceStatus.includes(status)) {
        const angle = (i / 48) * Math.PI * 2
        const radius = 150 + Math.random() * 100
        
        devices.push({
          id: `device_${i}`,
          name: `Device ${i + 1}`,
          value: 30,
          x: autoLayout.value ? undefined : 400 + Math.cos(angle) * radius,
          y: autoLayout.value ? undefined : 300 + Math.sin(angle) * radius,
          fixed: !autoLayout.value,
          nodeType: 'device',
          status: status,
          ip: `192.168.1.${100 + i}`,
          protocol: protocol,
          channel: channel,
          category: status === 'online' ? 1 : status === 'offline' ? 2 : 3,
          itemStyle: {
            color: status === 'online' ? '#67C23A' : 
                   status === 'offline' ? '#909399' : '#E6A23C'
          }
        })
        
        // 设备到服务的连接
        const targetService = i % 2 === 0 ? 'comsrv' : 'comsrv'
        links.push({
          source: `device_${i}`,
          target: targetService,
          value: 1,
          lineStyle: {
            color: status === 'online' ? '#67C23A' : 
                   status === 'offline' ? '#F56C6C' : '#E6A23C',
            type: status === 'offline' ? 'dashed' : 'solid'
          }
        })
      }
    }
    
    nodes.push(...devices)
  }
  
  return { nodes, links }
}

// 初始化拓扑图
const initTopology = () => {
  topologyChart = echarts.init(topologyRef.value)
  
  const { nodes, links } = generateTopologyData()
  
  const option = {
    tooltip: {
      formatter: (params) => {
        if (params.dataType === 'node') {
          const node = params.data
          let content = `<strong>${node.name}</strong><br/>`
          content += `${t('common.type')}: ${node.nodeType}<br/>`
          if (node.status) {
            content += `${t('common.status')}: ${getStatusLabel(node.status)}<br/>`
          }
          if (node.protocol) {
            content += `${t('topology.protocol')}: ${node.protocol}<br/>`
          }
          if (node.channel) {
            content += `${t('topology.channel')}: ${node.channel}<br/>`
          }
          return content
        } else {
          return `${params.data.source} → ${params.data.target}`
        }
      }
    },
    animationDurationUpdate: 1500,
    animationEasingUpdate: 'quinticInOut',
    series: [
      {
        type: 'graph',
        layout: autoLayout.value ? 'force' : 'none',
        data: nodes,
        links: links,
        categories: [
          { name: 'Service' },
          { name: 'Online' },
          { name: 'Offline' },
          { name: 'Warning' }
        ],
        roam: true,
        draggable: true,
        symbolSize: 50,
        label: {
          show: true,
          position: 'bottom',
          formatter: '{b}',
          fontSize: 12
        },
        labelLayout: {
          hideOverlap: true
        },
        edgeSymbol: ['circle', 'arrow'],
        edgeSymbolSize: [4, 10],
        lineStyle: {
          color: '#999',
          width: 2,
          curveness: 0.1
        },
        emphasis: {
          focus: 'adjacency',
          lineStyle: {
            width: 4
          }
        },
        force: {
          repulsion: 300,
          gravity: 0.1,
          edgeLength: 150,
          friction: 0.6
        }
      }
    ]
  }
  
  topologyChart.setOption(option)
  
  // 节点点击事件
  topologyChart.on('click', (params) => {
    if (params.dataType === 'node') {
      selectedNode.value = params.data
    }
  })
}

// 更新拓扑图
const updateTopology = () => {
  if (topologyChart) {
    const { nodes, links } = generateTopologyData()
    
    topologyChart.setOption({
      series: [{
        layout: autoLayout.value ? 'force' : 'none',
        data: nodes,
        links: links
      }]
    })
  }
}

// 加载数据
const loadData = () => {
  // 更新统计数据
  const onlineCount = Math.floor(Math.random() * 5) + 40
  const offlineCount = Math.floor(Math.random() * 3) + 3
  const warningCount = Math.floor(Math.random() * 3) + 1
  
  stats.value.onlineDevices = onlineCount
  stats.value.offlineDevices = offlineCount
  stats.value.warningDevices = warningCount
  stats.value.totalDevices = onlineCount + offlineCount + warningCount
  
  updateTopology()
  ElMessage.success(t('common.refreshSuccess'))
}

// 处理窗口大小变化
const handleResize = () => {
  topologyChart?.resize()
}

onMounted(async () => {
  await nextTick()
  initTopology()
  window.addEventListener('resize', handleResize)
})

onUnmounted(() => {
  topologyChart?.dispose()
  window.removeEventListener('resize', handleResize)
})
</script>

<style lang="scss" scoped>
@import '@/styles/design-tokens.scss';

.topology-container {
  height: calc(100vh - 120px);
  display: flex;
  flex-direction: column;
}

// Apple 风格页面头部
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--space-6);
  flex-shrink: 0;
  
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
    align-items: center;
    
    .el-checkbox {
      background: var(--color-background-elevated);
      padding: var(--space-2) var(--space-3);
      border-radius: var(--radius-lg);
      border: 1px solid var(--color-border-light);
      transition: all var(--duration-fast) var(--ease-in-out);
      
      :deep(.el-checkbox__label) {
        color: var(--color-text-secondary);
        font-weight: var(--font-weight-medium);
      }
      
      &:hover {
        border-color: var(--color-border);
      }
    }
    
    .el-button-group {
      .el-button {
        background: var(--color-background-elevated);
        border: 1px solid var(--color-border-light);
        color: var(--color-text-secondary);
        font-weight: var(--font-weight-medium);
        
        &:first-child {
          border-radius: var(--radius-lg) 0 0 var(--radius-lg);
        }
        
        &:last-child {
          border-radius: 0 var(--radius-lg) var(--radius-lg) 0;
        }
        
        &:hover {
          background: var(--color-background-hover);
          color: var(--color-text-primary);
          border-color: var(--color-border);
          z-index: 1;
        }
        
        &:active {
          background: var(--color-background-active);
        }
      }
    }
    
    > .el-button {
      background: var(--color-primary);
      border-color: var(--color-primary);
      color: var(--color-text-inverse);
      border-radius: var(--radius-lg);
      font-weight: var(--font-weight-medium);
      transition: all var(--duration-fast) var(--ease-in-out);
      
      &:hover {
        background: var(--color-primary-hover);
        border-color: var(--color-primary-hover);
        transform: translateY(-1px);
        box-shadow: var(--shadow-md);
      }
    }
  }
}

// Tesla 风格拓扑图卡片
.topology-card {
  height: calc(100vh - 200px);
  position: relative;
  background: var(--color-background-elevated);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-sm);
  overflow: hidden;
  
  :deep(.el-card__body) {
    height: 100%;
    padding: 0;
    position: relative;
  }
  
  .topology-graph {
    height: 100%;
    width: 100%;
    background: var(--color-background);
  }
  
  .topology-legend {
    position: absolute;
    bottom: var(--space-5);
    left: var(--space-5);
    background: var(--color-background-elevated);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    border: 1px solid var(--color-border-light);
    border-radius: var(--radius-lg);
    padding: var(--space-3) var(--space-4);
    display: flex;
    gap: var(--space-5);
    flex-wrap: wrap;
    box-shadow: var(--shadow-md);
    
    .legend-item {
      display: flex;
      align-items: center;
      gap: var(--space-2);
      font-size: var(--font-size-sm);
      color: var(--color-text-secondary);
      font-weight: var(--font-weight-medium);
      
      .legend-icon {
        width: 20px;
        height: 20px;
        border-radius: var(--radius-sm);
        box-shadow: var(--shadow-xs);
        
        &.service {
          background: var(--color-primary);
        }
        
        &.device-online {
          background: var(--color-success);
          border-radius: 50%;
        }
        
        &.device-offline {
          background: var(--color-text-quaternary);
          border-radius: 50%;
        }
        
        &.device-warning {
          background: var(--color-warning);
          border-radius: 50%;
        }
      }
      
      .legend-line {
        width: 30px;
        height: 2px;
        position: relative;
        
        &.normal {
          background: var(--color-success);
        }
        
        &.warning {
          background: var(--color-warning);
          background-image: repeating-linear-gradient(
            90deg,
            transparent,
            transparent 5px,
            var(--color-warning) 5px,
            var(--color-warning) 10px
          );
        }
        
        &.error {
          background: var(--color-danger);
          background-image: repeating-linear-gradient(
            90deg,
            transparent,
            transparent 5px,
            var(--color-danger) 5px,
            var(--color-danger) 10px
          );
        }
        
        &::after {
          content: '';
          position: absolute;
          right: -5px;
          top: -4px;
          width: 0;
          height: 0;
          border-left: 5px solid currentColor;
          border-top: 5px solid transparent;
          border-bottom: 5px solid transparent;
        }
      }
    }
  }
}

// 信息卡片
.info-card {
  margin-bottom: var(--space-5);
  background: var(--color-background-elevated);
  border: 1px solid var(--color-border-light);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-xs);
  transition: all var(--duration-normal) var(--ease-in-out);
  
  &:hover {
    box-shadow: var(--shadow-md);
  }
  
  &:last-child {
    margin-bottom: 0;
  }
  
  :deep(.el-card__header) {
    padding: var(--space-4) var(--space-5);
    border-bottom: 1px solid var(--color-border-light);
    background: var(--color-background-secondary);
    
    .el-card__header {
      font-size: var(--font-size-base);
      font-weight: var(--font-weight-semibold);
      color: var(--color-text-primary);
    }
  }
  
  :deep(.el-card__body) {
    padding: var(--space-5);
  }
  
  .stat-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: var(--space-3);
    padding: var(--space-2) 0;
    
    &:last-child {
      margin-bottom: 0;
    }
    
    .stat-label {
      color: var(--color-text-tertiary);
      font-size: var(--font-size-sm);
      font-weight: var(--font-weight-medium);
    }
    
    .stat-value {
      font-size: var(--font-size-lg);
      font-weight: var(--font-weight-semibold);
      
      &.success {
        color: var(--color-success);
      }
      
      &.danger {
        color: var(--color-danger);
      }
      
      &.warning {
        color: var(--color-warning);
      }
    }
  }
  
  .node-actions {
    margin-top: var(--space-4);
    display: flex;
    gap: var(--space-2);
    
    .el-button {
      flex: 1;
      height: 36px;
      border-radius: var(--radius-lg);
      font-weight: var(--font-weight-medium);
    }
  }
  
  :deep(.el-descriptions) {
    .el-descriptions__label {
      color: var(--color-text-tertiary);
      font-weight: var(--font-weight-medium);
    }
    
    .el-descriptions__content {
      color: var(--color-text-primary);
      font-weight: var(--font-weight-medium);
    }
  }
}

// 响应式布局
@media (max-width: 1200px) {
  .el-row {
    flex-direction: column;
    
    .el-col {
      max-width: 100% !important;
      width: 100% !important;
    }
  }
  
  .topology-card {
    height: 500px;
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
    
    .header-actions {
      width: 100%;
      flex-wrap: wrap;
    }
  }
}
</style>