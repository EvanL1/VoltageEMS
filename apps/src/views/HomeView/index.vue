<template>
  <div class="voltage-class home">
    <!-- <EnergyBgCopy></EnergyBgCopy> -->
    <div class="home-left">
      <div class="home-left-top">
        <EnergyCard
          class="home-left-top-item"
          v-for="item in energyDashboardList"
          :key="item.title"
          :title="item.title"
          :icon="item.icon"
          :value="item.value"
          :unit="item.unit"
        />
      </div>
      <div class="home-left-middle">
        <!-- <img :src="tuopuSvg" alt="">
          -->
        <HomeBg :data="tuopuData"></HomeBg>
      </div>
      <div class="home-left-bottom">
        <div class="home-left-LineChart">
          <ModuleCard title="Power Curve">
            <LineChart
              :xAxiosOption="xAxiosOption"
              :yAxiosOption="lineChartYAxiosOption"
              :series="lineChartSeries"
            />
          </ModuleCard>
        </div>
        <div class="home-left-EnergyChart">
          <ModuleCard title="Energy Chart">
            <StackedBarChart
              :xAxiosOption="xAxiosOption"
              :yAxiosOption="yAxiosOption"
              :series="exampleSeries"
            />
          </ModuleCard>
        </div>
      </div>
    </div>
    <div class="home-right">
      <div class="home-station">
        <ModuleCard title="Station infomation">
          <div class="home-stationList">
            <div v-for="item in stationInfoList" :key="item.title" class="home-stationItem">
              <EnergyCard
                :title="item.title"
                :icon="item.icon"
                :value="item.value"
                :unit="item.unit"
              />
            </div>
          </div>
        </ModuleCard>
      </div>
      <div class="home-device">
        <ModuleCard title="Device infomation">
          <!-- <div class="home-deviceValue">
              <div class="home-deviceValue-item" v-for="item in deviceInfoList" :key="item.title">
                <span class="deviceValue-item-title">{{ item.title }}:</span>
                <span class="deviceValue-item-value">{{ item.value }}</span>
                &nbsp;
                <span class="deviceValue-item-unit">{{ item.unit }}</span>
              </div>
            </div> -->
          <div class="home-decice-Carousel">
            <el-carousel
              ref="carouselRef"
              :autoplay="false"
              arrow="never"
              indicator-position="none"
              style="width: 100%; height: 100%"
            >
              <el-carousel-item
                v-for="(item, index) in deviceInfoList"
                :key="index"
                style="width: 100%; height: 100%"
              >
                <div class="home-decice-Carousel-item">
                  <div class="home-deviceValue">
                    <div
                      class="home-deviceValue-item"
                      v-for="dataItem in item.data"
                      :key="dataItem.title"
                    >
                      <span class="deviceValue-item-title">{{ dataItem.title }}:</span>
                      <span class="deviceValue-item-value">{{ dataItem.value }}</span>
                      &nbsp;
                      <span class="deviceValue-item-unit">{{ dataItem.unit }}</span>
                    </div>
                  </div>
                  <img :src="item.icon" />
                  <div class="item-name">{{ item.name }}</div>
                </div>
              </el-carousel-item>
            </el-carousel>

            <!-- 自定义左右切换按钮 -->
            <div class="custom-carousel-controls">
              <div class="custom-arrow custom-arrow-left" @click="handlePrev">
                <img :src="arrowLeftImg" alt="Previous" />
              </div>
              <div class="custom-arrow custom-arrow-right" @click="handleNext">
                <img :src="arrowRightImg" alt="Next" />
              </div>
            </div>
          </div>
        </ModuleCard>
      </div>
      <div class="home-alters">
        <ModuleCard title="Alters infomation">
          <div class="home-altersList">
            <div class="home-altersItem" v-for="item in alterInfoList" :key="item.id">
              <div class="alters__item-name">{{ item.deviceName }}</div>
              <img v-if="item.alterLevel == 'L1'" :src="alterL1" class="alters__item-icon" />
              <img v-else-if="item.alterLevel == 'L2'" :src="alterL2" class="alters__item-icon" />
              <img v-else-if="item.alterLevel == 'L3'" :src="alterL3" class="alters__item-icon" />
              <div class="alters__item-msg">{{ item.alterMsg }}</div>
            </div>
          </div>
        </ModuleCard>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { EnergyCard } from '@/types/home'

import ModuleCard from '@/components/card/ModuleCard.vue'
import StackedBarChart from '@/components/charts/StackedBarChart.vue'
import LineChart from '@/components/charts/lineChart.vue'

import alterL1 from '@/assets/icons/home-alter-L1.svg'
import alterL2 from '@/assets/icons/home-alter-L2.svg'
import alterL3 from '@/assets/icons/home-alter-L3.svg'

import arrowLeftImg from '@/assets/icons/arrow-left.svg'
import arrowRightImg from '@/assets/icons/arrow-right.svg'

import devicePV from '@/assets/icons/device-pv.svg'
import deviceDiesel from '@/assets/icons/device-diesel.svg'
import deviceBMS from '@/assets/icons/device-BMS.svg'
import devicePCS from '@/assets/icons/device-PCS.svg'
import deviceBattery from '@/assets/images/device-battery.png'

import PVEnergy from '@/assets/icons/PVEnergy.svg'
import DieselEnergy from '@/assets/icons/DieselEnergy.svg'
import EnergyUsed from '@/assets/icons/EnergyUsed.svg'
import SavingBilling from '@/assets/icons/SavingBilling.svg'
import ESSEnergyIcon from '@/assets/icons/ESSEnergy.svg'
import tuopuSvg from '@/assets/icons/home-tuopu.svg'
// import tuopu from '@/assets/icons/tuopu.svg'
import useWebSocket from '@/composables/useWebSocket'

import HomeBg from './HomeBg.vue'
useWebSocket(
  {
    channels: [2],
    dataTypes: ['T'],
    interval: 100,
    source: 'comsrv',
  },
  {
    onBatchDataUpdate: (data: any) => {
      const channel2TData = data.updates.find(
        (item: any) => item.channel_id === 2 && item.data_type === 'T',
      )?.values
      if (channel2TData) {
        tuopuData.value.ess.p = Number(channel2TData['3'].toFixed(1))
        tuopuData.value.ess.soc = Number(channel2TData['4'].toFixed(1))
        deviceInfoList[2].data[0].value = Number(channel2TData['3'].toFixed(1))
        deviceInfoList[2].data[1].value = Number(channel2TData['1'].toFixed(1))
      }
    },
  },
)
const tuopuData = ref({
  pv: {
    P: 0,
  },
  ess: {
    p: 0,
    soc: 0,
  },
  load: {
    P: 0,
  },
  diesel: {
    p: 0,
    oil: 0,
  },
})
const deviceInfoList = reactive([
  {
    data: [
      {
        title: 'P',
        value: 45,
        unit: 'KW',
      },
      {
        title: 'U',
        value: 22,
        unit: 'V',
      },
    ],
    icon: devicePV,
    name: 'PV',
  },
  {
    data: [
      {
        title: 'P',
        value: 45,
        unit: 'KW',
      },
      {
        title: 'U',
        value: 22,
        unit: 'V',
      },
    ],
    icon: deviceDiesel,
    name: 'Diesel Generator',
  },
  {
    data: [
      {
        title: 'P',
        value: 0,
        unit: 'KW',
      },
      {
        title: 'U',
        value: 0,
        unit: 'V',
      },
    ],
    icon: deviceBattery,
    name: 'ESS',
  },
  // {
  //   data: [
  //     {
  //       title: 'P',
  //       value: 45,
  //       unit: 'KW',
  //     },
  //     {
  //       title: 'U',
  //       value: 22,
  //       unit: 'V',
  //     },
  //   ],
  //   icon: devicePCS,
  //   name: 'ESS',
  // },
])
const energyDashboardList = reactive<EnergyCard[]>([
  {
    title: 'PV Energy',
    icon: PVEnergy,
    value: '150',
    unit: 'kWh',
  },
  {
    title: 'Diesel Energy',
    icon: DieselEnergy,
    value: '150',
    unit: 'KWh',
  },
  {
    title: 'Energy Used',
    icon: EnergyUsed,
    value: '150',
    unit: 'kWh',
  },
  {
    title: 'Saving Billing',
    icon: SavingBilling,
    value: '$ 40.7',
  },
])
const stationInfoList = reactive<EnergyCard[]>([
  {
    title: 'PV',
    icon: PVEnergy,
    value: '150',
    unit: 'kW',
  },
  {
    title: 'Diesel',
    icon: DieselEnergy,
    value: '150',
    unit: 'kW',
  },
  {
    title: 'ESS',
    icon: ESSEnergyIcon,
    value: '200',
    unit: 'KWh',
  },
])

const alterInfoList = reactive([
  {
    id: 1,
    deviceName: 'ESS',
    alterLevel: 'L1',
    alterMsg: 'Battery Overvoltage Alarm',
  },
  {
    id: 2,
    deviceName: 'PV',
    alterLevel: 'L2',
    alterMsg: 'Battery Overvoltage Alarm',
  },
  {
    id: 3,
    deviceName: 'Load',
    alterLevel: 'L3',
    alterMsg: 'Battery Overvoltage Alarm',
  },
])

const exampleXAxisData = [
  '0:00',
  '2:00',
  '4:00',
  '6:00',
  '8:00',
  '10:00',
  '12:00',
  '14:00',
  '16:00',
  '18:00',
  '20:00',
  '22:00',
]

// 新增lineChartSeries变量，专门用于LineChart
const lineChartSeries = [
  {
    name: 'PV',
    data: [10, 35, 20, 80, 60, 180, 120, 300, 180, 250, 90, 40],
    color: 'rgba(105, 203, 255, 1)',
  },
  {
    name: 'ESS',
    data: [500, 420, 480, 350, 370, 320, 400, 220, 300, 120, 200, 60],
    color: 'rgba(29, 134, 255, 1)',
  },
]

// exampleSeries 仍然保留给 StackedBarChart 使用
const exampleSeries = [
  {
    name: 'Diesel',
    data: [120, 135, 140, 160, 180, 200, 210, 190, 170, 160, 150, 140],
    color: 'rgb(3, 93, 239)',
  },
  {
    name: 'ESS',
    data: [80, 90, 100, 110, 120, 130, 140, 135, 130, 125, 120, 115],
    color: 'rgb(29, 134, 255)',
  },
  {
    name: 'PV',
    data: [0, 10, 30, 60, 100, 130, 150, 140, 120, 80, 30, 5],
    color: 'rgb(105, 203, 255)',
  },
]
const xAxiosOption = {
  xAxiosData: exampleXAxisData,
}
const yAxiosOption = {
  yUnit: 'kWh',
}
const lineChartYAxiosOption = {
  yUnit: 'kW',
}

// Carousel引用
const carouselRef = ref()

// 切换到上一张
const handlePrev = () => {
  carouselRef.value?.prev()
}

// 切换到下一张
const handleNext = () => {
  console.log('next')

  carouselRef.value?.next()
}
</script>

<style scoped lang="scss">
.home {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  justify-content: space-between;
  z-index: 2;

  &::before {
    content: '';
    position: absolute;
    top: -0.2rem;
    left: -0.2rem;
    width: calc(100% + 0.4rem);
    height: calc(100% + 0.4rem);
    background: url('@/assets/images/home-bg.png') no-repeat center center;
    background-size: 100% 100%;
    z-index: 1;
  }

  .home-left {
    position: relative;
    z-index: 2;
    width: calc(100% - 3.9rem);
    height: 100%;
    margin-right: 0.2rem;
    display: flex;
    flex-direction: column;
    justify-content: space-between;

    .home-left-top {
      width: 100%;
      height: 0.8rem;
      padding-top: 0.1rem;
      display: flex;
      justify-content: space-between;
      z-index: 1;

      .home-left-top-item {
        height: 0.7rem;
      }
    }

    .home-left-middle {
      width: 100%;
      height: calc(69% - 1.2rem);
      flex: 1;
      // background-image: url('@/assets/images/tuopu.png');
      // background-size: 100% 100%;
      // background-repeat: no-repeat;
      // background-position: center;

      img {
        width: 100%;
        height: 100%;
        object-fit: contain;
      }
    }

    .home-left-bottom {
      width: 100%;
      height: 30.89%;
      display: flex;
      justify-content: space-between;

      .home-left-EnergyChart {
        width: calc((100% - 0.2rem) / 2);
        height: 100%;
      }

      .home-left-LineChart {
        width: calc((100% - 0.2rem) / 2);
        height: 100%;
      }
    }
  }

  .home-right {
    position: relative;
    z-index: 2;

    width: 3.7rem;
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 0.2rem;

    .home-station {
      width: 100%;
      height: 36.75%;

      .home-stationList {
        height: 100%;
        padding-top: 0.2rem;

        .home-stationItem {
          height: 33.33%;
          padding-top: 0.12rem;
          padding-bottom: 0.13rem;
          border-bottom: 0.01rem dashed rgba(255, 255, 255, 0.2);

          &:last-child {
            border-bottom: none;
            // padding-bottom: 0;
            margin-bottom: 0;
          }
        }
      }
    }

    .home-device {
      width: 100%;
      height: 28.27%;

      .home-decice-Carousel {
        height: 100%;
        width: 100%;

        .home-decice-Carousel-item {
          height: 100%;
          width: 100%;
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;

          .home-deviceValue {
            width: 100%;
            padding: 0.15rem 0;
            margin-bottom: 0.2rem;
            border-bottom: 0.01rem dashed rgba(255, 255, 255, 0.2);
            display: flex;
            justify-content: space-between;

            .home-deviceValue-item {
              width: 50%;
              display: flex;
              align-items: flex-end;
              justify-content: center;
              font-size: 0.16rem;
              font-weight: 400;
              color: rgba(255, 255, 255, 0.6);
              height: 0.16rem;

              .deviceValue-item-title {
                font-size: 0.16rem;
                font-weight: 600;
                margin-right: 0.09rem;
              }

              .deviceValue-item-value {
                font-size: 0.22rem;
                font-weight: 700;
                color: #fff;
                line-height: 0.26rem;
              }

              .deviceValue-item-unit {
                font-size: 0.14rem;
                font-weight: 400;
              }
            }
          }

          img {
            width: 1.2rem;
            height: 0.73rem;
            object-fit: contain;
            margin-bottom: 0.05rem;
          }

          .item-name {
            font-size: 0.18rem;
            font-weight: 500;
            line-height: 100%;
            letter-spacing: 0%;
          }
        }
      }
    }

    .home-alters {
      height: 30.89%;
      width: 100%;

      .home-altersList {
        height: 100%;
        overflow-y: scroll;
        // 默认隐藏滚动条
        scrollbar-width: none;
        /* Firefox */
        -ms-overflow-style: none;
        /* IE and Edge */

        // Webkit浏览器隐藏滚动条
        &::-webkit-scrollbar {
          width: 0;
          height: 0;
        }

        // 鼠标悬停时显示滚动条
        &:hover {
          scrollbar-width: auto;
          /* Firefox */
          -ms-overflow-style: auto;
          /* IE and Edge */

          &::-webkit-scrollbar {
            width: 0.04rem;
            height: 0.04rem;
          }

          &::-webkit-scrollbar-thumb {
            border-radius: 0.02rem;
          }
        }

        .home-altersItem {
          min-height: 0.9rem;
          border-bottom: 0.01rem dashed rgba(255, 255, 255, 0.2);
          display: flex;
          align-items: center;

          .alters__item-name {
            width: 0.4rem;
            font-size: 0.16rem;
            font-weight: 700;
            line-height: 0.16rem;
            margin-right: 0.17rem;
          }

          .alters__item-icon {
            width: 0.46rem;
            height: 0.2rem;
            object-fit: contain;
            margin-right: 0.1rem;
          }

          .alters__item-msg {
            font-size: 0.14rem;
            line-height: 0.16rem;
            font-weight: 400;

            &:last-child {
              border-bottom: none;
            }
          }

          &:last-child {
            border-bottom: none;
          }
        }
      }
    }
  }
}

:deep(.el-carousel, .el-carousel .el-carousel__container) {
  height: 100% !important;
}

:deep(.el-carousel .el-carousel__container) {
  height: 100% !important;
}

:deep(.el-carousel__item) {
  display: flex;
  align-items: center;
  justify-content: center;
}

// 自定义carousel控制按钮样式
.custom-carousel-controls {
  width: 100%;
  height: 0.32rem;
  position: absolute;
  top: 50%;
  left: 0;
  right: 0;
  transform: translateY(-50%);
  z-index: 999;
}

.custom-arrow {
  position: absolute;
  width: 0.32rem;
  height: 0.32rem;
  cursor: pointer;

  img {
    width: 0.32rem;
    height: 0.32rem;
    object-fit: contain;
  }

  // &:hover {
  //   background-color: rgba(84, 98, 140, 1);
  // }
}

.custom-arrow-left {
  left: 0.1rem;
}

.custom-arrow-right {
  right: 0.1rem;
}
</style>
