<template>
  <div class="service-manager">
    <h2>服务管理</h2>
    
    <div class="service-controls">
      <el-button type="primary" @click="startAllServices">启动所有服务</el-button>
      <el-button type="danger" @click="stopAllServices">停止所有服务</el-button>
      <el-button type="info" @click="refreshStatus">刷新状态</el-button>
    </div>
    
    <el-table :data="servicesList" style="width: 100%" v-loading="loading">
      <el-table-column prop="id" label="服务ID" width="120" />
      <el-table-column prop="name" label="服务名称" width="200" />
      <el-table-column prop="status" label="状态">
        <template #default="scope">
          <el-tag :type="getStatusType(scope.row.status)">
            {{ getStatusText(scope.row.status) }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="uptime" label="运行时间">
        <template #default="scope">
          {{ formatUptime(scope.row.uptime) }}
        </template>
      </el-table-column>
      <el-table-column label="操作" width="250">
        <template #default="scope">
          <el-button 
            size="small" 
            type="primary" 
            @click="startService(scope.row.id)"
            :disabled="scope.row.status === 'running'">
            启动
          </el-button>
          <el-button 
            size="small" 
            type="warning" 
            @click="restartService(scope.row.id)"
            :disabled="scope.row.status !== 'running'">
            重启
          </el-button>
          <el-button 
            size="small" 
            type="danger" 
            @click="stopService(scope.row.id)"
            :disabled="scope.row.status !== 'running'">
            停止
          </el-button>
        </template>
      </el-table-column>
    </el-table>
    
    <div class="service-logs" v-if="selectedService">
      <h3>{{ selectedService.name }} 日志</h3>
      <pre>{{ serviceLogs }}</pre>
    </div>
  </div>
</template>

<script>
export default {
  name: 'ServiceManager',
  data() {
    return {
      services: {
        modsrv: { id: 'modsrv', name: '模型服务', status: 'unknown', uptime: 0 },
        comsrv: { id: 'comsrv', name: '通信服务', status: 'unknown', uptime: 0 },
        hissrv: { id: 'hissrv', name: '历史服务', status: 'unknown', uptime: 0 },
        netsrv: { id: 'netsrv', name: '网络服务', status: 'unknown', uptime: 0 }
      },
      selectedService: null,
      serviceLogs: '',
      loading: false,
      statusInterval: null
    };
  },
  computed: {
    servicesList() {
      return Object.values(this.services);
    },
    isElectron() {
      return window.electronAPI !== undefined;
    }
  },
  mounted() {
    if (this.isElectron) {
      this.setupElectronListeners();
      this.refreshStatus();
      
      // 定期刷新状态
      this.statusInterval = setInterval(() => {
        this.refreshStatus();
      }, 5000);
    }
  },
  beforeUnmount() {
    if (this.statusInterval) {
      clearInterval(this.statusInterval);
    }
  },
  methods: {
    setupElectronListeners() {
      // 监听服务状态更新
      window.electronAPI.onMessage('service-status', (result) => {
        this.loading = false;
        
        if (result.service) {
          // 单个服务状态更新
          this.updateServiceStatus(result.service, result);
        } else if (Array.isArray(result)) {
          // 多个服务状态更新
          result.forEach(status => {
            if (status.service) {
              this.updateServiceStatus(status.service, status);
            }
          });
        }
      });
    },
    
    updateServiceStatus(serviceId, status) {
      if (this.services[serviceId]) {
        this.services[serviceId] = {
          ...this.services[serviceId],
          status: status.status || 'unknown',
          uptime: status.uptime || 0,
          pid: status.pid
        };
      }
    },
    
    refreshStatus() {
      if (!this.isElectron) return;
      
      this.loading = true;
      window.electronAPI.sendMessage('service-control', { 
        action: 'all-status' 
      });
    },
    
    startService(serviceId) {
      if (!this.isElectron) return;
      
      this.loading = true;
      window.electronAPI.sendMessage('service-control', { 
        action: 'start', 
        service: serviceId 
      });
    },
    
    stopService(serviceId) {
      if (!this.isElectron) return;
      
      this.loading = true;
      window.electronAPI.sendMessage('service-control', { 
        action: 'stop', 
        service: serviceId 
      });
    },
    
    restartService(serviceId) {
      if (!this.isElectron) return;
      
      this.loading = true;
      window.electronAPI.sendMessage('service-control', { 
        action: 'restart', 
        service: serviceId 
      });
    },
    
    startAllServices() {
      if (!this.isElectron) return;
      
      this.loading = true;
      Object.keys(this.services).forEach(serviceId => {
        this.startService(serviceId);
      });
    },
    
    stopAllServices() {
      if (!this.isElectron) return;
      
      this.loading = true;
      Object.keys(this.services).forEach(serviceId => {
        if (this.services[serviceId].status === 'running') {
          this.stopService(serviceId);
        }
      });
    },
    
    getStatusType(status) {
      switch (status) {
        case 'running': return 'success';
        case 'stopped': return 'info';
        case 'error': return 'danger';
        default: return 'warning';
      }
    },
    
    getStatusText(status) {
      switch (status) {
        case 'running': return '运行中';
        case 'stopped': return '已停止';
        case 'error': return '错误';
        case 'unknown': return '未知';
        default: return status;
      }
    },
    
    formatUptime(seconds) {
      if (!seconds || seconds <= 0) return '未运行';
      
      const days = Math.floor(seconds / 86400);
      const hours = Math.floor((seconds % 86400) / 3600);
      const minutes = Math.floor((seconds % 3600) / 60);
      const secs = seconds % 60;
      
      let result = '';
      if (days > 0) result += `${days}天 `;
      if (hours > 0) result += `${hours}小时 `;
      if (minutes > 0) result += `${minutes}分钟 `;
      if (secs > 0 || result === '') result += `${secs}秒`;
      
      return result;
    }
  }
};
</script>

<style scoped>
.service-manager {
  padding: 20px;
}

.service-controls {
  margin-bottom: 20px;
}

.service-logs {
  margin-top: 20px;
  background-color: #f5f5f5;
  border-radius: 4px;
  padding: 10px;
}

.service-logs pre {
  max-height: 300px;
  overflow-y: auto;
  white-space: pre-wrap;
  font-family: monospace;
}
</style> 