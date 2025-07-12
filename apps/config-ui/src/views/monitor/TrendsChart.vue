<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { ElMessage } from 'element-plus';
import * as echarts from 'echarts';
import { 
  TrendCharts,
  Calendar,
  Download,
  FullScreen,
  Refresh
} from '@element-plus/icons-vue';

// 数据点列表
const dataPoints = ref([
  { id: 1, name: '主变压器A相电压', unit: 'V', selected: true, color: '#409EFF' },
  { id: 2, name: '主变压器B相电压', unit: 'V', selected: true, color: '#67C23A' },
  { id: 3, name: '主变压器C相电压', unit: 'V', selected: true, color: '#E6A23C' },
  { id: 4, name: '总有功功率', unit: 'kW', selected: false, color: '#F56C6C' },
  { id: 5, name: '总无功功率', unit: 'kVar', selected: false, color: '#909399' },
  { id: 6, name: '环境温度', unit: '°C', selected: false, color: '#B1B3B8' },
]);

// 时间范围
const timeRange = ref('1h');
const customTimeRange = ref([]);
const timeRangeOptions = [
  { value: '15m', label: '15分钟' },
  { value: '30m', label: '30分钟' },
  { value: '1h', label: '1小时' },
  { value: '3h', label: '3小时' },
  { value: '6h', label: '6小时' },
  { value: '12h', label: '12小时' },
  { value: '24h', label: '24小时' },
  { value: '7d', label: '7天' },
  { value: 'custom', label: '自定义' },
];

// 图表实例
let chartInstance: any = null;

// 选中的数据点
const selectedPoints = computed(() => {
  return dataPoints.value.filter(p => p.selected);
});

// 生成模拟数据
function generateMockData(pointName: string, hours: number) {
  const data = [];
  const now = Date.now();
  const interval = hours > 24 ? 3600000 : 60000; // 超过24小时用小时间隔，否则用分钟
  const points = Math.floor(hours * 3600000 / interval);
  
  let baseValue = 380;
  if (pointName.includes('功率')) baseValue = 1000;
  if (pointName.includes('温度')) baseValue = 25;
  
  for (let i = 0; i < points; i++) {
    const time = now - (points - i) * interval;
    const value = baseValue + (Math.random() - 0.5) * baseValue * 0.1;
    data.push([time, value.toFixed(2)]);
  }
  
  return data;
}

// 获取时间范围的小时数
function getHours(range: string) {
  switch (range) {
    case '15m': return 0.25;
    case '30m': return 0.5;
    case '1h': return 1;
    case '3h': return 3;
    case '6h': return 6;
    case '12h': return 12;
    case '24h': return 24;
    case '7d': return 168;
    default: return 1;
  }
}

// 更新图表
function updateChart() {
  if (!chartInstance) return;
  
  const hours = getHours(timeRange.value);
  const series = selectedPoints.value.map(point => ({
    name: point.name,
    type: 'line',
    smooth: true,
    symbol: 'none',
    lineStyle: {
      width: 2,
      color: point.color
    },
    data: generateMockData(point.name, hours),
    yAxisIndex: point.unit === '°C' ? 1 : 0 // 温度使用第二个Y轴
  }));
  
  const option = {
    title: {
      text: '历史趋势图',
      left: 'center'
    },
    tooltip: {
      trigger: 'axis',
      axisPointer: {
        type: 'cross',
        animation: false,
        label: {
          backgroundColor: '#505765'
        }
      }
    },
    legend: {
      data: selectedPoints.value.map(p => p.name),
      top: 40
    },
    toolbox: {
      feature: {
        dataZoom: {
          yAxisIndex: false
        },
        saveAsImage: {
          name: '历史趋势'
        }
      }
    },
    grid: {
      left: '3%',
      right: '3%',
      bottom: '15%',
      containLabel: true
    },
    xAxis: {
      type: 'time',
      boundaryGap: false,
      axisLine: { onZero: false }
    },
    yAxis: [
      {
        name: '电压(V) / 功率(kW/kVar)',
        type: 'value'
      },
      {
        name: '温度(°C)',
        type: 'value',
        position: 'right'
      }
    ],
    dataZoom: [
      {
        type: 'inside',
        start: 0,
        end: 100
      },
      {
        type: 'slider',
        start: 0,
        end: 100
      }
    ],
    series: series
  };
  
  chartInstance.setOption(option);
}

// 切换数据点选择
function togglePoint(point: any) {
  point.selected = !point.selected;
  updateChart();
}

// 刷新数据
function refreshData() {
  updateChart();
  ElMessage.success('数据已刷新');
}

// 导出数据
function exportData() {
  ElMessage.info('导出功能开发中...');
}

// 全屏显示
function toggleFullscreen() {
  ElMessage.info('全屏功能开发中...');
}

// 时间范围变化
function onTimeRangeChange() {
  if (timeRange.value !== 'custom') {
    updateChart();
  }
}

// 自定义时间范围变化
function onCustomTimeRangeChange() {
  if (customTimeRange.value && customTimeRange.value.length === 2) {
    // 处理自定义时间范围
    updateChart();
  }
}

onMounted(() => {
  // 初始化图表
  const chartDom = document.getElementById('trend-chart');
  if (chartDom) {
    chartInstance = echarts.init(chartDom);
    updateChart();
    
    // 响应窗口大小变化
    window.addEventListener('resize', () => {
      chartInstance?.resize();
    });
  }
});
</script>

<template>
  <div class="trends-chart">
    <!-- 工具栏 -->
    <el-card class="toolbar-card">
      <div class="toolbar">
        <div class="toolbar-left">
          <span class="label">时间范围：</span>
          <el-radio-group v-model="timeRange" @change="onTimeRangeChange">
            <el-radio-button 
              v-for="option in timeRangeOptions" 
              :key="option.value" 
              :value="option.value"
            >
              {{ option.label }}
            </el-radio-button>
          </el-radio-group>
          
          <el-date-picker
            v-if="timeRange === 'custom'"
            v-model="customTimeRange"
            type="datetimerange"
            range-separator="至"
            start-placeholder="开始时间"
            end-placeholder="结束时间"
            @change="onCustomTimeRangeChange"
            style="margin-left: 16px"
          />
        </div>
        
        <div class="toolbar-right">
          <el-button :icon="Refresh" @click="refreshData">刷新</el-button>
          <el-button :icon="Download" @click="exportData">导出</el-button>
          <el-button :icon="FullScreen" @click="toggleFullscreen">全屏</el-button>
        </div>
      </div>
    </el-card>
    
    <el-row :gutter="20">
      <!-- 数据点选择 -->
      <el-col :span="6">
        <el-card class="points-card">
          <template #header>
            <h3>数据点选择</h3>
          </template>
          
          <div class="points-list">
            <div 
              v-for="point in dataPoints" 
              :key="point.id"
              class="point-item"
              :class="{ selected: point.selected }"
              @click="togglePoint(point)"
            >
              <el-checkbox v-model="point.selected" />
              <div class="point-info">
                <span class="point-name">{{ point.name }}</span>
                <span class="point-unit">({{ point.unit }})</span>
              </div>
              <div 
                class="color-indicator" 
                :style="{ backgroundColor: point.color }"
              />
            </div>
          </div>
          
          <div class="points-stats">
            <el-tag type="info">
              已选择 {{ selectedPoints.length }} / {{ dataPoints.length }} 个数据点
            </el-tag>
          </div>
        </el-card>
      </el-col>
      
      <!-- 趋势图表 -->
      <el-col :span="18">
        <el-card class="chart-card">
          <div id="trend-chart" style="width: 100%; height: 600px;"></div>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<style lang="scss" scoped>
.trends-chart {
  .toolbar-card {
    margin-bottom: 20px;
    
    .toolbar {
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: 16px;
      
      .toolbar-left {
        display: flex;
        align-items: center;
        gap: 16px;
        
        .label {
          font-weight: 500;
          color: #606266;
        }
      }
      
      .toolbar-right {
        display: flex;
        gap: 12px;
      }
    }
  }
  
  .points-card {
    height: 660px;
    
    h3 {
      margin: 0;
      font-size: 16px;
      font-weight: 600;
    }
    
    .points-list {
      max-height: 500px;
      overflow-y: auto;
      
      .point-item {
        display: flex;
        align-items: center;
        gap: 12px;
        padding: 12px;
        margin-bottom: 8px;
        border-radius: 4px;
        cursor: pointer;
        transition: all 0.3s;
        
        &:hover {
          background-color: #f5f7fa;
        }
        
        &.selected {
          background-color: #ecf5ff;
          border: 1px solid #b3d8ff;
        }
        
        .point-info {
          flex: 1;
          
          .point-name {
            font-weight: 500;
            color: #303133;
          }
          
          .point-unit {
            color: #909399;
            font-size: 13px;
            margin-left: 4px;
          }
        }
        
        .color-indicator {
          width: 20px;
          height: 4px;
          border-radius: 2px;
        }
      }
    }
    
    .points-stats {
      margin-top: 16px;
      text-align: center;
    }
  }
  
  .chart-card {
    height: 660px;
  }
}
</style>