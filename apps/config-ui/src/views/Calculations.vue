<script setup lang="ts">
import { ref } from 'vue';
import { ElMessage } from 'element-plus';

const calculations = ref([
  { id: 1, name: '功率计算', type: 'formula', expression: 'voltage * current', enabled: true },
  { id: 2, name: '平均值计算', type: 'aggregation', expression: 'AVG(temperature)', enabled: true },
  { id: 3, name: '累积量计算', type: 'accumulation', expression: 'SUM(power)', enabled: false },
]);
</script>

<template>
  <div class="calculations">
    <el-card>
      <template #header>
        <div class="card-header">
          <h3>计算配置</h3>
          <el-button type="primary">新建计算</el-button>
        </div>
      </template>
      
      <el-table :data="calculations" style="width: 100%">
        <el-table-column prop="id" label="ID" width="60" />
        <el-table-column prop="name" label="名称" />
        <el-table-column prop="type" label="类型" width="120">
          <template #default="{ row }">
            <el-tag>{{ row.type === 'formula' ? '公式' : row.type === 'aggregation' ? '聚合' : '累积' }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="expression" label="表达式" />
        <el-table-column prop="enabled" label="状态" width="80">
          <template #default="{ row }">
            <el-switch v-model="row.enabled" />
          </template>
        </el-table-column>
        <el-table-column label="操作" width="180">
          <template #default>
            <el-button size="small">编辑</el-button>
            <el-button size="small" type="danger">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>
  </div>
</template>

<style lang="scss" scoped>
.calculations {
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    
    h3 {
      margin: 0;
      font-size: 18px;
      font-weight: 600;
    }
  }
}
</style>