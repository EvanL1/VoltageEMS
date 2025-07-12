<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { ElMessage } from 'element-plus';
import { 
  CircleCheck, 
  CircleClose, 
  Warning,
  Refresh,
  View,
  Setting
} from '@element-plus/icons-vue';

// 通道状态数据
const channelsStatus = ref([
  {
    id: 1,
    name: 'Modbus TCP 主站',
    protocol: 'Modbus TCP',
    status: 'online',
    address: '192.168.1.100:502',
    pointsTotal: 450,
    pointsOnline: 445,
    lastUpdate: new Date(),
    errorCount: 0,
    txBytes: 125678,
    rxBytes: 234567,
    txRate: 1024,
    rxRate: 2048,
    quality: 98.5
  },
  {
    id: 2,
    name: 'IEC 60870-5-104',
    protocol: 'IEC104',
    status: 'online',
    address: '192.168.1.101:2404',
    pointsTotal: 380,
    pointsOnline: 380,
    lastUpdate: new Date(),
    errorCount: 0,
    txBytes: 89012,
    rxBytes: 156789,
    txRate: 768,
    rxRate: 1536,
    quality: 100
  },
  {
    id: 3,
    name: 'Modbus RTU 从站',
    protocol: 'Modbus RTU',
    status: 'warning',
    address: 'COM3 (9600,8,N,1)',
    pointsTotal: 120,
    pointsOnline: 115,
    lastUpdate: new Date(),
    errorCount: 5,
    txBytes: 45678,
    rxBytes: 67890,
    txRate: 256,
    rxRate: 512,
    quality: 95.8
  },
  {
    id: 4,
    name: 'CAN总线',
    protocol: 'CAN',
    status: 'offline',
    address: 'CAN0 (250kbps)',
    pointsTotal: 200,
    pointsOnline: 0,
    lastUpdate: new Date(Date.now() - 300000),
    errorCount: 150,
    txBytes: 0,
    rxBytes: 0,
    txRate: 0,
    rxRate: 0,
    quality: 0
  }
]);

// 自动刷新
const autoRefresh = ref(true);
let refreshTimer: any = null;

// 统计信息
const statistics = computed(() => {
  const total = channelsStatus.value.length;
  const online = channelsStatus.value.filter(c => c.status === 'online').length;
  const warning = channelsStatus.value.filter(c => c.status === 'warning').length;
  const offline = channelsStatus.value.filter(c => c.status === 'offline').length;
  const totalPoints = channelsStatus.value.reduce((sum, c) => sum + c.pointsTotal, 0);
  const onlinePoints = channelsStatus.value.reduce((sum, c) => sum + c.pointsOnline, 0);
  
  return {
    total,
    online,
    warning,
    offline,
    totalPoints,
    onlinePoints,
    availability: total > 0 ? ((online + warning) / total * 100).toFixed(1) : 0
  };
});

// 获取状态颜色
function getStatusColor(status: string) {
  switch (status) {
    case 'online': return 'var(--success-color)';
    case 'warning': return 'var(--warning-color)';
    case 'offline': return 'var(--danger-color)';
    default: return 'var(--text-muted)';
  }
}

// 获取状态图标
function getStatusIcon(status: string) {
  switch (status) {
    case 'online': return CircleCheck;
    case 'warning': return Warning;
    case 'offline': return CircleClose;
    default: return CircleClose;
  }
}

// 获取状态文本
function getStatusText(status: string) {
  switch (status) {
    case 'online': return '在线';
    case 'warning': return '警告';
    case 'offline': return '离线';
    default: return '未知';
  }
}

// 格式化字节数
function formatBytes(bytes: number) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  return (bytes / 1024 / 1024).toFixed(1) + ' MB';
}

// 格式化速率
function formatRate(rate: number) {
  if (rate < 1024) return rate + ' B/s';
  if (rate < 1024 * 1024) return (rate / 1024).toFixed(1) + ' KB/s';
  return (rate / 1024 / 1024).toFixed(1) + ' MB/s';
}

// 刷新数据
function refreshData() {
  // 模拟数据更新
  channelsStatus.value.forEach(channel => {
    if (channel.status !== 'offline') {
      // 更新流量
      channel.txBytes += Math.floor(Math.random() * 1000);
      channel.rxBytes += Math.floor(Math.random() * 2000);
      channel.txRate = Math.floor(Math.random() * 2048);
      channel.rxRate = Math.floor(Math.random() * 4096);
      
      // 更新在线点数
      if (channel.status === 'online') {
        channel.pointsOnline = channel.pointsTotal - Math.floor(Math.random() * 5);
      }
      
      // 更新时间
      channel.lastUpdate = new Date();
      
      // 更新质量
      channel.quality = ((channel.pointsOnline / channel.pointsTotal) * 100).toFixed(1);
    }
  });
}

// 查看通道详情
function viewChannelDetail(channel: any) {
  ElMessage.info(`查看通道 ${channel.name} 详情`);
}

// 配置通道
function configureChannel(channel: any) {
  ElMessage.info(`配置通道 ${channel.name}`);
}

// 手动刷新
function manualRefresh() {
  refreshData();
  ElMessage.success('通道状态已更新');
}

// 启动自动刷新
function startAutoRefresh() {
  if (refreshTimer) clearInterval(refreshTimer);
  if (autoRefresh.value) {
    refreshTimer = setInterval(refreshData, 2000);
  }
}

onMounted(() => {
  startAutoRefresh();
});

onUnmounted(() => {
  if (refreshTimer) clearInterval(refreshTimer);
});

import { watch } from 'vue';
watch(autoRefresh, () => {
  startAutoRefresh();
});
</script>

<template>
  <div class="channels-status">
    <!-- 统计栏 -->
    <div class="stats-bar">
      <div class="stat-item">
        <div class="stat-value">{{ statistics.total }}</div>
        <div class="stat-label">通道总数</div>
      </div>
      <div class="stat-divider"></div>
      <div class="stat-item">
        <el-icon style="color: #67C23A; font-size: 24px; margin-bottom: 4px;"><CircleCheck /></el-icon>
        <div class="stat-value">{{ statistics.online }}</div>
        <div class="stat-label">在线通道</div>
      </div>
      <div class="stat-divider"></div>
      <div class="stat-item">
        <el-icon style="color: #E6A23C; font-size: 24px; margin-bottom: 4px;"><Warning /></el-icon>
        <div class="stat-value">{{ statistics.warning }}</div>
        <div class="stat-label">警告通道</div>
      </div>
      <div class="stat-divider"></div>
      <div class="stat-item">
        <el-icon style="color: #F56C6C; font-size: 24px; margin-bottom: 4px;"><CircleClose /></el-icon>
        <div class="stat-value">{{ statistics.offline }}</div>
        <div class="stat-label">离线通道</div>
      </div>
      <div class="stat-divider"></div>
      <div class="stat-item">
        <div class="stat-value">{{ statistics.onlinePoints }}/{{ statistics.totalPoints }}</div>
        <div class="stat-label">数据点</div>
      </div>
      <div class="stat-divider"></div>
      <div class="stat-item">
        <div class="stat-value">{{ statistics.availability }}%</div>
        <div class="stat-label">可用率</div>
      </div>
    </div>
    
    <!-- 工具栏 -->
    <div class="toolbar">
      <div class="toolbar-left">
        <el-checkbox v-model="autoRefresh">自动刷新</el-checkbox>
      </div>
      <div class="toolbar-right">
        <el-button :icon="Refresh" @click="manualRefresh">刷新</el-button>
      </div>
    </div>
    
    <!-- 通道状态列表 -->
    <el-row :gutter="20" class="channels-grid">
      <el-col v-for="channel in channelsStatus" :key="channel.id" :span="12">
        <el-card class="channel-card" :class="channel.status" shadow="hover">
          <template #header>
            <div class="card-header">
              <div class="channel-info">
                <el-icon :color="getStatusColor(channel.status)" :size="20">
                  <component :is="getStatusIcon(channel.status)" />
                </el-icon>
                <h4>{{ channel.name }}</h4>
                <el-tag size="small" effect="plain">{{ channel.protocol }}</el-tag>
              </div>
              <div class="channel-actions">
                <el-button :icon="View" circle size="small" @click="viewChannelDetail(channel)" />
                <el-button :icon="Setting" circle size="small" @click="configureChannel(channel)" />
              </div>
            </div>
          </template>
          
          <div class="channel-content">
            <div class="info-row">
              <span class="label">状态：</span>
              <el-tag :color="getStatusColor(channel.status)" effect="dark" size="small">
                {{ getStatusText(channel.status) }}
              </el-tag>
            </div>
            
            <div class="info-row">
              <span class="label">地址：</span>
              <span class="value">{{ channel.address }}</span>
            </div>
            
            <div class="info-row">
              <span class="label">数据点：</span>
              <span class="value">
                <span :class="{ 'error-text': channel.pointsOnline < channel.pointsTotal }">
                  {{ channel.pointsOnline }}
                </span> / {{ channel.pointsTotal }}
                <el-progress 
                  :percentage="channel.pointsTotal > 0 ? (channel.pointsOnline / channel.pointsTotal * 100) : 0" 
                  :stroke-width="6"
                  style="width: 100px; display: inline-block; margin-left: 10px"
                />
              </span>
            </div>
            
            <div class="info-row">
              <span class="label">通信质量：</span>
              <span class="value">
                {{ channel.quality }}%
                <el-progress 
                  :percentage="Number(channel.quality)" 
                  :stroke-width="6"
                  :color="Number(channel.quality) > 95 ? '#67C23A' : Number(channel.quality) > 90 ? '#E6A23C' : '#F56C6C'"
                  style="width: 100px; display: inline-block; margin-left: 10px"
                />
              </span>
            </div>
            
            <div class="traffic-info">
              <div class="traffic-item">
                <span class="traffic-label">发送：</span>
                <span class="traffic-value">{{ formatBytes(channel.txBytes) }}</span>
                <span class="traffic-rate">({{ formatRate(channel.txRate) }})</span>
              </div>
              <div class="traffic-item">
                <span class="traffic-label">接收：</span>
                <span class="traffic-value">{{ formatBytes(channel.rxBytes) }}</span>
                <span class="traffic-rate">({{ formatRate(channel.rxRate) }})</span>
              </div>
            </div>
            
            <div class="info-row">
              <span class="label">最后更新：</span>
              <span class="value">{{ channel.lastUpdate.toLocaleTimeString() }}</span>
            </div>
            
            <div class="info-row" v-if="channel.errorCount > 0">
              <span class="label">错误次数：</span>
              <span class="value error-text">{{ channel.errorCount }}</span>
            </div>
          </div>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<style lang="scss" scoped>
.channels-status {
  padding: 0;
  
  .stats-bar {
    display: flex;
    align-items: center;
    padding: 20px 32px;
    background: var(--glass-bg);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--glass-border);
    border-radius: 12px;
    margin: 20px 32px;
    gap: 32px;
    box-shadow: var(--shadow-md);
    position: relative;
    overflow: hidden;
    
    &::before {
      content: '';
      position: absolute;
      top: 0;
      left: -100%;
      width: 100%;
      height: 100%;
      background: linear-gradient(90deg, transparent 0%, rgba(98, 106, 239, 0.2) 50%, transparent 100%);
      animation: scan 3s linear infinite;
    }
    
    .stat-item {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 8px;
      position: relative;
      z-index: 1;
      transition: transform 0.3s ease;
      
      &:hover {
        transform: translateY(-2px) scale(1.05);
        
        .stat-value {
          filter: brightness(1.2);
        }
      }
      
      .stat-value {
        font-size: 32px;
        font-weight: 700;
        line-height: 1;
        transition: all 0.3s ease;
      }
      
      // 不同状态的颜色
      &:nth-child(1) .stat-value {
        color: var(--info-color);
      }
      
      &:nth-child(3) .stat-value {
        color: var(--success-color);
        text-shadow: 0 0 20px var(--success-color);
      }
      
      &:nth-child(5) .stat-value {
        color: var(--warning-color);
        text-shadow: 0 0 20px var(--warning-color);
      }
      
      &:nth-child(7) .stat-value {
        color: var(--danger-color);
        text-shadow: 0 0 20px var(--danger-color);
      }
      
      .stat-label {
        font-size: 14px;
        color: var(--text-secondary);
        text-transform: uppercase;
        letter-spacing: 1px;
        font-weight: 500;
      }
    }
    
    .stat-divider {
      width: 1px;
      height: 40px;
      background: linear-gradient(180deg, transparent 0%, var(--primary-color) 50%, transparent 100%);
      opacity: 0.3;
    }
  }
  
  @keyframes scan {
    from { left: -100%; }
    to { left: 200%; }
  }
  
  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px 32px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-light);
  }
  
  .channels-grid {
    padding: 32px;
  }
  
  .channel-card {
    margin-bottom: 20px;
    transition: all 0.3s;
    background: var(--glass-bg);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--glass-border);
    border-radius: 12px;
    overflow: hidden;
    position: relative;
    
    &::before {
      content: '';
      position: absolute;
      top: 0;
      left: 0;
      right: 0;
      height: 2px;
      background: var(--primary-gradient);
      transform: scaleX(0);
      transition: transform 0.3s ease;
    }
    
    &:hover {
      transform: translateY(-4px);
      box-shadow: 0 8px 32px rgba(98, 106, 239, 0.3);
      border-color: var(--primary-color);
      
      &::before {
        transform: scaleX(1);
      }
    }
    
    &.online {
      &::before {
        background: linear-gradient(90deg, var(--success-color) 0%, var(--accent-cyan) 100%);
      }
    }
    
    &.warning {
      &::before {
        background: linear-gradient(90deg, var(--warning-color) 0%, var(--accent-pink) 100%);
      }
    }
    
    &.offline {
      opacity: 0.7;
      
      &::before {
        background: linear-gradient(90deg, var(--danger-color) 0%, var(--accent-pink) 100%);
      }
      
      :deep(.el-card__header) {
        background: rgba(255, 56, 56, 0.1);
      }
    }
    
    &.warning {
      :deep(.el-card__header) {
        background: rgba(255, 149, 0, 0.1);
      }
    }
    
    .card-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      
      .channel-info {
        display: flex;
        align-items: center;
        gap: 12px;
        
        h4 {
          margin: 0;
          font-size: 16px;
          font-weight: 600;
          color: var(--text-primary);
          transition: all 0.3s ease;
        }
        
        .el-tag {
          background: var(--glass-bg);
          border-color: var(--primary-color);
          color: var(--primary-color);
        }
      }
      
      .channel-actions {
        display: flex;
        gap: 8px;
        
        .el-button {
          background: transparent;
          border-color: var(--border-light);
          
          &:hover {
            border-color: var(--primary-color);
            color: var(--primary-color);
          }
        }
      }
    }
    
    .channel-content {
      .info-row {
        display: flex;
        align-items: center;
        margin-bottom: 12px;
        
        .label {
          width: 80px;
          color: #909399;
          font-size: 14px;
        }
        
        .value {
          flex: 1;
          color: #303133;
          font-size: 14px;
          display: flex;
          align-items: center;
        }
        
        .error-text {
          color: #F56C6C;
          font-weight: 600;
        }
      }
      
      .traffic-info {
        display: flex;
        gap: 24px;
        margin: 12px 0;
        padding: 12px;
        background: var(--glass-bg);
        backdrop-filter: var(--glass-blur);
        border-radius: 8px;
        border: 1px solid var(--glass-border);
        position: relative;
        overflow: hidden;
        
        &::before {
          content: '';
          position: absolute;
          top: 0;
          left: 0;
          right: 0;
          bottom: 0;
          background: linear-gradient(45deg, rgba(0, 212, 255, 0.05) 0%, transparent 100%);
          animation: pulse 3s ease-in-out infinite;
        }
        
        .traffic-item {
          display: flex;
          align-items: center;
          gap: 8px;
          position: relative;
          z-index: 1;
          
          .traffic-label {
            color: var(--text-secondary);
            font-size: 13px;
          }
          
          .traffic-value {
            font-weight: 600;
            color: var(--accent-cyan);
            text-shadow: 0 0 10px rgba(0, 212, 255, 0.5);
          }
          
          .traffic-rate {
            color: var(--success-color);
            font-size: 12px;
          }
        }
      }
    }
  }
}
</style>