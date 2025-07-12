<script setup lang="ts">
import { computed } from 'vue';
import { useRouter } from 'vue-router';
import type { ServiceInfo } from '@/types/config';

const props = defineProps<{
  service: ServiceInfo;
}>();

const router = useRouter();

const statusType = computed(() => {
  switch (props.service.status) {
    case 'running':
      return 'success';
    case 'stopped':
      return 'info';
    case 'error':
      return 'danger';
    default:
      return 'warning';
  }
});

const statusText = computed(() => {
  switch (props.service.status) {
    case 'running':
      return '运行中';
    case 'stopped':
      return '已停止';
    case 'error':
      return '异常';
    default:
      return '未知';
  }
});

function navigateToConfig() {
  router.push(`/service/${props.service.name}`);
}
</script>

<template>
  <el-card class="service-card" :body-style="{ padding: '20px' }">
    <div class="card-header">
      <div class="service-info">
        <h4>{{ service.name }}</h4>
        <span class="version">v{{ service.version }}</span>
      </div>
      <el-tag :type="statusType" size="small">
        {{ statusText }}
      </el-tag>
    </div>
    
    <el-descriptions :column="1" size="small" class="metrics">
      <el-descriptions-item label="运行时间">
        {{ service.uptime }}
      </el-descriptions-item>
      <el-descriptions-item label="内存使用">
        {{ service.memory }}
      </el-descriptions-item>
      <el-descriptions-item label="连接数">
        {{ service.connections }}
      </el-descriptions-item>
    </el-descriptions>
    
    <div class="card-footer">
      <el-button
        type="primary"
        size="small"
        @click="navigateToConfig"
      >
        配置管理
      </el-button>
      <el-button size="small">
        查看日志
      </el-button>
    </div>
  </el-card>
</template>

<style lang="scss" scoped>
.service-card {
  margin-bottom: 20px;
  transition: all 0.3s;
  
  &:hover {
    box-shadow: 0 2px 12px 0 rgba(0, 0, 0, 0.1);
  }
  
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 15px;
    
    .service-info {
      h4 {
        margin: 0;
        font-size: 16px;
        font-weight: 500;
      }
      
      .version {
        color: #909399;
        font-size: 12px;
      }
    }
  }
  
  .metrics {
    margin-bottom: 15px;
    
    :deep(.el-descriptions__label) {
      width: 80px;
    }
  }
  
  .card-footer {
    display: flex;
    gap: 10px;
  }
}
</style>