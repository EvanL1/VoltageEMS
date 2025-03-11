<template>
  <div class="voltage-class battery-overview">
    <div class="battery-overview__left">
      <ModuleCard title="Voltage">
        <div class="battery-overview__contain">
          <div class="overview__header">
            <div class="header__Value">
              <span class="header-name">#124 max</span>
              <span class="header-value">1000</span>
              <span class="header-unit">V</span>
            </div>
            <div class="header__Value">
              <span class="header-name">#124 Min</span>
              <span class="header-value">36.5</span>
              <span class="header-unit">V</span>
            </div>
          </div>
          <div class="overview__content">
            <div class="content__item" v-for="item in contentItemData" :key="item.title">
              <span class="item-name">{{ item.title }}</span>
              <span class="item-value">
                {{ item.value }}<span class="item-unit">{{ item.unit }}</span>
              </span>
            </div>
          </div>
        </div>
      </ModuleCard>
    </div>
    <div class="battery-overview__right">
      <ModuleCard title="Temperature">
        <div class="battery-overview__contain">
          <div class="overview__header">
            <div class="header__Value">
              <span class="header-name">#124 max</span>
              <span class="header-value">1000</span>
              <span class="header-unit">V</span>
            </div>
            <div class="header__Value">
              <span class="header-name">#124 Min</span>
              <span class="header-value">36.5</span>
              <span class="header-unit">V</span>
            </div>
          </div>
          <div class="overview__content">
            <div class="content__item" v-for="item in contentItemData" :key="item.title">
              <span class="item-name">{{ item.title }}</span>
              <span class="item-value"
                >{{ item.value }}&nbsp; <span class="item-unit">{{ item.unit }}</span>
              </span>
            </div>
          </div>
        </div>
      </ModuleCard>
    </div>
  </div>
</template>
<script setup lang="ts">
const contentItemData = [
  {
    title: 'Voltage',
    value: '1000',
    unit: 'V',
  },
  {
    title: 'Current',
    value: '500',
    unit: 'A',
  },
  {
    title: 'Power',
    value: '200',
    unit: 'kW',
  },
  {
    title: 'Coolant Temp',
    value: '36',
    unit: '°C',
  },
]
</script>
<style scoped lang="scss">
.voltage-class.battery-overview {
  height: 100%;
  width: 100%;
  display: flex;
  justify-content: space-between;
  .battery-overview__left,
  .battery-overview__right {
    width: calc((100% - 20px) / 2);
    height: 100%;
    .battery-overview__contain {
      padding-top: 20px;
      .overview__header {
        display: flex;
        align-items: flex-end;
        justify-content: space-between;
        padding-bottom: 15px;
        border-bottom: 1px dashed rgba(255, 255, 255, 0.2);
        .header__Value {
          color: rgba(255, 255, 255, 0.6);
          .header-name {
            font-size: 16px;
            line-height: 16px;
          }
          .header-value {
            font-weight: 700;
            font-size: 30px;
            line-height: 30px;
            color: #fff;
            margin: 0 10px;
          }
          .header-unit {
            font-size: 14px;
            line-height: 14px;
          }
        }
      }
      .overview__content {
        display: flex;
        gap: 20px;
        align-items: center;
        flex-wrap: wrap;
        padding-top: 20px;
        .content__item {
          width: calc((100% - 40px) / 3);
          height: 12.58%;
          padding: 20px;
          background: rgba(84, 98, 140, 0.5);
          border: 1px solid;

          border-image: linear-gradient(
              117.64deg,
              #94a6c5 2.73%,
              rgba(148, 166, 197, 0) 31.73%,
              rgba(148, 166, 197, 0.344221) 71.62%,
              #94a6c5 97.67%
            )
            1;
          backdrop-filter: blur(10px);
          display: flex;
          flex-direction: column;
          justify-content: space-between;

          .item-name {
            font-size: 16px;
            line-height: 16px;
            color: rgba(255, 255, 255, 0.6);
          }
          .item-value {
            width: 100%;
            font-size: 30px;
            line-height: 30px;
            color: #fff;
            text-align: right;
            .item-unit {
              color: rgba(255, 255, 255, 0.6);
              font-size: 14px;
              line-height: 14px;
              text-align: right;
            }
          }
        }
      }
    }
  }
}
</style>
