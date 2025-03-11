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