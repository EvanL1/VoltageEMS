<template>
  <div class="voltage-class pv-overview">
    <div class="pv-overview__header">
      <div class="card-item" v-for="item in energyCardData" :key="item.title">
        <PVCard :title="item.title" :icon="item.icon" :value="item.value" :unit="item.unit" />
      </div>
    </div>
    <div class="pv-overview__content"></div>
  </div>
</template>
<script setup lang="ts">
import powerIcon from '@/assets/icons/Power.svg'
import oilIcon from '@/assets/icons/Oil.svg'
import voltageIcon from '@/assets/icons/Voltage.svg'
import coolantTempIcon from '@/assets/icons/CoolantTemp.svg'
const energyCardData = [
  {
    title: 'Power',
    icon: powerIcon,
    value: '35',
    unit: 'kW',
  },

  {
    title: 'oil',
    icon: oilIcon,
    value: '56',
    unit: '%',
  },
  {
    title: 'Voltage',
    icon: voltageIcon,
    value: '220',
    unit: 'V',
  },
  {
    title: 'Coolant Temp',
    icon: coolantTempIcon,
    value: '96',
    unit: '°F',
  },
]
</script>
<style scoped lang="scss">
.voltage-class.pv-overview {
  height: 100%;
  width: 100%;
  display: flex;
  flex-direction: column;
  position: relative;
  z-index: 1;

  .pv-overview__header {
    width: 100%;
    padding: 0.2rem 0;
    display: flex;
    justify-content: space-between;
    align-items: center;

    &::after {
      content: '';
      position: absolute;
      top: -0.72rem;
      left: -0.2rem;
      width: calc(100% + 0.4rem);
      height: calc(100vh - 0.85rem);
      background-image: url('@/assets/images/DieselGenerator-bg.png');
      background-repeat: no-repeat;
      background-size: 100% 100%;
      background-position: center;
      z-index: 0;
      pointer-events: none;
    }

    .card-item {
      height: 1rem;
      width: calc((100% - 1.2rem) / 4);
      position: relative;
      z-index: 1;
    }
  }

  .pv-overview__content {
    width: 100%;
    flex: 1;


    // background-image: url('@/assets/images/DieselGenerator-bg.png');
    // background-repeat: no-repeat;
    // background-size: 100% 100%;

  }
}
</style>
