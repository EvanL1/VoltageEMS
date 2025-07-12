<script setup lang="ts">
import { ref } from 'vue';
import { ElMessage } from 'element-plus';
import { Warning, Bell, CircleCheck } from '@element-plus/icons-vue';

const rules = ref([
  { id: 1, name: '温度超限', type: 'threshold', condition: '> 80°C', level: 'critical', enabled: true },
  { id: 2, name: '电压异常', type: 'range', condition: '< 180V 或 > 260V', level: 'warning', enabled: true },
  { id: 3, name: '通信中断', type: 'timeout', condition: '> 60秒', level: 'error', enabled: true },
  { id: 4, name: '功率因数低', type: 'threshold', condition: '< 0.85', level: 'info', enabled: false },
]);

function getLevelType(level: string) {
  switch(level) {
    case 'critical': return 'danger';
    case 'error': return 'danger';
    case 'warning': return 'warning';
    case 'info': return 'info';
    default: return 'info';
  }
}

function getLevelIcon(level: string) {
  switch(level) {
    case 'critical': return Warning;
    case 'error': return Warning;
    case 'warning': return Bell;
    case 'info': return CircleCheck;
    default: return CircleCheck;
  }
}
</script>

<template>
  <div class="alarm-rules">
    <el-card>
      <template #header>
        <div class="card-header">
          <h3>告警规则</h3>
          <el-button type="primary">新建规则</el-button>
        </div>
      </template>
      
      <el-table :data="rules" style="width: 100%">
        <el-table-column prop="id" label="ID" width="60" />
        <el-table-column prop="name" label="规则名称" />
        <el-table-column prop="type" label="类型" width="100">
          <template #default="{ row }">
            <el-tag size="small">
              {{ row.type === 'threshold' ? '阈值' : row.type === 'range' ? '范围' : '超时' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="condition" label="触发条件" />
        <el-table-column prop="level" label="告警级别" width="100">
          <template #default="{ row }">
            <el-tag :type="getLevelType(row.level)">
              <el-icon><component :is="getLevelIcon(row.level)" /></el-icon>
              {{ row.level === 'critical' ? '严重' : row.level === 'error' ? '错误' : row.level === 'warning' ? '警告' : '信息' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="enabled" label="状态" width="80">
          <template #default="{ row }">
            <el-switch v-model="row.enabled" />
          </template>
        </el-table-column>
        <el-table-column label="操作" width="200">
          <template #default>
            <el-button size="small">编辑</el-button>
            <el-button size="small">测试</el-button>
            <el-button size="small" type="danger">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>
  </div>
</template>

<style lang="scss" scoped>
.alarm-rules {
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