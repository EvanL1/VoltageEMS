<template>
  <div class="doughnut-chart" ref="chartRef"></div>
</template>

<script setup lang="ts">
import * as echarts from 'echarts/core'
import { PieChart } from 'echarts/charts'
import { TooltipComponent, LegendComponent, TitleComponent } from 'echarts/components'
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

echarts.use([PieChart, TooltipComponent, LegendComponent, TitleComponent, CanvasRenderer])

// ================== 示例数据（可用数据，供参考，实际使用时请传递props） ==================
// 下面是一些可用于该组件的示例数据，已注释，方便你参考和测试
/*
  const exampleSeries = [
    {
      name: '光伏发电',
      value: 45,
      color: '#4FADF7'
    },
    {
      name: '柴油发电',
      value: 30,
      color: '#F6C85F'
    },
    {
      name: '储能放电',
      value: 25,
      color: '#6DD400'
    }
  ]
  
  // 传递给组件的props示例
  const series = exampleSeries
  const title = '能源分布'
  const radius = ['40%', '70%'] // 内半径和外半径
  */

// 定义数据类型
interface SeriesData {
  name: string
  value: number
  color: string
}

const props = defineProps<{
  series: SeriesData[]
  title?: string
  radius?: [string, string] // 内半径和外半径，如 ['40%', '70%']
  center?: [string, string] // 圆心位置，如 ['50%', '50%']
}>()

const chartRef = ref<HTMLDivElement | null>(null)
let chartInstance: echarts.ECharts | null = null

// 初始化echarts
const initChart = () => {
  if (!chartRef.value) return
  if (chartInstance) {
    chartInstance.dispose()
  }
  chartInstance = echarts.init(chartRef.value)

  // 获取配置参数，使用默认值
  const title = props.title || ''
  const radius = props.radius || ['40%', '70%']
  const center = props.center || ['50%', '50%']

  chartInstance.setOption({
    title: {
      text: title,
      left: 'center',
      top: pxToResponsive(10),
      textStyle: {
        color: 'rgba(255, 255, 255, 0.8)',
        fontFamily: 'Arimo',
        fontWeight: 500,
        fontSize: pxToResponsive(14),
      },
    },
    grid: {
      left: 0,
      right: 0,
      top: pxToResponsive(45),
      bottom: pxToResponsive(15),
      containLabel: true,
    },
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
    tooltip: {
      trigger: 'item',
      confine: true,
      formatter: '{a} <br/>{b}: {c} ({d}%)',
      textStyle: {
        fontFamily: 'Arimo',
        fontWeight: 400,
        fontSize: pxToResponsive(12),
      },
    },
    series: [
      {
        name: title || '数据分布',
        type: 'pie',
        radius: radius,
        center: center,
        data: props.series.map((s) => ({
          name: s.name,
          value: s.value,
          itemStyle: {
            color: s.color,
          },
        })),
        label: {
          show: true,
          position: 'outside',
          formatter: '{b}\n{d}%',
          color: 'rgba(255, 255, 255, 0.8)',
          fontSize: pxToResponsive(12),
          fontFamily: 'Arimo',
          fontWeight: 400,
        },
        labelLine: {
          show: true,
          lineStyle: {
            color: 'rgba(255, 255, 255, 0.3)',
            width: 1,
          },
        },
        emphasis: {
          scale: true,
          scaleSize: pxToResponsive(5),
          label: {
            fontSize: pxToResponsive(14),
            fontWeight: 600,
          },
        },
        animationType: 'scale',
        animationEasing: 'elasticOut',
        animationDelay: (idx: number) => Math.random() * 200,
      },
    ],
  })
}

onMounted(() => {
  initChart()
  window.addEventListener('resize', resizeChart)
})

const resizeChart = () => {
  chartInstance?.resize()
}

watch(
  () => [props.series, props.title, props.radius, props.center],
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
.doughnut-chart {
  width: 100%;
  height: 100%;
}
</style>
