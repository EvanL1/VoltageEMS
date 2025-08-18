/**
 * API使用示例
 * 演示如何在Vue组件中使用widgets相关的API接口
 */

import WidgetAPI, { type Widget, type WidgetListParams } from './widgets'

/**
 * 使用示例类
 * 包含各种API调用的示例代码
 */
export class ApiUsageExample {
  /**
   * 获取Widget列表的示例
   * 在Vue组件的setup函数中使用
   */
  static async getWidgetListExample() {
    try {
      // 设置查询参数
      const params: WidgetListParams = {
        page: 1,
        pageSize: 10,
        type: 'chart',
        keyword: '销售',
        isActive: true,
        sortBy: 'createTime',
        sortOrder: 'desc',
      }

      // 调用API获取Widget列表
      const response = await WidgetAPI.getWidgetList(params)

      console.log('Widget列表获取成功:', response.data)
      return response.data
    } catch (error) {
      console.error('获取Widget列表失败:', error)
      throw error
    }
  }

  /**
   * 创建Widget的示例
   */
  static async createWidgetExample() {
    try {
      // 准备创建数据
      const widgetData = {
        name: 'my-sales-chart',
        type: 'chart',
        title: '销售数据图表',
        description: '显示最近30天的销售趋势',
        config: {
          chartType: 'line',
          dataSource: '/api/sales/trend',
          refreshInterval: 300000, // 5分钟刷新一次
          colors: ['#409EFF', '#67C23A', '#E6A23C'],
        },
        position: {
          x: 0,
          y: 0,
          width: 6,
          height: 4,
        },
        isActive: true,
      }

      // 调用API创建Widget
      const response = await WidgetAPI.createWidget(widgetData)

      console.log('Widget创建成功:', response.data)
      return response.data
    } catch (error) {
      console.error('创建Widget失败:', error)
      throw error
    }
  }

  /**
   * 更新Widget的示例
   */
  static async updateWidgetExample(widgetId: number) {
    try {
      // 准备更新数据
      const updateData = {
        title: '更新后的销售图表',
        description: '显示最近60天的销售趋势',
        config: {
          chartType: 'bar',
          dataSource: '/api/sales/trend-60d',
          refreshInterval: 600000, // 10分钟刷新一次
        },
      }

      // 调用API更新Widget
      const response = await WidgetAPI.updateWidget(widgetId, updateData)

      console.log('Widget更新成功:', response.data)
      return response.data
    } catch (error) {
      console.error('更新Widget失败:', error)
      throw error
    }
  }

  /**
   * 获取图表数据的示例
   */
  static async getChartDataExample(widgetId: number) {
    try {
      // 设置查询参数
      const params = {
        dateRange: ['2024-01-01', '2024-01-31'] as [string, string],
        granularity: 'day' as const,
      }

      // 调用API获取图表数据
      const response = await WidgetAPI.getChartData(widgetId, params)

      console.log('图表数据获取成功:', response.data)
      return response.data
    } catch (error) {
      console.error('获取图表数据失败:', error)
      throw error
    }
  }
}

/**
 * Vue组件中的使用示例
 * 可以在Vue组件的setup函数中这样使用
 */
export const vueComponentUsageExample = `
<template>
  <div class="v-class widget-management">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>Widget管理</span>
          <el-button @click="createWidget">创建Widget</el-button>
        </div>
      </template>
      
      <el-table :data="widgetList" style="width: 100%">
        <el-table-column prop="name" label="名称" />
        <el-table-column prop="title" label="标题" />
        <el-table-column prop="type" label="类型" />
        <el-table-column prop="isActive" label="状态">
          <template #default="{ row }">
            <el-tag :type="row.isActive ? 'success' : 'danger'">
              {{ row.isActive ? '激活' : '未激活' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="操作">
          <template #default="{ row }">
            <el-button size="small" @click="editWidget(row)">编辑</el-button>
            <el-button size="small" type="danger" @click="deleteWidget(row.id)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import WidgetAPI, { type Widget } from '@/api/widgets'

// 响应式数据
const widgetList = ref<Widget[]>([])
const loading = ref(false)

// 获取Widget列表
const fetchWidgetList = async () => {
  try {
    loading.value = true
    const response = await WidgetAPI.getWidgetList({
      page: 1,
      pageSize: 20
    })
    widgetList.value = response.data.list
  } catch (error) {
    console.error('获取Widget列表失败:', error)
  } finally {
    loading.value = false
  }
}

// 创建Widget
const createWidget = async () => {
  try {
    const widgetData = {
      name: 'new-widget',
      type: 'chart',
      title: '新建图表',
      description: '这是一个新建的图表组件',
      config: {},
      position: { x: 0, y: 0, width: 4, height: 3 }
    }
    
    await WidgetAPI.createWidget(widgetData)
    await fetchWidgetList() // 刷新列表
  } catch (error) {
    console.error('创建Widget失败:', error)
  }
}

// 编辑Widget
const editWidget = async (widget: Widget) => {
  try {
    const updateData = {
      title: widget.title + ' (已更新)'
    }
    
    await WidgetAPI.updateWidget(widget.id, updateData)
    await fetchWidgetList() // 刷新列表
  } catch (error) {
    console.error('更新Widget失败:', error)
  }
}

// 删除Widget
const deleteWidget = async (widgetId: number) => {
  try {
    await WidgetAPI.deleteWidget(widgetId)
    await fetchWidgetList() // 刷新列表
  } catch (error) {
    console.error('删除Widget失败:', error)
  }
}

// 组件挂载时获取数据
onMounted(() => {
  fetchWidgetList()
})
</script>

<style lang="scss" scoped>
.widget-management {
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
}
</style>
`
