<template>
  <div class="voltage-class devices-pv__content">
    <div class="devices-pv__tables">
      <!-- 左侧表格 -->
      <div class="devices-pv__table devices-pv__table--left">
        <el-table :data="leftTableData" table-layout="fixed">
          <el-table-column prop="name" label="Name" />
          <el-table-column prop="value" label="Value" />
          <el-table-column prop="unit" label="Unit" />
        </el-table>
      </div>
      <!-- 右侧表格 -->
      <div class="devices-pv__table devices-pv__table--right">
        <el-table :data="rightTableData" table-layout="fixed">
          <el-table-column prop="name" label="Name" />
          <el-table-column prop="status" label="Status" />
        </el-table>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
// 左侧表格数据
const leftTableData = ref([
  { name: 'PV1', value: 120, unit: 'V' },
  { name: 'PV2', value: 118, unit: 'V' },
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
.voltage-class {
  .devices-pv__content {
    width: 100%;
    height: 100%;
    margin-top: 20px;
    .devices-pv__tables {
      display: flex;
      flex-direction: row;
      justify-content: space-between;
      width: 100%;
      height: 100%;
      .devices-pv__table {
        width: calc((100% - 20px) / 2);
        height: 100%;
      }
    }
  }
}
</style>
