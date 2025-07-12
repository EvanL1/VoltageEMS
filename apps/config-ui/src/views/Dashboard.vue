<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import { ElMessage } from 'element-plus';
import { 
  CircleCheck, 
  Warning, 
  CircleClose,
  TrendCharts,
  Timer,
  Connection,
  Document,
  Bell,
  Setting,
  Refresh,
  WarningFilled,
  Clock,
  TopRight,
  BottomRight
} from '@element-plus/icons-vue';

const router = useRouter();

// 服务状态数据
const services = ref([
  { name: 'comsrv', displayName: '通信服务', status: 'running', uptime: '15d 3h 42m', cpu: 12.5, memory: 256 },
  { name: 'modsrv', displayName: '计算服务', status: 'running', uptime: '15d 3h 42m', cpu: 8.3, memory: 192 },
  { name: 'hissrv', displayName: '历史服务', status: 'running', uptime: '15d 3h 40m', cpu: 15.2, memory: 512 },
  { name: 'netsrv', displayName: '网络服务', status: 'stopped', uptime: '-', cpu: 0, memory: 0 },
  { name: 'alarmsrv', displayName: '告警服务', status: 'running', uptime: '15d 3h 42m', cpu: 5.1, memory: 128 },
  { name: 'rulesrv', displayName: '规则服务', status: 'error', uptime: '2h 15m', cpu: 0, memory: 0 },
]);

// 配置统计
const configStats = ref({
  channels: { total: 12, active: 10, error: 1 },
  points: { total: 1250, telemetry: 800, signal: 300, control: 100, adjustment: 50 },
  alarms: { total: 85, critical: 3, warning: 12, info: 70 },
  templates: 15
});

// 最近活动
const recentActivities = ref([
  { time: '10:15', type: 'config', action: '更新通道配置', target: 'Modbus TCP 主站', user: 'admin' },
  { time: '09:42', type: 'alarm', action: '新增告警规则', target: '温度超限告警', user: 'admin' },
  { time: '08:30', type: 'channel', action: '创建新通道', target: 'IEC104 备用通道', user: 'operator' },
  { time: '昨天 17:20', type: 'backup', action: '配置备份', target: '全量备份', user: 'system' },
  { time: '昨天 14:05', type: 'point', action: '导入点表', target: '新增150个测点', user: 'admin' },
]);

// 待处理事项
const pendingTasks = ref([
  { id: 1, type: 'warning', title: 'netsrv 服务未启动', description: '网络服务处于停止状态，可能影响数据上传' },
  { id: 2, type: 'error', title: 'rulesrv 服务异常', description: '规则服务运行异常，请检查配置文件' },
  { id: 3, type: 'info', title: '2个配置待验证', description: '通道配置已修改，需要验证后生效' },
  { id: 4, type: 'warning', title: '点表映射不完整', description: 'CAN总线通道有15个点未配置映射' },
]);

const loading = ref(false);

// 计算服务状态统计
const serviceStats = computed(() => {
  const running = services.value.filter(s => s.status === 'running').length;
  const stopped = services.value.filter(s => s.status === 'stopped').length;
  const error = services.value.filter(s => s.status === 'error').length;
  return { running, stopped, error, total: services.value.length };
});

// 获取状态颜色
function getStatusColor(status: string) {
  switch (status) {
    case 'running': return '#67C23A';
    case 'stopped': return '#909399';
    case 'error': return '#F56C6C';
    default: return '#909399';
  }
}

// 获取状态图标
function getStatusIcon(status: string) {
  switch (status) {
    case 'running': return CircleCheck;
    case 'stopped': return CircleClose;
    case 'error': return Warning;
    default: return CircleClose;
  }
}

// 获取任务类型颜色
function getTaskTypeColor(type: string) {
  switch (type) {
    case 'error': return 'danger';
    case 'warning': return 'warning';
    case 'info': return 'info';
    default: return 'info';
  }
}

// 导航到服务配置
function navigateToService(serviceName: string) {
  router.push(`/service/${serviceName}`);
}

// 导航到指定页面
function navigateTo(path: string) {
  router.push(path);
}

// 刷新数据
async function refreshData() {
  loading.value = true;
  // 模拟刷新延迟
  setTimeout(() => {
    loading.value = false;
    ElMessage.success('数据已刷新');
  }, 1000);
}

onMounted(() => {
  // 可以在这里加载实际的状态数据
});
</script>

<template>
  <div class="dashboard data-grid-bg">
    <!-- 顶部统计卡片 -->
    <el-row :gutter="20" class="stats-row">
      <el-col :span="6">
        <el-card class="stats-card advanced-card pulse-wave" @click="navigateTo('/channels')">
          <div class="stats-content">
            <div class="stats-icon" style="background: #409EFF20;">
              <el-icon :size="24" color="#409EFF"><Connection /></el-icon>
            </div>
            <div class="stats-info">
              <div class="stats-value">{{ configStats.channels.active }} / {{ configStats.channels.total }}</div>
              <div class="stats-label">活跃通道</div>
            </div>
          </div>
        </el-card>
      </el-col>
      <el-col :span="6">
        <el-card class="stats-card advanced-card pulse-wave" @click="navigateTo('/point-table')">
          <div class="stats-content">
            <div class="stats-icon" style="background: #67C23A20;">
              <el-icon :size="24" color="#67C23A"><Document /></el-icon>
            </div>
            <div class="stats-info">
              <div class="stats-value">{{ configStats.points.total }}</div>
              <div class="stats-label">配置点数</div>
            </div>
          </div>
        </el-card>
      </el-col>
      <el-col :span="6">
        <el-card class="stats-card advanced-card pulse-wave" @click="navigateTo('/alarm-rules')">
          <div class="stats-content">
            <div class="stats-icon" style="background: #E6A23C20;">
              <el-icon :size="24" color="#E6A23C"><Bell /></el-icon>
            </div>
            <div class="stats-info">
              <div class="stats-value">{{ configStats.alarms.total }}</div>
              <div class="stats-label">告警规则</div>
            </div>
          </div>
        </el-card>
      </el-col>
      <el-col :span="6">
        <el-card class="stats-card advanced-card pulse-wave" @click="navigateTo('/settings/templates')">
          <div class="stats-content">
            <div class="stats-icon" style="background: #909EFF20;">
              <el-icon :size="24" color="#909EFF"><Setting /></el-icon>
            </div>
            <div class="stats-info">
              <div class="stats-value">{{ configStats.templates }}</div>
              <div class="stats-label">配置模板</div>
            </div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <el-row :gutter="20">
      <!-- 服务状态 -->
      <el-col :span="14">
        <el-card class="service-status-card advanced-card">
          <template #header>
            <div class="card-header">
              <h3>服务状态</h3>
              <div class="header-actions">
                <el-tag size="small">
                  {{ serviceStats.running }} 运行 / {{ serviceStats.total }} 总数
                </el-tag>
                <el-button size="small" :icon="Refresh" @click="refreshData" :loading="loading" class="button-3d hover-indicator" />
              </div>
            </div>
          </template>
          <el-table :data="services" style="width: 100%">
            <el-table-column label="服务" width="150">
              <template #default="{ row }">
                <el-link @click="navigateToService(row.name)" type="primary">
                  {{ row.displayName }}
                </el-link>
              </template>
            </el-table-column>
            <el-table-column label="状态" width="120">
              <template #default="{ row }">
                <div class="status-cell">
                  <div :class="['status-indicator', row.status === 'running' ? 'online' : row.status === 'stopped' ? 'offline' : 'pending']"></div>
                  <span>{{ row.status === 'running' ? '运行中' : row.status === 'stopped' ? '已停止' : '异常' }}</span>
                </div>
              </template>
            </el-table-column>
            <el-table-column prop="uptime" label="运行时间" width="120" />
            <el-table-column label="资源使用">
              <template #default="{ row }">
                <div class="resource-usage" v-if="row.status === 'running'">
                  <div class="cpu-usage">
                    <span class="usage-label">CPU</span>
                    <div class="usage-bar">
                      <div class="energy-bar" style="width: 60px; margin: 0 8px;"></div>
                    </div>
                    <span class="usage-value">{{ row.cpu }}%</span>
                  </div>
                  <span class="memory-text">内存 {{ row.memory }}MB</span>
                </div>
                <span v-else>-</span>
              </template>
            </el-table-column>
          </el-table>
        </el-card>
      </el-col>

      <!-- 待处理事项 -->
      <el-col :span="10">
        <el-card class="pending-tasks-card advanced-card">
          <template #header>
            <div class="card-header">
              <h3>待处理事项</h3>
              <el-badge :value="pendingTasks.length" />
            </div>
          </template>
          <div class="task-list">
            <div v-for="task in pendingTasks" :key="task.id" class="task-item">
              <el-tag :type="getTaskTypeColor(task.type)" size="small">
                {{ task.type === 'error' ? '错误' : task.type === 'warning' ? '警告' : '提示' }}
              </el-tag>
              <div class="task-content">
                <div class="task-title">{{ task.title }}</div>
                <div class="task-description">{{ task.description }}</div>
              </div>
            </div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <el-row :gutter="20">
      <!-- 点表分布 -->
      <el-col :span="8">
        <el-card class="point-distribution-card advanced-card">
          <template #header>
            <h3>点表分布</h3>
          </template>
          <div class="point-stats">
            <div class="point-stat-item">
              <span class="point-type">遥测 (YC)</span>
              <el-progress :percentage="Math.round(configStats.points.telemetry / configStats.points.total * 100)" />
              <span class="point-count">{{ configStats.points.telemetry }}</span>
            </div>
            <div class="point-stat-item">
              <span class="point-type">遥信 (YX)</span>
              <el-progress :percentage="Math.round(configStats.points.signal / configStats.points.total * 100)" color="#67C23A" />
              <span class="point-count">{{ configStats.points.signal }}</span>
            </div>
            <div class="point-stat-item">
              <span class="point-type">遥控 (YK)</span>
              <el-progress :percentage="Math.round(configStats.points.control / configStats.points.total * 100)" color="#E6A23C" />
              <span class="point-count">{{ configStats.points.control }}</span>
            </div>
            <div class="point-stat-item">
              <span class="point-type">遥调 (YT)</span>
              <el-progress :percentage="Math.round(configStats.points.adjustment / configStats.points.total * 100)" color="#909399" />
              <span class="point-count">{{ configStats.points.adjustment }}</span>
            </div>
          </div>
        </el-card>
      </el-col>

      <!-- 最近活动 -->
      <el-col :span="16">
        <el-card class="recent-activities-card advanced-card">
          <template #header>
            <h3>最近活动</h3>
          </template>
          <el-timeline>
            <el-timeline-item
              v-for="(activity, index) in recentActivities"
              :key="index"
              :timestamp="activity.time"
              placement="top"
            >
              <div class="activity-content">
                <el-tag size="small" :type="activity.type === 'alarm' ? 'warning' : 'info'">
                  {{ activity.action }}
                </el-tag>
                <span class="activity-target">{{ activity.target }}</span>
                <span class="activity-user">by {{ activity.user }}</span>
              </div>
            </el-timeline-item>
          </el-timeline>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<style lang="scss" scoped>
.dashboard {
  .stats-row {
    margin-bottom: 20px;
  }

  .stats-card {
    cursor: pointer;
    transition: all 0.3s;
    background: var(--glass-bg) !important;
    backdrop-filter: var(--glass-blur);
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
      background: linear-gradient(135deg, transparent 0%, rgba(98, 106, 239, 0.1) 100%);
      opacity: 0;
      transition: opacity 0.3s ease;
    }

    &:hover {
      transform: translateY(-4px) scale(1.02);
      box-shadow: 0 8px 32px rgba(98, 106, 239, 0.3);
      border-color: var(--primary-color);

      &::before {
        opacity: 1;
      }

      .stats-value {
        text-shadow: 0 0 20px currentColor;
      }
    }

    :deep(.el-card__body) {
      background: transparent;
    }

    .stats-content {
      display: flex;
      align-items: center;
      gap: 16px;
      position: relative;
      z-index: 1;

      .stats-icon {
        width: 48px;
        height: 48px;
        border-radius: 12px;
        display: flex;
        align-items: center;
        justify-content: center;
        // 移除浮动动画，保持静态
        box-shadow: 0 4px 16px rgba(0, 0, 0, 0.2);
      }

      .stats-info {
        flex: 1;

        .stats-value {
          font-size: 28px;
          font-weight: 700;
          background: linear-gradient(135deg, var(--primary-color), var(--accent-cyan));
          -webkit-background-clip: text;
          -webkit-text-fill-color: transparent;
          background-clip: text;
          line-height: 1.2;
          transition: all 0.3s ease;
        }

        .stats-label {
          font-size: 14px;
          color: var(--text-secondary);
          margin-top: 4px;
          text-transform: uppercase;
          letter-spacing: 1px;
        }
      }
    }
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;

    h3 {
      margin: 0;
      font-size: 16px;
      font-weight: 600;
    }

    .header-actions {
      display: flex;
      align-items: center;
      gap: 12px;
    }
  }


  .service-status-card {
    margin-bottom: 20px;
    background: var(--glass-bg) !important;
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--glass-border);

    :deep(.el-card__header) {
      background: rgba(255, 255, 255, 0.02);
      border-bottom: 1px solid var(--glass-border);
    }

    :deep(.el-card__body) {
      background: transparent;
    }

    :deep(.el-table) {
      background: transparent;
      
      th {
        background: rgba(255, 255, 255, 0.03);
        color: var(--text-primary);
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.5px;
        border-bottom: 1px solid var(--glass-border);
      }

      td {
        color: var(--text-primary);
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
      }

      .el-table__row {
        transition: all 0.3s ease;

        &:hover {
          background: rgba(98, 106, 239, 0.1);
          td {
            color: var(--text-primary);
          }
        }
      }
    }

    .status-cell {
      display: flex;
      align-items: center;
      gap: 8px;

      span {
        font-weight: 500;
      }
    }

    .resource-usage {
      display: flex;
      align-items: center;
      gap: 16px;

      .cpu-usage {
        display: flex;
        align-items: center;
        gap: 4px;
        
        .usage-label {
          font-size: 11px;
          color: var(--text-secondary);
          font-weight: 500;
        }
        
        .usage-bar {
          display: flex;
          align-items: center;
        }
        
        .usage-value {
          font-size: 12px;
          color: var(--accent-cyan);
          font-weight: 600;
          min-width: 40px;
        }
      }

      .memory-text {
        font-size: 12px;
        color: var(--text-secondary);
      }
    }
  }

  .pending-tasks-card {
    margin-bottom: 20px;
    background: var(--glass-bg) !important;
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--glass-border);

    :deep(.el-card__header) {
      background: rgba(255, 255, 255, 0.02);
      border-bottom: 1px solid var(--glass-border);
    }

    :deep(.el-card__body) {
      background: transparent;
    }
    
    .task-list {
      .task-item {
        display: flex;
        gap: 12px;
        padding: 12px 0;
        border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        transition: all 0.3s ease;

        &:hover {
          transform: translateX(8px);
          background: rgba(98, 106, 239, 0.05);
          margin: 0 -12px;
          padding: 12px;
        }

        &:last-child {
          border-bottom: none;
          padding-bottom: 0;
        }

        .task-content {
          flex: 1;

          .task-title {
            font-weight: 500;
            color: var(--text-primary);
            margin-bottom: 4px;
          }

          .task-description {
            font-size: 13px;
            color: var(--text-secondary);
            line-height: 1.4;
          }
        }

        :deep(.el-tag) {
          border: none;
          font-weight: 500;
          backdrop-filter: blur(10px);
        }
      }
    }
  }

  .point-distribution-card {
    margin-bottom: 20px;
    background: var(--glass-bg) !important;
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--glass-border);

    :deep(.el-card__header) {
      background: rgba(255, 255, 255, 0.02);
      border-bottom: 1px solid var(--glass-border);
    }

    :deep(.el-card__body) {
      background: transparent;
    }

    .point-stats {
      .point-stat-item {
        display: flex;
        align-items: center;
        gap: 12px;
        margin-bottom: 16px;

        &:last-child {
          margin-bottom: 0;
        }

        .point-type {
          width: 80px;
          font-size: 14px;
          color: var(--text-secondary);
          font-weight: 500;
        }

        :deep(.el-progress) {
          flex: 1;
          
          .el-progress-bar__outer {
            background: rgba(255, 255, 255, 0.1);
          }

          .el-progress-bar__inner {
            background: linear-gradient(90deg, var(--primary-color), var(--accent-cyan));
            box-shadow: 0 0 10px currentColor;
          }

          .el-progress__text {
            color: var(--text-primary);
          }
        }

        .point-count {
          width: 50px;
          text-align: right;
          font-weight: 700;
          color: var(--accent-cyan);
          text-shadow: 0 0 10px rgba(0, 212, 255, 0.5);
        }
      }
    }
  }

  .recent-activities-card {
    margin-bottom: 20px;
    background: var(--glass-bg) !important;
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--glass-border);

    :deep(.el-card__header) {
      background: rgba(255, 255, 255, 0.02);
      border-bottom: 1px solid var(--glass-border);
    }

    :deep(.el-card__body) {
      background: transparent;
    }

    :deep(.el-timeline) {
      .el-timeline-item__wrapper {
        .el-timeline-item__timestamp {
          color: var(--text-secondary);
          font-weight: 500;
        }
      }

      .el-timeline-item__node {
        background: var(--primary-color);
        box-shadow: 0 0 10px var(--primary-color);
      }

      .el-timeline-item__tail {
        border-left: 2px solid var(--glass-border);
      }
    }

    .activity-content {
      display: flex;
      align-items: center;
      gap: 12px;

      .activity-target {
        font-weight: 500;
        color: var(--text-primary);
      }

      .activity-user {
        font-size: 13px;
        color: var(--text-secondary);
      }

      :deep(.el-tag) {
        border: none;
        backdrop-filter: blur(10px);
        font-weight: 500;
      }
    }
  }

  // 添加所有标题的霓虹光效
  h3 {
    color: var(--text-primary);
    position: relative;
    
    &::after {
      content: '';
      position: absolute;
      bottom: -4px;
      left: 0;
      width: 30px;
      height: 2px;
      background: var(--primary-gradient);
      box-shadow: 0 0 10px var(--primary-color);
      transition: width 0.3s ease;
    }
  }

  .card-header:hover h3::after {
    width: 60px;
  }

  .el-row {
    margin-bottom: 0;
    
    & + .el-row {
      margin-top: 20px;
    }
  }
}
</style>