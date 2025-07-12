<script setup lang="ts">
import { ref } from 'vue';
import { ElMessage } from 'element-plus';

const influxConfig = ref({
  url: 'http://localhost:8086',
  token: '',
  organization: 'voltage-ems',
  bucket: 'voltage-data',
  batchSize: 5000,
  flushInterval: 1000,
  retentionDays: 30,
});

const connectionStatus = ref('disconnected');
const testing = ref(false);

async function testConnection() {
  testing.value = true;
  connectionStatus.value = 'testing';
  
  // 模拟测试连接
  setTimeout(() => {
    connectionStatus.value = 'connected';
    testing.value = false;
    ElMessage.success('InfluxDB连接测试成功');
  }, 2000);
}

function saveConfig() {
  ElMessage.success('InfluxDB配置已保存');
}
</script>

<template>
  <div class="influxdb-settings">
    <el-card>
      <template #header>
        <h3>InfluxDB 配置</h3>
      </template>
      
      <el-form :model="influxConfig" label-width="140px">
        <el-form-item label="服务器URL" required>
          <el-input v-model="influxConfig.url" placeholder="http://localhost:8086" />
        </el-form-item>
        
        <el-form-item label="访问令牌" required>
          <el-input v-model="influxConfig.token" type="password" placeholder="请输入InfluxDB访问令牌" show-password />
        </el-form-item>
        
        <el-form-item label="组织" required>
          <el-input v-model="influxConfig.organization" placeholder="组织名称" />
        </el-form-item>
        
        <el-form-item label="存储桶" required>
          <el-input v-model="influxConfig.bucket" placeholder="数据存储桶名称" />
        </el-form-item>
        
        <el-divider content-position="left">写入设置</el-divider>
        
        <el-form-item label="批量大小">
          <el-input-number v-model="influxConfig.batchSize" :min="100" :max="50000" :step="1000" />
          <span class="form-help">每批次写入的数据点数量</span>
        </el-form-item>
        
        <el-form-item label="刷新间隔(ms)">
          <el-input-number v-model="influxConfig.flushInterval" :min="100" :max="60000" :step="100" />
          <span class="form-help">批量写入的时间间隔</span>
        </el-form-item>
        
        <el-form-item label="数据保留天数">
          <el-input-number v-model="influxConfig.retentionDays" :min="1" :max="3650" />
          <span class="form-help">历史数据保留时间</span>
        </el-form-item>
        
        <el-form-item label="连接状态">
          <el-tag :type="connectionStatus === 'connected' ? 'success' : connectionStatus === 'testing' ? 'warning' : 'info'">
            {{ connectionStatus === 'connected' ? '已连接' : connectionStatus === 'testing' ? '测试中...' : '未连接' }}
          </el-tag>
        </el-form-item>
        
        <el-form-item>
          <el-button type="primary" @click="saveConfig">保存配置</el-button>
          <el-button @click="testConnection" :loading="testing">测试连接</el-button>
        </el-form-item>
      </el-form>
    </el-card>
  </div>
</template>

<style lang="scss" scoped>
.influxdb-settings {
  h3 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
  }
  
  .form-help {
    margin-left: 12px;
    font-size: 12px;
    color: #909399;
  }
}
</style>