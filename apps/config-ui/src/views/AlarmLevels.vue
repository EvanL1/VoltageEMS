<script setup lang="ts">
import { ref } from 'vue';

const levels = ref([
  { id: 1, name: '严重', code: 'critical', color: '#F56C6C', priority: 1, sound: true, email: true, sms: true },
  { id: 2, name: '错误', code: 'error', color: '#F56C6C', priority: 2, sound: true, email: true, sms: false },
  { id: 3, name: '警告', code: 'warning', color: '#E6A23C', priority: 3, sound: true, email: false, sms: false },
  { id: 4, name: '信息', code: 'info', color: '#909399', priority: 4, sound: false, email: false, sms: false },
]);
</script>

<template>
  <div class="alarm-levels">
    <el-card>
      <template #header>
        <h3>告警等级配置</h3>
      </template>
      
      <el-table :data="levels" style="width: 100%">
        <el-table-column prop="priority" label="优先级" width="80" sortable />
        <el-table-column prop="name" label="等级名称" />
        <el-table-column prop="code" label="等级代码" />
        <el-table-column label="颜色" width="100">
          <template #default="{ row }">
            <div style="display: flex; align-items: center; gap: 8px;">
              <div :style="{ width: '24px', height: '24px', borderRadius: '4px', backgroundColor: row.color }"></div>
              <span>{{ row.color }}</span>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="通知方式" width="240">
          <template #default="{ row }">
            <el-checkbox v-model="row.sound" label="声音" />
            <el-checkbox v-model="row.email" label="邮件" />
            <el-checkbox v-model="row.sms" label="短信" />
          </template>
        </el-table-column>
        <el-table-column label="操作" width="120">
          <template #default>
            <el-button size="small">编辑</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>
  </div>
</template>

<style lang="scss" scoped>
.alarm-levels {
  h3 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
  }
}
</style>