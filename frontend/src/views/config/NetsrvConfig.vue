<template>
  <div class="config-editor">
    <el-card>
      <template #header>
        <div class="card-header">
          <h2>netsrv 配置</h2>
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
        <p>netsrv 配置页面正在开发中...</p>
      </div>
    </el-card>
  </div>
</template>

<script>
import { mapState, mapActions } from 'vuex'

export default {
  name: 'NetsrvConfig',
  data() {
    return {
      config: {}
    }
  },
  computed: {
    ...mapState({
      loading: state => state.loading,
      error: state => state.error,
      savedConfig: state => state.configs.netsrv
    })
  },
  methods: {
    ...mapActions(['fetchConfig', 'saveConfig']),
    
    async loadConfig() {
      // 在实际连接后端前，这里只是模拟
      console.log('加载 netsrv 配置')
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