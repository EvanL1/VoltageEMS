<template>
  <div class="system-page">
    <!-- 配置导航栏 -->
    <div class="config-nav">
      <el-tabs tab-position="left" v-model="activeTab" class="config-tabs">
        <el-tab-pane name="modsrv" label="modsrv">
          <div class="tab-content">
            <ModsrvConfig />
          </div>
        </el-tab-pane>
        <el-tab-pane name="netsrv" label="netsrv">
          <div class="tab-content">
            <NetsrvConfig />
          </div>
        </el-tab-pane>
        <el-tab-pane name="comsrv" label="comsrv">
          <div class="tab-content">
            <ComsrvConfig />
          </div>
        </el-tab-pane>
        <el-tab-pane name="hissrv" label="hissrv">
          <div class="tab-content">
            <HissrvConfig />
          </div>
        </el-tab-pane>
        <el-tab-pane name="mosquitto" label="Mosquitto">
          <div class="tab-content">
            <MosquittoConfig />
          </div>
        </el-tab-pane>
      </el-tabs>
    </div>
  </div>
</template>

<script>
import ModsrvConfig from './config/ModsrvConfig.vue'
import NetsrvConfig from './config/NetsrvConfig.vue'
import ComsrvConfig from './config/ComsrvConfig.vue'
import HissrvConfig from './config/HissrvConfig.vue'
import MosquittoConfig from './config/MosquittoConfig.vue'

export default {
  name: 'SystemView',
  components: {
    ModsrvConfig,
    NetsrvConfig,
    ComsrvConfig,
    HissrvConfig,
    MosquittoConfig
  },
  data() {
    return {
      activeTab: 'modsrv'
    }
  },
  watch: {
    // 监听路由参数，切换对应的标签页
    '$route.params.service': {
      immediate: true,
      handler(service) {
        if (service) {
          this.activeTab = service;
        }
      }
    }
  }
}
</script>

<style scoped>
.system-page {
  padding: 0;
  height: 100%;
}

.config-nav {
  height: 100%;
  border-radius: 4px;
  background-color: #fff;
  box-shadow: 0 1px 4px rgba(0, 21, 41, 0.08);
}

.config-tabs {
  height: 100%;
}

.tab-content {
  padding: 0 10px;
}

:deep(.el-tabs__header) {
  margin-right: 20px;
}

:deep(.el-tabs__item) {
  height: 60px;
  line-height: 60px;
  text-align: center;
  font-size: 16px;
}

:deep(.el-tabs__active-bar) {
  width: 4px !important;
}
</style> 