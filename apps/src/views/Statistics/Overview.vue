<template>
  <div class="voltage-class curves">
    <!-- 表格区域 -->
    <div class="curves__content">
      <!-- 表格工具栏 -->
      <div class="curves__toolbar">
        <div class="curves__toolbar-left" ref="toolbarLeftRef">
          <!-- 选择框 -->
          <el-select
            v-model="selectedFilter"
            placeholder="请选择筛选条件"
            @change="handleFilterChange"
            :append-to="toolbarLeftRef"
          >
            <el-option label="All" value="all" />
            <el-option label="High Priority" value="high" />
            <el-option label="Medium Priority" value="medium" />
            <el-option label="Low Priority" value="low" />
          </el-select>
        </div>

        <div class="curves__toolbar-right">
          <div class="curves__toolbar-time-btns" @click="handleTimeBtnClick">
            <div
              v-for="btn in timeBtnList"
              :key="btn.value"
              class="curves__toolbar-time-btn"
              :class="{ 'is-active': selectedTimeBtn === btn.value }"
              :data-value="btn.value"
            >
              {{ btn.label }}
            </div>
          </div>
        </div>
      </div>

      <!-- 图表区域 -->
      <div class="curves__charts">
        <!-- 概览卡片 -->
        <div class="curves__chart-item">
          <div class="chart__review">
            <div class="chart__review-header">
              <div class="chart__review-header-title">Energy consumption</div>
              <div class="chart__review-header-value">
                1000 &nbsp;<span class="chart__review-header-unit">kWh</span>
              </div>
            </div>
            <div class="chart__review-content">
              <div class="chart__review-content-list">
                <div
                  v-for="item in stationInfoList"
                  :key="item.title"
                  class="chart__review-content-item"
                >
                  <EnergyCard
                    :title="item.title"
                    :icon="item.icon"
                    :value="item.value"
                    :unit="item.unit"
                  />
                </div>
              </div>
            </div>
          </div>
        </div>
        <!-- 其余图表卡片 -->
        <div class="curves__chart-item" v-for="(item, idx) in 5" :key="idx">
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
  </div>
</template>

<script setup lang="ts">
const toolbarLeftRef = ref<HTMLElement | null>(null)
const activeTab = ref<'current' | 'history'>('current')
const selectedFilter = ref('all')

// 时间按钮列表
const timeBtnList = [
  { label: '6 Hour', value: '6h' },
  { label: '1 Day', value: '1d' },
  { label: '1 Week', value: '1w' },
  { label: '1 Month', value: '1m' },
]
const stationInfoList = reactive([
  {
    title: 'PV',
    icon: 'PVEnergy',
    value: '150',
    unit: 'kWh',
  },
  {
    title: 'ESS',
    icon: 'ESS',
    value: '150',
    unit: 'kWh',
  },
  // {
  //   title: 'DG',
  //   icon: 'DG',
  //   value: '145',
  //   unit: 'kWh', // 修正单位大小写
  // },
])
// 当前选中的时间按钮
const selectedTimeBtn = ref('6h')

// 事件代理处理时间按钮点击
const handleTimeBtnClick = (event: MouseEvent) => {
  const target = event.target as HTMLElement
  // 查找最近的按钮元素
  const btn = target.closest('.curves__toolbar-time-btn') as HTMLElement | null
  if (btn && btn.dataset.value) {
    selectedTimeBtn.value = btn.dataset.value
    console.log(selectedTimeBtn.value)
  }
}

const handleFilterChange = () => {
  console.log('handleFilterChange')
}
const handleExport = () => {
  console.log('handleExport')
}
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
const xAxiosOption = {
  xAxiosData: exampleXAxisData,
}
const yAxiosOption = {
  yUnit: 'kWh',
}
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
</script>

<style scoped lang="scss">
.voltage-class.curves {
  height: 100%;
  width: 100%;
  .curves__content {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
  }
  .curves__toolbar {
    padding: 20px 0;
    display: flex;
    align-items: center;
    justify-content: space-between;

    .curves__toolbar-left {
      display: flex;
      align-items: center;
      gap: 16px;
    }

    .curves__toolbar-time-btns {
      height: 32px;
      display: flex;
      align-items: center;
      background-color: rgba(255, 255, 255, 0.04);
      .curves__toolbar-time-btn {
        height: 28px;
        line-height: 22px;
        padding: 3px 10px;
        font-size: 14px;
        background: transparent;
        border-right: 1px solid rgba(255, 255, 255, 0.2);
        cursor: pointer;
        &:last-child {
          border-right: none;
        }
        &.is-active {
          background: rgba(255, 255, 255, 0.2);
        }
      }
    }
  }
  .curves__charts {
    height: calc(100% - 72px);
    display: flex;
    flex-wrap: wrap;
    gap: 20px;
    .curves__chart-item {
      width: calc((100% - 40px) / 3);
      height: calc((100% - 20px) / 2);
      .chart__review {
        width: 100%;
        height: 100%;
        padding: 20px;
        display: flex;
        flex-direction: column;
        background-color: rgba(84, 98, 140, 0.2);
        border: 1px solid;

        border-image: linear-gradient(
            117.01deg,
            rgba(148, 166, 197, 0.3) 3.11%,
            rgba(148, 166, 197, 0) 31.6%,
            rgba(148, 166, 197, 0.103266) 70.79%,
            rgba(148, 166, 197, 0.3) 96.39%
          )
          1;
        backdrop-filter: blur(10px);
        .chart__review-header {
          height: 83px;
          padding: 0 20px;
          background-color: rgba(84, 98, 140, 0.2);
          border: 1px solid;

          border-image: linear-gradient(
              117.01deg,
              rgba(148, 166, 197, 0.3) 3.11%,
              rgba(148, 166, 197, 0) 31.6%,
              rgba(148, 166, 197, 0.103266) 70.79%,
              rgba(148, 166, 197, 0.3) 96.39%
            )
            1;
          backdrop-filter: blur(10px);
          display: flex;
          justify-content: space-between;
          align-items: center;
          .chart__review-header-title {
            font-weight: 700;
            font-size: 26px;
            line-height: 100%;
            letter-spacing: 0%;
          }
          .chart__review-header-value {
            font-weight: 700;
            font-size: 22px;
            line-height: 30px;
            letter-spacing: 0%;
            .chart__review-header-unit {
              font-size: 14px;
              line-height: 30px;
              letter-spacing: 0%;
              color: rgba(255, 255, 255, 0.6);
            }
          }
        }
        .chart__review-content {
          flex: 1;
          padding-top: 20px;
          .chart__review-content-list {
            height: 100%;
            overflow-y: auto;
            .chart__review-content-item {
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
      }
    }
  }
  :deep(.el-select__popper.el-popper) {
    top: 222px !important;
  }
}
</style>
