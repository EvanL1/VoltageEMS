<template>
  <div class="condition-node">
    <div class="condition-header">
      <span class="condition-index">#{{ index + 1 }}</span>
      <el-button type="danger" size="small" text @click="$emit('remove')">
        <el-icon><Delete /></el-icon>
      </el-button>
    </div>
    
    <el-row :gutter="10">
      <el-col :span="6">
        <el-select v-model="localCondition.field" placeholder="Select field" size="small">
          <el-option-group label="Measurements">
            <el-option label="Voltage" value="voltage" />
            <el-option label="Current" value="current" />
            <el-option label="Power" value="power" />
            <el-option label="Temperature" value="temperature" />
            <el-option label="Pressure" value="pressure" />
          </el-option-group>
          <el-option-group label="Status">
            <el-option label="Device Status" value="device_status" />
            <el-option label="Alarm Active" value="alarm_active" />
            <el-option label="Connection Status" value="connection_status" />
          </el-option-group>
        </el-select>
      </el-col>
      
      <el-col :span="4">
        <el-select v-model="localCondition.operator" placeholder="Operator" size="small">
          <el-option label="=" value="=" />
          <el-option label="!=" value="!=" />
          <el-option label=">" value=">" />
          <el-option label=">=" value=">=" />
          <el-option label="<" value="<" />
          <el-option label="<=" value="<=" />
          <el-option label="contains" value="contains" />
          <el-option label="between" value="between" />
        </el-select>
      </el-col>
      
      <el-col :span="6">
        <el-input
          v-model="localCondition.value"
          placeholder="Value"
          size="small"
        />
      </el-col>
      
      <el-col :span="4" v-if="localCondition.operator === 'between'">
        <el-input
          v-model="localCondition.value2"
          placeholder="Max value"
          size="small"
        />
      </el-col>
      
      <el-col :span="4">
        <el-input
          v-model="localCondition.unit"
          placeholder="Unit"
          size="small"
        />
      </el-col>
    </el-row>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { Delete } from '@element-plus/icons-vue'

const props = defineProps<{
  condition: any
  index: number
}>()

const emit = defineEmits<{
  remove: []
  update: [condition: any]
}>()

const localCondition = ref({ ...props.condition })

watch(localCondition, (newVal) => {
  emit('update', newVal)
}, { deep: true })
</script>

<style lang="scss" scoped>
.condition-node {
  padding: 15px;
  border: 1px solid #e4e7ed;
  border-radius: 4px;
  margin-bottom: 10px;
  
  .condition-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
    
    .condition-index {
      font-weight: 500;
      color: #909399;
    }
  }
}
</style>