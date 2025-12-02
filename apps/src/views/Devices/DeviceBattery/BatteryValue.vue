<template>
  <div class="voltage-class devices-pv__content">
    <div class="devices-pv__tables">
      <div class="update-time">Update Time: {{ updateTime }}</div>
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
    </div>
  </div>
</template>

<script setup lang="ts">
import useWebSocket from '@/composables/useWebSocket'
import type { LeftTableItem, RightTableItem } from '@/types/deviceMonitoring'

// 使用 public 目录下资源时，改为通过 URL + fetch 加载
const telemetryUrl = '/pointCSV/battery/telemetry.csv'
const signalUrl = '/pointCSV/battery/signal.csv'
const pcsTelemetryUrl = '/pointCSV/battery/pcsTelemetry.csv'
const pcsSignalUrl = '/pointCSV/battery/pcsSignal.csv'

const BatteryleftTableData = ref<LeftTableItem[]>([])
const updateTime = ref('')
function parseTelemetryCsv(csvText: string): LeftTableItem[] {
  const lines = csvText
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l.length > 0)
  if (lines.length <= 1) return []
  // header: point_id,signal_name,scale,offset,unit,reverse,data_type
  const rows = lines.slice(1)
  const result: LeftTableItem[] = []
  for (const line of rows) {
    const cols = line.split(',')
    const idStr = cols[0]
    const name = cols[1] ?? ''
    const unit = cols[4] ?? ''
    const pointId = Number.parseInt(idStr, 10)
    if (Number.isNaN(pointId)) continue
    result.push({ pointId, name, unit, value: null })
  }
  return result
}

BatteryleftTableData.value = []
const BatteryrightTableData = ref<RightTableItem[]>([])

function parseSignalCsv(csvText: string): RightTableItem[] {
  const lines = csvText
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l.length > 0)
  if (lines.length <= 1) return []
  const rows = lines.slice(1)
  const result: RightTableItem[] = []
  for (const line of rows) {
    const cols = line.split(',')
    const name = cols[1] ?? ''
    const pointId = Number.parseInt(cols[0], 10)
    if (Number.isNaN(pointId)) continue
    result.push({ name, status: null, pointId, updateTime: '' })
  }
  return result
}

BatteryrightTableData.value = []
const PCSleftTableData = ref<LeftTableItem[]>([])
const PCSrightTableData = ref<RightTableItem[]>([])
// 初始化数据：通过 fetch 读取 public 下 CSV
onMounted(async () => {
  try {
    const [telemetryText, signalText, pcsTelemetryText, pcsSignalText] = await Promise.all([
      fetch(telemetryUrl).then((r) => r.text()),
      fetch(signalUrl).then((r) => r.text()),
      fetch(pcsTelemetryUrl).then((r) => r.text()),
      fetch(pcsSignalUrl).then((r) => r.text()),
    ])
    BatteryleftTableData.value = parseTelemetryCsv(telemetryText)
    BatteryrightTableData.value = parseSignalCsv(signalText)
    PCSleftTableData.value = parseTelemetryCsv(pcsTelemetryText)
    PCSrightTableData.value = parseSignalCsv(pcsSignalText)
  } catch (err) {
    console.error('加载设备点位 CSV 失败:', err)
  }
})
useWebSocket(
  'deviceBatteryValue',
  {
    source: 'comsrv',
    channels: [2, 1],
    dataTypes: ['T', 'S'],
    interval: 1000,
  },
  {
    onBatchDataUpdate: (data, timestamp) => {
      updateTime.value = timestamp ? new Date(timestamp).toLocaleString() : ''
      const channel2TData = data.updates.find(
        (item: any) => item.channel_id === 2 && item.data_type === 'T',
      )?.values
      if (channel2TData) {
        console.log(channel2TData, 'channel2TData')
        BatteryleftTableData.value.forEach((item) => {
          item.value = channel2TData[item.pointId]?.toFixed(3)
        })
      }
      const channel2SData = data.updates.find(
        (item: any) => item.channel_id === 2 && item.data_type === 'S',
      )?.values
      if (channel2SData) {
        console.log(channel2SData, 'channel2SData')
        BatteryrightTableData.value.forEach((item) => {
          item.status = channel2SData[item.pointId]
        })
      }
      const channel1SData = data.updates.find(
        (item: any) => item.channel_id === 1 && item.data_type === 'S',
      )?.values
      if (channel1SData) {
        console.log(channel1SData, 'channel1SData')
        PCSrightTableData.value.forEach((item) => {
          item.status = channel1SData[item.pointId]
        })
      }
      const channel1TData = data.updates.find(
        (item: any) => item.channel_id === 1 && item.data_type === 'T',
      )?.values
      if (channel1TData) {
        console.log(channel1TData, 'channel1TData')
        PCSleftTableData.value.forEach((item) => {
          item.value = channel1TData[item.pointId]?.toFixed(3)
        })
      }
    },
  },
)
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
