<template>
  <div class="voltage-class pv-overview">
    <div class="pv-overview__right">
      <BatteryCard
        class="battery-card"
        v-for="item in batteryCardData"
        :key="item.title"
        :title="item.title"
        :value="item.value"
        :unit="item.unit"
      ></BatteryCard>
    </div>
  </div>
</template>
<script setup lang="ts">
import useWebSocket from '@/composables/useWebSocket'
interface BatteryCardItem {
  id: number
  title: string
  value?: number | null | string
  unit?: string
  pointId: number
}
useWebSocket(
  'deviceBatteryOverview',
  {
    channels: [2],
    dataTypes: ['T'],
    interval: 1000,
    source: 'comsrv',
  },
  {
    onDataUpdate: (data) => {
      console.log(data, 'data')
    },
    onBatchDataUpdate: (data) => {
      const channel2TData = data.updates.find(
        (item: any) => item.channel_id === 2 && item.data_type === 'T',
      )?.values
      if (channel2TData) {
        console.log(channel2TData, 'channel2TData')
        batteryCardData.value.forEach((item) => {
          item.value = channel2TData[item.pointId]?.toFixed(3)
        })
      }
    },
  },
)
const batteryCardData = ref<BatteryCardItem[]>([
  // {
  //   id: 1,
  //   title: 'Off Grid',
  // },
  // {
  //   id: 2,
  //   title: 'Cell Count',
  //   value: 0,
  // },
  {
    id: 4,
    title: 'SoC',
    value: '-',
    unit: '%',
    pointId: 4,
  },
  {
    id: 3,
    title: 'SoH',
    value: '-',
    unit: '%',
    pointId: 5,
  },

  // {
  //   id: 5,
  //   title: 'Energy',
  //   value: 0,
  //   unit: 'kWh',
  // },
  // {
  //   id: 6,
  //   title: 'Cycle',
  //   value: 0,
  // },
  {
    id: 7,
    title: 'Voltage',
    value: '-',
    unit: 'V',
    pointId: 1,
  },
  // {
  //   id: 8,
  //   title: 'Running Status',
  //   value: '',
  // },
  // {
  //   id: 9,
  //   title: 'Cooling',
  //   value: '',
  // },
  {
    id: 10,
    title: 'Current',
    value: '-',
    unit: 'A',
    pointId: 2,
  },
  // {
  //   id: 11,
  //   title: 'Max Cell Temp',
  //   value: null,
  //   unit: '°F',
  //   pointId: 23,
  // },
  // {
  //   id: 11,
  //   title: 'Alarms',
  //   value: '',
  // },
  {
    id: 12,
    title: 'Power',
    value: '-',
    unit: 'KW',
    pointId: 3,
  },
  // {
  //   id: 13,
  //   title: 'Min Temp',
  //   value: 0,
  //   unit: '°F',
  // },
  {
    id: 14,
    title: 'Max Cell Voltage',
    value: '-',
    unit: 'V',
    pointId: 15,
  },
  {
    id: 15,
    title: 'Min Cell Voltage',
    value: '-',
    unit: 'V',
    pointId: 18,
  },
  {
    id: 20,
    title: 'Avg Cell Voltage',
    value: '-',
    unit: 'V',
    pointId: 21,
  },

  {
    id: 16,
    title: 'Permit Charge Power',
    value: '-',
    unit: 'KW',
    pointId: 6,
  },
  {
    id: 21,
    title: 'Cell Voltage Difference',
    value: '-',
    unit: 'V',
    pointId: 22,
  },
  {
    id: 17,
    title: 'Permit Discharge Power',
    value: '-',
    unit: 'KW',
    pointId: 7,
  },
  {
    id: 18,
    title: 'Permit Charge Current',
    value: '-',
    unit: 'A',
    pointId: 8,
  },
  {
    id: 19,
    title: 'Permit Discharge Current',
    value: '-',
    unit: 'A',
    pointId: 9,
  },
])
</script>
<style scoped lang="scss">
.voltage-class.pv-overview {
  height: 100%;
  width: 100%;
  display: flex;
  justify-content: flex-end;
  position: relative; // 确保z-index生效
  z-index: 1;

  &::after {
    content: '';
    position: absolute;
    // top: -0.72rem;
    left: -0.2rem;
    width: calc(100% + 0.4rem);
    // height: calc(100vh - 0.85rem); // 减去header高度
    height: calc(100% + 0.2rem);
    // margin-top: 0.72rem;
    background-image: url('@/assets/images/battery-bg.png');
    background-repeat: no-repeat;
    background-size: 60% 100%;
    background-position: left center;
    z-index: 0;
    pointer-events: none; // 防止背景阻挡用户交互
  }

  .pv-overview__right {
    width: 48.2%;
    height: 100%;
    overflow: auto;
    display: flex;
    flex-wrap: wrap;
    gap: 0.16rem;
    position: relative;
    z-index: 2; // 确保内容在背景之上

    .battery-card {
      height: 18%;
      width: calc((100% - 0.32rem) / 3);
      margin-bottom: 0.01rem;
    }
  }
}
</style>
