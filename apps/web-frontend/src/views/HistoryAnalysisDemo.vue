<template>
  <div class="history-analysis">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>历史数据分析（演示模式）</span>
          <el-space>
            <el-button type="primary" @click="showInfo">关于 Grafana 集成</el-button>
            <el-button @click="exportDashboard">导出</el-button>
          </el-space>
        </div>
      </template>

      <!-- 工具栏 -->
      <div class="toolbar">
        <el-space size="large">
          <el-select v-model="selectedDevice" placeholder="选择设备" style="width: 200px">
            <el-option label="所有设备" value="all" />
            <el-option label="变压器 #1" value="device_001" />
            <el-option label="变压器 #2" value="device_002" />
            <el-option label="配电柜 #1" value="device_003" />
          </el-select>

          <el-date-picker
            v-model="timeRange"
            type="datetimerange"
            range-separator="至"
            start-placeholder="开始时间"
            end-placeholder="结束时间"
            format="YYYY-MM-DD HH:mm"
            :shortcuts="timeShortcuts"
          />
        </el-space>
      </div>

      <!-- 仪表板标签页 -->
      <el-tabs v-model="activeTab">
        <el-tab-pane label="系统总览" name="overview">
          <div class="demo-dashboard">
            <el-alert
              title="Grafana 集成演示"
              type="info"
              :closable="false"
              show-icon
            >
              <template #default>
                <p>这里将嵌入 Grafana 仪表板，显示系统整体运行状态和关键指标。</p>
                <p>当前选择：设备 {{ selectedDevice }}，时间范围 {{ formatTimeRange() }}</p>
              </template>
            </el-alert>
            
            <div class="demo-content">
              <el-row :gutter="20" style="margin-top: 20px">
                <el-col :span="6">
                  <el-statistic title="在线设备" :value="156" suffix="台" />
                </el-col>
                <el-col :span="6">
                  <el-statistic title="今日能耗" :value="1234.56" suffix="kWh" :precision="2" />
                </el-col>
                <el-col :span="6">
                  <el-statistic title="平均负载" :value="78.5" suffix="%" :precision="1" />
                </el-col>
                <el-col :span="6">
                  <el-statistic title="运行时长" :value="999" suffix="小时" />
                </el-col>
              </el-row>
              
              <div class="chart-placeholder">
                <el-empty description="Grafana 图表将显示在这里">
                  <el-button type="primary" @click="showSetupGuide">查看配置指南</el-button>
                </el-empty>
              </div>
            </div>
          </div>
        </el-tab-pane>

        <el-tab-pane label="设备分析" name="device">
          <div class="demo-dashboard">
            <div class="chart-placeholder">
              <el-empty description="设备运行数据详细分析图表" />
            </div>
          </div>
        </el-tab-pane>

        <el-tab-pane label="能耗分析" name="energy">
          <div class="demo-dashboard">
            <div class="chart-placeholder">
              <el-empty description="能源消耗趋势和效率分析图表" />
            </div>
          </div>
        </el-tab-pane>

        <el-tab-pane label="告警历史" name="alarm">
          <div class="demo-dashboard">
            <el-table :data="alarmData" style="width: 100%">
              <el-table-column prop="time" label="时间" width="180" />
              <el-table-column prop="device" label="设备" width="150" />
              <el-table-column prop="type" label="类型" width="120">
                <template #default="scope">
                  <el-tag :type="getAlarmType(scope.row.type)">
                    {{ scope.row.type }}
                  </el-tag>
                </template>
              </el-table-column>
              <el-table-column prop="description" label="描述" />
              <el-table-column label="操作" width="100">
                <template #default>
                  <el-button link type="primary" size="small">查看</el-button>
                </template>
              </el-table-column>
            </el-table>
          </div>
        </el-tab-pane>
      </el-tabs>
    </el-card>

    <!-- 信息对话框 -->
    <el-dialog
      v-model="infoDialogVisible"
      title="Grafana 集成说明"
      width="600px"
    >
      <div class="info-content">
        <h4>功能特性</h4>
        <ul>
          <li>通过 iframe 无缝嵌入 Grafana 仪表板</li>
          <li>自动处理认证，用户无需二次登录</li>
          <li>支持动态传递参数（设备、时间范围等）</li>
          <li>隐藏 Grafana UI，保持界面一致性</li>
        </ul>
        
        <h4>所需服务</h4>
        <ul>
          <li>Grafana 服务（端口 3000）</li>
          <li>Hissrv 历史数据服务（端口 8080）</li>
          <li>API 网关（端口 3001）</li>
          <li>Nginx 反向代理</li>
        </ul>
        
        <h4>数据流程</h4>
        <ol>
          <li>用户在前端选择设备和时间范围</li>
          <li>GrafanaEmbed 组件构建带参数的 URL</li>
          <li>通过 iframe 加载 Grafana 仪表板</li>
          <li>Grafana 从 Hissrv 查询历史数据</li>
          <li>实时更新显示图表</li>
        </ol>
      </div>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'

const activeTab = ref('overview')
const selectedDevice = ref('all')
const timeRange = ref([
  new Date(Date.now() - 24 * 60 * 60 * 1000),
  new Date()
])
const infoDialogVisible = ref(false)

// 模拟告警数据
const alarmData = ref([
  {
    time: '2025-01-07 10:30:00',
    device: '变压器 #1',
    type: '高温',
    description: '温度超过阈值 85°C'
  },
  {
    time: '2025-01-07 09:15:00',
    device: '配电柜 #1',
    type: '过载',
    description: '负载超过额定功率 120%'
  },
  {
    time: '2025-01-07 08:00:00',
    device: '变压器 #2',
    type: '通信',
    description: '设备通信中断 5 分钟'
  }
])

// 时间快捷选项
const timeShortcuts = [
  {
    text: '最近1小时',
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setTime(start.getTime() - 3600 * 1000)
      return [start, end]
    }
  },
  {
    text: '最近24小时',
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setTime(start.getTime() - 3600 * 1000 * 24)
      return [start, end]
    }
  },
  {
    text: '最近7天',
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setTime(start.getTime() - 3600 * 1000 * 24 * 7)
      return [start, end]
    }
  }
]

const formatTimeRange = () => {
  if (!timeRange.value || timeRange.value.length !== 2) return ''
  const start = timeRange.value[0].toLocaleString('zh-CN')
  const end = timeRange.value[1].toLocaleString('zh-CN')
  return `${start} 至 ${end}`
}

const getAlarmType = (type) => {
  const typeMap = {
    '高温': 'danger',
    '过载': 'warning',
    '通信': 'info'
  }
  return typeMap[type] || 'info'
}

const showInfo = () => {
  infoDialogVisible.value = true
}

const showSetupGuide = () => {
  ElMessageBox.alert(
    `<div>
      <p>要启用完整的 Grafana 集成，请：</p>
      <ol>
        <li>启动 Grafana 服务：<code>docker run -d -p 3000:3000 grafana/grafana</code></li>
        <li>启动 Hissrv 服务：<code>cd services/Hissrv && cargo run</code></li>
        <li>启动 API 网关：<code>cd services/apigateway && cargo run</code></li>
        <li>配置 Nginx 反向代理</li>
      </ol>
    </div>`,
    '配置指南',
    {
      dangerouslyUseHTMLString: true,
      confirmButtonText: '确定'
    }
  )
}

const exportDashboard = () => {
  ElMessage.info('导出功能将在服务启动后可用')
}
</script>

<style scoped>
.history-analysis {
  height: 100%;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.toolbar {
  margin-bottom: 20px;
  padding: 16px;
  background-color: #f5f7fa;
  border-radius: 4px;
}

.demo-dashboard {
  min-height: 500px;
}

.demo-content {
  padding: 20px 0;
}

.chart-placeholder {
  margin-top: 30px;
  height: 400px;
  border: 2px dashed #dcdfe6;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: var(--color-background-elevated);
}

.info-content h4 {
  color: #303133;
  margin: 20px 0 10px 0;
}

.info-content h4:first-child {
  margin-top: 0;
}

.info-content ul,
.info-content ol {
  margin: 0;
  padding-left: 20px;
  color: #606266;
  line-height: 2;
}

.info-content code {
  background-color: #f5f7fa;
  padding: 2px 6px;
  border-radius: 3px;
  font-family: monospace;
  font-size: 14px;
}

/* 统计数字样式 */
:deep(.el-statistic__content) {
  font-size: 30px;
}

:deep(.el-statistic__head) {
  color: #909399;
}
</style>