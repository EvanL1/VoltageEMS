<template>
  <div class="voltage-class devices-pv__content">
    <div class="devices-pv__tables">
      <div class="update-time">Update Time: {{ updateTime }}</div>
      <LoadingBg :loading="globalStore.loading">
        <el-tabs v-model="activeTab" type="card" class="devices-pv__tabs">
          <el-tab-pane label="Battery" name="battery">
            <DeviceMonitoringTable
              :leftTableData="BatteryleftTableData"
              :rightTableData="BatteryrightTableData"
            />
          </el-tab-pane>
          <el-tab-pane label="PCS" name="pcs">
            <DeviceMonitoringTable
              :leftTableData="PCSleftTableData"
              :rightTableData="PCSrightTableData"
            />
          </el-tab-pane>
        </el-tabs>
      </LoadingBg>
    </div>
  </div>
</template>

<script setup lang="ts">
import LoadingBg from '@/components/common/LoadingBg.vue'
import { useGlobalStore } from '@/stores/global'
import type { LeftTableItem, RightTableItem } from '@/types/deviceMonitoring'
import { getPointsTables } from '@/api/channelsManagement'
import type { PointInfoResponse } from '@/types/channelConfiguration'
import { ref } from 'vue'
import useWebSocket from '@/composables/useWebSocket'

const globalStore = useGlobalStore()

const BatteryleftTableData = ref<LeftTableItem[]>([])
const BatteryrightTableData = ref<RightTableItem[]>([])
const PCSleftTableData = ref<LeftTableItem[]>([])
const PCSrightTableData = ref<RightTableItem[]>([])
const updateTime = ref('')

// 订阅 WebSocket - ValueMonitoring 使用 comsrv 源
useWebSocket(
  {
    source: 'comsrv',
    channels: [2, 1],
    dataTypes: ['T', 'S'],
    interval: 1000,
  },
  {
    onBatchDataUpdate: (data: any, timestamp?: string) => {
      if (timestamp) {
        // 处理时间戳：可能是ISO字符串或Unix时间戳（秒级）字符串
        const timestampNum = Number(timestamp)
        if (!isNaN(timestampNum) && timestampNum > 0) {
          // 如果是数字字符串，判断是秒级还是毫秒级
          // 如果小于1e12，认为是秒级，需要乘以1000
          updateTime.value = new Date(timestampNum < 1e12 ? timestampNum * 1000 : timestampNum).toLocaleString()
        } else {
          // 尝试作为ISO字符串解析
          updateTime.value = new Date(timestamp).toLocaleString()
        }
      } else {
        updateTime.value = ''
      }
      const channel2TData = data.updates.find(
        (item: any) => item.channel_id === 2 && item.data_type === 'T',
      )?.values
      if (channel2TData) {
        BatteryleftTableData.value.forEach((item) => {
          const pointValue = channel2TData[item.pointId]
          if (pointValue !== undefined && pointValue !== null) {
            item.value = pointValue
          }
        })
      }
      const channel2SData = data.updates.find(
        (item: any) => item.channel_id === 2 && item.data_type === 'S',
      )?.values
      if (channel2SData) {
        BatteryrightTableData.value.forEach((item) => {
          const pointValue = channel2SData[item.pointId]
          if (pointValue !== undefined && pointValue !== null) {
            item.status = pointValue
            item.updateTime = updateTime.value
          }
        })
      }
      const channel1SData = data.updates.find(
        (item: any) => item.channel_id === 1 && item.data_type === 'S',
      )?.values
      if (channel1SData) {
        PCSrightTableData.value.forEach((item) => {
          const pointValue = channel1SData[item.pointId]
          if (pointValue !== undefined && pointValue !== null) {
            item.status = pointValue
            item.updateTime = updateTime.value
          }
        })
      }
      const channel1TData = data.updates.find(
        (item: any) => item.channel_id === 1 && item.data_type === 'T',
      )?.values
      if (channel1TData) {
        PCSleftTableData.value.forEach((item) => {
          const pointValue = channel1TData[item.pointId]
          if (pointValue !== undefined && pointValue !== null) {
            item.value = pointValue
            item.updateTime = updateTime.value
          }
        })
      }
    },
  },
)

// 初始化数据：通过 API 获取点位数据
onMounted(async () => {
  try {
    // Battery 通道 id 为 2
    const batteryRes = await getPointsTables(2)
    if (batteryRes?.success && batteryRes.data) {
      const batteryData = batteryRes.data as PointInfoResponse
      BatteryleftTableData.value =
        batteryData.telemetry?.map((p) => ({
          pointId: p.point_id,
          name: p.signal_name || '',
          unit: p.unit || '',
          value: null,
        })) || []
      BatteryrightTableData.value =
        batteryData.signal?.map((p) => ({
          pointId: p.point_id,
          name: p.signal_name || '',
          status: null,
          updateTime: '',
        })) || []
    }

    // PCS 通道 id 为 1
    const pcsRes = await getPointsTables(1)
    if (pcsRes?.success && pcsRes.data) {
      const pcsData = pcsRes.data as PointInfoResponse
      PCSleftTableData.value =
        pcsData.telemetry?.map((p) => ({
          pointId: p.point_id,
          name: p.signal_name || '',
          unit: p.unit || '',
          value: null,
        })) || []
      PCSrightTableData.value =
        pcsData.signal?.map((p) => ({
          pointId: p.point_id,
          name: p.signal_name || '',
          status: null,
          updateTime: '',
        })) || []
    }
  } catch (err) {
    console.error('加载设备点位数据失败:', err)
  }
})
const activeTab = ref<'battery' | 'pcs'>('battery')
</script>

<style scoped lang="scss">
.devices-pv__content {
  width: 100%;
  height: 100%;

  .devices-pv__tables {
    position: relative;
    height: 100%;
    width: 100%;
    .update-time {
      position: absolute;
      top: 0.1rem;
      right: 0;
      // font-size: 0.1rem;
      color: #fff;
    }
  }
}

:deep(.el-tabs) {
  height: 100% !important;
}

// :deep(.el-tabs__content) {
//   height: calc(100% - 0.55rem) !important;
// }

:deep(.el-tab-pane) {
  height: 100% !important;
}
</style>
