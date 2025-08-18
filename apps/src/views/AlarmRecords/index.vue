<template>
  <div class="voltage-class alarm-records">
    <!-- 页面头部 -->
    <div class="alarm-records__header">
      <div class="alarm-records__tabs">
        <el-button
          :type="activeTab === 'current' ? 'primary' : 'warning'"
          @click="activeTab = 'current'"
          class="alarm-records__tab-btn"
        >
          <img :src="alarmCurrentIcon" class="alarm-records__tab-icon" />
          Current
        </el-button>
        <el-button
          :type="activeTab === 'history' ? 'primary' : 'warning'"
          @click="activeTab = 'history'"
          class="alarm-records__tab-btn"
        >
          <img :src="alarmHistoryIcon" class="alarm-records__tab-icon" />
          History
        </el-button>
      </div>
    </div>
    <!-- 表格区域 -->
    <div class="alarm-records__content">
      <!-- 表格工具栏 -->
      <div class="alarm-records__toolbar">
        <div class="alarm-records__toolbar-left" ref="toolbarLeftRef">
          <!-- 选择框 -->
          <el-select
            v-model="selectedFilter"
            @change="handleFilterChange"
            :append-to="toolbarLeftRef"
            clearable
          >
            <el-option label="All" value="all" />
            <el-option label="High Priority" value="high" />
            <el-option label="Medium Priority" value="medium" />
            <el-option label="Low Priority" value="low" />
          </el-select>
        </div>

        <div class="alarm-records__toolbar-right">
          <!-- 导出按钮 - 只在history模式下显示 -->
          <el-button
            v-if="activeTab === 'history'"
            type="primary"
            @click="handleExport"
            class="alarm-records__export-btn"
          >
            <img :src="alarmExportIcon" class="alarm-records__export-icon" />
            Export
          </el-button>
        </div>
      </div>

      <!-- 表格 -->
      <div class="alarm-records__table">
        <el-table
          :data="tableData"
          v-loading="loading"
          element-loading-background="rgba(0, 0, 0, 0.1)"
          class="alarm-records__table-content"
        >
          <el-table-column prop="name" label="Name" min-width="1.2rem" />
          <el-table-column prop="device" label="Device" min-width="1.2rem" />
          <el-table-column prop="level" label="Level" min-width="1rem"> </el-table-column>
          <el-table-column prop="startTime" label="Start Time" min-width="1.6rem" />
          <!-- End Time列 - 只在history模式下显示 -->
          <el-table-column
            v-if="activeTab === 'history'"
            prop="endTime"
            label="End Time"
            min-width="1.6rem"
          />
        </el-table>

        <!-- 分页组件 -->
        <div class="alarm-records__pagination">
          <el-pagination
            v-model:current-page="pagination.page"
            v-model:page-size="pagination.pageSize"
            :page-sizes="[10, 20, 50, 100]"
            :total="pagination.total"
            layout="total, sizes, prev, pager, next"
            @size-change="handlePageSizeChange"
            @current-change="handleCurrentPageChange"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { AlarmRecord } from '@/types/alarm'
import { useTableData, type TableConfig } from '@/composables/useTableData'

// 正确引入SVG图标，避免部署后图片加载不出来
import alarmCurrentIcon from '@/assets/icons/alarm-current.svg'
import alarmHistoryIcon from '@/assets/icons/alarm-history.svg'
import alarmExportIcon from '@/assets/icons/alarm-export.svg'

// 响应式数据
const activeTab = ref<'current' | 'history'>('current')
const selectedFilter = ref('all')
const toolbarLeftRef = ref<HTMLElement | null>(null)
// 表格配置
const tableConfig: TableConfig = {
  listUrl: '/api/alarms',
  defaultPageSize: 20,
}

// 使用 useTableData composable
const { loading, tableData, pagination, handlePageChange } = useTableData<AlarmRecord>(tableConfig)

// 模拟数据
const mockCurrentData: AlarmRecord[] = [
  {
    id: '1',
    name: 'Temperature Alert',
    device: 'Sensor-001',
    level: 'High',
    startTime: '2024-01-15 10:30:00',
    status: 'active',
  },
  {
    id: '2',
    name: 'Voltage Warning',
    device: 'Device-002',
    level: 'Medium',
    startTime: '2024-01-15 09:15:00',
    status: 'active',
  },
  {
    id: '3',
    name: 'Connection Lost',
    device: 'Network-003',
    level: 'Low',
    startTime: '2024-01-15 08:45:00',
    status: 'acknowledged',
  },
  {
    id: '4',
    name: 'System Overload',
    device: 'Server-005',
    level: 'High',
    startTime: '2024-01-15 07:30:00',
    status: 'active',
  },
  {
    id: '5',
    name: 'Network Timeout',
    device: 'Router-001',
    level: 'Medium',
    startTime: '2024-01-15 06:45:00',
    status: 'acknowledged',
  },
]

const mockHistoryData: AlarmRecord[] = [
  {
    id: '6',
    name: 'Temperature Alert',
    device: 'Sensor-001',
    level: 'High',
    startTime: '2024-01-14 10:30:00',
    endTime: '2024-01-14 11:30:00',
    status: 'resolved',
  },
  {
    id: '7',
    name: 'Voltage Warning',
    device: 'Device-002',
    level: 'Medium',
    startTime: '2024-01-14 09:15:00',
    endTime: '2024-01-14 10:15:00',
    status: 'resolved',
  },
  {
    id: '8',
    name: 'Connection Lost',
    device: 'Network-003',
    level: 'Low',
    startTime: '2024-01-14 08:45:00',
    endTime: '2024-01-14 09:45:00',
    status: 'resolved',
  },
  {
    id: '9',
    name: 'System Error',
    device: 'Server-004',
    level: 'High',
    startTime: '2024-01-13 15:20:00',
    endTime: '2024-01-13 16:20:00',
    status: 'resolved',
  },
  {
    id: '10',
    name: 'Memory Overflow',
    device: 'Server-006',
    level: 'High',
    startTime: '2024-01-13 14:10:00',
    endTime: '2024-01-13 15:10:00',
    status: 'resolved',
  },
  {
    id: '11',
    name: 'Disk Space Low',
    device: 'Storage-001',
    level: 'Medium',
    startTime: '2024-01-13 13:30:00',
    endTime: '2024-01-13 14:30:00',
    status: 'resolved',
  },
  {
    id: '12',
    name: 'Service Unavailable',
    device: 'Service-001',
    level: 'Low',
    startTime: '2024-01-13 12:45:00',
    endTime: '2024-01-13 13:45:00',
    status: 'resolved',
  },
]
// 处理筛选条件变化
const handleFilterChange = () => {
  // 重置到第一页并重新加载数据
  handlePageChange(1)
}

// 模拟获取表格数据的方法
const loadMockData = async () => {
  try {
    // 模拟API调用延迟
    await new Promise((resolve) => setTimeout(resolve, 500))

    // 根据当前标签获取数据
    const baseData = activeTab.value === 'current' ? mockCurrentData : mockHistoryData

    // 根据筛选条件过滤数据
    let filteredData = baseData
    if (selectedFilter.value !== 'all') {
      filteredData = baseData.filter((item) => item.level.toLowerCase() === selectedFilter.value)
    }

    // 模拟分页
    const startIndex = (pagination.page - 1) * pagination.pageSize
    const endIndex = startIndex + pagination.pageSize
    const pageData = filteredData.slice(startIndex, endIndex)

    // 更新表格数据
    tableData.value = pageData
  } catch (error) {
    ElMessage.error('获取数据失败')
    console.error('获取数据失败:', error)
  }
}

// 处理分页变化
const handlePageSizeChange = (newPageSize: number) => {
  handlePageChange(1, newPageSize)
}

const handleCurrentPageChange = (newPage: number) => {
  handlePageChange(newPage)
}

// 处理导出
const handleExport = () => {
  ElMessage.success('导出功能已触发')
  // 这里可以添加实际的导出逻辑
  console.log('导出数据:', tableData.value)
}

// 监听标签切换
watch(activeTab, () => {
  // 切换标签时重置到第一页
  // handlePageChange(1)
  loadMockData()
})

// 组件挂载时获取数据
onMounted(async () => {
  await loadMockData()
})
</script>

<style scoped lang="scss">
.voltage-class.alarm-records {
  height: 100%;
  display: flex;
  flex-direction: column;
  .alarm-records__header {
    padding-bottom: 20px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    .alarm-records__tabs {
      display: flex;
      align-items: center;
      gap: 16px;
      .alarm-records__tab-btn {
        display: flex;
        align-items: center;
        gap: 10px;
        .alarm-records__tab-icon {
          width: 16px;
          height: 16px;
          margin-right: 8px;
        }
      }
    }
  }
  .alarm-records__content {
    flex: 1;
    display: flex;
    flex-direction: column;

    .alarm-records__toolbar {
      padding: 20px 0;
      display: flex;
      align-items: center;
      justify-content: space-between;

      .alarm-records__toolbar-left {
        display: flex;
        align-items: center;
        gap: 16px;
      }

      .alarm-records__toolbar-right {
        display: flex;
        align-items: center;
        gap: 16px;

        .alarm-records__export-btn {
          display: flex;
          align-items: center;
          gap: 10px;
          .alarm-records__export-icon {
            width: 16px;
            height: 16px;
          }
        }
      }
    }

    .alarm-records__table {
      flex: 1;
      width: 100%;
      display: flex;
      flex-direction: column;

      .alarm-records__table-content {
        width: 100%;
        flex: 1;
        overflow-y: auto;
      }

      .alarm-records__pagination {
        padding-top: 20px;
        display: flex;
        justify-content: flex-end;
      }
    }
  }
}
:deep(.el-switch) {
  height: 20px !important;
}
:deep(.el-select__popper.el-popper) {
  top: 222px !important;
}
</style>
