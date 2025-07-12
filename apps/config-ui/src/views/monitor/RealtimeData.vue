<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed, watch } from 'vue';
import { ElMessage } from 'element-plus';
import { 
  Refresh, 
  Search, 
  Download, 
  FullScreen,
  CircleCheck,
  CircleClose
} from '@element-plus/icons-vue';

// 模拟实时数据
const realtimeData = ref([
  { id: 1, name: '主变压器A相电压', value: 380.5, unit: 'V', quality: 'good', timestamp: new Date(), trend: 'up', channelId: 1 },
  { id: 2, name: '主变压器B相电压', value: 381.2, unit: 'V', quality: 'good', timestamp: new Date(), trend: 'stable', channelId: 1 },
  { id: 3, name: '主变压器C相电压', value: 379.8, unit: 'V', quality: 'good', timestamp: new Date(), trend: 'down', channelId: 1 },
  { id: 4, name: '总有功功率', value: 1250.5, unit: 'kW', quality: 'good', timestamp: new Date(), trend: 'up', channelId: 2 },
  { id: 5, name: '总无功功率', value: 450.2, unit: 'kVar', quality: 'good', timestamp: new Date(), trend: 'stable', channelId: 2 },
  { id: 6, name: '功率因数', value: 0.95, unit: '', quality: 'good', timestamp: new Date(), trend: 'stable', channelId: 2 },
  { id: 7, name: '环境温度', value: 25.3, unit: '°C', quality: 'good', timestamp: new Date(), trend: 'up', channelId: 3 },
  { id: 8, name: '环境湿度', value: 65, unit: '%', quality: 'good', timestamp: new Date(), trend: 'stable', channelId: 3 },
  { id: 9, name: '开关状态', value: 1, unit: '', quality: 'good', timestamp: new Date(), trend: 'stable', channelId: 4, type: 'bool' },
  { id: 10, name: '告警状态', value: 0, unit: '', quality: 'bad', timestamp: new Date(), trend: 'stable', channelId: 4, type: 'bool' },
]);

// 通道列表
const channels = ref([
  { id: 0, name: '全部通道' },
  { id: 1, name: 'Modbus主站' },
  { id: 2, name: 'IEC104通道' },
  { id: 3, name: '环境监测' },
  { id: 4, name: '开关量采集' },
]);

// 搜索和筛选
const searchQuery = ref('');
const selectedChannel = ref(0);
const refreshInterval = ref(1000);
const autoRefresh = ref(true);
const showOnlyAbnormal = ref(false);

// 刷新定时器
let refreshTimer: any = null;

// 过滤后的数据
const filteredData = computed(() => {
  let data = realtimeData.value;
  
  // 按通道筛选
  if (selectedChannel.value > 0) {
    data = data.filter(item => item.channelId === selectedChannel.value);
  }
  
  // 按搜索词筛选
  if (searchQuery.value) {
    data = data.filter(item => 
      item.name.toLowerCase().includes(searchQuery.value.toLowerCase())
    );
  }
  
  // 只显示异常数据
  if (showOnlyAbnormal.value) {
    data = data.filter(item => item.quality !== 'good');
  }
  
  return data;
});

// 获取质量状态颜色
function getQualityColor(quality: string) {
  switch (quality) {
    case 'good': return 'var(--success-color)';
    case 'bad': return 'var(--danger-color)';
    case 'uncertain': return 'var(--warning-color)';
    default: return 'var(--text-muted)';
  }
}

// 获取趋势图标
function getTrendIcon(trend: string) {
  switch (trend) {
    case 'up': return '↑';
    case 'down': return '↓';
    default: return '→';
  }
}

// 获取趋势颜色
function getTrendColor(trend: string) {
  switch (trend) {
    case 'up': return 'var(--danger-color)';
    case 'down': return 'var(--success-color)';
    default: return 'var(--info-color)';
  }
}

// 格式化值显示
function formatValue(item: any) {
  if (item.type === 'bool') {
    return item.value === 1 ? '开' : '关';
  }
  return item.value.toFixed(2);
}

// 数据更新动画状态
const updatingItems = ref(new Set<number>());

// 刷新数据
function refreshData() {
  // 模拟数据更新
  realtimeData.value.forEach(item => {
    const oldValue = item.value;
    
    // 随机更新数值
    if (item.type !== 'bool') {
      const change = (Math.random() - 0.5) * 2;
      item.value += change;
      
      // 更新趋势
      if (change > 0.5) item.trend = 'up';
      else if (change < -0.5) item.trend = 'down';
      else item.trend = 'stable';
      
      // 记录变化
      if (Math.abs(change) > 0.1) {
        updatingItems.value.add(item.id);
        
        // 300ms 后移除动画状态
        setTimeout(() => {
          updatingItems.value.delete(item.id);
        }, 300);
      }
    } else {
      // 布尔值随机切换
      if (Math.random() > 0.95) {
        item.value = item.value === 1 ? 0 : 1;
        updatingItems.value.add(item.id);
        
        setTimeout(() => {
          updatingItems.value.delete(item.id);
        }, 500);
      }
    }
    
    // 更新时间戳
    item.timestamp = new Date();
    
    // 随机设置质量
    const rand = Math.random();
    const oldQuality = item.quality;
    if (rand > 0.95) item.quality = 'bad';
    else if (rand > 0.9) item.quality = 'uncertain';
    else item.quality = 'good';
    
    // 质量变化时添加动画
    if (oldQuality !== item.quality) {
      updatingItems.value.add(item.id);
      setTimeout(() => {
        updatingItems.value.delete(item.id);
      }, 1000);
    }
  });
}

// 启动自动刷新
function startAutoRefresh() {
  if (refreshTimer) clearInterval(refreshTimer);
  if (autoRefresh.value) {
    refreshTimer = setInterval(refreshData, refreshInterval.value);
  }
}

// 手动刷新
function manualRefresh() {
  refreshData();
  ElMessage.success('数据已刷新');
}

// 导出数据
function exportData() {
  ElMessage.info('导出功能开发中...');
}

// 全屏显示
function toggleFullscreen() {
  ElMessage.info('全屏功能开发中...');
}

onMounted(() => {
  startAutoRefresh();
});

onUnmounted(() => {
  if (refreshTimer) clearInterval(refreshTimer);
});

// 监听自动刷新和间隔变化
watch([autoRefresh, refreshInterval], () => {
  startAutoRefresh();
});
</script>

<template>
  <div class="realtime-data">
    <!-- 工具栏 -->
    <div class="toolbar">
      <div class="toolbar-left">
        <el-select v-model="selectedChannel" placeholder="选择通道" style="width: 200px">
          <el-option
            v-for="channel in channels"
            :key="channel.id"
            :label="channel.name"
            :value="channel.id"
          />
        </el-select>
        
        <el-input
          v-model="searchQuery"
          placeholder="搜索数据点..."
          :prefix-icon="Search"
          style="width: 300px"
          clearable
        />
        
        <el-checkbox v-model="showOnlyAbnormal">只显示异常</el-checkbox>
      </div>
      
      <div class="toolbar-right">
        <el-checkbox v-model="autoRefresh">自动刷新</el-checkbox>
        
        <el-select v-model="refreshInterval" :disabled="!autoRefresh" style="width: 100px">
          <el-option :value="500" label="0.5秒" />
          <el-option :value="1000" label="1秒" />
          <el-option :value="2000" label="2秒" />
          <el-option :value="5000" label="5秒" />
        </el-select>
        
        <el-button :icon="Refresh" @click="manualRefresh">刷新</el-button>
        <el-button :icon="Download" @click="exportData">导出</el-button>
        <el-button :icon="FullScreen" @click="toggleFullscreen">全屏</el-button>
      </div>
    </div>
    
    <!-- 数据统计 -->
    <div class="stats-bar">
      <div class="stat-item">
        <div class="stat-value">{{ filteredData.length }}</div>
        <div class="stat-label">数据点总数</div>
      </div>
      <div class="stat-divider"></div>
      <div class="stat-item">
        <el-icon style="color: #67C23A; font-size: 24px; margin-bottom: 4px;"><CircleCheck /></el-icon>
        <div class="stat-value">{{ filteredData.filter(d => d.quality === 'good').length }}</div>
        <div class="stat-label">正常数据</div>
      </div>
      <div class="stat-divider"></div>
      <div class="stat-item">
        <el-icon style="color: #F56C6C; font-size: 24px; margin-bottom: 4px;"><CircleClose /></el-icon>
        <div class="stat-value">{{ filteredData.filter(d => d.quality === 'bad').length }}</div>
        <div class="stat-label">异常数据</div>
      </div>
      <div class="stat-divider"></div>
      <div class="stat-item">
        <div class="stat-value">{{ refreshInterval / 1000 }}</div>
        <div class="stat-label">刷新间隔(秒)</div>
      </div>
    </div>
    
    <!-- 实时数据表格 -->
    <el-table 
      :data="filteredData" 
      style="width: 100%"
      height="calc(100vh - 350px)"
      :row-class-name="(row) => row.row.quality === 'bad' ? 'error-row' : ''"
    >
        <el-table-column prop="name" label="数据点名称" min-width="200" />
        
        <el-table-column label="实时值" width="150">
          <template #default="{ row }">
            <div class="value-cell" :class="{ 
              'updating': updatingItems.has(row.id)
            }">
              <span class="value">{{ formatValue(row) }}</span>
              <span class="unit" v-if="row.unit">{{ row.unit }}</span>
              <span 
                class="trend" 
                :style="{ color: getTrendColor(row.trend) }"
                v-if="row.type !== 'bool'"
              >
                {{ getTrendIcon(row.trend) }}
              </span>
            </div>
          </template>
        </el-table-column>
        
        <el-table-column label="质量" width="100">
          <template #default="{ row }">
            <el-tag 
              :color="getQualityColor(row.quality)" 
              effect="dark"
              size="small"
            >
              {{ row.quality === 'good' ? '正常' : row.quality === 'bad' ? '异常' : '不确定' }}
            </el-tag>
          </template>
        </el-table-column>
        
        <el-table-column label="更新时间" width="180">
          <template #default="{ row }">
            {{ row.timestamp.toLocaleTimeString() }}
          </template>
        </el-table-column>
        
        <el-table-column label="所属通道" width="150">
          <template #default="{ row }">
            {{ channels.find(c => c.id === row.channelId)?.name }}
          </template>
        </el-table-column>
    </el-table>
  </div>
</template>

<style lang="scss" scoped>
.realtime-data {
  padding: 0;
  
  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px 32px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-light);
    gap: 16px;
    
    .toolbar-left,
    .toolbar-right {
      display: flex;
      align-items: center;
      gap: 16px;
    }
  }
  
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
    
    // 移除shimmer动画，保持静态
    
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
          text-shadow: 0 0 30px currentColor;
        }
      }
      
      .stat-value {
        font-size: 32px;
        font-weight: 700;
        color: var(--primary-color);
        line-height: 1;
        transition: all 0.3s ease;
        
        // 数字变化动画
        &.changing {
          animation: pulse 0.3s ease;
        }
      }
      
      .stat-label {
        font-size: 14px;
        color: var(--text-secondary);
        text-transform: uppercase;
        letter-spacing: 1px;
        font-weight: 500;
      }
      
      // 状态指示器颜色
      &:nth-child(3) .stat-value {
        color: var(--success-color);
      }
      
      &:nth-child(5) .stat-value {
        color: var(--danger-color);
      }
    }
    
    .stat-divider {
      width: 1px;
      height: 40px;
      background: linear-gradient(180deg, transparent 0%, var(--primary-color) 50%, transparent 100%);
      opacity: 0.3;
    }
  }
  
  
  .value-cell {
    display: flex;
    align-items: center;
    gap: 8px;
    
    .value {
      font-weight: 700;
      font-size: 16px;
      color: var(--accent-cyan);
      text-shadow: 0 0 10px rgba(0, 212, 255, 0.5);
      transition: all 0.3s ease;
      
      &:hover {
        text-shadow: 0 0 20px rgba(0, 212, 255, 0.8);
      }
    }
    
    .unit {
      color: var(--text-secondary);
      font-size: 14px;
      opacity: 0.8;
    }
    
    .trend {
      font-weight: bold;
      font-size: 16px;
      animation: pulse 2s ease-in-out infinite;
    }
    
    // 数据更新动画 - 只有缩放，没有颜色变化
    &.updating {
      .value {
        animation: valueUpdate 0.3s ease;
      }
    }
  }
  
  @keyframes valueUpdate {
    0% { transform: scale(1); }
    50% { transform: scale(1.2); filter: brightness(1.5); }
    100% { transform: scale(1); }
  }
  
  @keyframes valueUp {
    0% { transform: translateY(0); }
    50% { transform: translateY(-5px); }
    100% { transform: translateY(0); }
  }
  
  @keyframes valueDown {
    0% { transform: translateY(0); }
    50% { transform: translateY(5px); }
    100% { transform: translateY(0); }
  }
  
  :deep(.el-table) {
    border: none;
    font-size: 14px;
    background: transparent;
    
    th {
      background: var(--glass-bg);
      backdrop-filter: var(--glass-blur);
      font-weight: 600;
      color: var(--text-primary);
      border-bottom: 1px solid var(--border-light);
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }
    
    td {
      border-bottom: 1px solid var(--border-light);
      color: var(--text-primary);
    }
    
    .el-table__row {
      transition: all 0.3s ease;
      
      &:hover {
        background: rgba(98, 106, 239, 0.1);
        box-shadow: 0 0 20px rgba(98, 106, 239, 0.2);
        
        td {
          color: var(--text-primary);
        }
      }
    }
    
    &.error-row {
      background: rgba(255, 56, 56, 0.1);
      
      td {
        color: var(--danger-color);
      }
    }
  }
}
</style>