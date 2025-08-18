<template>
  <div class="voltage-class home">
    <EnergyBg></EnergyBg>
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
          <div class="home-deviceList">
            <div class="home-deviceValue">
              <div class="home-deviceValue-item" v-for="item in deviceInfoList" :key="item.title">
                <span class="deviceValue-item-title">{{ item.title }}:</span>
                <span class="deviceValue-item-value">{{ item.value }}</span>
                &nbsp;
                <span class="deviceValue-item-unit">{{ item.unit }}</span>
              </div>
            </div>
            <div class="home-decice-Carousel">
              <el-carousel
                ref="carouselRef"
                :autoplay="false"
                arrow="never"
                indicator-position="none"
              >
                <el-carousel-item v-for="item in 4" :key="item">
                  <div class="home-decice-Carousel-item">
                    <img src="../../assets/images/device-PV.png" />
                    <div class="item-name">PV</div>
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
import EnergyBg from './EnergyBg.vue'

const energyDashboardList = reactive<EnergyCard[]>([
  {
    title: 'PV Energy',
    icon: 'PVEnergy',
    value: '150',
    unit: 'kWh',
  },
  {
    title: 'Diesel Energy',
    icon: 'DieselEnergy',
    value: '150',
    unit: 'KWh',
  },
  {
    title: 'Energy Used',
    icon: 'EnergyUsed',
    value: '150',
    unit: 'kWh',
  },
  {
    title: 'Saving Billing',
    icon: 'SavingBilling',
    value: '$ 40.7',
  },
])
const stationInfoList = reactive<EnergyCard[]>([
  {
    title: 'PV',
    icon: 'PVEnergy',
    value: '150',
    unit: 'kWh',
  },
  {
    title: 'Diesel',
    icon: 'DieselEnergy',
    value: '150',
    unit: 'kWh',
  },
  {
    title: 'ESS',
    icon: 'ESSEnergy',
    value: '145',
    unit: 'KWh',
  },
])

const deviceInfoList = reactive([
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
.voltage-class {
  &.home {
    width: 100%;
    height: 100%;
    display: flex;
    justify-content: space-between;
    .home-left {
      width: calc(100% - 390px);
      height: 100%;
      margin-right: 20px;
      display: flex;
      flex-direction: column;
      justify-content: space-between;
      .home-left-top {
        width: 100%;
        padding-top: 10px;
        display: flex;
        justify-content: space-between;
        .home-left-top-item {
          height: 70px;
        }
      }
      .home-left-bottom {
        width: 100%;
        height: 30.89%;
        display: flex;
        justify-content: space-between;
        .home-left-EnergyChart {
          width: calc((100% - 20px) / 2);
          height: 100%;
        }
        .home-left-LineChart {
          width: calc((100% - 20px) / 2);
          height: 100%;
        }
      }
    }
    .home-right {
      width: 370px;
      height: 100%;
      display: flex;
      flex-direction: column;
      justify-content: space-between;
      gap: 20px;
      .home-station {
        width: 100%;
        height: 36.75%;
        .home-stationList {
          height: 100%;
          padding-top: 20px;
          overflow-y: scroll;
          .home-stationItem {
            margin-bottom: 12px;
            padding-bottom: 13px;
            border-bottom: 1px dashed rgba(255, 255, 255, 0.2);
            &:last-child {
              border-bottom: none;
              padding-bottom: 0;
              margin-bottom: 0;
            }
          }
        }
      }
      .home-device {
        width: 100%;
        height: 28.27%;
        .home-deviceList {
          height: 100%;
          padding-top: 20px;
          display: flex;
          flex-direction: column;
          .home-deviceValue {
            width: 100%;
            padding: 20px 0;
            margin-bottom: 20px;
            border-bottom: 1px dashed rgba(255, 255, 255, 0.2);
            display: flex;
            justify-content: space-between;
            .home-deviceValue-item {
              min-width: 130px;
              display: flex;
              align-items: flex-end;
              font-size: 16px;
              font-weight: 400;
              color: rgba(255, 255, 255, 0.6);
              height: 16px;
              .deviceValue-item-title {
                font-size: 16px;
                font-weight: 600;
                margin-right: 9px;
              }
              .deviceValue-item-value {
                font-size: 22px;
                font-weight: 700;
                color: #fff;
                line-height: 26px;
              }
              .deviceValue-item-unit {
                font-size: 14px;
                font-weight: 400;
              }
            }
          }
          .home-decice-Carousel {
            width: 100%;
            flex: 1;
            position: relative;
            .home-decice-Carousel-item {
              width: 100%;
              height: 100%;
              display: flex;
              align-items: center;
              justify-content: center;
              flex-direction: column;
              img {
                width: 100%;
                height: calc(100% - 23px);
                object-fit: contain;
                margin-bottom: 5px;
              }
              .item-name {
                font-size: 18px;
                font-weight: 500;
                line-height: 100%;
                letter-spacing: 0%;
              }
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
          .home-altersItem {
            min-height: 90px;
            border-bottom: 1px dashed rgba(255, 255, 255, 0.2);
            display: flex;
            align-items: center;
            .alters__item-name {
              width: 40px;
              font-size: 16px;
              font-weight: 700;
              line-height: 16px;
              margin-right: 17px;
            }
            .alters__item-icon {
              width: 46px;
              height: 20px;
              object-fit: contain;
              margin-right: 10px;
            }

            .alters__item-msg {
              font-size: 14px;
              line-height: 16px;
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
  height: 32px;
  position: absolute;
  top: 50%;
  left: 0;
  right: 0;
  transform: translateY(-50%);
  z-index: 999;
}

.custom-arrow {
  position: absolute;
  width: 32px;
  height: 32px;
  cursor: pointer;

  img {
    width: 32px;
    height: 32px;
    object-fit: contain;
  }

  // &:hover {
  //   background-color: rgba(84, 98, 140, 1);
  // }
}

.custom-arrow-left {
  left: 10px;
}

.custom-arrow-right {
  right: 10px;
}
</style>
