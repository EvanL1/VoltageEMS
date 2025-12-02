<template>
  <div class="voltage-class pv__content">
    <div class="devices-pv__tables">
      <!-- 左侧表格 -->
      <div class="devices-pv__table devices-pv__table--left">
        <el-table :data="leftTableData" table-layout="fixed" height="100%">
          <el-table-column prop="name" label="Name" />
          <el-table-column prop="value" label="Value" />
          <el-table-column prop="unit" label="Unit" />
          <el-table-column prop="updateTime" label="Update Time" />
        </el-table>
      </div>
      <!-- 右侧表格 -->
      <div class="devices-pv__table devices-pv__table--right">
        <el-table :data="rightTableData" table-layout="fixed" height="100%">
          <el-table-column prop="name" label="Name" />
          <el-table-column prop="status" label="Status" />
          <el-table-column prop="updateTime" label="Update Time" />
        </el-table>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import useWebSocket from '@/composables/useWebSocket'
import type { SubscriptionConfig } from '@/types/websocket'

import type { LeftTableItem, RightTableItem } from '@/types/deviceMonitoring'

// 左侧表格数据
const leftTableData = ref<LeftTableItem[]>([
  { name: 'Oil Temperature', value: 85.2, unit: '°C', updateTime: '' },
  { name: 'Phase A Voltage', value: 220.5, unit: 'V', updateTime: '' },
  { name: 'Phase A Current', value: 15.8, unit: 'A', updateTime: '' },
  { name: 'Phase B Voltage', value: 219.8, unit: 'V', updateTime: '' },
  { name: 'Phase B Current', value: 16.2, unit: 'A', updateTime: '' },
  { name: 'Phase C Voltage', value: 221.1, unit: 'V', updateTime: '' },
  { name: 'Phase C Current', value: 15.9, unit: 'A', updateTime: '' },
  { name: 'Active Power', value: 10.5, unit: 'kW', updateTime: '' },
  { name: 'Reactive Power', value: 2.3, unit: 'kVar', updateTime: '' },
  { name: 'Power Factor', value: 0.95, unit: '', updateTime: '' },
  { name: 'Frequency', value: 50.02, unit: 'Hz', updateTime: '' },
  { name: 'Energy', value: 1250.8, unit: 'kWh', updateTime: '' },
  { name: 'Pressure 1', value: 0.85, unit: 'MPa', updateTime: '' },
  { name: 'Pressure 2', value: 0.92, unit: 'MPa', updateTime: '' },
  { name: 'Flow Rate 1', value: 45.6, unit: 'm³/h', updateTime: '' },
  { name: 'Flow Rate 2', value: 42.3, unit: 'm³/h', updateTime: '' },
  { name: 'Liquid Level 1', value: 2.45, unit: 'm', updateTime: '' },
  { name: 'Liquid Level 2', value: 1.87, unit: 'm', updateTime: '' },
  { name: 'Speed', value: 1500.0, unit: 'rpm', updateTime: '' },
  { name: 'Torque', value: 125.6, unit: 'Nm', updateTime: '' },
  { name: 'Vibration X', value: 0.045, unit: 'mm/s', updateTime: '' },
  { name: 'Vibration Y', value: 0.038, unit: 'mm/s', updateTime: '' },
  { name: 'Vibration Z', value: 0.052, unit: 'mm/s', updateTime: '' },
  { name: 'Noise', value: 65.3, unit: 'dB', updateTime: '' },
  { name: 'Humidity', value: 45.2, unit: '%', updateTime: '' },
  { name: 'Dew Point Temperature', value: 12.8, unit: '°C', updateTime: '' },
  { name: 'CO Concentration', value: 25.0, unit: 'ppm', updateTime: '' },
  { name: 'CO2 Concentration', value: 450.0, unit: 'ppm', updateTime: '' },
  { name: 'O2 Concentration', value: 20.8, unit: '%', updateTime: '' },
  { name: 'Flue Gas Temperature', value: 180.5, unit: '°C', updateTime: '' },
])

// 右侧表格数据
const rightTableData = ref<RightTableItem[]>([
  { name: 'Oil Temperature Switch', status: 1, updateTime: '' },
  { name: 'Phase A Voltage Switch', status: 0, updateTime: '' },
  { name: 'Phase A Current Switch', status: 1, updateTime: '' },
  { name: 'Phase B Voltage Switch', status: 1, updateTime: '' },
  { name: 'Phase B Current Switch', status: 0, updateTime: '' },
  { name: 'Phase C Voltage Switch', status: 1, updateTime: '' },
  { name: 'Phase C Current Switch', status: 1, updateTime: '' },
  { name: 'Active Power Switch', status: 0, updateTime: '' },
  { name: 'Reactive Power Switch', status: 1, updateTime: '' },
  { name: 'Power Factor Switch', status: 1, updateTime: '' },
  { name: 'Frequency Switch', status: 0, updateTime: '' },
  { name: 'Energy Switch', status: 1, updateTime: '' },
  { name: 'Pressure 1 Switch', status: 1, updateTime: '' },
  { name: 'Pressure 2 Switch', status: 0, updateTime: '' },
  { name: 'Flow Rate 1 Switch', status: 1, updateTime: '' },
  { name: 'Flow Rate 2 Switch', status: 1, updateTime: '' },
  { name: 'Liquid Level 1 Switch', status: 0, updateTime: '' },
  { name: 'Liquid Level 2 Switch', status: 1, updateTime: '' },
  { name: 'Speed Switch', status: 1, updateTime: '' },
  { name: 'Torque Switch', status: 0, updateTime: '' },
  { name: 'Vibration X Switch', status: 1, updateTime: '' },
  { name: 'Vibration Y Switch', status: 1, updateTime: '' },
  { name: 'Vibration Z Switch', status: 0, updateTime: '' },
  { name: 'Noise Switch', status: 1, updateTime: '' },
  { name: 'Humidity Switch', status: 0, updateTime: '' },
  { name: 'Dew Point Temperature Switch', status: 1, updateTime: '' },
  { name: 'CO Concentration Switch', status: 1, updateTime: '' },
  { name: 'CO2 Concentration Switch', status: 0, updateTime: '' },
  { name: 'O2 Concentration Switch', status: 1, updateTime: '' },
  { name: 'Flue Gas Temperature Switch', status: 1, updateTime: '' },
])

// 更新时间的格式化函数
const formatUpdateTime = () => {
  const now = new Date()
  return now.toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

// 模拟数据更新
let updateTimer: number | null = null

const updateData = () => {
  const currentTime = formatUpdateTime()

  // 更新左侧表格数据（模拟数值变化）
  leftTableData.value.forEach((item, index) => {
    // 为每个数据项添加一些随机变化
    const randomChange = (Math.random() - 0.5) * 0.1 // ±5% 的随机变化
    const baseValue = getBaseValue(index)
    item.value = Math.round(baseValue * (1 + randomChange) * 100) / 100
    item.updateTime = currentTime
  })

  // 更新右侧表格数据（随机切换状态）
  rightTableData.value.forEach((item) => {
    // 随机切换开关状态
    if (Math.random() < 0.1) {
      // 10% 的概率切换状态
      item.status = item.status === 1 ? 0 : 1
    }
    item.updateTime = currentTime
  })
}

// 获取基础数值的函数
const getBaseValue = (index: number): number => {
  const baseValues = [
    85.2, 220.5, 15.8, 219.8, 16.2, 221.1, 15.9, 10.5, 2.3, 0.95, 50.02, 1250.8, 0.85, 0.92, 45.6,
    42.3, 2.45, 1.87, 1500.0, 125.6, 0.045, 0.038, 0.052, 65.3, 45.2, 12.8, 25.0, 450.0, 20.8,
    180.5,
  ]
  return baseValues[index] || 0
}

// 组件挂载时启动定时器
onMounted(() => {
  // 立即更新一次数据
  updateData()

  // 每3秒更新一次数据
  updateTimer = setInterval(updateData, 3000)
})

// 组件卸载时清除定时器
onUnmounted(() => {
  if (updateTimer) {
    clearInterval(updateTimer)
    updateTimer = null
  }
})
</script>

<style scoped lang="scss">
.voltage-class.pv__content {
  width: 100%;
  height: calc(100% - 0.2rem);
  margin-top: 0.2rem;

  .devices-pv__tables {
    display: flex;
    flex-direction: row;
    justify-content: space-between;
    width: 100%;
    height: 100%;
    gap: 0.2rem;

    .devices-pv__table {
      width: calc((100% - 0.2rem) / 2);
      height: 100%;

      :deep(.el-table) {
        // background: rgba(84, 98, 140, 0.2);
        // overflow: hidden;

        .el-table__header-wrapper {
          th {
            background: rgba(58, 82, 121, 0.8);
            color: #fff;
            border-bottom: 0.01rem solid rgba(255, 255, 255, 0.1);

            .cell {
              color: #fff;
              font-weight: 600;
            }
          }
        }

        .el-table__body-wrapper {
          td {
            background: transparent;
            color: #fff;
            border-bottom: 0.01rem solid rgba(255, 255, 255, 0.05);

            .cell {
              color: #fff;
            }
          }

          tr:hover>td {
            background: rgba(64, 158, 255, 0.1);
          }
        }
      }
    }
  }
}
</style>
