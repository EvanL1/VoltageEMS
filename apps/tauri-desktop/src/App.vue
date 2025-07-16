<template>
  <el-config-provider :locale="locale">
    <router-view />
  </el-config-provider>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import enUs from 'element-plus/es/locale/lang/en'

const locale = ref(enUs)
</script>

<style lang="scss">
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body, #app {
  height: 100%;
  width: 100%;
}

.app-container {
  height: 100%;
  background-color: #f5f7fa;
  
  .el-container {
    height: 100%;
  }
  
  .app-header {
    background-color: #1e1e1e;
    color: #fff;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.12);
    
    .header-content {
      display: flex;
      justify-content: space-between;
      align-items: center;
      height: 100%;
      
      h1 {
        font-size: 24px;
        font-weight: 500;
      }
      
      .header-info {
        font-size: 14px;
        opacity: 0.8;
      }
    }
  }
  
  .app-main {
    padding: 0;
    overflow: auto;
  }
}
</style>