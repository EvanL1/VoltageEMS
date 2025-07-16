<template>
  <div class="config-editor">
    <el-card>
      <template #header>
        <div class="card-header">
          <h2>Mosquitto 配置</h2>
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
              <el-form-item label="监听端口">
                <el-input-number v-model="config.listener" :min="1" :max="65535" />
              </el-form-item>
              
              <el-form-item label="允许匿名访问">
                <el-switch v-model="config.allow_anonymous" />
              </el-form-item>
            </el-tab-pane>
            
            <el-tab-pane label="持久化设置">
              <el-form-item label="启用持久化">
                <el-switch v-model="config.persistence" />
              </el-form-item>
              
              <el-form-item label="持久化路径">
                <el-input v-model="config.persistence_location" />
              </el-form-item>
            </el-tab-pane>
            
            <el-tab-pane label="日志设置">
              <el-form-item label="日志目标">
                <el-select v-model="config.log_dest" style="width: 100%">
                  <el-option label="文件" value="file" />
                  <el-option label="标准输出" value="stdout" />
                  <el-option label="系统日志" value="syslog" />
                </el-select>
              </el-form-item>
              
              <el-form-item label="日志文件路径" v-if="config.log_dest === 'file'">
                <el-input v-model="config.log_file" />
              </el-form-item>
              
              <el-form-item label="日志类型">
                <el-select v-model="config.log_type" style="width: 100%">
                  <el-option label="全部" value="all" />
                  <el-option label="信息" value="info" />
                  <el-option label="通知" value="notice" />
                  <el-option label="警告" value="warning" />
                  <el-option label="错误" value="error" />
                </el-select>
              </el-form-item>
            </el-tab-pane>
            
            <el-tab-pane label="安全设置">
              <el-form-item label="启用 TLS">
                <el-switch v-model="config.tls_enabled" />
              </el-form-item>
              
              <template v-if="config.tls_enabled">
                <el-form-item label="证书文件">
                  <el-input v-model="config.certfile" />
                </el-form-item>
                
                <el-form-item label="密钥文件">
                  <el-input v-model="config.keyfile" />
                </el-form-item>
                
                <el-form-item label="CA 文件">
                  <el-input v-model="config.cafile" />
                </el-form-item>
              </template>
              
              <el-form-item label="启用密码认证">
                <el-switch v-model="config.password_file_enabled" />
              </el-form-item>
              
              <el-form-item label="密码文件路径" v-if="config.password_file_enabled">
                <el-input v-model="config.password_file" />
              </el-form-item>
            </el-tab-pane>
            
            <el-tab-pane label="原始配置">
              <el-form-item label="配置文件内容">
                <el-input
                  type="textarea"
                  v-model="rawConfig"
                  :rows="20"
                  @change="parseRawConfig"
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
  name: 'MosquittoConfig',
  data() {
    return {
      rawConfig: '',
      config: {
        listener: 1883,
        allow_anonymous: true,
        persistence: true,
        persistence_location: '/mosquitto/data/',
        log_dest: 'file',
        log_file: '/mosquitto/log/mosquitto.log',
        log_type: 'all',
        tls_enabled: false,
        certfile: '',
        keyfile: '',
        cafile: '',
        password_file_enabled: false,
        password_file: ''
      }
    }
  },
  computed: {
    ...mapState({
      loading: state => state.loading,
      error: state => state.error,
      savedConfig: state => state.configs.mosquitto
    })
  },
  methods: {
    ...mapActions(['fetchConfig', 'saveConfig']),
    
    async loadConfig() {
      await this.fetchConfig('mosquitto')
      if (this.savedConfig) {
        // 如果是原始文本格式，需要解析
        if (typeof this.savedConfig === 'string') {
          this.rawConfig = this.savedConfig
          this.parseRawConfig()
        } else {
          this.config = { ...this.savedConfig }
          this.generateRawConfig()
        }
      }
    },
    
    async saveConfig() {
      try {
        // 根据当前视图生成最终配置
        this.generateRawConfig()
        
        await this.$store.dispatch('saveConfig', {
          service: 'mosquitto',
          config: this.rawConfig
        })
        this.$message.success('配置保存成功')
      } catch (error) {
        this.$message.error('配置保存失败')
      }
    },
    
    resetConfig() {
      if (this.savedConfig) {
        if (typeof this.savedConfig === 'string') {
          this.rawConfig = this.savedConfig
          this.parseRawConfig()
        } else {
          this.config = { ...this.savedConfig }
          this.generateRawConfig()
        }
      }
    },
    
    parseRawConfig() {
      // 从原始配置文本解析配置对象
      const lines = this.rawConfig.split('\n')
      const config = {
        listener: 1883,
        allow_anonymous: true,
        persistence: true,
        persistence_location: '/mosquitto/data/',
        log_dest: 'file',
        log_file: '/mosquitto/log/mosquitto.log',
        log_type: 'all',
        tls_enabled: false,
        certfile: '',
        keyfile: '',
        cafile: '',
        password_file_enabled: false,
        password_file: ''
      }
      
      lines.forEach(line => {
        line = line.trim()
        if (!line || line.startsWith('#')) return
        
        const parts = line.split(' ')
        const key = parts[0]
        const value = parts.slice(1).join(' ')
        
        switch (key) {
          case 'listener':
            config.listener = parseInt(value)
            break
          case 'allow_anonymous':
            config.allow_anonymous = value === 'true'
            break
          case 'persistence':
            config.persistence = value === 'true'
            break
          case 'persistence_location':
            config.persistence_location = value
            break
          case 'log_dest':
            config.log_dest = value
            if (value === 'file') {
              // 下一个参数应该是文件路径
              const fileIndex = lines.findIndex(l => 
                l.trim().startsWith('log_dest file')
              )
              if (fileIndex >= 0 && fileIndex < lines.length - 1) {
                const nextLine = lines[fileIndex + 1].trim()
                if (nextLine && !nextLine.startsWith('#')) {
                  config.log_file = nextLine.split(' ').slice(1).join(' ')
                }
              }
            }
            break
          case 'log_type':
            config.log_type = value
            break
          case 'certfile':
            config.tls_enabled = true
            config.certfile = value
            break
          case 'keyfile':
            config.tls_enabled = true
            config.keyfile = value
            break
          case 'cafile':
            config.tls_enabled = true
            config.cafile = value
            break
          case 'password_file':
            config.password_file_enabled = true
            config.password_file = value
            break
        }
      })
      
      this.config = config
    },
    
    generateRawConfig() {
      // 从配置对象生成原始配置文本
      let rawConfig = '# Mosquitto 配置文件\n\n'
      
      // 基本配置
      rawConfig += '# 基本配置\n'
      rawConfig += `listener ${this.config.listener}\n`
      rawConfig += `allow_anonymous ${this.config.allow_anonymous}\n\n`
      
      // 持久化设置
      rawConfig += '# 持久化设置\n'
      rawConfig += `persistence ${this.config.persistence}\n`
      if (this.config.persistence) {
        rawConfig += `persistence_location ${this.config.persistence_location}\n\n`
      }
      
      // 日志设置
      rawConfig += '# 日志设置\n'
      rawConfig += `log_dest ${this.config.log_dest}`
      if (this.config.log_dest === 'file') {
        rawConfig += ` ${this.config.log_file}`
      }
      rawConfig += '\n'
      rawConfig += `log_type ${this.config.log_type} \n\n`
      
      // 安全设置
      if (this.config.tls_enabled) {
        rawConfig += '# TLS/SSL 设置\n'
        if (this.config.certfile) {
          rawConfig += `certfile ${this.config.certfile}\n`
        }
        if (this.config.keyfile) {
          rawConfig += `keyfile ${this.config.keyfile}\n`
        }
        if (this.config.cafile) {
          rawConfig += `cafile ${this.config.cafile}\n`
        }
        rawConfig += '\n'
      }
      
      if (this.config.password_file_enabled) {
        rawConfig += '# 认证设置\n'
        rawConfig += `password_file ${this.config.password_file}\n\n`
      }
      
      this.rawConfig = rawConfig
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