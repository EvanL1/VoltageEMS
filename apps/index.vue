<template>
  <div class="voltage-class analytics">
    <!-- 表格区域 -->
    <div class="analytics__content">
      <!-- 表格工具栏 -->
      <div class="analytics__toolbar">
        <div class="analytics__toolbar-left">
          <!-- 选择框 -->
          <div class="analytics__toolbar-left-title">Analytics Dashboard</div>
        </div>

        <div class="analytics__toolbar-right">
          <div class="analytics__toolbar-time-btns" @click="handleTimeBtnClick">
            <div
              v-show="selectedTimeBtn === 'custom'"
              class="analytics__toolbar-time-interval"
              ref="toolbarRightRef"
            >
              <el-select
                v-model="timeInterval"
                @change="getAllAnalyticsData"
                :append-to="toolbarRightRef"
                placeholder="Select Time Interval"
              >
                <el-option
                  v-for="btn in intervalList"
                  :key="btn.value"
                  :label="btn.label"
                  :value="btn.value"
                />
              </el-select>
            </div>
            <el-date-picker
              v-if="selectedTimeBtn === 'custom'"
              v-model="rangeArray"
              type="datetimerange"
              value-format="YYYY-MM-DD HH:mm:ss"
              format="YYYY-MM-DD HH:mm:ss"
              range-separator="To"
              :default-time="defaultTime"
              start-placeholder="Select Start Date"
              end-placeholder="Select End Date"
              :teleported="false"
              @change="getAllAnalyticsData"
            />
            <div
              v-for="btn in timeBtnList"
              :key="btn.value"
              class="analytics__toolbar-time-btn"
              :class="{ 'is-active': selectedTimeBtn === btn.value }"
              :data-value="btn.value"
            >
              {{ btn.label }}
            </div>
          </div>
        </div>
      </div>

      <!-- 表格 -->
      <div class="analytics__charts">
        <div class="analytics__chart-item">
          <ModuleCard title="Energy Structure" :loading="!stationLoading && chart1Loading">
            <DoughnutChart title="Energy Structure" :series="chart1Series" />
          </ModuleCard>
        </div>
        <div class="analytics__chart-item">
          <ModuleCard title="Power Monitoring" :loading="!stationLoading && chart2Loading">
            <LineChart
              title="Power Monitoring"
              :xAxiosOption="xAxiosOption"
              :yAxiosOption="poweryAxiosOption"
              :series="chart2Series"
            />
          </ModuleCard>
        </div>
        <div class="analytics__chart-item">
          <ModuleCard
            title="Energy Storage Operation Status"
            :loading="!stationLoading && chart3Loading"
          >
            <LineAndBarChart
              title="Energy Storage Operation Status"
              :xAxiosOption="xAxiosOption"
              :yAxiosOption="EssyAxiosOption"
              :lineSeries="chart3Series.lineSeries"
              :barSeries="chart3Series.barSeries"
            />
          </ModuleCard>
        </div>
        <div class="analytics__chart-item">
          <ModuleCard
            title="Electricity Generation Statistics"
            :loading="!stationLoading && chart4Loading"
          >
            <MultiBarChar
              title="Electricity Generation Statistics"
              :xAxiosOption="xAxiosOption"
              :yAxiosOption="generateyAxiosOption"
              :series="chart4Series"
            />
          </ModuleCard>
        </div>
        <div class="analytics__chart-item">
          <ModuleCard
            title="Energy Supply And Demand Balance"
            :loading="!stationLoading && chart5Loading"
          >
            <MultiBarChar
              title="Energy Supply And Demand Balance"
              :xAxiosOption="xAxiosOption"
              :yAxiosOption="energyyAxiosOption"
              :series="chart5Series"
            />
          </ModuleCard>
        </div>
        <div class="analytics__chart-item">
          <ModuleCard title="Economic Benefit Analysis" :loading="!stationLoading && chart6Loading">
            <SingleBarChart
              title="Economic Benefit Analysis"
              :xAxiosOption="xAxiosOption"
              :yAxiosOption="economicyAxiosOption"
              :series="economicSeries"
            />
          </ModuleCard>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
// import PVEnergy from '@/assets/icons/PVEnergy.svg'
// import ESS from '@/assets/icons/ESSEnergy.svg'
// import DG from '@/assets/icons/DGEnergy.svg'
import { getTimeRangeArray } from '@/utils/time'
import { getAnalyticsData } from '@/api/station'
import { useRoute } from 'vue-router'
import { cancelAllPendingRequests } from '@/utils/request'
import type { AnalyticsIntervalData } from '@/types/Stations'
// import { getSmartTimeStep } from '@/utils/time'
defineProps<{
  stationLoading: boolean
}>()

const route = useRoute()
const toolbarRightRef = ref<HTMLElement | null>(null)
// 时间按钮列表
const timeBtnList: { label: string; value: '6hours' | '1day' | '1week' | '1month' | 'custom' }[] = [
  { label: 'Custom', value: 'custom' },
  { label: '6 Hour', value: '6hours' },
  { label: '1 Day', value: '1day' },
  { label: '1 Week', value: '1week' },
  { label: '1 Month', value: '1month' },
]
const defaultTime: [Date, Date] = [new Date(2000, 0, 1, 0, 0, 0), new Date(2000, 0, 1, 23, 59, 59)]
// 当前选中的时间按钮
const selectedTimeBtn = ref<'6hours' | '1day' | '1week' | '1month' | 'custom'>('6hours')
const stationId = ref<number | string>(route.params.stationId as string | number)
const chart1Loading = ref(false)
const chart2Loading = ref(false)
const chart3Loading = ref(false)
const chart4Loading = ref(false)
const chart5Loading = ref(false)
const chart6Loading = ref(false)
const interval = ref('')
const timeInterval = ref('15m')
const intervalList = ref([
  { label: '30 Seconds', value: '30s' },
  { label: '1 Minute', value: '1m' },
  { label: '15 Minutes', value: '15m' },
  { label: '1 Hour', value: '1h' },
  { label: '6 Hours', value: '6h' },
  { label: '1 Day', value: '1d' },
  { label: '1 Week', value: '1w' },
  { label: '1 Month', value: '1M' },
  { label: '1 Year', value: '1y' },
])
const chart1Series = ref([
  {
    name: 'PV',
    value: 0,
    color: '#ee6666',
  },
  {
    name: 'DG',
    value: 0,
    color: '#ffe58f',
  },
  {
    name: 'ESS',
    value: 0,
    color: '#91CC75',
  },
])
const chart2Series = ref([
  {
    name: 'PV',
    data: [] as number[],
    color: '#ee6666',
  },
  {
    name: 'DG',
    data: [] as number[],
    color: '#ffe58f',
  },
  {
    name: 'ESS',
    data: [] as number[],
    color: '#91CC75',
  },
])

const poweryAxiosOption = {
  yUnit: 'kW',
}

const chart3Series = ref({
  barSeries: [
    {
      name: 'charge',
      data: [] as number[],

      color: '#fc8452',
    },
    {
      name: 'discharge',
      data: [] as number[],
      color: '#597ef7',
    },
  ],
  lineSeries: [
    {
      name: 'SOC',
      data: [] as number[],
      color: '#66bb6a',
    },
  ],
})
const EssyAxiosOption = {
  yUnit: ['kw', '%'],
}

const chart4Series = ref([
  {
    name: 'PV',
    data: [] as number[],
    color: '#ee6666',
  },
  {
    name: 'DG',
    data: [] as number[],
    color: '#ffe58f',
  },
])
const generateyAxiosOption = {
  yUnit: 'kWh',
}

const chart5Series = ref([
  {
    name: 'PV',
    data: [] as number[],
    color: '#ee6666',
    stack: 'generation',
  },
  {
    name: 'DG',
    data: [] as number[],
    color: '#ffe58f',
    stack: 'generation',
  },
  {
    name: 'ESS charge',
    data: [] as number[],
    color: '#91CC75',
    stack: 'generation',
  },
  {
    name: 'ESS discharge',
    data: [] as number[],
    color: '#91CC75',
    stack: 'load',
  },

  {
    name: 'Load',
    data: [] as number[],
    color: '#1d86ff',
    stack: 'load',
  },
])
const energyyAxiosOption = {
  yUnit: 'kWh',
}

const economicSeries = ref([
  {
    name: 'total revenue',
    data: [] as number[],
    color: '#9ACD32',
  },
])
const economicyAxiosOption = {
  yUnit: '$',
}
const xAxiosOption = ref({
  xAxiosData: [] as string[],
})
const rangeArray = ref<string[]>([])

// 事件代理处理时间按钮点击
const handleTimeBtnClick = (event: MouseEvent) => {
  const target = event.target as HTMLElement
  // 查找最近的按钮元素
  const btn = target.closest('.analytics__toolbar-time-btn') as HTMLElement | null
  if (btn && btn.dataset.value) {
    selectedTimeBtn.value = btn.dataset.value as '6hours' | '1day' | '1week' | '1month' | 'custom'
    rangeArray.value = []
    if (selectedTimeBtn.value !== 'custom') {
      getAllAnalyticsData()
    }
  }
}
const fetchChart1Data = async () => {
  try {
    chart1Loading.value = true
    chart1Series.value.forEach((item: any) => {
      item.data = []
    })
    const res = await getAnalyticsData({
      stationId: stationId.value,
      startTime: rangeArray.value[0],
      endTime: rangeArray.value[1],
      chartType: 'CHART1',
      interval: interval.value,
      isInterval: false,
    })
    if (res.code === 200) {
      chart1Series.value[0].value = (res.data.data1 as number) || 0
      chart1Series.value[1].value = (res.data.data2 as number) || 0
      chart1Series.value[2].value = (res.data.data3 as number) || 0
    }
  } finally {
    chart1Loading.value = false
  }
}
const fetchChart2Data = async () => {
  try {
    chart2Loading.value = true
    chart2Series.value.forEach((item: any) => {
      item.data = []
    })
    const res = await getAnalyticsData({
      stationId: stationId.value,
      startTime: rangeArray.value[0],
      endTime: rangeArray.value[1],
      chartType: 'CHART2',
      interval: interval.value,
      isInterval: true,
    })
    if (res.code === 200) {
      // 修复数据更新逻辑：直接更新每个系列的data数组
      xAxiosOption.value.xAxiosData = (res.data['data1'] as any[]).map((item: any) =>
        item.ts.replace(' ', '\n'),
      )
      chart2Series.value.forEach((item: any, seriesIndex: number) => {
        const dataKey = `data${seriesIndex + 1}` as keyof AnalyticsIntervalData
        if (res.data[dataKey] && Array.isArray(res.data[dataKey])) {
          item.data = (res.data[dataKey] as any[]).map((item: any) => item.numberValue)
        }
      })
    }
  } finally {
    chart2Loading.value = false
  }
}
const fetchChart3Data = async () => {
  try {
    chart3Loading.value = true
    chart3Series.value.barSeries.forEach((item: any) => {
      item.data = []
    })
    chart3Series.value.lineSeries.forEach((item: any) => {
      item.data = []
    })
    const res = await getAnalyticsData({
      stationId: stationId.value,
      startTime: rangeArray.value[0],
      endTime: rangeArray.value[1],
      chartType: 'CHART3',
      interval: interval.value,
      isInterval: true,
    })
    if (res.code === 200) {
      xAxiosOption.value.xAxiosData = (res.data['data1'] as any[]).map((item: any) =>
        item.ts.replace(' ', '\n'),
      )
      chart3Series.value.barSeries.forEach((item: any, seriesIndex: number) => {
        const dataKey = `data${seriesIndex + 2}` as keyof AnalyticsIntervalData
        if (res.data[dataKey] && Array.isArray(res.data[dataKey])) {
          item.data = (res.data[dataKey] as any[]).map((item: any) => item.numberValue)
        }
      })
      chart3Series.value.lineSeries.forEach((item: any, seriesIndex: number) => {
        const dataKey = `data${seriesIndex + 1}` as keyof AnalyticsIntervalData
        if (res.data[dataKey] && Array.isArray(res.data[dataKey])) {
          item.data = (res.data[dataKey] as any[]).map((item: any) => item.numberValue)
        }
      })
    }
  } finally {
    chart3Loading.value = false
  }
}
const fetchChart4Data = async () => {
  try {
    chart4Loading.value = true
    chart4Series.value.forEach((item: any) => {
      item.data = []
    })
    const res = await getAnalyticsData({
      stationId: stationId.value,
      startTime: rangeArray.value[0],
      endTime: rangeArray.value[1],
      chartType: 'CHART4',
      interval: interval.value,
      isInterval: true,
    })
    if (res.code === 200) {
      // 修复数据更新逻辑：直接更新每个系列的data数组
      xAxiosOption.value.xAxiosData = (res.data['data1'] as any[]).map((item: any) =>
        item.ts.replace(' ', '\n'),
      )
      chart4Series.value.forEach((item: any, seriesIndex: number) => {
        const dataKey = `data${seriesIndex + 1}` as keyof AnalyticsIntervalData
        if (res.data[dataKey] && Array.isArray(res.data[dataKey])) {
          item.data = (res.data[dataKey] as any[]).map((item: any) => item.numberValue)
        }
      })
      // chart4Series.value[0].data = (res.data as AnalyticsIntervalData).data1.map((item: any) => item.numberValue || 0)
      // chart4Series.value[1].data = (res.data as AnalyticsIntervalData).data2.map((item: any) => item.numberValue || 0)
    }
  } finally {
    chart4Loading.value = false
  }
}
const fetchChart5Data = async () => {
  try {
    chart5Loading.value = true
    chart5Series.value.forEach((item: any) => {
      item.data = []
    })
    const res = await getAnalyticsData({
      stationId: stationId.value,
      startTime: rangeArray.value[0],
      endTime: rangeArray.value[1],
      chartType: 'CHART5',
      interval: interval.value,
      isInterval: true,
    })
    if (res.code === 200) {
      // 修复数据更新逻辑：直接更新每个系列的data数组
      xAxiosOption.value.xAxiosData = (res.data['data1'] as any[]).map((item: any) =>
        item.ts.replace(' ', '\n'),
      )
      chart5Series.value.forEach((item1: any, seriesIndex: number) => {
        const dataKey = `data${seriesIndex + 1}` as keyof AnalyticsIntervalData
        if (res.data[dataKey] && Array.isArray(res.data[dataKey])) {
          item1.data = (res.data[dataKey] as any[]).map((item: any) => item.numberValue)
        }
      })
    }
  } finally {
    chart5Loading.value = false
  }
}
const fetchChart6Data = async () => {
  try {
    chart6Loading.value = true
    economicSeries.value.forEach((item: any) => {
      item.data = []
    })
    const res = await getAnalyticsData({
      stationId: stationId.value,
      startTime: rangeArray.value[0],
      endTime: rangeArray.value[1],
      chartType: 'CHART6',
      interval: interval.value,
      isInterval: true,
    })
    if (res.code === 200) {
      // 修复数据更新逻辑：直接更新每个系列的data数组
      xAxiosOption.value.xAxiosData = (res.data['data1'] as any[]).map((item: any) =>
        item.ts.replace(' ', '\n'),
      )
      economicSeries.value.forEach((item: any, seriesIndex: number) => {
        const dataKey = `data${seriesIndex + 1}` as keyof AnalyticsIntervalData
        if (res.data[dataKey] && Array.isArray(res.data[dataKey])) {
          item.data = (res.data[dataKey] as any[]).map((item: any) => item.numberValue)
        }
      })
      // economicSeries.value[0].data = (res.data as AnalyticsIntervalData).data1.map((item: any) => item.numberValue || 0)
    }
  } finally {
    chart6Loading.value = false
  }
}
const getAllAnalyticsData = () => {
  cancelAllPendingRequests()

  // chart1Loading.value = true
  // chart2Loading.value = true
  // chart3Loading.value = true
  // chart4Loading.value = true
  // chart5Loading.value = true
  // chart6Loading.value = true
  setTimeout(() => {
    if (selectedTimeBtn.value === 'custom') {
      interval.value = timeInterval.value
      if (rangeArray.value.every((item: string) => item === '')) return
    } else {
      rangeArray.value = getTimeRangeArray(selectedTimeBtn.value)
      switch (selectedTimeBtn.value) {
        case '6hours':
          interval.value = '1h'
          break
        case '1day':
          interval.value = '1h'
          break
        case '1week':
          interval.value = '1d'
          break
        case '1month':
          interval.value = '1d'
          break
        default:
          return
      }
    }

    fetchChart1Data()
    fetchChart2Data()
    fetchChart3Data()
    fetchChart4Data()
    fetchChart5Data()
    fetchChart6Data()
  }, 100)
}

// 路由变化时取消当前页面请求并重新获取数据；首次进入也执行（使用动态路由参数）
watch(
  () => route.params.stationId,
  (id) => {
    if (id) {
      stationId.value = id as string | number
    }
    cancelAllPendingRequests()
    getAllAnalyticsData()
  },
  { immediate: true },
)
</script>

<style scoped lang="scss">
.analytics {
  height: 100%;
  width: 100%;

  .analytics__content {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .analytics__toolbar {
    padding-bottom: 0.2rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 0.01rem solid rgba(255, 255, 255, 0.1);

    margin-bottom: 0.2rem;

    .analytics__toolbar-left {
      display: flex;
      align-items: center;
      gap: 0.16rem;

      .analytics__toolbar-left-title {
        font-size: 0.2rem;
        line-height: 0.22rem;
        color: #ffffff;
      }
    }

    .analytics__toolbar-right {
      display: flex;
      align-items: center;
      gap: 0.2rem;

      .analytics__toolbar-time-btns {
        position: relative;
        height: 0.32rem;
        display: flex;
        align-items: center;

        .analytics__toolbar-time-interval {
          position: relative;
          margin-right: 0.2rem;
        }

        .analytics__toolbar-time-btn {
          background-color: rgba(255, 255, 255, 0.4);
          height: 0.32rem;
          line-height: 0.32rem;
          padding: 0 0.1rem;
          font-size: 0.14rem;
          background: transparent;
          border-right: 0.01rem solid rgba(255, 255, 255, 0.2);
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
  }

  .analytics__charts {
    height: calc(100% - 0.72rem);
    display: flex;
    flex-wrap: wrap;
    gap: 0.2rem;

    .analytics__chart-item {
      width: calc((100% - 0.4rem) / 3);
      height: calc(50% - 0.1rem);

      // .chart__review {
      //   width: 100%;
      //   height: 100%;
      //   padding: 0.2rem;
      //   display: flex;
      //   flex-direction: column;
      //   background-color: rgba(84, 98, 140, 0.2);
      //   border: 0.01rem solid;

      //   border-image: linear-gradient(117.01deg,
      //       rgba(148, 166, 197, 0.3) 3.11%,
      //       rgba(148, 166, 197, 0) 31.6%,
      //       rgba(148, 166, 197, 0.103266) 70.79%,
      //       rgba(148, 166, 197, 0.3) 96.39%) 1;
      //   backdrop-filter: blur(0.1rem);

      //   .chart__review-header {
      //     height: 0.83rem;
      //     // padding: 0 rem;
      //     // background-color: rgba(84, 98, 140, 0.2);
      //     // border: 0.01rem solid;

      //     // border-image: linear-gradient(117.01deg,
      //     //     rgba(148, 166, 197, 0.3) 3.11%,
      //     //     rgba(148, 166, 197, 0) 31.6%,
      //     //     rgba(148, 166, 197, 0.103266) 70.79%,
      //     //     rgba(148, 166, 197, 0.3) 96.39%) 1;
      //     // backdrop-filter: blur(0.1rem);
      //     display: flex;
      //     justify-content: space-between;
      //     align-items: center;

      //     .chart__review-header-title {
      //       font-weight: 700;
      //       font-size: 0.26rem;
      //       line-height: 100%;
      //       letter-spacing: 0%;
      //     }

      //     .chart__review-header-value {
      //       font-weight: 700;
      //       font-size: 0.22rem;
      //       line-height: 0.3rem;
      //       letter-spacing: 0%;

      //       .chart__review-header-unit {
      //         font-size: 0.14rem;
      //         line-height: 0.3rem;
      //         letter-spacing: 0%;
      //         color: rgba(255, 255, 255, 0.6);
      //       }
      //     }
      //   }

      //   .chart__review-content {
      //     flex: 1;
      //     padding-top: 0.15rem;

      //     .chart__review-content-list {
      //       height: 100%;
      //       overflow-y: hidden;

      //       .chart__review-content-item {
      //         height:calc((100% - 0.32rem) / 3);
      //         margin-bottom: 0.12rem;
      //         padding-bottom: 0.13rem;
      //         border-bottom: 0.01rem dashed rgba(255, 255, 255, 0.2);

      //         &:last-child {
      //           border-bottom: none;
      //           padding-bottom: 0;
      //           margin-bottom: 0;
      //         }
      //       }
      //     }
      //   }
      // }
    }
  }
}
</style>
