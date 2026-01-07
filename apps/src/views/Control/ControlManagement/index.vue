<template>
  <div class="voltage-class rule-management">
    <LoadingBg :loading="loading">
      <div class="rule-management__header">
        <div class="rule-management__search-form" ref="levelSelectRef">
          <el-form :model="filters" :inline="true" class="test-form">
            <el-form-item label="Keyword:">
              <el-input v-model="filters.keyword" placeholder="Please enter keyword" />
            </el-form-item>
            <el-form-item label="Alarm Level:">
              <el-select
                v-model="filters.warning_level"
                placeholder="Please select level"
                clearable
                :append-to="levelSelectRef"
              >
                <el-option label="L1" :value="1" />
                <el-option label="L2" :value="2" />
                <el-option label="L3" :value="3" />
              </el-select>
            </el-form-item>
            <el-form-item label="Switch:">
              <el-select
                v-model="filters.enabled"
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
            <IconButton
              type="warning"
              :icon="tableRefreshIcon"
              text="Reload"
              custom-class="rule-management__btn"
              @click="reloadFilters"
            />
            <IconButton
              type="primary"
              :icon="tableSearchIcon"
              text="Search"
              custom-class="rule-management__btn"
              @click="fetchTableData(true)"
            />
            <IconButton
              v-permission="['Admin']"
              type="primary"
              :icon="userAddIcon"
              text="New rule"
              custom-class="rule-management__btn"
              @click="handleAddUser"
            />
          </div>
        </div>
      </div>
      <div class="rule-management__table">
        <el-table
          :data="tableData"
          class="rule-management__table-content"
          align="left"
          table-layout="fixed"
        >
          <el-table-column prop="id" label="Rule ID" show-overflow-tooltip />
          <el-table-column prop="rule_name" label="Rule Name" show-overflow-tooltip />
          <el-table-column prop="warning_level" label="Alarm Level">
            <template #default="{ row }">
              <img
                :src="warningLevelList[row.warning_level as 1 | 2 | 3]"
                class="rule-management__table-icon"
                alt="level icon"
              />
            </template>
          </el-table-column>
          <!-- <el-table-column prop="monitor_data" label="Monitor Data" show-overflow-tooltip>
            <template #default="{ row }">
              {{ formatMonitorData(row) }}
            </template>
          </el-table-column> -->
          <!-- <el-table-column prop="condition" label="Condition" show-overflow-tooltip>
            <template #default="{ row }">
              {{ formatCondition(row) }}
            </template>
          </el-table-column> -->
          <!-- <el-table-column prop="notification" label="Notification" show-overflow-tooltip>
            <template #default="{ row }">
              {{ Array.isArray(row.notification) ? row.notification.join(', ') : row.notification }}
            </template>
          </el-table-column> -->
          <el-table-column prop="created_at" label="Created At" show-overflow-tooltip>
          </el-table-column>
          <el-table-column prop="enabled" label="Switch" show-overflow-tooltip>
            <template #default="{ row }">
              <el-switch :model-value="row.enabled" @change="handleSwitchChange(row)" />
            </template>
          </el-table-column>
          <el-table-column label="Operation" fixed="right" v-permission="['Admin']">
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
            @current-change="handlePageChange"
          />
        </div>
      </div>
    </LoadingBg>
    <RulesOperationForm
      ref="rulesOperationFormRef"
      @submit="fetchTableData(true)"
      @cancel="handleRuleCancel"
    />
  </div>
</template>

<script setup lang="ts">
// 正确引入SVG图标，避免部署后图片加载不出
import tableRefreshIcon from '@/assets/icons/table-refresh.svg'
import tableSearchIcon from '@/assets/icons/table-search.svg'
import userAddIcon from '@/assets/icons/user-add.svg'
import tableEditIcon from '@/assets/icons/table-edit.svg'
import tableDeleteIcon from '@/assets/icons/table-delect.svg'
import level1Icon from '@/assets/icons/home-alter-L1.svg'
import level2Icon from '@/assets/icons/home-alter-L2.svg'
import level3Icon from '@/assets/icons/home-alter-L3.svg'
import RulesOperationForm from './RulesOperationForm.vue'
import type { Operator, RuleInfo } from '@/types/controlManagement'

import { useTableData, type TableConfig } from '@/composables/useTableData'
import { enableRule, disableRule } from '@/api/alarm'

const tableConfig: TableConfig = {
  listUrl: '/alarmApi/rules',
  deleteUrl: '/alarmApi/rules/{id}',
  defaultPageSize: 20,
}
const warningLevelList = {
  1: level1Icon,
  2: level2Icon,
  3: level3Icon,
}
const {
  loading,
  tableData,
  pagination,
  handlePageSizeChange,
  fetchTableData,
  filters,
  handlePageChange,
  reloadFilters,
} = useTableData<RuleInfo>(tableConfig)

filters.keyword = ''
filters.warning_level = null
filters.enabled = null

const levelSelectRef = ref<HTMLElement | null>(null)

const rulesOperationFormRef = ref()

// 格式�?MonitorData
const formatMonitorData = (row: RuleInfo) => {
  if (!row) return '-'
  return [row.service_type, row.point_id, row.data_type]
    .filter((v) => v !== null && v !== undefined && v !== '')
    .join(' / ')
}

const formatCondition = (row: RuleInfo) => {
  if (!row || !row.operator || row.value === null || row.value === undefined) return '-'
  return `${row.operator} ${row.value}`
}

// 添加规则
const handleAddUser = () => {
  rulesOperationFormRef.value?.open(undefined, 'create')
}

// 编辑规则
const handleEdit = (row: RuleInfo) => {
  rulesOperationFormRef.value?.open(row.id, 'edit')
}

// 删除规则
const handleDelete = async (row: RuleInfo) => {
  await ElMessageBox.confirm(
    `Are you sure you want to delete rule "${row.rule_name}"?`,
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
  }
}
const handleSwitchChange = async (row: RuleInfo) => {
  if (!row.enabled) {
    const res = await enableRule(row.id)
    if (res.message) {
      fetchTableData()
    }
  } else {
    const res = await disableRule(row.id)
    if (res.message) {
      fetchTableData()
    }
  }
}

// 处理规则表单取消
const handleRuleCancel = () => {
  console.log('Rule form cancelled')
}
</script>

<style scoped lang="scss">
.voltage-class .rule-management {
  position: relative; // 为对话框提供定位上下�?
  height: 100%;
  width: 100%;
  display: flex;
  flex-direction: column;

  .rule-management__header {
    // margin-bottom: 0.2rem;

    .rule-management__search-form {
      position: relative;
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding-bottom: 0.2rem;

      .form-oprations {
        display: flex;
        align-items: flex-start;
        gap: 0.1rem;
      }
    }

    .rule-management__btn {
      display: flex;
      align-items: center;
      gap: 0.08rem;

      .rule-management__btn-icon {
        width: 0.14rem;
        height: 0.14rem;
        margin-right: 0.08rem;
      }
    }
  }

  .rule-management__table {
    height: calc(100% - 0.52rem);
    // max-width: 16.6rem;
    display: flex;
    flex-direction: column;

    .rule-management__table-content {
      height: calc(100% - 0.92rem);
      overflow-y: auto;

      .rule-management__operation {
        display: flex;
        align-items: center;
        gap: 0.2rem;

        .rule-management__operation-item {
          cursor: pointer;
          display: flex;
          align-items: center;

          img {
            width: 0.14rem;
            height: 0.14rem;
            margin-right: 0.04rem;
            object-fit: contain;
          }
        }
      }

      .rule-management__table-icon {
        width: 0.46rem;
        height: 0.2rem;
        object-fit: contain;
      }
    }

    .rule-management__pagination {
      display: flex;
      justify-content: flex-end;
      margin: 0.2rem 0;
    }
  }

  :deep(.el-switch) {
    height: 0.22rem;
  }

  // :deep(.el-select__popper.el-popper) {
  //   top: 0.44rem !important;
  // }

  :deep(.test-form.el-form--inline .el-form-item) {
    margin-bottom: 0rem !important;
  }

  :deep(.el-form--inline .el-form-item) {
    margin-bottom: 0.4rem !important;
  }
}
</style>
