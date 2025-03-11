<template>
  <div class="voltage-class devices-diesel__content">
    <!-- 左侧表格 -->
    <div class="devices-diesel__table devices-diesel__table--left">
      <el-table :data="leftTableData">
        <el-table-column prop="name" label="Name" show-overflow-tooltip />
        <el-table-column prop="value" label="Value" show-overflow-tooltip />
        <el-table-column prop="unit" label="Unit" show-overflow-tooltip />
      </el-table>
    </div>
    <!-- 右侧表格 -->
    <div class="devices-diesel__table devices-diesel__table--right">
      <el-table :data="rightTableData">
        <el-table-column prop="name" label="Name" show-overflow-tooltip />
        <el-table-column prop="status" label="Status" show-overflow-tooltip />
      </el-table>
    </div>
  </div>
</template>

<script setup lang="ts">
// 左侧表格数据
const leftTableData = ref([
  { name: 'diesel1', value: 120, unit: 'V' },
  { name: 'diesel2', value: 118, unit: 'V' },
  { name: 'Current', value: 15, unit: 'A' },
  { name: 'Power', value: 1.8, unit: 'kW' },
  { name: 'Energy', value: 12.5, unit: 'kWh' },
])

// 右侧表格数据
const rightTableData = ref([
  { name: 'Inverter', status: 'Running' },
  { name: 'Battery', status: 'Charging' },
  { name: 'Grid', status: 'Connected' },
  { name: 'Diesel Generator', status: 'Standby' },
  { name: 'Load', status: 'Normal' },
])
</script>

<style scoped lang="scss">
.voltage-class.devices-diesel__content {
  width: 100%;
  height: 100%;
  padding-top: 20px;
  display: flex;
  flex-direction: row;
  justify-content: space-between;
  .devices-diesel__table {
    width: calc((100% - 20px) / 2);
    height: 100%;
    // min-width: 0; // 允许表格收缩
    // overflow: hidden; //
  }
}
</style>
