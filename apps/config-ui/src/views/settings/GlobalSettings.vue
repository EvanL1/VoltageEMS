<script setup lang="ts">
import { ref } from 'vue';
import { ElMessage } from 'element-plus';

const settings = ref({
  systemName: 'VoltageEMS',
  systemVersion: '1.0.0',
  timezone: 'Asia/Shanghai',
  language: 'zh-CN',
  dateFormat: 'YYYY-MM-DD',
  timeFormat: 'HH:mm:ss',
  logLevel: 'info',
  logRetentionDays: 30,
  autoBackup: true,
  backupInterval: 24,
});

function saveSettings() {
  ElMessage.success('全局设置已保存');
}
</script>

<template>
  <div class="global-settings">
    <el-card>
      <template #header>
        <h3>全局设置</h3>
      </template>
      
      <el-form :model="settings" label-width="140px">
        <el-divider content-position="left">系统信息</el-divider>
        
        <el-form-item label="系统名称">
          <el-input v-model="settings.systemName" />
        </el-form-item>
        
        <el-form-item label="系统版本">
          <el-input v-model="settings.systemVersion" disabled />
        </el-form-item>
        
        <el-divider content-position="left">区域设置</el-divider>
        
        <el-form-item label="时区">
          <el-select v-model="settings.timezone">
            <el-option label="Asia/Shanghai" value="Asia/Shanghai" />
            <el-option label="Asia/Tokyo" value="Asia/Tokyo" />
            <el-option label="UTC" value="UTC" />
          </el-select>
        </el-form-item>
        
        <el-form-item label="语言">
          <el-select v-model="settings.language">
            <el-option label="简体中文" value="zh-CN" />
            <el-option label="English" value="en-US" />
          </el-select>
        </el-form-item>
        
        <el-form-item label="日期格式">
          <el-select v-model="settings.dateFormat">
            <el-option label="YYYY-MM-DD" value="YYYY-MM-DD" />
            <el-option label="DD/MM/YYYY" value="DD/MM/YYYY" />
            <el-option label="MM/DD/YYYY" value="MM/DD/YYYY" />
          </el-select>
        </el-form-item>
        
        <el-divider content-position="left">日志设置</el-divider>
        
        <el-form-item label="日志级别">
          <el-select v-model="settings.logLevel">
            <el-option label="Debug" value="debug" />
            <el-option label="Info" value="info" />
            <el-option label="Warning" value="warning" />
            <el-option label="Error" value="error" />
          </el-select>
        </el-form-item>
        
        <el-form-item label="日志保留天数">
          <el-input-number v-model="settings.logRetentionDays" :min="7" :max="365" />
        </el-form-item>
        
        <el-divider content-position="left">备份设置</el-divider>
        
        <el-form-item label="自动备份">
          <el-switch v-model="settings.autoBackup" />
        </el-form-item>
        
        <el-form-item label="备份间隔(小时)">
          <el-input-number v-model="settings.backupInterval" :min="1" :max="168" :disabled="!settings.autoBackup" />
        </el-form-item>
        
        <el-form-item>
          <el-button type="primary" @click="saveSettings">保存设置</el-button>
          <el-button>恢复默认</el-button>
        </el-form-item>
      </el-form>
    </el-card>
  </div>
</template>

<style lang="scss" scoped>
.global-settings {
  h3 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
  }
}
</style>