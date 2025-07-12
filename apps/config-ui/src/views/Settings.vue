<script setup lang="ts">
import { ref } from 'vue';

const settings = ref({
  redis: {
    url: 'redis://localhost:6379',
    connectionTimeout: 5000,
    retryAttempts: 3,
  },
  ui: {
    theme: 'light',
    language: 'zh-CN',
    autoRefresh: true,
    refreshInterval: 30,
  },
  notifications: {
    enabled: true,
    soundEnabled: false,
    desktopEnabled: true,
  },
});

function saveSettings() {
  // TODO: 保存设置到本地存储或配置文件
  localStorage.setItem('voltage-config-ui-settings', JSON.stringify(settings.value));
  ElMessage.success('设置已保存');
}

function resetSettings() {
  settings.value = {
    redis: {
      url: 'redis://localhost:6379',
      connectionTimeout: 5000,
      retryAttempts: 3,
    },
    ui: {
      theme: 'light',
      language: 'zh-CN',
      autoRefresh: true,
      refreshInterval: 30,
    },
    notifications: {
      enabled: true,
      soundEnabled: false,
      desktopEnabled: true,
    },
  };
  ElMessage.info('设置已重置');
}
</script>

<template>
  <div class="settings">
    <h2>系统设置</h2>
    
    <el-form :model="settings" label-width="140px">
      <el-divider content-position="left">Redis 连接</el-divider>
      
      <el-form-item label="Redis URL">
        <el-input v-model="settings.redis.url" placeholder="redis://localhost:6379" />
      </el-form-item>
      
      <el-form-item label="连接超时">
        <el-input-number
          v-model="settings.redis.connectionTimeout"
          :min="1000"
          :max="30000"
          :step="1000"
        />
        <span style="margin-left: 10px">毫秒</span>
      </el-form-item>
      
      <el-form-item label="重试次数">
        <el-input-number
          v-model="settings.redis.retryAttempts"
          :min="0"
          :max="10"
        />
      </el-form-item>
      
      <el-divider content-position="left">界面设置</el-divider>
      
      <el-form-item label="主题">
        <el-radio-group v-model="settings.ui.theme">
          <el-radio label="light">浅色</el-radio>
          <el-radio label="dark">深色</el-radio>
        </el-radio-group>
      </el-form-item>
      
      <el-form-item label="语言">
        <el-select v-model="settings.ui.language">
          <el-option label="简体中文" value="zh-CN" />
          <el-option label="English" value="en-US" />
        </el-select>
      </el-form-item>
      
      <el-form-item label="自动刷新">
        <el-switch v-model="settings.ui.autoRefresh" />
      </el-form-item>
      
      <el-form-item label="刷新间隔" v-if="settings.ui.autoRefresh">
        <el-input-number
          v-model="settings.ui.refreshInterval"
          :min="10"
          :max="300"
          :step="10"
        />
        <span style="margin-left: 10px">秒</span>
      </el-form-item>
      
      <el-divider content-position="left">通知设置</el-divider>
      
      <el-form-item label="启用通知">
        <el-switch v-model="settings.notifications.enabled" />
      </el-form-item>
      
      <el-form-item label="声音提醒" v-if="settings.notifications.enabled">
        <el-switch v-model="settings.notifications.soundEnabled" />
      </el-form-item>
      
      <el-form-item label="桌面通知" v-if="settings.notifications.enabled">
        <el-switch v-model="settings.notifications.desktopEnabled" />
      </el-form-item>
      
      <el-form-item>
        <el-button type="primary" @click="saveSettings">保存设置</el-button>
        <el-button @click="resetSettings">重置默认</el-button>
      </el-form-item>
    </el-form>
  </div>
</template>

<style lang="scss" scoped>
.settings {
  max-width: 800px;
  
  h2 {
    margin-bottom: 20px;
    font-size: 24px;
  }
  
  .el-divider {
    margin: 30px 0 20px;
  }
}
</style>