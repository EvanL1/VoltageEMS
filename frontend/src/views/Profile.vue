<template>
  <div class="page-container">
    <div class="page-header">
      <h1>{{ $t('menu.pageName') }}</h1>
      <div class="header-actions">
        <el-button type="primary">
          <el-icon><Plus /></el-icon>
          {{ $t('common.add') }}
        </el-button>
      </div>
    </div>

    <el-card>
      <div class="coming-soon">
        <el-icon :size="64" color="#409eff">
          <Tools />
        </el-icon>
        <h2>{{ $t('common.developing', '功能开发中') }}</h2>
        <p>{{ $t('common.developingDesc', '该功能正在开发中，敬请期待') }}</p>
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { Plus, Tools } from '@element-plus/icons-vue'
</script>

<style lang="scss" scoped>
.page-container {
  padding: 20px;
}

.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
  
  h1 {
    margin: 0;
    font-size: 24px;
    font-weight: 500;
  }
}

.coming-soon {
  text-align: center;
  padding: 80px 20px;
  
  h2 {
    margin: 20px 0 10px;
    color: #303133;
  }
  
  p {
    color: #909399;
  }
}
</style>