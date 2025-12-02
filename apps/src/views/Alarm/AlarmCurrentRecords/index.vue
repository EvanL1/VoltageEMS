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
      </div>

      <!-- 表格 -->
      <div class="alarm-records__table">
        <el-table :data="tableData" class="alarm-records__table-content">
          <el-table-column prop="rule_name" label="Name" min-width="1.2rem" />
          <el-table-column prop="channel_id" label="Channel ID" min-width="1.2rem" />
          <el-table-column prop="warning_level" label="Level" min-width="1rem">
            <template #default="scope">
              <img
                :src="levelIconList[scope.row.warning_level as 1 | 2 | 3]"
                class="alarm-records__table-icon"
                alt="level icon"
              />
            </template>
          </el-table-column>
          <el-table-column prop="triggered_at" label="Start Time" min-width="1.6rem" />
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

// import alarmExportIcon from '@/assets/icons/alarm-export.svg'
import level1Icon from '@/assets/icons/home-alter-L1.svg'
import level2Icon from '@/assets/icons/home-alter-L2.svg'
import level3Icon from '@/assets/icons/home-alter-L3.svg'

const levelIconList = {
  1: level1Icon,
  2: level2Icon,
  3: level3Icon,
}
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

// const warningLevelList = [
//   {
//     label: 'L1',
//     value: 1,
//   },

//   {
//     label: 'L2',
//     value: 2,
//   },
//   {
//     label: 'L3',
//     value: 3,
//   },
// ]
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
      position: relative;
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
