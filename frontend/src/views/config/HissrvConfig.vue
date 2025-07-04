<template>
  <div class="config-editor">
    <el-card>
      <template #header>
        <div class="card-header">
          <h2>hissrv 配置</h2>
          <div class="header-actions">
            <el-button type="primary" @click="saveConfig" :loading="loading">保存配置</el-button>
            <el-button @click="resetConfig">重置</el-button>
          </div>
        </div>
      </template>
      
      <div v-if="loading" class="loading-container">
        <el-skeleton :rows="10" animated />
      </div>
      
      <div v-else-if="error" class="error-container">
        <el-alert
          :title="error"
          type="error"
          description="无法加载配置文件，请检查服务状态或网络连接。"
          show-icon
        />
        <el-button class="mt-20" @click="fetchConfig">重试</el-button>
      </div>
      
      <div v-else>
        <p>hissrv 配置页面正在开发中...</p>
      </div>
    </el-card>
  </div>
</template>

<script>
import { mapState, mapActions } from 'vuex'

export default {
  name: 'HissrvConfig',
  data() {
    return {
      config: {}
    }
  },
  computed: {
    ...mapState({
      loading: state => state.loading,
      error: state => state.error,
      savedConfig: state => state.configs.hissrv
    })
  },
  methods: {
    ...mapActions(['fetchConfig', 'saveConfig']),
    
    async loadConfig() {
      // 在实际连接后端前，这里只是模拟
      console.log('加载 hissrv 配置')
    },
    
    async saveConfig() {
      // 在实际连接后端前，这里只是模拟
      this.$message.success('配置保存成功（模拟）')
    },
    
    resetConfig() {
      // 在实际连接后端前，这里只是模拟
      this.$message.info('配置已重置（模拟）')
    }
  },
  created() {
    this.loadConfig()
  }
}
</script>

<style scoped>
@import '@/styles/design-tokens.scss';

.config-editor {
  padding: var(--page-padding);
  background: var(--color-background);
  min-height: 100vh;
}

/* Apple Style Page Header */
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--spacing-xl);
}

.card-header h2 {
  font-size: var(--font-size-page-title);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  margin: 0;
  letter-spacing: -0.5px;
}

.header-actions {
  display: flex;
  gap: var(--spacing-md);
}

/* Tesla Style Cards */
.el-card {
  background: var(--color-surface);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  border: 1px solid var(--color-border);
  transition: all 0.3s ease;
}

.el-card:hover {
  box-shadow: var(--shadow-md);
  transform: translateY(-2px);
}

:deep(.el-card__header) {
  border-bottom: 1px solid var(--color-border);
  padding: var(--spacing-xl);
  background: var(--color-surface-hover);
}

:deep(.el-card__body) {
  padding: var(--spacing-xl);
}

/* Loading and Error States */
.loading-container, .error-container {
  padding: var(--spacing-xxl);
  text-align: center;
  min-height: 400px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}

.mt-20 {
  margin-top: var(--spacing-lg);
}

/* Enhanced Buttons */
:deep(.el-button) {
  border-radius: var(--radius-md);
  font-weight: var(--font-weight-medium);
  padding: var(--spacing-sm) var(--spacing-lg);
  transition: all 0.3s ease;
  border: none;
  box-shadow: var(--shadow-sm);
}

:deep(.el-button--primary) {
  background: var(--gradient-primary);
  color: white;
}

:deep(.el-button--primary:hover) {
  transform: translateY(-1px);
  box-shadow: var(--shadow-md);
  opacity: 0.9;
}

:deep(.el-button--default) {
  background: var(--color-surface);
  color: var(--color-text-primary);
  border: 1px solid var(--color-border);
}

:deep(.el-button--default:hover) {
  background: var(--color-surface-hover);
  border-color: var(--color-primary);
}

/* Alert Styling */
:deep(.el-alert) {
  border-radius: var(--radius-md);
  border: none;
  box-shadow: var(--shadow-sm);
}

:deep(.el-alert--error) {
  background: linear-gradient(135deg, #fee, #fdd);
}

/* Skeleton Loading */
:deep(.el-skeleton__item) {
  background: linear-gradient(90deg, var(--color-surface) 25%, var(--color-surface-hover) 50%, var(--color-surface) 75%);
  background-size: 200% 100%;
  animation: skeleton-loading 1.4s ease infinite;
}

@keyframes skeleton-loading {
  0% {
    background-position: 200% 0;
  }
  100% {
    background-position: -200% 0;
  }
}

/* Responsive Design */
@media (max-width: 768px) {
  .config-editor {
    padding: var(--spacing-md);
  }
  
  .card-header {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--spacing-md);
  }
  
  .header-actions {
    width: 100%;
  }
  
  .header-actions .el-button {
    flex: 1;
  }
}
</style> 