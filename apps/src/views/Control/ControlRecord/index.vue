<template>
  <div class="voltage-class alarm-records">
    <LoadingBg :loading="loading">
      <!-- 表格工具-->
      <div class="alarm-records__toolbar">
        <div class="alarm-records__toolbar-left" ref="toolbarLeftRef">
          <!-- 选择-->
          <el-select
            v-model="filters.warning_level"
            @change="fetchTableData(true)"
            :append-to="toolbarLeftRef"
            placeholder="select warning level"
            clearable
          >
            <el-option label="L1" :value="1" />
            <el-option label="L2" :value="2" />
            <el-option label="L3" :value="3" />
          </el-select>
        </div>

        <div class="alarm-records__toolbar-right">
          <!-- 导出按钮 - 只在history模式下显-->
          <IconButton
            v-if="activeTab === 'history'"
            type="primary"
            :icon="alarmExportIcon"
            text="Export"
            custom-class="alarm-records__export-btn"
            @click="handleExport"
          />
        </div>
      </div>

      <!-- 表格 -->
      <div class="alarm-records__table">
        <el-table
          :data="tableData"
          class="alarm-records__table-content"
          table-layout="fixed"
          align="left"
        >
          <el-table-column prop="rule_name" label="Name" />
          <el-table-column prop="channel_id" label="Channel ID" />
          <el-table-column prop="warning_level" label="Level">
            <template #default="scope">
              <img
                :src="warningLevelList[scope.row.warning_level as 1 | 2 | 3]"
                class="alarm-records__table-icon"
                alt="level icon"
              />
            </template>
          </el-table-column>
          <el-table-column prop="triggered_at" label="Start Time" />
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
import type { CurrentAlarmData } from '@/types/alarm'
import { useTableData, type TableConfig } from '@/composables/useTableData'

import alarmExportIcon from '@/assets/icons/alarm-export.svg'
import level1Icon from '@/assets/icons/home-alter-L1.svg'
import level2Icon from '@/assets/icons/home-alter-L2.svg'
import level3Icon from '@/assets/icons/home-alter-L3.svg'
// 响应式数据
const activeTab = ref<'current' | 'history'>('current')
const toolbarLeftRef = ref<HTMLElement | null>(null)
// 表格配置
const tableConfig: TableConfig = {
  listUrl: '/alarmApi/alerts',
  defaultPageSize: 20,
}

// 使用 useTableData composable
const {
  loading,
  tableData,
  pagination,
  handlePageSizeChange,
  fetchTableData,
  filters,
  handlePageChange,
} = useTableData<CurrentAlarmData>(tableConfig)

filters.warning_level = undefined
const warningLevelList = {
  1: level1Icon,
  2: level2Icon,
  3: level3Icon,
}

// 处理导出
const handleExport = () => {
  ElMessage.success('导出功能已触发')
  // 这里可以添加实际的导出逻辑
  console.log('导出数据:', tableData.value)
}
</script>

<style scoped lang="scss">
.voltage-class.alarm-records {
  height: 100%;
  display: flex;
  flex-direction: column;

  .alarm-records__toolbar {
    padding-bottom: 0.2rem;
    display: flex;
    align-items: center;
    justify-content: space-between;

    .alarm-records__toolbar-left {
      display: flex;
      align-items: center;
      gap: 0.16rem;
    }

    .alarm-records__toolbar-right {
      display: flex;
      align-items: center;
      gap: 0.16rem;

      .alarm-records__export-btn {
        display: flex;
        align-items: center;
        gap: 0.1rem;

        .alarm-records__export-icon {
          width: 0.16rem;
          height: 0.16rem;
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
}
</style>
