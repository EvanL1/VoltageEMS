<template>
  <div class="config-editor">
    <el-card>
      <template #header>
        <div class="card-header">
          <h2>modsrv 配置</h2>
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
        <el-form label-position="top">
          <el-tabs type="border-card">
            <el-tab-pane label="基本配置">
              <el-form-item label="服务名称">
                <el-input v-model="config.service_name" />
              </el-form-item>
              
              <el-form-item label="日志级别">
                <el-select v-model="config.log_level" style="width: 100%">
                  <el-option label="DEBUG" value="debug" />
                  <el-option label="INFO" value="info" />
                  <el-option label="WARN" value="warn" />
                  <el-option label="ERROR" value="error" />
                </el-select>
              </el-form-item>
              
              <el-form-item label="日志路径">
                <el-input v-model="config.log_path" />
              </el-form-item>
            </el-tab-pane>
            
            <el-tab-pane label="Redis 配置">
              <el-form-item label="Redis 地址">
                <el-input v-model="config.redis.host" />
              </el-form-item>
              
              <el-form-item label="Redis 端口">
                <el-input-number v-model="config.redis.port" :min="1" :max="65535" />
              </el-form-item>
              
              <el-form-item label="Redis 数据库">
                <el-input-number v-model="config.redis.db" :min="0" :max="15" />
              </el-form-item>
            </el-tab-pane>
            
            <el-tab-pane label="模型配置">
              <el-form-item label="模型更新间隔 (ms)">
                <el-input-number v-model="config.model.update_interval_ms" :min="100" :step="100" />
              </el-form-item>
              
              <el-form-item label="启用模型">
                <el-switch v-model="config.model.enabled" />
              </el-form-item>
              
              <el-form-item label="模型参数">
                <el-input
                  type="textarea"
                  v-model="modelParamsStr"
                  :rows="10"
                  @change="updateModelParams"
                />
              </el-form-item>
            </el-tab-pane>
          </el-tabs>
        </el-form>
      </div>
    </el-card>
  </div>
</template>

<script>
import { mapState, mapActions } from 'vuex'

export default {
  name: 'ModsrvConfig',
  data() {
    return {
      modelParamsStr: '',
      config: {
        service_name: 'modsrv',
        log_level: 'info',
        log_path: '/var/log/ems',
        redis: {
          host: 'redis',
          port: 6379,
          db: 0
        },
        model: {
          update_interval_ms: 1000,
          enabled: true,
          params: {}
        }
      }
    }
  },
  computed: {
    ...mapState({
      loading: state => state.loading,
      error: state => state.error,
      savedConfig: state => state.configs.modsrv
    })
  },
  methods: {
    ...mapActions(['fetchConfig', 'saveConfig']),
    
    async loadConfig() {
      await this.fetchConfig('modsrv')
      if (this.savedConfig) {
        this.config = { ...this.savedConfig }
        this.modelParamsStr = JSON.stringify(this.config.model.params, null, 2)
      }
    },
    
    async saveConfig() {
      try {
        await this.$store.dispatch('saveConfig', {
          service: 'modsrv',
          config: this.config
        })
        this.$message.success('配置保存成功')
      } catch (error) {
        this.$message.error('配置保存失败')
      }
    },
    
    resetConfig() {
      if (this.savedConfig) {
        this.config = { ...this.savedConfig }
        this.modelParamsStr = JSON.stringify(this.config.model.params, null, 2)
      }
    },
    
    updateModelParams() {
      try {
        this.config.model.params = JSON.parse(this.modelParamsStr)
      } catch (error) {
        this.$message.error('JSON 格式错误，请检查')
      }
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

/* Enhanced Tabs */
:deep(.el-tabs--border-card) {
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  box-shadow: none;
}

:deep(.el-tabs__header) {
  background: var(--color-surface-hover);
  border-bottom: 1px solid var(--color-border);
}

:deep(.el-tabs__item) {
  color: var(--color-text-secondary);
  font-weight: var(--font-weight-medium);
  transition: all 0.3s ease;
}

:deep(.el-tabs__item:hover) {
  color: var(--color-primary);
}

:deep(.el-tabs__item.is-active) {
  color: var(--color-primary);
  background: var(--color-surface);
  font-weight: var(--font-weight-semibold);
}

:deep(.el-tabs__content) {
  padding: var(--spacing-xl);
}

/* Enhanced Form */
:deep(.el-form-item__label) {
  color: var(--color-text-secondary);
  font-weight: var(--font-weight-medium);
  margin-bottom: var(--spacing-sm);
}

/* Enhanced Inputs */
:deep(.el-input__inner) {
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border);
  padding: var(--spacing-sm) var(--spacing-md);
  font-size: var(--font-size-base);
  transition: all 0.3s ease;
}

:deep(.el-input__inner:focus) {
  border-color: var(--color-primary);
  box-shadow: 0 0 0 3px var(--color-primary-light);
}

:deep(.el-textarea__inner) {
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border);
  padding: var(--spacing-md);
  font-family: 'SF Mono', Monaco, 'Cascadia Code', 'Roboto Mono', Consolas, 'Courier New', monospace;
  font-size: var(--font-size-sm);
  transition: all 0.3s ease;
}

:deep(.el-textarea__inner:focus) {
  border-color: var(--color-primary);
  box-shadow: 0 0 0 3px var(--color-primary-light);
}

/* Enhanced Select */
:deep(.el-select .el-input__inner) {
  cursor: pointer;
}

/* Enhanced Input Number */
:deep(.el-input-number) {
  width: 100%;
}

:deep(.el-input-number__increase),
:deep(.el-input-number__decrease) {
  background: var(--color-surface-hover);
  border-left: 1px solid var(--color-border);
}

/* Enhanced Switch */
:deep(.el-switch__core) {
  border-radius: var(--radius-full);
  background: var(--color-border);
}

:deep(.el-switch.is-checked .el-switch__core) {
  background: var(--gradient-primary);
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