<template>
  <div class="voltage-class rule-management" ref="ruleManagementRef">
    <LoadingBg :loading="loading">
      <div class="rule-management__header">
        <div class="rule-management__search-form" ref="levelSelectRef">
          <el-form :model="filters" :inline="true" class="test-form">
            <el-form-item label="Protocol:">
              <el-select
                v-model="filters.protocol"
                placeholder="select protocol"
                :teleported="false"
                clearable
              >
                <el-option
                  v-for="option in PROTOCOL_OPTIONS"
                  :key="option.value"
                  :label="option.label"
                  :value="option.value"
                />
              </el-select>
            </el-form-item>
            <el-form-item label="Enabled:">
              <el-select
                v-model="filters.enabled"
                placeholder="select enabled status"
                clearable
                :teleported="false"
                :fit-input-width="true"
              >
                <el-option label="Enabled" :value="true" />
                <el-option label="Disabled" :value="false" />
              </el-select>
            </el-form-item>
            <el-form-item label="Connected:">
              <el-select
                v-model="filters.connected"
                placeholder="select connected status"
                clearable
                :fit-input-width="true"
                :teleported="false"
              >
                <el-option label="Connected" :value="true" />
                <el-option label="Disconnected" :value="false" />
              </el-select>
            </el-form-item>
          </el-form>
          <div class="form-oprations">
            <IconButton
              type="warning"
              :icon="tableRefreshIcon"
              text="Reload"
              custom-class="rule-management__btn"
              @click="handleRefresh"
            />
            <IconButton
              type="primary"
              :icon="tableSearchIcon"
              text="Search"
              custom-class="rule-management__btn"
              @click="handleSearch"
            />
            <IconButton
              type="primary"
              :icon="userAddIcon"
              text="New Channel"
              custom-class="rule-management__btn"
              @click="addChannel"
            />
          </div>
        </div>
        <div class="rule-management__table-operations"></div>
      </div>
      <div class="rule-management__table">
        <el-table
          :data="tableData"
          class="rule-management__table-content"
          align="left"
          table-layout="fixed"
          :expand-row-keys="expandedRows"
          row-key="id"
        >
          <el-table-column prop="id" label="ID" min-width="80" />
          <el-table-column prop="name" label="Name" min-width="160" show-overflow-tooltip />
          <el-table-column
            prop="description"
            label="Description"
            show-overflow-tooltip
            min-width="160"
          >
            <template #default="{ row }">
              <span>{{ row.description || '-' }}</span>
            </template>
          </el-table-column>
          <el-table-column prop="protocol" label="Protocol" min-width="140" show-overflow-tooltip />
          <el-table-column prop="enabled" label="Enabled" min-width="100">
            <template #default="{ row, $index }">
              <el-switch
                v-model="row.enabled"
                @change="handleEnabledChange($event, row, $index)"
                :loading="channelControlLoadings[$index][0]"
              />
            </template>
          </el-table-column>
          <el-table-column prop="connected" label="Connected" min-width="100">
            <template #default="{ row }">
              <span
                :style="{
                  color: row.connected ? '#67C23A' : '#F56C6C',
                  fontWeight: '500',
                }"
              >
                {{ row.connected ? 'Connected' : 'Disconnected' }}
              </span>
            </template>
          </el-table-column>
          <!-- <el-table-column prop="error_count" label="Error Count" /> -->
          <el-table-column label="Operation" fixed="right" min-width="140">
            <template #default="{ row }">
              <div class="rule-management__operation">
                <div class="rule-management__operation-item" @click="handleDetail(row)">
                  <img :src="buttonDetailIcon" />
                  <span class="rule-management__operation-text">Detail</span>
                </div>
                <div class="rule-management__operation-item" @click="handlePointsTables(row)">
                  <img :src="buttonPointsIcon" />
                  <span class="rule-management__operation-text">Points </span>
                </div>
                <!-- <div class="rule-management__operation-item" @click="handleMappings(row)">
                  <img :src="tableEditIcon" />
                  <span class="rule-management__operation-text">Mappings</span>
                </div> -->
                <div
                  class="rule-management__operation-item"
                  @click="
                    deleteRow(
                      row.id,
                      `Are you sure you want to delete channel ${row.name}?`,
                      ruleManagementRef,
                    )
                  "
                >
                  <img :src="tableDeleteIcon" />
                  <span class="rule-management__operation-text">Delete</span>
                </div>

                <!-- <el-dropdown
                  trigger="click"
                  placement="bottom"
                  :show-arrow="false"
                  :teleported="false"
                >
                  <div class="rule-management__operation-item">
                    <img :src="tableDeleteIcon" />
                    <span class="rule-management__operation-text">control</span>
                  </div>
                  <template #dropdown>
                    <el-dropdown-menu>
                      <el-dropdown-item @click="handleControl(row, 'start')"
                        >Start</el-dropdown-item
                      >
                      <el-dropdown-item @click="handleControl(row, 'stop')">Stop</el-dropdown-item>
                      <el-dropdown-item @click="handleControl(row, 'restart')"
                        >Restart</el-dropdown-item
                      >
                    </el-dropdown-menu>
                  </template>
                </el-dropdown> -->
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
            :teleported="false"
            layout="total, sizes, prev, pager, next"
            @size-change="handlePageSizeChange"
            @current-change="handlePageChange"
          />
        </div>
      </div>
    </LoadingBg>
    <ChannelDetailDialog
      ref="ChannelDetailDialogRef"
      @submit="fetchTableData"
      @cancel="handleChannelDetailCancel"
    />

    <!-- Points Tables 对话框 -->
    <PointsTablesDialog ref="PointsTablesDialogRef" />
  </div>
</template>

<script setup lang="ts">
// 正确引入SVG图标，避免部署后图片加载不出�?
import tableRefreshIcon from '@/assets/icons/table-refresh.svg'
import tableSearchIcon from '@/assets/icons/table-search.svg'
import userAddIcon from '@/assets/icons/user-add.svg'
import tableDeleteIcon from '@/assets/icons/table-delect.svg'
import buttonDetailIcon from '@/assets/icons/button-detail.svg'
import buttonPointsIcon from '@/assets/icons/button-point.svg'
import ChannelDetailDialog from './components/ChannelDetailDialog.vue'
import PointsTablesDialog from './components/PointsTablesDialog.vue'
import { ChangeChannelEnabled } from '@/api/channelsManagement'
import type { ChannelListItem, ChannelDetail, PointInfo } from '@/types/channelConfiguration'
import { PROTOCOL_OPTIONS } from '@/types/channelConfiguration'
import { ElMessage } from 'element-plus'
import type { TableInstance } from 'element-plus'
import { useTableData, type TableConfig } from '@/composables/useTableData'

const tableConfig: TableConfig = {
  listUrl: '/comApi/api/channels',
  deleteUrl: '/comApi/api/channels/{id}',
  defaultPageSize: 20,
}
const {
  loading,
  tableData,
  pagination,
  handlePageSizeChange,
  fetchTableData,
  filters,
  handlePageChange,
  deleteRow,
} = useTableData<ChannelListItem>(tableConfig)

filters.productName = ''
filters.enabled = null
filters.connected = null
const tableRef = ref<TableInstance | null>(null)
const columns = ref([
  {
    prop: 'id',
    label: 'ID',
    width: 100,
  },
  {
    prop: 'name',
    label: 'Name',
    width: 160,
  },
  {
    prop: 'description',
    label: 'Description',
    width: 160,
  },
  {
    prop: 'protocol',
    label: 'Protocol',
    width: 140,
  },
  {
    prop: 'enabled',
    label: 'Enabled',
    width: 100,
  },
  {
    prop: 'connected',
    label: 'Connected',
    width: 140,
  },
  {
    prop: 'error_count',
    label: 'Error Count',
    width: 100,
  },
  {
    label: 'Operation',
    fixed: 'right',
    width: 280,
  },
])
const levelSelectRef = ref<HTMLElement | null>(null)
const ChannelDetailDialogRef = ref()
const PointsTablesDialogRef = ref()
const ruleManagementRef = ref<HTMLElement | null>(null)
// 展开行控制
const expandedRows = ref<number[]>([])
// 编辑状态管理 - 每个通道独立的编辑状态
const editingChannels = ref<Set<number>>(new Set())
const editPointsData = ref<Record<number, Record<string, PointInfo[]>>>({})

// 点位数据存储
const pointsData = ref<Record<number, Record<string, PointInfo[]>>>({})

// 每个通道的 tab 状态
const channelTabs = ref<Record<number, string>>({})

const channelControlLoadings = ref<boolean[][]>([])
// 检查特定通道是否在编辑状态
const isChannelEditing = (channelId: number) => {
  return editingChannels.value.has(channelId)
}

// 获取通道的活跃 tab
const getActiveTab = (channelId: number) => {
  if (!channelTabs.value[channelId]) {
    channelTabs.value[channelId] = 'telemetry'
  }
  return channelTabs.value[channelId]
}

// 设置通道的活跃 tab
const setActiveTab = (channelId: number, tabName: string) => {
  channelTabs.value[channelId] = tabName
}

// Points Tables 对话框相关数据
const currentChannelId = ref<number>(0)

watch(
  tableData,
  (newVal) => {
    // 只有当数组长度发生变化时才更新loading状态，避免不必要的更新
    if (channelControlLoadings.value.length !== newVal.length) {
      channelControlLoadings.value = newVal.map(() => [false, false])
    }
  },
  { deep: false },
)

// 查看详情
const handleDetail = (row: ChannelListItem) => {
  ChannelDetailDialogRef.value?.open(row.id)
}

// 添加通道
const addChannel = () => {
  ChannelDetailDialogRef.value?.open()
}

// 处理 Points Tables
const handlePointsTables = (row: ChannelListItem) => {
  currentChannelId.value = row.id as number
  PointsTablesDialogRef.value?.open(row.id as number, row.name, row.protocol)
}

// 处理启用状态变化
const handleEnabledChange = async (state: boolean, row: ChannelListItem, index: number) => {
  channelControlLoadings.value[index][0] = true
  row.enabled = !state
  try {
    const res = await ChangeChannelEnabled(row.id as number, state)
    if (res.success) {
      // 更新成功后刷新数据，而不是直接修改row
      await fetchTableData()
    } else {
      // 如果失败，恢复原状态
      row.enabled = !state
    }
  } catch (error) {
    // 发生错误时恢复原状态
    row.enabled = !state
    ElMessage.error(error as string)
  } finally {
    channelControlLoadings.value[index][0] = false
  }
}

const handleRefresh = () => {
  filters.protocol = ''
  filters.enabled = null
  filters.connected = null
  fetchTableData(true)
}

const handleSearch = () => {
  fetchTableData(true)
}

// // 控制通道状态
// const handleControl = (row: ChannelListItem, operation: 'start' | 'stop' | 'restart') => {
//   controlChannelStatus(row.id as number, operation)
// }

// 通道详情弹窗提交
const handleChannelDetailSubmit = (data: ChannelDetail) => {
  console.log('Channel detail submitted:', data)
}

// 通道详情弹窗取消
const handleChannelDetailCancel = () => {
  console.log('Channel detail cancelled')
}
</script>

<style scoped lang="scss">
.voltage-class .rule-management {
  // position: relative;
  height: 100%;
  width: 100%;
  display: flex;
  flex-direction: column;

  .rule-management__header {
    margin: 0.2rem 0;

    .rule-management__search-form {
      position: relative;
      display: flex;
      align-items: center;
      justify-content: space-between;
      //   padding-bottom: 0.2rem;
      :deep(.el-form-item) {
        margin-bottom: 0;
      }
      .form-oprations {
        display: flex;
        align-items: flex-start;
        gap: 0.1rem;
      }
    }

    .rule-management__table-operations {
      width: 100%;
      //   padding-top: 0.2rem;
      //   border-top: 0.01rem solid rgba(255, 255, 255, 0.1);
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
    height: calc(100% - 0.72rem);
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
        .position-relative {
          position: relative;
        }
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
          .rule-management__operation-text {
            font-size: 0.14rem;
            color: #fff;
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

  .rule-management__expand-content {
    .rule-management__edit-controls {
      display: flex;
      gap: 0.1rem;
      margin-bottom: 0.2rem;
      padding: 0.1rem 0;
      border-bottom: 1px solid rgba(255, 255, 255, 0.1);

      .el-button {
        display: flex;
        align-items: center;
        font-size: 0.14rem;
        padding: 0.08rem 0.16rem;
        border-radius: 0.04rem;

        img {
          width: 0.14rem;
          height: 0.14rem;
          margin-right: 0.04rem;
          object-fit: contain;
        }
      }
    }
  }
}
</style>
