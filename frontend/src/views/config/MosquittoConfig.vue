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
.config-editor {
  padding: 20px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.header-actions {
  display: flex;
  gap: 10px;
}

.loading-container, .error-container {
  padding: 20px;
  text-align: center;
}

.mt-20 {
  margin-top: 20px;
}
</style> 