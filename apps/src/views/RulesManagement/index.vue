<template>
  <div class="voltage-class rule-management">
    <div class="rule-management__header">
      <div class="rule-management__search-form" ref="levelSelectRef">
        <el-form :model="formData" :inline="true">
          <el-form-item label="Rule Name">
            <el-input v-model="formData.ruleName" placeholder="Please enter rule name" />
          </el-form-item>
          <el-form-item label="Level">
            <el-select
              v-model="formData.alarmLevel"
              placeholder="Please select level"
              clearable
              :append-to="levelSelectRef"
            >
              <el-option label="L1" :value="1" />
              <el-option label="L2" :value="2" />
              <el-option label="L3" :value="3" />
            </el-select>
          </el-form-item>
          <el-form-item label="Switch">
            <el-select
              v-model="formData.enabled"
              placeholder="Please select switch"
              clearable
              :append-to="levelSelectRef"
            >
              <el-option label="On" :value="true" />
              <el-option label="Off" :value="false" />
            </el-select>
          </el-form-item>
        </el-form>
        <div class="form-oprations">
          <el-button type="warning" class="rule-management__btn">
            <img :src="tableRefreshIcon" class="rule-management__btn-icon" />
            reload</el-button
          >
          <el-button type="primary" class="rule-management__btn">
            <img :src="tableSearchIcon" class="rule-management__btn-icon" />
            search</el-button
          >
        </div>
      </div>
      <div class="rule-management__table-operations">
        <el-button type="primary" @click="handleAddUser" class="rule-management__btn">
          <img :src="userAddIcon" class="rule-management__btn-icon" />
          New a Rule
        </el-button>
      </div>
    </div>
    <div class="rule-management__table">
      <el-table
        :data="tableData"
        v-loading="loading"
        class="rule-management__table-content"
        align="left"
        table-layout="fixed"
      >
        <el-table-column prop="id" label="Rule ID" show-overflow-tooltip />
        <el-table-column prop="ruleName" label="Rule Name" show-overflow-tooltip />
        <el-table-column prop="alarmLevel" label="Warning Level" show-overflow-tooltip>
          <template #default="{ row }"> L{{ row.alarmLevel }} </template>
        </el-table-column>
        <el-table-column prop="monitorData" label="Monitor Data" show-overflow-tooltip>
          <template #default="{ row }">
            {{ formatMonitorData(row.monitorData) }}
          </template>
        </el-table-column>
        <el-table-column prop="condition" label="Condition" show-overflow-tooltip>
          <template #default="{ row }">
            {{ formatCondition(row.condition) }}
          </template>
        </el-table-column>
        <el-table-column prop="notification" label="Notification" show-overflow-tooltip>
          <template #default="{ row }">
            {{ Array.isArray(row.notification) ? row.notification.join(', ') : row.notification }}
          </template>
        </el-table-column>
        <el-table-column prop="time" label="Time" show-overflow-tooltip>
          <template #default="{ row }">
            {{ formatDateTime(row.time) }}
          </template>
        </el-table-column>
        <el-table-column prop="enabled" label="Switch" show-overflow-tooltip>
          <template #default="{ row }">
            <el-switch :model-value="row.enabled" disabled />
          </template>
        </el-table-column>
        <el-table-column label="Operation" fixed="right" show-overflow-tooltip>
          <template #default="{ row }">
            <div class="rule-management__operation">
              <div class="rule-management__operation-item" @click="handleEdit(row)">
                <img :src="tableEditIcon" />
                <span class="rule-management__operation-text">Edit</span>
              </div>
              <div class="rule-management__operation-item" @click="handleDelete(row)">
                <img :src="tableDeleteIcon" />
                <span class="rule-management__operation-text">Delete</span>
              </div>
            </div>
          </template>
        </el-table-column>
      </el-table>

      <div class="rule-management__pagination">
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
    <RulesOperationForm
      ref="rulesOperationFormRef"
      @submit="handleRuleSubmit"
      @cancel="handleRuleCancel"
    />
  </div>
</template>

<script setup lang="ts">
// 正确引入SVG图标，避免部署后图片加载不出来
import tableRefreshIcon from '@/assets/icons/table-refresh.svg'
import tableSearchIcon from '@/assets/icons/table-search.svg'
import userAddIcon from '@/assets/icons/user-add.svg'
import tableEditIcon from '@/assets/icons/table-edit.svg'
import tableDeleteIcon from '@/assets/icons/table-delect.svg'

import RulesOperationForm from './RulesOperationForm.vue'
// RuleInfo 结构参考 RuleFormModel
type Operator = 'gt' | 'gte' | 'lt' | 'lte' | 'eq'

type RuleInfo = {
  id: number
  ruleName: string
  monitorData: {
    part1: string | number | null
    part2: string | number | null
    part3: string | number | null
  }
  alarmLevel: string | number | null
  condition: {
    operator: Operator | null
    threshold: number | null
  }
  notification: string[]
  enabled: boolean
  time: string
}

import { useTableData, type TableConfig } from '@/composables/useTableData'

interface RuleFormModel {
  ruleName: string
  monitorData: {
    part1: string | number | null
    part2: string | number | null
    part3: string | number | null
  }
  alarmLevel: string | number | null
  condition: {
    operator: Operator | null
    threshold: number | null
  }
  notification: string[]
  enabled: boolean
}

const tableConfig: TableConfig = {
  listUrl: '/api/rules',
  deleteUrl: '/api/rules/{id}',
  batchDeleteUrl: '/api/rules/batch-delete',
  enableDelete: true,
  enableBatchDelete: true,
  defaultPageSize: 20,
  deleteConfirmMessage: 'Are you sure you want to delete this rule?',
  batchDeleteConfirmMessage: 'Are you sure you want to delete selected rules?',
}

const {
  loading,
  tableData,
  pagination: paginationData,
  handlePageChange,
} = useTableData<RuleInfo>(tableConfig)

const formData = reactive({
  ruleName: '',
  alarmLevel: 1,
  enabled: true,
})
const pagination = reactive({
  page: 1,
  pageSize: 20,
  total: 0,
})

const levelSelectRef = ref<HTMLElement | null>(null)
const switchSelectRef = ref<HTMLElement | null>(null)
watch(
  paginationData,
  (newPagination) => {
    pagination.page = newPagination.page
    pagination.pageSize = newPagination.pageSize
    pagination.total = newPagination.total
  },
  { immediate: true },
)

// 模拟数据 - 结构参考 RuleFormModel
const mockData: RuleInfo[] = [
  {
    id: 1,
    ruleName: 'CPU Usage High',
    monitorData: { part1: 'CPU', part2: 'Usage', part3: null },
    alarmLevel: 1,
    condition: { operator: 'gt', threshold: 90 },
    notification: ['email', 'sms'],
    enabled: true,
    time: '2024-01-15T10:30:00Z',
  },
  {
    id: 2,
    ruleName: 'Memory Usage Warning',
    monitorData: { part1: 'Memory', part2: 'Usage', part3: null },
    alarmLevel: 2,
    condition: { operator: 'gt', threshold: 80 },
    notification: ['email'],
    enabled: true,
    time: '2024-01-14T16:45:00Z',
  },
  {
    id: 3,
    ruleName: 'Disk Space Low',
    monitorData: { part1: 'Disk', part2: 'Space', part3: 'C:' },
    alarmLevel: 3,
    condition: { operator: 'lt', threshold: 10 },
    notification: ['sms'],
    enabled: false,
    time: '2024-01-10T14:20:00Z',
  },
  {
    id: 4,
    ruleName: 'Network Latency',
    monitorData: { part1: 'Network', part2: 'Latency', part3: null },
    alarmLevel: 2,
    condition: { operator: 'gt', threshold: 200 },
    notification: [],
    enabled: true,
    time: '2024-01-05T12:10:00Z',
  },
]
const rulesOperationFormRef = ref()

onMounted(async () => {
  await loadMockData()
})

const loadMockData = async () => {
  await new Promise((resolve) => setTimeout(resolve, 500))
  tableData.value = mockData
  pagination.total = mockData.length
}

const handlePageSizeChange = (newPageSize: number) => {
  handlePageChange(1, newPageSize)
}

const handleCurrentPageChange = (newPage: number) => {
  handlePageChange(newPage)
}

// 格式化 MonitorData
const formatMonitorData = (monitorData: RuleInfo['monitorData']) => {
  if (!monitorData) return '-'
  return [monitorData.part1, monitorData.part2, monitorData.part3]
    .filter((v) => v !== null && v !== undefined && v !== '')
    .join(' / ')
}

// 格式化 Condition
const operatorMap: Record<Operator, string> = {
  gt: '>',
  gte: '≥',
  lt: '<',
  lte: '≤',
  eq: '=',
}
const formatCondition = (condition: RuleInfo['condition']) => {
  if (
    !condition ||
    !condition.operator ||
    condition.threshold === null ||
    condition.threshold === undefined
  )
    return '-'
  return `${operatorMap[condition.operator]} ${condition.threshold}`
}

// 格式化日期时间，输出格式如 2025/07/08 12:40:32
const formatDateTime = (dateTime: string) => {
  if (!dateTime) return '-'
  const date = new Date(dateTime)
  const year = date.getFullYear()
  const pad = (n: number) => (n < 10 ? '0' + n : n)
  const month = pad(date.getMonth() + 1)
  const day = pad(date.getDate())
  const hour = pad(date.getHours())
  const minute = pad(date.getMinutes())
  const second = pad(date.getSeconds())
  return `${year}/${month}/${day} ${hour}:${minute}:${second}`
}

// 添加规则
const handleAddUser = () => {
  rulesOperationFormRef.value?.open(undefined, 'create')
}

// 编辑规则
const handleEdit = (row: RuleInfo) => {
  rulesOperationFormRef.value?.open(
    {
      ruleName: row.ruleName,
      monitorData: row.monitorData,
      alarmLevel: row.alarmLevel,
      condition: row.condition,
      notification: row.notification,
      enabled: row.enabled,
    },
    'edit',
  )
}

// 删除规则
const handleDelete = async (row: RuleInfo) => {
  try {
    await ElMessageBox.confirm(
      `Are you sure you want to delete rule "${row.ruleName}"?`,
      'Delete Rule',
      {
        confirmButtonText: 'Delete',
        cancelButtonText: 'Cancel',
        type: 'warning',
      },
    )

    const index = tableData.value.findIndex((item: RuleInfo) => item.id === row.id)
    if (index > -1) {
      tableData.value.splice(index, 1)
      ElMessage.success('Rule deleted successfully')
    }
  } catch (error) {
    if (error !== 'cancel') {
      ElMessage.info('Delete cancelled')
    }
  }
}

// 处理规则表单提交
const handleRuleSubmit = (formData: RuleFormModel) => {
  console.log('Rule form submitted:', formData)
  // 这里可以调用API保存数据
  ElMessage.success('Rule saved successfully')
}

// 处理规则表单取消
const handleRuleCancel = () => {
  console.log('Rule form cancelled')
}
</script>

<style scoped lang="scss">
.voltage-class .rule-management {
  position: relative; // 为对话框提供定位上下文
  height: 100%;
  width: 100%;
  display: flex;
  flex-direction: column;
  .rule-management__header {
    margin-bottom: 20px;
    .rule-management__search-form {
      display: flex;
      align-items: center;
      justify-content: space-between;
      .form-oprations {
        height: 52px;
        display: flex;
        align-items: flex-start;
        gap: 10px;
      }
    }
    .rule-management__table-operations {
      width: 100%;
      padding-top: 20px;
      border-top: 1px solid rgba(255, 255, 255, 0.1);
    }
    .rule-management__btn {
      display: flex;
      align-items: center;
      gap: 8px;
      .rule-management__btn-icon {
        width: 14px;
        height: 14px;
        margin-right: 8px;
      }
    }
  }

  .rule-management__table {
    flex: 1;
    // max-width: 1660px;
    display: flex;
    flex-direction: column;
    .rule-management__table-content {
      flex: 1;
      overflow-y: auto;
      .rule-management__operation {
        display: flex;
        align-items: center;
        gap: 20px;
        .rule-management__operation-item {
          cursor: pointer;
          display: flex;
          align-items: center;
          img {
            width: 14px;
            height: 14px;
            margin-right: 4px;
            object-fit: contain;
          }
        }
      }
    }

    .rule-management__pagination {
      display: flex;
      justify-content: flex-end;
      margin-top: 20px;
    }
  }
  :deep(.el-switch) {
    height: 22px;
  }
  :deep(.el-select__popper.el-popper) {
    top: 44px !important;
  }
}
</style>
