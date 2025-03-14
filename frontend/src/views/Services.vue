<template>
  <div class="services-page">
    <div class="page-header">
      <h1>服务管理</h1>
      <p>管理和监控系统服务</p>
    </div>
    
    <div class="page-content">
      <el-card>
        <template #header>
          <div class="card-header">
            <span>系统服务</span>
            <span class="electron-badge" v-if="isElectronEnv">Electron</span>
          </div>
        </template>
        
        <ServiceManager v-if="isElectronEnv" />
        
        <div v-else class="electron-notice">
          <el-alert
            title="此功能仅在Electron桌面应用中可用"
            type="warning"
            :closable="false">
            <p>服务管理功能需要Electron环境支持。请下载并安装桌面应用以使用此功能。</p>
          </el-alert>
        </div>
      </el-card>
      
      <el-card class="mt-20">
        <template #header>
          <div class="card-header">
            <span>系统信息</span>
          </div>
        </template>
        
        <div class="system-info">
          <div class="info-item">
            <span class="label">应用版本:</span>
            <span class="value">{{ appVersion }}</span>
          </div>
          <div class="info-item">
            <span class="label">运行环境:</span>
            <span class="value">{{ platformInfo }}</span>
          </div>
          <div class="info-item">
            <span class="label">Electron:</span>
            <span class="value">{{ isElectronEnv ? '是' : '否' }}</span>
          </div>
        </div>
      </el-card>
    </div>
  </div>
</template>

<script>
import ServiceManager from '@/components/electron/ServiceManager.vue';
import { isElectron, getAppVersion, getPlatform } from '@/utils/electron';

export default {
  name: 'ServicesView',
  components: {
    ServiceManager
  },
  data() {
    return {
      isElectronEnv: isElectron(),
      appVersion: getAppVersion(),
      platformInfo: getPlatform()
    };
  }
};
</script>

<style scoped>
.services-page {
  padding: 20px;
}

.page-header {
  margin-bottom: 20px;
}

.page-header h1 {
  margin: 0;
  font-size: 24px;
  color: #303133;
}

.page-header p {
  margin: 5px 0 0;
  color: #606266;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.electron-badge {
  background-color: #409EFF;
  color: white;
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 12px;
}

.electron-notice {
  padding: 20px;
  text-align: center;
}

.mt-20 {
  margin-top: 20px;
}

.system-info {
  padding: 10px;
}

.info-item {
  margin-bottom: 10px;
  display: flex;
}

.info-item .label {
  font-weight: bold;
  width: 120px;
}

.info-item .value {
  flex: 1;
}
</style> 