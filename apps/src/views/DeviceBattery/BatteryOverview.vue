<template>
  <div class="voltage-class pv-overview">
    <div class="pv-overview__left"></div>
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
const batteryCardData = [
  {
    id: 1,
    title: 'Off Grid',
  },
  {
    id: 2,
    title: 'Cell Count',
    value: '96',
  },
  {
    id: 3,
    title: 'SoH',
    value: '99',
    unit: '%',
  },
  {
    id: 4,
    title: 'SoC',
    value: '72',
    unit: '%',
  },
  {
    id: 5,
    title: 'Cell Count',
    value: '96',
  },
  {
    id: 6,
    title: 'SoH',
    value: '99',
    unit: '%',
  },
  {
    id: 7,
    title: 'SoC',
    value: '72',
    unit: '%',
  },
  {
    id: 8,
    title: 'Cell Count',
    value: '96',
  },
  {
    id: 9,
    title: 'SoH',
    value: '99',
    unit: '%',
  },
  {
    id: 10,
    title: 'SoC',
    value: '72',
    unit: '%',
  },
  {
    id: 11,
    title: 'Cell Count',
    value: '96',
  },
  {
    id: 11,
    title: 'SoH',
    value: '99',
    unit: '%',
  },
  {
    id: 12,
    title: 'SoC',
    value: '72',
    unit: '%',
  },
  {
    id: 13,
    title: 'SoH',
    value: '99',
    unit: '%',
  },
  {
    id: 14,
    title: 'SoC',
    value: '72',
    unit: '%',
  },
]
</script>
<style scoped lang="scss">
.voltage-class.pv-overview {
  height: 100%;
  width: 100%;
  display: flex;
  justify-content: space-between;
  .pv-overview__left {
    width: 48.2%;
    background-image: url('@/assets/images/battery-bg.png');
    background-repeat: no-repeat;
    background-size: 100% 102%;
    background-position: center;
  }
  .pv-overview__right {
    width: 48.2%;
    height: 100%;
    overflow: auto;
    display: flex;
    flex-wrap: wrap;
    gap: 16px;
    .battery-card {
      height: 18%;
      width: calc((100% - 32px) / 3);
      margin-bottom: 1px;
    }
  }
}
</style>
