<template>
  <div class="voltage-class alarm-records">
    <LoadingBg :loading="loading">
      <!-- 表格工具�?-->
      <div class="alarm-records__toolbar">
        <div class="alarm-records__toolbar-left" ref="toolbarLeftRef">
          <el-form :model="filters" inline>
            <el-form-item label="Warning Level:">
              <el-select
                v-model="filters.warning_level"
                :append-to="toolbarLeftRef"
                clearable
                placeholder="select warning level"
              >
                <el-option label="L1" :value="1" />
                <el-option label="L2" :value="2" />
                <el-option label="L3" :value="3" />
              </el-select>
            </el-form-item>
            <el-form-item label="Start Time:">
              <el-date-picker
                v-model="filters.start_time"
                type="datetime"
                placeholder="Select start time"
                format="YYYY-MM-DD HH:mm:ss"
                :disabled-date="disableStartDate"
                :disabled-time="disableStartTime"
                @change="handleStartTimeChange"
                :teleported="false"
                clearable
              />
            </el-form-item>
            <el-form-item label="End Time:">
              <el-date-picker
                v-model="filters.end_time"
                type="datetime"
                placeholder="Select end time"
                format="YYYY-MM-DD HH:mm:ss"
                :disabled-date="disableEndDate"
                :disabled-time="disableEndTime"
                @change="handleEndTimeChange"
                :teleported="false"
                clearable
              />
            </el-form-item>
          </el-form>
        </div>

        <div class="alarm-records__toolbar-right">
          <IconButton
            type="warning"
            :icon="reloadIcon"
            text="Reload"
            custom-class="alarm-records__export-btn"
            @click="refreshData"
          />
          <IconButton
            type="primary"
            :icon="searchIcon"
            text="search"
            custom-class="alarm-records__export-btn"
            @click="fetchTableData"
          />
          <IconButton
            type="primary"
            :icon="alarmExportIcon"
            text="Export"
            custom-class="alarm-records__export-btn"
            @click="exportData(`Alarm_History_${Date.now().toString()}.csv`)"
          />
        </div>
      </div>

      <!-- 表格 -->
      <div class="alarm-records__table">
        <el-table :data="tableData" class="alarm-records__table-content">
          <el-table-column prop="rule_name" label="Name" min-width="1.2rem" />
          <el-table-column prop="channel_id" label="Channel ID" min-width="1.2rem" />
          <el-table-column prop="warning_level" label="Level" min-width="1rem">
            <template #default="scope">
              <img
                :src="warningLevelList[scope.row.warning_level as 1 | 2 | 3]"
                class="alarm-records__table-icon"
                alt="level icon"
              />
            </template>
          </el-table-column>
          <el-table-column prop="triggered_at" label="Start Time" min-width="1.6rem" />
          <el-table-column prop="recovered_at" label="End Time" min-width="1.6rem" />
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
            @current-change="handlePageChange"
          />
        </div>
      </div>
    </LoadingBg>
  </div>
</template>

<script setup lang="ts">
import type { HistoryAlarmData } from '@/types/alarm'
import { useTableData } from '@/composables/useTableData'

import alarmExportIcon from '@/assets/icons/alarm-export.svg'
import searchIcon from '@/assets/icons/table-search.svg'
import reloadIcon from '@/assets/icons/table-refresh.svg'
import level1Icon from '@/assets/icons/home-alter-L1.svg'
import level2Icon from '@/assets/icons/home-alter-L2.svg'
import level3Icon from '@/assets/icons/home-alter-L3.svg'

const toolbarLeftRef = ref<HTMLElement | null>(null)
const warningLevelList = {
  1: level1Icon,
  2: level2Icon,
  3: level3Icon,
}

// 使用 useTableData composable
const {
  loading,
  tableData,
  pagination,
  handlePageSizeChange,
  handlePageChange,
  fetchTableData,
  filters,
  exportData,
} = useTableData<HistoryAlarmData>({
  listUrl: '/alarmApi/alert-events',
  exportUrl: '/alarmApi/alert-events/export',
  enableExport: true,
  defaultPageSize: 20,
})

// 初始化filters
filters.warning_level = null
filters.start_time = null
filters.end_time = null

// 处理开始时间变化
const handleStartTimeChange = (value: Date | null) => {
  // 记录原始 Date 以便禁用规则计算
  filters.startTime = value || null
  // 如果开始时间晚于或等于结束时间，清空结束时间
  if (value && filters.endTime && value.getTime() >= new Date(filters.endTime).getTime()) {
    filters.endTime = null
    filters.end_time = null
  }
  // 转为后端需要的字符串格式
  filters.start_time = value ? value.toLocaleString('sv-SE').replace(' ', 'T') : null
}

// 处理结束时间变化
const handleEndTimeChange = (value: Date | null) => {
  // 记录原始 Date 以便禁用规则计算
  let adjusted: Date | null = value ? new Date(value) : null
  // 若时间未指定（00:00:00），默认设置到当天 23:59:59
  if (
    adjusted &&
    adjusted.getHours() === 0 &&
    adjusted.getMinutes() === 0 &&
    adjusted.getSeconds() === 0
  ) {
    adjusted.setHours(23, 59, 59, 999)
  }
  filters.endTime = adjusted || null
  // 如果结束时间早于或等于开始时间，清空开始时间
  if (
    adjusted &&
    filters.startTime &&
    adjusted.getTime() <= new Date(filters.startTime).getTime()
  ) {
    filters.startTime = null
    filters.start_time = null
  }
  // 转为后端需要的字符串格式
  filters.end_time = adjusted ? adjusted.toLocaleString('sv-SE').replace(' ', 'T') : null
}

// 禁用开始时间的日期选择
const disableStartDate = (time: Date) => {
  if (!filters.endTime) return false
  // 开始日期不得晚于结束日期（同日允许，具体时间由 disableStartTime 控制）
  return time.getTime() > new Date(filters.endTime).getTime()
}

// 禁用开始时间的时间选择
const disableStartTime = (date: Date, type: string) => {
  if (!filters.endTime || type !== 'minute') return {}
  const endTime = new Date(filters.endTime)
  if (date.getDate() === endTime.getDate()) {
    return {
      disabledHours: () =>
        Array.from({ length: 24 }, (_, i) => i).filter((h) => h > endTime.getHours()),
      disabledMinutes: () =>
        Array.from({ length: 60 }, (_, i) => i).filter((m) => m > endTime.getMinutes()),
    }
  }
  return {}
}

// 禁用结束时间的日期选择
const disableEndDate = (time: Date) => {
  if (!filters.startTime) return false
  // 结束日期不得早于开始日期（同日允许，具体时间由 disableEndTime 控制）
  return time.getTime() < new Date(filters.startTime).getTime()
}

// 禁用结束时间的时间选择
const disableEndTime = (date: Date, type: string) => {
  if (!filters.startTime || type !== 'minute') return {}
  const startTime = new Date(filters.startTime)
  if (date.getDate() === startTime.getDate()) {
    return {
      disabledHours: () =>
        Array.from({ length: 24 }, (_, i) => i).filter((h) => h < startTime.getHours()),
      disabledMinutes: () =>
        Array.from({ length: 60 }, (_, i) => i).filter((m) => m < startTime.getMinutes()),
    }
  }
  return {}
}
const refreshData = () => {
  filters.warning_level = null
  filters.start_time = null
  filters.end_time = null
  filters.startTime = null
  filters.endTime = null
  fetchTableData(true)
}
// 处理导出
</script>

<style scoped lang="scss">
.voltage-class.alarm-records {
  position: relative;
  height: 100%;
  display: flex;
  flex-direction: column;

  .alarm-records__toolbar {
    padding-bottom: 0.2rem;
    display: flex;
    align-items: center;
    justify-content: space-between;

    .alarm-records__toolbar-left {
      position: relative;
      display: flex;
      align-items: center;
      gap: 0.16rem;
    }

    .alarm-records__toolbar-right {
      display: flex;
      align-items: center;

      .alarm-records__export-btn {
        display: flex;
        align-items: center;
        gap: 0.1rem;

        .alarm-records__export-icon {
          width: 0.16rem;
          height: 0.16rem;
          margin-right: 0.08rem;
        }
      }
    }
  }

  .alarm-records__table {
    height: calc(100% - 0.52rem);
    width: 100%;
    display: flex;
    flex-direction: column;

    .alarm-records__table-content {
      width: 100%;
      height: calc(100% - 0.92rem);
      overflow-y: auto;

      .alarm-records__table-icon {
        width: 0.46rem;
        height: 0.2rem;
        object-fit: contain;
      }
    }

    .alarm-records__pagination {
      padding: 0.2rem 0;
      display: flex;
      justify-content: flex-end;
    }
  }

  :deep(.el-form.el-form--inline .el-form-item) {
    margin-bottom: 0;
  }
}

// :deep(.el-select__popper.el-popper) {
//   top: 0.44rem !important;
// }
</style>
