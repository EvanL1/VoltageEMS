<template>
  <div class="voltage-class operationLog">
    <!-- 表格区域 -->
    <div class="operationLog__table">
      <el-table :data="tableData" v-loading="loading" class="operationLog__table-content">
        <el-table-column prop="user" label="User" min-width="120" />
        <el-table-column prop="role" label="Role" min-width="100" />
        <el-table-column prop="action" label="Action" min-width="120" />
        <el-table-column prop="device" label="Device" min-width="120" />
        <el-table-column prop="result" label="Result" min-width="100" />
        <el-table-column prop="time" label="Time" min-width="160" />
        <el-table-column prop="ip" label="IP Address" min-width="140" />
      </el-table>

      <!-- 分页组件 -->
      <div class="operationLog__pagination">
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
</template>

<script setup lang="ts">
import { useTableData, type TableConfig } from '@/composables/useTableData'

// 操作日志数据类型
interface OperationLogRecord {
  id: string
  user: string
  role: string
  action: string
  device: string
  result: string
  time: string
  ip: string
}

// 表格配置
const tableConfig: TableConfig = {
  listUrl: '/api/operation-logs',
  defaultPageSize: 20,
}

// 使用 useTableData composable
const {
  loading,
  tableData,
  pagination: paginationData,
  handlePageChange,
} = useTableData<OperationLogRecord>(tableConfig)

// 本地分页状态
const pagination = reactive({
  page: 1,
  pageSize: 20,
  total: 0,
})

// 监听paginationData变化
watch(
  paginationData,
  (newPagination) => {
    pagination.page = newPagination.page
    pagination.pageSize = newPagination.pageSize
    pagination.total = newPagination.total
  },
  { immediate: true },
)

// 模拟数据
const mockData: OperationLogRecord[] = [
  {
    id: '1',
    user: 'Alice',
    role: 'Admin',
    action: 'Login',
    device: 'Web',
    result: 'Success',
    time: '2024-06-01 09:00:00',
    ip: '192.168.1.10',
  },
  {
    id: '2',
    user: 'Bob',
    role: 'Operator',
    action: 'Restart Device',
    device: 'Sensor-001',
    result: 'Success',
    time: '2024-06-01 09:05:00',
    ip: '192.168.1.11',
  },
  {
    id: '3',
    user: 'Charlie',
    role: 'Viewer',
    action: 'View Report',
    device: 'Web',
    result: 'Success',
    time: '2024-06-01 09:10:00',
    ip: '192.168.1.12',
  },
  {
    id: '4',
    user: 'Alice',
    role: 'Admin',
    action: 'Modify Settings',
    device: 'Server-005',
    result: 'Failed',
    time: '2024-06-01 09:15:00',
    ip: '192.168.1.10',
  },
  {
    id: '5',
    user: 'Bob',
    role: 'Operator',
    action: 'Logout',
    device: 'Web',
    result: 'Success',
    time: '2024-06-01 09:20:00',
    ip: '192.168.1.11',
  },
  {
    id: '6',
    user: 'Diana',
    role: 'Admin',
    action: 'Delete User',
    device: 'Web',
    result: 'Success',
    time: '2024-06-01 09:25:00',
    ip: '192.168.1.13',
  },
  {
    id: '7',
    user: 'Eve',
    role: 'Operator',
    action: 'Update Firmware',
    device: 'Device-002',
    result: 'Success',
    time: '2024-06-01 09:30:00',
    ip: '192.168.1.14',
  },
  {
    id: '8',
    user: 'Frank',
    role: 'Viewer',
    action: 'View Log',
    device: 'Web',
    result: 'Success',
    time: '2024-06-01 09:35:00',
    ip: '192.168.1.15',
  },
  {
    id: '9',
    user: 'Grace',
    role: 'Admin',
    action: 'Add Device',
    device: 'Router-001',
    result: 'Success',
    time: '2024-06-01 09:40:00',
    ip: '192.168.1.16',
  },
  {
    id: '10',
    user: 'Heidi',
    role: 'Operator',
    action: 'Acknowledge Alarm',
    device: 'Sensor-002',
    result: 'Success',
    time: '2024-06-01 09:45:00',
    ip: '192.168.1.17',
  },
]

// 加载模拟数据到tableData
const loadMockData = () => {
  // 模拟分页
  const start = (pagination.page - 1) * pagination.pageSize
  const end = start + pagination.pageSize
  tableData.value = mockData.slice(start, end)
  pagination.total = mockData.length
}

// 分页处理
const handlePageSizeChange = (newPageSize: number) => {
  handlePageChange(1, newPageSize)
}
const handleCurrentPageChange = (newPage: number) => {
  handlePageChange(newPage)
}

// 组件挂载时加载数据
onMounted(() => {
  loadMockData()
})

// 监听分页变化，重新加载数据
watch(
  () => [pagination.page, pagination.pageSize],
  () => {
    loadMockData()
  },
)
</script>

<style scoped lang="scss">
.voltage-class.operationLog {
  height: 100%;
  width: 100%;
  padding-top: 20px;
  display: flex;

  .operationLog__table {
    width: 100%;
    height: 100%;

    .operationLog__table-content {
      width: 100%;
      max-height: 810px;
      overflow-y: auto;
    }

    .operationLog__pagination {
      padding: 20px 0;
      display: flex;
      justify-content: flex-end;
    }
  }
}
</style>
