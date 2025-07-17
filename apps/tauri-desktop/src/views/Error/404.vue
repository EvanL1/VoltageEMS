<template>
  <div class="error-page">
    <div class="error-content">
      <div class="error-icon">
        <el-icon size="120" color="#909399"><DocumentDelete /></el-icon>
      </div>
      <h1>404</h1>
      <h2>Page Not Found</h2>
      <p>Sorry, the page you are looking for does not exist.</p>
      <el-button type="primary" @click="goHome">
        <el-icon><House /></el-icon>
        Back to Home
      </el-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useRouter } from 'vue-router'
import { DocumentDelete, House } from '@element-plus/icons-vue'

const router = useRouter()

function goHome() {
  router.push('/')
}
</script>

<style lang="scss" scoped>
.error-page {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #f5f7fa;
  
  .error-content {
    text-align: center;
    
    .error-icon {
      margin-bottom: 20px;
    }
    
    h1 {
      font-size: 72px;
      font-weight: 600;
      color: #303133;
      margin: 0;
    }
    
    h2 {
      font-size: 24px;
      color: #606266;
      margin: 10px 0;
    }
    
    p {
      color: #909399;
      margin: 20px 0 30px;
    }
  }
}
</style>