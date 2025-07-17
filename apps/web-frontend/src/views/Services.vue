<template>
  <div class="services-dashboard">
    <!-- 服务状态概览 -->
    <div class="service-overview">
      <h2>服务状态监控</h2>
      <div class="service-cards">
        <div 
          v-for="service in services" 
          :key="service.name"
          class="service-card"
          :class="{ 'service-error': service.status === 'error' }">
          <div class="service-header">
            <el-icon :size="24" :color="getStatusColor(service.status)">
              <component :is="service.icon" />
            </el-icon>
            <h3>{{ service.displayName }}</h3>
          </div>
          <div class="service-status">
            <el-tag :type="getStatusType(service.status)">{{ service.statusText }}</el-tag>
          </div>
          <div class="service-info">
            <p>{{ service.description }}</p>
            <div class="service-metrics">
              <div v-for="metric in service.metrics" :key="metric.label" class="metric">
                <span class="metric-label">{{ metric.label }}:</span>
                <span class="metric-value">{{ metric.value }}</span>
              </div>
            </div>
          </div>
          <div class="service-actions">
            <el-button 
              v-if="service.status === 'running'" 
              size="small" 
              type="danger" 
              @click="stopService(service.name)">
              停止
            </el-button>
            <el-button 
              v-else 
              size="small" 
              type="success" 
              @click="startService(service.name)">
              启动
            </el-button>
            <el-button 
              size="small" 
              @click="restartService(service.name)">
              重启
            </el-button>
            <el-button 
              size="small" 
              type="primary" 
              @click="configureService(service.name)">
              配置
            </el-button>
          </div>
        </div>
      </div>
    </div>

    <!-- 数据流向图 -->
    <div class="data-flow-section">
      <h2>系统数据流向</h2>
      <div class="data-flow-diagram">
        <div class="flow-container">
          <!-- 设备层 -->
          <div class="flow-layer devices-layer">
            <h4>设备层</h4>
            <div class="flow-nodes">
              <div class="flow-node device">
                <el-icon><el-icon-cpu /></el-icon>
                <span>Modbus设备</span>
              </div>
              <div class="flow-node device">
                <el-icon><el-icon-connection /></el-icon>
                <span>CAN设备</span>
              </div>
              <div class="flow-node device">
                <el-icon><el-icon-switch /></el-icon>
                <span>IEC60870</span>
              </div>
              <div class="flow-node device">
                <el-icon><el-icon-switch-button /></el-icon>
                <span>GPIO</span>
              </div>
            </div>
          </div>
          
          <!-- 通信层 -->
          <div class="flow-arrow down"></div>
          <div class="flow-layer communication-layer">
            <h4>通信服务层</h4>
            <div class="flow-nodes">
              <div class="flow-node service-node comsrv">
                <el-icon><el-icon-connection /></el-icon>
                <span>comsrv</span>
                <div class="node-detail">工业协议通信</div>
              </div>
            </div>
          </div>
          
          <!-- Redis层 -->
          <div class="flow-arrow down"></div>
          <div class="flow-layer redis-layer">
            <h4>实时数据总线</h4>
            <div class="flow-nodes">
              <div class="flow-node redis">
                <el-icon><el-icon-data-board /></el-icon>
                <span>Redis</span>
                <div class="node-detail">实时数据缓存</div>
              </div>
            </div>
          </div>
          
          <!-- 处理层 -->
          <div class="flow-arrow down split"></div>
          <div class="flow-layer processing-layer">
            <h4>数据处理层</h4>
            <div class="flow-nodes">
              <div class="flow-node service-node modsrv">
                <el-icon><el-icon-data-analysis /></el-icon>
                <span>modsrv</span>
                <div class="node-detail">计算控制</div>
              </div>
              <div class="flow-node service-node hissrv">
                <el-icon><el-icon-timer /></el-icon>
                <span>hissrv</span>
                <div class="node-detail">历史存储</div>
              </div>
              <div class="flow-node service-node netsrv">
                <el-icon><el-icon-upload /></el-icon>
                <span>netsrv</span>
                <div class="node-detail">云端转发</div>
              </div>
              <div class="flow-node service-node alarmsrv">
                <el-icon><el-icon-bell /></el-icon>
                <span>alarmsrv</span>
                <div class="node-detail">告警管理</div>
              </div>
            </div>
          </div>
          
          <!-- 存储/输出层 -->
          <div class="flow-arrow down"></div>
          <div class="flow-layer output-layer">
            <h4>存储与输出层</h4>
            <div class="flow-nodes">
              <div class="flow-node storage">
                <el-icon><el-icon-files /></el-icon>
                <span>InfluxDB</span>
              </div>
              <div class="flow-node cloud">
                <el-icon><el-icon-cloudy /></el-icon>
                <span>云平台</span>
              </div>
              <div class="flow-node ui">
                <el-icon><el-icon-monitor /></el-icon>
                <span>前端展示</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 关键指标 -->
    <div class="metrics-section">
      <h2>系统关键指标</h2>
      <el-row :gutter="20">
        <el-col :span="6">
          <el-statistic title="活跃设备" :value="activeDevices" suffix="台" />
        </el-col>
        <el-col :span="6">
          <el-statistic title="数据点位" :value="dataPoints" suffix="个" />
        </el-col>
        <el-col :span="6">
          <el-statistic title="消息吞吐" :value="messageThroughput" suffix="条/秒" />
        </el-col>
        <el-col :span="6">
          <el-statistic title="活跃告警" :value="activeAlarms" suffix="个">
            <template #suffix>
              <el-tag v-if="activeAlarms > 0" type="danger" size="small">{{ activeAlarms }}</el-tag>
            </template>
          </el-statistic>
        </el-col>
      </el-row>
    </div>
  </div>
</template>

<script>
export default {
  name: 'ServicesView',
  data() {
    return {
      services: [
        {
          name: 'comsrv',
          displayName: '通信服务',
          status: 'running',
          statusText: '运行中',
          icon: 'el-icon-connection',
          description: '工业协议通信服务，支持Modbus、CAN、IEC60870等',
          metrics: [
            { label: '连接设备', value: '12' },
            { label: '消息速率', value: '156/s' },
            { label: 'CPU', value: '23%' }
          ]
        },
        {
          name: 'modsrv',
          displayName: '模型服务',
          status: 'running',
          statusText: '运行中',
          icon: 'el-icon-data-analysis',
          description: '实时计算和控制逻辑，DAG工作流引擎',
          metrics: [
            { label: '活跃任务', value: '8' },
            { label: '计算延迟', value: '12ms' },
            { label: 'CPU', value: '45%' }
          ]
        },
        {
          name: 'hissrv',
          displayName: '历史服务',
          status: 'running',
          statusText: '运行中',
          icon: 'el-icon-timer',
          description: '时序数据持久化，Redis到InfluxDB',
          metrics: [
            { label: '写入速率', value: '1.2k/s' },
            { label: '存储占用', value: '45GB' },
            { label: 'CPU', value: '15%' }
          ]
        },
        {
          name: 'netsrv',
          displayName: '网络服务',
          status: 'running',
          statusText: '运行中',
          icon: 'el-icon-upload',
          description: '数据转发服务，支持MQTT、HTTP',
          metrics: [
            { label: '上传速率', value: '256/s' },
            { label: '队列长度', value: '120' },
            { label: 'CPU', value: '18%' }
          ]
        },
        {
          name: 'alarmsrv',
          displayName: '告警服务',
          status: 'running',
          statusText: '运行中',
          icon: 'el-icon-bell',
          description: '智能告警分类与管理',
          metrics: [
            { label: '活跃告警', value: '3' },
            { label: '今日告警', value: '45' },
            { label: 'CPU', value: '8%' }
          ]
        }
      ],
      activeDevices: 12,
      dataPoints: 3456,
      messageThroughput: 1567,
      activeAlarms: 3
    }
  },
  methods: {
    getStatusColor(status) {
      const colors = {
        running: '#67C23A',
        stopped: '#909399',
        error: '#F56C6C'
      };
      return colors[status] || '#909399';
    },
    getStatusType(status) {
      const types = {
        running: 'success',
        stopped: 'info',
        error: 'danger'
      };
      return types[status] || 'info';
    },
    startService(serviceName) {
      this.$message.success(`启动服务: ${serviceName}`);
      // 实际实现中调用后端API
    },
    stopService(serviceName) {
      this.$confirm(`确定要停止 ${serviceName} 服务吗?`, '警告', {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }).then(() => {
        this.$message.success(`停止服务: ${serviceName}`);
        // 实际实现中调用后端API
      });
    },
    restartService(serviceName) {
      this.$message.success(`重启服务: ${serviceName}`);
      // 实际实现中调用后端API
    },
    configureService(serviceName) {
      this.$router.push(`/system?service=${serviceName}`);
    }
  },
  mounted() {
    // 定期更新服务状态
    this.updateInterval = setInterval(() => {
      // 更新服务状态和指标
    }, 5000);
  },
  beforeUnmount() {
    clearInterval(this.updateInterval);
  }
}
</script>

<style scoped>
.services-dashboard {
  padding: 20px;
}

/* 服务状态卡片 */
.service-overview {
  margin-bottom: 30px;
}

.service-cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: 20px;
  margin-top: 15px;
}

.service-card {
  background: white;
  border-radius: 8px;
  padding: 20px;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.1);
  transition: all 0.3s;
}

.service-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
}

.service-card.service-error {
  border-left: 4px solid #F56C6C;
}

.service-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 10px;
}

.service-header h3 {
  margin: 0;
  font-size: 18px;
}

.service-status {
  margin-bottom: 15px;
}

.service-info {
  margin-bottom: 20px;
}

.service-info p {
  color: #666;
  margin-bottom: 10px;
  font-size: 14px;
}

.service-metrics {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.metric {
  display: flex;
  justify-content: space-between;
  font-size: 13px;
}

.metric-label {
  color: #909399;
}

.metric-value {
  font-weight: bold;
  color: #409EFF;
}

.service-actions {
  display: flex;
  gap: 10px;
  border-top: 1px solid #eee;
  padding-top: 15px;
}

/* 数据流向图 */
.data-flow-section {
  background: white;
  border-radius: 8px;
  padding: 20px;
  margin-bottom: 30px;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.1);
}

.data-flow-diagram {
  margin-top: 20px;
}

.flow-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}

.flow-layer {
  width: 100%;
  background: #f5f7fa;
  border-radius: 8px;
  padding: 15px;
  text-align: center;
}

.flow-layer h4 {
  margin: 0 0 10px 0;
  color: #606266;
}

.flow-nodes {
  display: flex;
  justify-content: center;
  gap: 20px;
  flex-wrap: wrap;
}

.flow-node {
  background: white;
  border-radius: 6px;
  padding: 10px 15px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 5px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  min-width: 100px;
}

.flow-node .el-icon {
  font-size: 24px;
}

.flow-node span {
  font-weight: bold;
  font-size: 14px;
}

.node-detail {
  font-size: 12px;
  color: #909399;
}

.flow-arrow {
  width: 2px;
  height: 30px;
  background: #409EFF;
  position: relative;
}

.flow-arrow::after {
  content: '';
  position: absolute;
  bottom: -8px;
  left: -4px;
  width: 0;
  height: 0;
  border-left: 5px solid transparent;
  border-right: 5px solid transparent;
  border-top: 8px solid #409EFF;
}

/* 层级样式 */
.devices-layer { background-color: #fef0f0; }
.communication-layer { background-color: #f0f9ff; }
.redis-layer { background-color: #fdf6ec; }
.processing-layer { background-color: #f0f9ff; }
.output-layer { background-color: #f5f7fa; }

/* 节点类型样式 */
.device { border: 2px solid #E6A23C; }
.service-node { border: 2px solid #409EFF; }
.redis { border: 2px solid #F56C6C; }
.storage { border: 2px solid #67C23A; }
.cloud { border: 2px solid #909399; }
.ui { border: 2px solid #409EFF; }

/* 关键指标 */
.metrics-section {
  background: white;
  border-radius: 8px;
  padding: 20px;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.1);
}
</style>