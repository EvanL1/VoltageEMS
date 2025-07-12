<script setup lang="ts">
import { ref } from 'vue';
import { ElMessage } from 'element-plus';

const activeTab = ref('overview');
</script>

<template>
  <div class="data-flow">
    <el-card>
      <template #header>
        <h3>数据流配置</h3>
      </template>
      <el-tabs v-model="activeTab">
        <el-tab-pane label="概览" name="overview">
          <el-empty description="数据流配置功能开发中..." />
        </el-tab-pane>
        <el-tab-pane label="数据源" name="sources">
          <el-empty description="数据源配置功能开发中..." />
        </el-tab-pane>
        <el-tab-pane label="处理规则" name="rules">
          <el-empty description="处理规则配置功能开发中..." />
        </el-tab-pane>
        <el-tab-pane label="输出目标" name="targets">
          <el-empty description="输出目标配置功能开发中..." />
        </el-tab-pane>
      </el-tabs>
    </el-card>
  </div>
</template>

<style lang="scss" scoped>
.data-flow {
  h3 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
  }
}
</style>