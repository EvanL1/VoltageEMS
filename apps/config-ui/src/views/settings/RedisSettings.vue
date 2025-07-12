<script setup lang="ts">
import { ref } from 'vue';
import { ElMessage } from 'element-plus';

const redisConfig = ref({
  host: 'localhost',
  port: 6379,
  password: '',
  database: 0,
  connectionTimeout: 5000,
  maxRetries: 3,
  retryDelay: 1000,
  poolSize: 10,
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
    ElMessage.success('Redis连接测试成功');
  }, 2000);
}

function saveConfig() {
  ElMessage.success('Redis配置已保存');
}
</script>

<template>
  <div class="redis-settings">
    <el-card>
      <template #header>
        <h3>Redis 配置</h3>
      </template>
      
      <el-form :model="redisConfig" label-width="140px">
        <el-form-item label="主机地址" required>
          <el-input v-model="redisConfig.host" placeholder="localhost" />
        </el-form-item>
        
        <el-form-item label="端口" required>
          <el-input-number v-model="redisConfig.port" :min="1" :max="65535" />
        </el-form-item>
        
        <el-form-item label="密码">
          <el-input v-model="redisConfig.password" type="password" placeholder="留空表示无密码" show-password />
        </el-form-item>
        
        <el-form-item label="数据库">
          <el-input-number v-model="redisConfig.database" :min="0" :max="15" />
        </el-form-item>
        
        <el-form-item label="连接超时(ms)">
          <el-input-number v-model="redisConfig.connectionTimeout" :min="1000" :max="30000" :step="1000" />
        </el-form-item>
        
        <el-form-item label="最大重试次数">
          <el-input-number v-model="redisConfig.maxRetries" :min="0" :max="10" />
        </el-form-item>
        
        <el-form-item label="重试延迟(ms)">
          <el-input-number v-model="redisConfig.retryDelay" :min="100" :max="5000" :step="100" />
        </el-form-item>
        
        <el-form-item label="连接池大小">
          <el-input-number v-model="redisConfig.poolSize" :min="1" :max="100" />
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
.redis-settings {
  h3 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
  }
}
</style>