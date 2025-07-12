<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { logger } from '@/utils/logger';
import { Document, Refresh, Delete, Download } from '@element-plus/icons-vue';

const logs = ref<string[]>([]);
const logPath = ref('');
const autoRefresh = ref(true);
let refreshTimer: any = null;

// 获取日志路径
async function getLogPath() {
  try {
    logPath.value = await invoke<string>('get_log_path');
    logger.info('Log path retrieved', { path: logPath.value });
  } catch (error) {
    logger.error('Failed to get log path', { error });
  }
}

// 读取最近的日志
async function loadLogs() {
  try {
    const recentLogs = await invoke<string[]>('read_recent_logs', { lines: 100 });
    logs.value = recentLogs;
  } catch (error) {
    logger.error('Failed to load logs', { error });
  }
}

// 刷新日志
function refreshLogs() {
  loadLogs();
}

// 清空显示的日志
function clearLogs() {
  logs.value = [];
  logger.clearLogBuffer();
}

// 导出日志
async function exportLogs() {
  const content = await logger.exportLogs();
  const blob = new Blob([content], { type: 'text/plain' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `voltage-config-logs-${new Date().toISOString().split('T')[0]}.txt`;
  a.click();
  URL.revokeObjectURL(url);
}

// 自动刷新
function startAutoRefresh() {
  if (refreshTimer) clearInterval(refreshTimer);
  if (autoRefresh.value) {
    refreshTimer = setInterval(loadLogs, 2000);
  }
}

onMounted(() => {
  getLogPath();
  loadLogs();
  startAutoRefresh();
});

onUnmounted(() => {
  if (refreshTimer) clearInterval(refreshTimer);
});

// 监听自动刷新变化
import { watch } from 'vue';
watch(autoRefresh, () => {
  startAutoRefresh();
});
</script>

<template>
  <div class="log-viewer">
    <div class="log-header">
      <div class="header-left">
        <h3>
          <el-icon><Document /></el-icon>
          系统日志
        </h3>
        <el-text type="info" size="small">{{ logPath }}</el-text>
      </div>
      <div class="header-right">
        <el-checkbox v-model="autoRefresh">自动刷新</el-checkbox>
        <el-button :icon="Refresh" @click="refreshLogs">刷新</el-button>
        <el-button :icon="Delete" @click="clearLogs">清空</el-button>
        <el-button :icon="Download" @click="exportLogs">导出</el-button>
      </div>
    </div>
    
    <div class="log-content">
      <pre v-if="logs.length > 0">{{ logs.join('\n') }}</pre>
      <el-empty v-else description="暂无日志" />
    </div>
  </div>
</template>

<style lang="scss" scoped>
.log-viewer {
  height: 100%;
  display: flex;
  flex-direction: column;
  
  .log-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px;
    border-bottom: 1px solid #e4e7ed;
    background: #fff;
    
    .header-left {
      display: flex;
      align-items: center;
      gap: 12px;
      
      h3 {
        margin: 0;
        font-size: 16px;
        display: flex;
        align-items: center;
        gap: 8px;
      }
    }
    
    .header-right {
      display: flex;
      align-items: center;
      gap: 12px;
    }
  }
  
  .log-content {
    flex: 1;
    overflow: auto;
    background: #1e1e1e;
    color: #d4d4d4;
    padding: 16px;
    
    pre {
      margin: 0;
      font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
      font-size: 13px;
      line-height: 1.5;
      white-space: pre-wrap;
      word-wrap: break-word;
    }
  }
}
</style>