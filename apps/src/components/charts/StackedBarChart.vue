<template>
  <div class="stacked-bar-chart" ref="chartRef"></div>
</template>

<script setup lang="ts">
import * as echarts from 'echarts/core'
import { BarChart } from 'echarts/charts'
import { TooltipComponent, GridComponent, LegendComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import { useGlobalStore } from '@/stores/global'
import { pxToResponsive } from '@/utils/responsive'

const globalStore = useGlobalStore()

// 监听侧边栏折叠状态变化
watch(
  () => globalStore.isCollapse,
  () => {
    // 延迟重新绘制，确保DOM更新完成
    nextTick(() => {
      setTimeout(() => {
        chartInstance?.dispose()
        initChart()
      }, 300)
    })
  },
)

echarts.use([BarChart, TooltipComponent, GridComponent, LegendComponent, CanvasRenderer])

// ================== 示例数据（可用数据，供参考，实际使用时请传递props） ==================
// 下面是一些可用于该组件的示例数据，已注释，方便你参考和测试
/*
const exampleXAxisData = ['周一', '周二', '周三', '周四', '周五', '周六', '周日']

const exampleSeries = [
  {
    name: '光伏发电',
    data: [120, 132, 101, 134, 90, 230, 210],
    color: '#4FADF7'
  },
  {
    name: '柴油发电',
    data: [220, 182, 191, 234, 290, 330, 310],
    color: '#F6C85F'
  },
  {
    name: '储能放电',
    data: [150, 232, 201, 154, 190, 330, 410],
    color: '#6DD400'
  }
]

// 传递给组件的props示例
const xAxiosOption = {
  xAxiosData: exampleXAxisData,
  xUnit: '天'
}
const yAxiosOption = {
  yUnit: 'kWh'
}
const series = exampleSeries
*/

// 定义数据类型
interface SeriesData {
  name: string
  data: number[]
  color: string
}

interface XAxisOption {
  xAxiosData: string[]
  xUnit?: string
}

interface YAxisOption {
  yUnit?: string
}

const props = defineProps<{
  xAxiosOption: XAxisOption
  yAxiosOption: YAxisOption
  series: SeriesData[]
}>()

const chartRef = ref<HTMLDivElement | null>(null)
let chartInstance: echarts.ECharts | null = null

// 自定义tooltip formatter，保证内容不换行错位
function customTooltipFormatter(params: any) {
  const tooltipWidth = pxToResponsive(220)
  const tooltipMinHeight = pxToResponsive(100)
  const name = params[0]?.axisValueLabel || params[0]?.name || ''
  let html = `
    <div style="
      max-width:${tooltipWidth}px;
      display:flex;
      flex-direction:column;
      gap:${pxToResponsive(8)}px;
    ">
      <div style="
        color:rgba(255,255,255,0.85);
        font-size:${pxToResponsive(14)}px;
        font-family:Arimo;
        font-weight:600;
        width:100%;
        margin-bottom:${pxToResponsive(4)}px;
      ">${name}</div>
  `
  params.forEach((item: any) => {
    // 颜色圆点
    html += `
      <div style="
        display:flex;
        align-items:center;
        justify-content:space-between;
        font-size:${pxToResponsive(12)}px;
        font-family:Arimo;
        color:rgba(255,255,255,0.85);
        line-height:${pxToResponsive(18)}px;
        margin-bottom:${pxToResponsive(2)}px;
      ">
        <div style="display:flex;align-items:center;gap:${pxToResponsive(6)}px;">
          <span style="
            display:inline-block;
            width:${pxToResponsive(8)}px;
            height:${pxToResponsive(8)}px;
            border-radius:50%;
            background:${item.color};
            margin-right:${pxToResponsive(4)}px;
          "></span>
          <span>${item.seriesName}</span>
        </div>
        <div style="font-weight:600;">${item.value}${props.yAxiosOption.yUnit ? ' ' + props.yAxiosOption.yUnit : ''}</div>
      </div>
    `
  })
  html += '</div>'
  return html
}

// 初始化echarts
const initChart = () => {
  if (!chartRef.value) return
  if (chartInstance) {
    console.log('dispose')
    chartInstance.dispose()
  }
  console.log('init', chartRef.value.clientWidth)
  chartInstance = echarts.init(chartRef.value)

  // 获取x轴和y轴单位
  const xUnit = props.xAxiosOption.xUnit || ''
  const yUnit = props.yAxiosOption.yUnit || ''

  // 计算总值用于背景柱
  const totalData = props.xAxiosOption.xAxiosData.map((_, index) => {
    return props.series.reduce((sum, s) => sum + (s.data[index] || 0), 0)
  })

  // 根据柱子数目自动计算bar宽度，使用pxToResponsive转换
  const chartWidth = chartRef.value.clientWidth || 600
  const dataCount = props.xAxiosOption.xAxiosData.length
  const availableWidth = chartWidth - pxToResponsive(100) // 减去左右边距
  const barSpacing = Math.max(pxToResponsive(15), (availableWidth * 0.1) / dataCount) // 柱子间距
  const barWidth = Math.min(
    pxToResponsive(60),
    (availableWidth - barSpacing * (dataCount - 1)) / dataCount,
  )

  const seriesData = [
    // 透明背景系列
    {
      name: 'background',
      type: 'bar',
      barWidth: barWidth,
      barGap: '-100%',
      itemStyle: {
        color: 'rgba(255,255,255,0)',
      },
      data: totalData,
      showBackground: true,
      backgroundStyle: {
        color: 'rgba(252, 252, 253, 0.04)',
      },
      silent: true,
      emphasis: { disabled: true },
      tooltip: { show: false },
      label: { show: false },
      z: 0,
    },
    ...props.series.map((s) => ({
      name: s.name,
      type: 'bar',
      data: s.data,
      stack: 'data',
      emphasis: { focus: 'series' },
      barWidth: barWidth * 0.7,
      barGap: '-85%',
      itemStyle: {
        color: s.color,
      },
      z: 1,
    })),
  ]

  chartInstance.setOption({
    legend: {
      show: true,
      type: 'plain',
      orient: 'horizontal',
      right: 0,
      top: 0,
      itemWidth: pxToResponsive(12),
      itemHeight: pxToResponsive(12),
      itemGap: pxToResponsive(25),
      textStyle: {
        color: 'rgba(255, 255, 255, 0.6)',
        fontSize: pxToResponsive(12),
        fontFamily: 'Arimo',
        fontWeight: 400,
      },
      data: props.series.map((s) => s.name),
    },
    grid: {
      left: 0,
      right: 0,
      top: pxToResponsive(45),
      bottom: pxToResponsive(15),
      containLabel: true,
    },
    tooltip: {
      trigger: 'axis',
      confine: true,
      backgroundColor: '#3f4f75',
      borderColor: 'rgba(255,255,255,0.12)',
      borderWidth: pxToResponsive(1),
      padding: [pxToResponsive(10), pxToResponsive(16), pxToResponsive(10), pxToResponsive(16)],
      extraCssText: `
        border-radius: ${pxToResponsive(8)}px;
        box-shadow: 0 ${pxToResponsive(4)}px ${pxToResponsive(16)}px 0 rgba(0,0,0,0.12);
        min-width: ${pxToResponsive(220)}px;
        max-width: ${pxToResponsive(220)}px;
        min-height: ${pxToResponsive(60)}px;
      `,
      textStyle: {
        fontFamily: 'Arimo',
        fontWeight: 400,
        fontSize: pxToResponsive(12),
        color: 'rgba(255,255,255,0.85)',
        lineHeight: pxToResponsive(18),
      },
      axisPointer: {
        type: 'shadow',
        shadowStyle: {
          color: 'rgba(79, 173, 247, 0.08)',
        },
        lineStyle: {
          width: pxToResponsive(1),
        },
      },
      // 使用自定义formatter
      formatter: customTooltipFormatter,
    },
    xAxis: {
      type: 'category',
      name: xUnit,
      nameTextStyle: {
        color: 'rgba(255, 255, 255, 0.6)',
        fontFamily: 'Arimo',
        fontWeight: 400,
        fontSize: pxToResponsive(12),
        padding: [pxToResponsive(10), 0, 0, 0],
      },
      data: props.xAxiosOption.xAxiosData,
      axisTick: {
        alignWithLabel: true,
        lineStyle: { color: '#fff' },
      },
      axisLine: { show: false },
      axisLabel: {
        color: 'rgba(255, 255, 255, 0.6)',
        fontFamily: 'Arimo',
        fontWeight: 400,
        fontSize: pxToResponsive(12),
      },
      splitLine: { show: false }, // 清除x轴网格线
      // 控制柱体居中的关键配置
      boundaryGap: true,
    },
    yAxis: {
      type: 'value',
      name: yUnit,
      nameTextStyle: {
        color: 'rgba(255, 255, 255, 0.6)',
        fontFamily: 'Arimo',
        fontWeight: 400,
        fontSize: pxToResponsive(12),
        align: 'right', // 水平居中对齐
        padding: [0, pxToResponsive(8), pxToResponsive(5), 0], // 上右下左的内边距，向下偏移5px
      },
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: {
        color: 'rgba(255, 255, 255, 0.6)',
        fontFamily: 'Arimo',
        fontWeight: 400,
        fontSize: pxToResponsive(12),
      },
      splitLine: {
        show: true,
        lineStyle: {
          color: '#fff',
          type: 'dashed', // y轴网格线设为虚线
          opacity: 0.2,
        },
      },
    },
    series: seriesData,
  })
}

onMounted(() => {
  initChart()
  window.addEventListener('resize', resizeChart)
})

const resizeChart = () => {
  setTimeout(() => {
    chartInstance?.resize()
  }, 300)
}

watch(
  () => [props.xAxiosOption.xAxiosData, props.series],
  () => {
    initChart()
  },
  { deep: true },
)

onBeforeUnmount(() => {
  window.removeEventListener('resize', resizeChart)
  chartInstance?.dispose()
})
</script>

<style scoped lang="scss">
.stacked-bar-chart {
  width: 100%;
  height: 100%;
}
</style>
