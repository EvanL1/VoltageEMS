<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { ElMessage, ElMessageBox } from 'element-plus';
import { 
  Bell,
  CircleCheck,
  Warning,
  WarningFilled,
  InfoFilled,
  Refresh,
  View,
  Check,
  Close,
  Document
} from '@element-plus/icons-vue';

// 告警数据
const alarms = ref([
  {
    id: 1,
    level: 'critical',
    title: '主变压器A相电压超限',
    description: '主变压器A相电压达到 420V，超过设定上限 400V',
    source: 'Modbus主站',
    point: '主变压器A相电压',
    value: 420,
    limit: 400,
    status: 'active',
    occurTime: new Date(Date.now() - 300000),
    ackTime: null,
    ackUser: null,
    duration: 300
  },
  {
    id: 2,
    level: 'warning',
    title: '环境温度偏高',
    description: '配电室环境温度达到 35°C，接近告警阈值',
    source: '环境监测',
    point: '环境温度',
    value: 35,
    limit: 38,
    status: 'active',
    occurTime: new Date(Date.now() - 600000),
    ackTime: null,
    ackUser: null,
    duration: 600
  },
  {
    id: 3,
    level: 'info',
    title: 'CAN总线通信中断',
    description: 'CAN总线通道失去连接',
    source: 'CAN总线',
    point: '通道状态',
    value: 0,
    limit: 1,
    status: 'acknowledged',
    occurTime: new Date(Date.now() - 1800000),
    ackTime: new Date(Date.now() - 1200000),
    ackUser: 'operator',
    duration: 600
  },
  {
    id: 4,
    level: 'warning',
    title: '功率因数偏低',
    description: '总功率因数降至 0.85，低于设定值 0.90',
    source: 'IEC104通道',
    point: '功率因数',
    value: 0.85,
    limit: 0.90,
    status: 'cleared',
    occurTime: new Date(Date.now() - 3600000),
    ackTime: new Date(Date.now() - 3000000),
    ackUser: 'admin',
    clearTime: new Date(Date.now() - 1800000),
    duration: 1800
  }
]);

// 筛选条件
const filterLevel = ref('all');
const filterStatus = ref('all');
const filterSource = ref('all');
const autoRefresh = ref(true);

// 告警级别定义
const alarmLevels = [
  { value: 'all', label: '全部级别' },
  { value: 'critical', label: '严重', color: '#F56C6C' },
  { value: 'warning', label: '警告', color: '#E6A23C' },
  { value: 'info', label: '信息', color: '#909399' }
];

// 告警状态定义
const alarmStatuses = [
  { value: 'all', label: '全部状态' },
  { value: 'active', label: '活动' },
  { value: 'acknowledged', label: '已确认' },
  { value: 'cleared', label: '已清除' }
];

// 告警源列表
const alarmSources = computed(() => {
  const sources = [...new Set(alarms.value.map(a => a.source))];
  return [
    { value: 'all', label: '全部来源' },
    ...sources.map(s => ({ value: s, label: s }))
  ];
});

// 刷新定时器
let refreshTimer: any = null;

// 过滤后的告警
const filteredAlarms = computed(() => {
  let data = alarms.value;
  
  if (filterLevel.value !== 'all') {
    data = data.filter(a => a.level === filterLevel.value);
  }
  
  if (filterStatus.value !== 'all') {
    data = data.filter(a => a.status === filterStatus.value);
  }
  
  if (filterSource.value !== 'all') {
    data = data.filter(a => a.source === filterSource.value);
  }
  
  return data.sort((a, b) => b.occurTime.getTime() - a.occurTime.getTime());
});

// 统计信息
const statistics = computed(() => {
  const total = alarms.value.length;
  const active = alarms.value.filter(a => a.status === 'active').length;
  const critical = alarms.value.filter(a => a.level === 'critical' && a.status === 'active').length;
  const warning = alarms.value.filter(a => a.level === 'warning' && a.status === 'active').length;
  const info = alarms.value.filter(a => a.level === 'info' && a.status === 'active').length;
  
  return { total, active, critical, warning, info };
});

// 获取级别颜色
function getLevelColor(level: string) {
  switch (level) {
    case 'critical': return 'var(--danger-color)';
    case 'warning': return 'var(--warning-color)';
    case 'info': return 'var(--info-color)';
    default: return 'var(--text-muted)';
  }
}

// 获取级别图标
function getLevelIcon(level: string) {
  switch (level) {
    case 'critical': return WarningFilled;
    case 'warning': return Warning;
    case 'info': return InfoFilled;
    default: return Bell;
  }
}

// 获取状态标签
function getStatusTag(status: string) {
  switch (status) {
    case 'active': return { text: '活动', type: 'danger' };
    case 'acknowledged': return { text: '已确认', type: 'warning' };
    case 'cleared': return { text: '已清除', type: 'success' };
    default: return { text: '未知', type: 'info' };
  }
}

// 格式化持续时间
function formatDuration(seconds: number) {
  if (seconds < 60) return `${seconds}秒`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}分钟`;
  return `${Math.floor(seconds / 3600)}小时${Math.floor((seconds % 3600) / 60)}分钟`;
}

// 确认告警
function acknowledgeAlarm(alarm: any) {
  if (alarm.status !== 'active') {
    ElMessage.warning('该告警已被处理');
    return;
  }
  
  ElMessageBox.confirm(
    `确认告警：${alarm.title}？`,
    '确认操作',
    {
      confirmButtonText: '确认',
      cancelButtonText: '取消',
      type: 'warning',
    }
  ).then(() => {
    alarm.status = 'acknowledged';
    alarm.ackTime = new Date();
    alarm.ackUser = 'admin';
    ElMessage.success('告警已确认');
  });
}

// 清除告警
function clearAlarm(alarm: any) {
  if (alarm.status === 'cleared') {
    ElMessage.warning('该告警已清除');
    return;
  }
  
  ElMessageBox.confirm(
    `清除告警：${alarm.title}？`,
    '清除操作',
    {
      confirmButtonText: '清除',
      cancelButtonText: '取消',
      type: 'warning',
    }
  ).then(() => {
    alarm.status = 'cleared';
    alarm.clearTime = new Date();
    ElMessage.success('告警已清除');
  });
}

// 查看告警详情
function viewAlarmDetail(alarm: any) {
  ElMessage.info(`查看告警详情：${alarm.title}`);
}

// 刷新数据
function refreshData() {
  // 模拟新告警
  if (Math.random() > 0.8) {
    const newAlarm = {
      id: Date.now(),
      level: ['critical', 'warning', 'info'][Math.floor(Math.random() * 3)],
      title: '新告警事件',
      description: '这是一个模拟的新告警',
      source: alarmSources.value[Math.floor(Math.random() * (alarmSources.value.length - 1)) + 1].label,
      point: '测试点',
      value: Math.random() * 100,
      limit: 50,
      status: 'active',
      occurTime: new Date(),
      ackTime: null,
      ackUser: null,
      duration: 0
    };
    alarms.value.unshift(newAlarm);
  }
  
  // 更新持续时间
  alarms.value.forEach(alarm => {
    if (alarm.status === 'active' || alarm.status === 'acknowledged') {
      alarm.duration = Math.floor((Date.now() - alarm.occurTime.getTime()) / 1000);
    }
  });
}

// 手动刷新
function manualRefresh() {
  refreshData();
  ElMessage.success('告警数据已刷新');
}

// 批量确认
function batchAcknowledge() {
  const activeAlarms = filteredAlarms.value.filter(a => a.status === 'active');
  if (activeAlarms.length === 0) {
    ElMessage.warning('没有需要确认的告警');
    return;
  }
  
  ElMessageBox.confirm(
    `确认所有活动告警（${activeAlarms.length}个）？`,
    '批量确认',
    {
      confirmButtonText: '确认',
      cancelButtonText: '取消',
      type: 'warning',
    }
  ).then(() => {
    activeAlarms.forEach(alarm => {
      alarm.status = 'acknowledged';
      alarm.ackTime = new Date();
      alarm.ackUser = 'admin';
    });
    ElMessage.success(`已确认 ${activeAlarms.length} 个告警`);
  });
}

// 导出告警记录
function exportAlarms() {
  ElMessage.info('导出功能开发中...');
}

// 启动自动刷新
function startAutoRefresh() {
  if (refreshTimer) clearInterval(refreshTimer);
  if (autoRefresh.value) {
    refreshTimer = setInterval(refreshData, 5000);
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
  <div class="alarms-monitor">
    <!-- 统计栏 -->
    <div class="stats-bar">
      <div class="stat-item">
        <div class="stat-value">{{ statistics.total }}</div>
        <div class="stat-label">告警总数</div>
      </div>
      <div class="stat-divider"></div>
      <div class="stat-item">
        <el-icon style="color: #E6A23C; font-size: 24px; margin-bottom: 4px;"><Bell /></el-icon>
        <div class="stat-value">{{ statistics.active }}</div>
        <div class="stat-label">活动告警</div>
      </div>
      <div class="stat-divider"></div>
      <div class="stat-item">
        <el-icon style="color: #F56C6C; font-size: 24px; margin-bottom: 4px;"><WarningFilled /></el-icon>
        <div class="stat-value">{{ statistics.critical }}</div>
        <div class="stat-label">严重告警</div>
      </div>
      <div class="stat-divider"></div>
      <div class="stat-item">
        <el-icon style="color: #E6A23C; font-size: 24px; margin-bottom: 4px;"><Warning /></el-icon>
        <div class="stat-value">{{ statistics.warning }}</div>
        <div class="stat-label">警告告警</div>
      </div>
    </div>
    
    <!-- 工具栏 -->
    <div class="toolbar">
      <div class="toolbar-left">
        <el-select v-model="filterLevel" placeholder="告警级别" style="width: 120px">
          <el-option
            v-for="level in alarmLevels"
            :key="level.value"
            :label="level.label"
            :value="level.value"
          />
        </el-select>
        
        <el-select v-model="filterStatus" placeholder="告警状态" style="width: 120px">
          <el-option
            v-for="status in alarmStatuses"
            :key="status.value"
            :label="status.label"
            :value="status.value"
          />
        </el-select>
        
        <el-select v-model="filterSource" placeholder="告警源" style="width: 150px">
          <el-option
            v-for="source in alarmSources"
            :key="source.value"
            :label="source.label"
            :value="source.value"
          />
        </el-select>
        
        <el-checkbox v-model="autoRefresh">自动刷新</el-checkbox>
      </div>
      
      <div class="toolbar-right">
        <el-button @click="batchAcknowledge">批量确认</el-button>
        <el-button :icon="Refresh" @click="manualRefresh">刷新</el-button>
        <el-button :icon="Document" @click="exportAlarms">导出</el-button>
      </div>
    </div>
    
    <!-- 告警列表 -->
    <el-table :data="filteredAlarms" style="width: 100%" height="calc(100vh - 350px)">
        <el-table-column width="50">
          <template #default="{ row }">
            <el-icon :color="getLevelColor(row.level)" :size="20">
              <component :is="getLevelIcon(row.level)" />
            </el-icon>
          </template>
        </el-table-column>
        
        <el-table-column label="告警标题" min-width="250">
          <template #default="{ row }">
            <div class="alarm-title">
              <span>{{ row.title }}</span>
              <el-tag 
                v-if="row.status === 'active' && row.duration < 300" 
                type="danger" 
                size="small" 
                effect="dark"
              >
                新
              </el-tag>
            </div>
          </template>
        </el-table-column>
        
        <el-table-column prop="source" label="告警源" width="120" />
        
        <el-table-column label="级别" width="80">
          <template #default="{ row }">
            <el-tag :color="getLevelColor(row.level)" effect="dark" size="small">
              {{ alarmLevels.find(l => l.value === row.level)?.label }}
            </el-tag>
          </template>
        </el-table-column>
        
        <el-table-column label="状态" width="100">
          <template #default="{ row }">
            <el-tag :type="getStatusTag(row.status).type" size="small">
              {{ getStatusTag(row.status).text }}
            </el-tag>
          </template>
        </el-table-column>
        
        <el-table-column label="发生时间" width="180">
          <template #default="{ row }">
            {{ row.occurTime.toLocaleString() }}
          </template>
        </el-table-column>
        
        <el-table-column label="持续时间" width="120">
          <template #default="{ row }">
            {{ formatDuration(row.duration) }}
          </template>
        </el-table-column>
        
        <el-table-column label="操作" width="200" fixed="right">
          <template #default="{ row }">
            <el-button 
              v-if="row.status === 'active'" 
              type="primary" 
              size="small" 
              :icon="Check"
              @click="acknowledgeAlarm(row)"
            >
              确认
            </el-button>
            <el-button 
              v-if="row.status !== 'cleared'" 
              type="success" 
              size="small" 
              :icon="CircleCheck"
              @click="clearAlarm(row)"
            >
              清除
            </el-button>
            <el-button 
              size="small" 
              :icon="View"
              @click="viewAlarmDetail(row)"
            >
              详情
            </el-button>
          </template>
        </el-table-column>
    </el-table>
  </div>
</template>

<style lang="scss" scoped>
.alarms-monitor {
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
    
    // 告警闪烁效果
    &::before {
      content: '';
      position: absolute;
      top: 0;
      left: 0;
      right: 0;
      bottom: 0;
      background: radial-gradient(circle at var(--x, 50%) var(--y, 50%), rgba(255, 56, 56, 0.2) 0%, transparent 50%);
      opacity: 0;
      animation: alarm-pulse 2s ease-in-out infinite;
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
          filter: brightness(1.3);
        }
      }
      
      .stat-value {
        font-size: 32px;
        font-weight: 700;
        line-height: 1;
        transition: all 0.3s ease;
      }
      
      // 告警级别颜色
      &:nth-child(1) .stat-value {
        color: var(--text-primary);
      }
      
      &:nth-child(3) .stat-value {
        color: var(--warning-color);
        text-shadow: 0 0 20px var(--warning-color);
        animation: blink 2s ease-in-out infinite;
      }
      
      &:nth-child(5) .stat-value {
        color: var(--danger-color);
        text-shadow: 0 0 20px var(--danger-color);
        animation: blink 1.5s ease-in-out infinite;
      }
      
      &:nth-child(7) .stat-value {
        color: var(--warning-color);
        text-shadow: 0 0 20px var(--warning-color);
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
      background: linear-gradient(180deg, transparent 0%, var(--danger-color) 50%, transparent 100%);
      opacity: 0.3;
    }
  }
  
  @keyframes alarm-pulse {
    0%, 100% { opacity: 0; }
    50% { opacity: 1; }
  }
  
  @keyframes blink {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.6; }
  }
  
  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px 32px;
    background: #fff;
    border-bottom: 1px solid #e4e7ed;
    gap: 16px;
    
    .toolbar-left,
    .toolbar-right {
      display: flex;
      align-items: center;
      gap: 16px;
    }
  }
  
  .alarm-title {
    display: flex;
    align-items: center;
    gap: 8px;
    
    span {
      color: var(--text-primary);
      font-weight: 500;
    }
    
    .el-tag {
      animation: pulse 1s ease-in-out infinite;
    }
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
        
        td {
          color: var(--text-primary);
        }
      }
      
      // 活动告警行闪烁
      &:has(.el-tag--danger) {
        animation: alarm-row-pulse 2s ease-in-out infinite;
      }
    }
  }
  
  @keyframes alarm-row-pulse {
    0%, 100% { background: transparent; }
    50% { background: rgba(255, 56, 56, 0.1); }
  }
}
</style>