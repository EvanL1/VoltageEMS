<template>
  <div class="voltage-class pv__content">
    <div class="devices-pv__tables">
      <div class="update-time">Update Time: {{ updateTime }}</div>
      <LoadingBg :loading="globalStore.loading">
        <div class="devices-pv__tables-content">
          <DeviceMonitoringTable :leftTableData="leftTableData" :rightTableData="rightTableData" />
        </div>
      </LoadingBg>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import useWebSocket from '@/composables/useWebSocket'
import DeviceMonitoringTable from '@/components/device/DeviceMonitoringTable.vue'
import LoadingBg from '@/components/common/LoadingBg.vue'
import { useGlobalStore } from '@/stores/global'
import type { LeftTableItem, RightTableItem } from '@/types/deviceMonitoring'
import { getPointsTables } from '@/api/channelsManagement'
import type { PointInfoResponse } from '@/types/channelConfiguration'

const globalStore = useGlobalStore()

const leftTableData = ref<LeftTableItem[]>([])
const rightTableData = ref<RightTableItem[]>([])
const updateTime = ref('')

// 初始化数据：通过 API 获取点位数据（暂时使用通道 3，与 DieselGenerator 相同）
onMounted(async () => {
  try {
    const res = await getPointsTables(3)
    if (res?.success && res.data) {
      const data = res.data as PointInfoResponse
      leftTableData.value =
        data.telemetry?.map((p) => ({
          pointId: p.point_id,
          name: p.signal_name || '',
          unit: p.unit || '',
          value: null,
        })) || []
      rightTableData.value =
        data.signal?.map((p) => ({
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

// WebSocket 订阅（暂时使用通道 3）
useWebSocket(
  {
    source: 'comsrv',
    channels: [3],
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
      const channel3TData = data.updates.find(
        (item: any) => item.channel_id === 3 && item.data_type === 'T',
      )?.values
      if (channel3TData) {
        leftTableData.value.forEach((item) => {
          const pointValue = channel3TData[item.pointId]
          if (pointValue !== undefined && pointValue !== null) {
            item.value = pointValue
          }
        })
      }
      const channel3SData = data.updates.find(
        (item: any) => item.channel_id === 3 && item.data_type === 'S',
      )?.values
      if (channel3SData) {
        rightTableData.value.forEach((item) => {
          const pointValue = channel3SData[item.pointId]
          if (pointValue !== undefined && pointValue !== null) {
            item.status = pointValue
            item.updateTime = updateTime.value
          }
        })
      }
    },
  },
)
</script>

<style scoped lang="scss">
.voltage-class.pv__content {
  width: 100%;
  height: calc(100% - 0.4rem);

  .devices-pv__tables {
    width: 100%;
    height: 100%;
    gap: 0.2rem;
    .update-time {
      text-align: right;
      margin: 0.1rem 0;
      color: #fff;
    }

    .devices-pv__tables-content {
      width: 100%;
      height: 100%;
    }
  }
}
</style>
