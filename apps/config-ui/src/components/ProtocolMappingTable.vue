<template>
  <el-table :data="mappings" height="400" style="width: 100%">
    <el-table-column prop="point_id" label="点位ID" width="80" />
    <el-table-column prop="signal_name" label="信号名称" />
    
    <!-- Modbus 特定字段 -->
    <template v-if="isModbus">
      <el-table-column prop="slave_id" label="从站ID" width="80" />
      <el-table-column prop="function_code" label="功能码" width="80" />
      <el-table-column prop="register_address" label="寄存器" width="80" />
      <el-table-column prop="data_format" label="数据格式" width="100" />
      <el-table-column prop="byte_order" label="字节序" width="80" />
    </template>
    
    <!-- IEC60870 特定字段 -->
    <template v-else-if="isIEC60870">
      <el-table-column prop="common_address" label="公共地址" width="100" />
      <el-table-column prop="ioa_address" label="IOA地址" width="100" />
      <el-table-column prop="type_id" label="类型ID" width="80" />
      <el-table-column prop="cause_of_transmission" label="传输原因" width="100" />
    </template>
    
    <!-- CAN 特定字段 -->
    <template v-else-if="isCAN">
      <el-table-column prop="can_id" label="CAN ID" width="100">
        <template #default="scope">
          {{ formatCanId(scope.row.can_id) }}
        </template>
      </el-table-column>
      <el-table-column prop="start_bit" label="起始位" width="80" />
      <el-table-column prop="bit_length" label="位长度" width="80" />
      <el-table-column prop="byte_order" label="字节序" width="100" />
      <el-table-column prop="value_type" label="值类型" width="100" />
      <el-table-column prop="factor" label="因子" width="80" />
      <el-table-column prop="offset" label="偏移" width="80" />
    </template>
    
    <el-table-column prop="description" label="描述" min-width="150" />
    <el-table-column label="操作" width="100" fixed="right">
      <template #default="scope">
        <el-button size="small" type="text" @click="$emit('edit', scope.row)">编辑</el-button>
        <el-button size="small" type="text" @click="$emit('delete', scope.row)">删除</el-button>
      </template>
    </el-table-column>
  </el-table>
</template>

<script setup>
import { computed } from 'vue'

const props = defineProps({
  mappings: Array,
  protocolType: String
})

const emit = defineEmits(['edit', 'delete'])

const isModbus = computed(() => props.protocolType === 'modbus_tcp' || props.protocolType === 'modbus_rtu')
const isIEC60870 = computed(() => props.protocolType === 'iec60870' || props.protocolType === 'iec104' || props.protocolType === 'iec101')
const isCAN = computed(() => props.protocolType === 'can')

function formatCanId(canId) {
  if (typeof canId === 'number') {
    return '0x' + canId.toString(16).toUpperCase()
  }
  return canId
}
</script>