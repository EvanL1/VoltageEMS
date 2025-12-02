<template>
  <div class="voltage-class rule-management">
    <LoadingBg :loading="loading">
      <div class="rule-management__header">
        <div class="rule-management__search-form" ref="levelSelectRef">
          <el-form :model="filters" :inline="true" class="test-form">
            <el-form-item label="productName:">
              <el-select
                v-model="filters.product_name"
                placeholder="Please select productName"
                clearable
                filterable
                :append-to="levelSelectRef"
              >
                <el-option
                  v-for="opt in productOptions"
                  :key="opt.value"
                  :label="opt.label"
                  :value="opt.value"
                />
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
              text="New Instance"
              custom-class="rule-management__btn"
              @click="handleAddUser"
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
          row-key="instance_id"
        >
          <el-table-column prop="instance_id" label="ID" width="100" />
          <el-table-column prop="instance_name" label="Instance Name" min-width="200" />
          <el-table-column prop="product_name" label="Product Name" min-width="200" />
          <el-table-column label="Operation" fixed="right" min-width="100">
            <template #default="{ row }">
              <div class="rule-management__operation">
                <div class="rule-management__operation-item" @click="handleDetail(row)">
                  <img :src="detailIcon" />
                  <span class="rule-management__operation-text">Detail</span>
                </div>
                <div class="rule-management__operation-item" @click="openPointsDialog(row)">
                  <img :src="pointIcon" />
                  <span class="rule-management__operation-text">Points</span>
                </div>
                <!-- <div class="rule-management__operation-item" @click="openMappingsDialog(row)">
                  <img :src="tableEditIcon" />
                  <span class="rule-management__operation-text">Mappings</span>
                </div> -->
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
    <InstanceDetailDialog
      ref="instanceDetailDialogRef"
      :product-options="productOptions"
      @submit="handleDetailSubmit"
    />

    <!-- Points Tables 对话框（DeviceConfiguration 专用） -->
    <PointsTablesDialog ref="PointsTablesDialogRef" />

    <!-- Mappings 对话框（DeviceConfiguration 专用） -->
    <!-- <MappingsDialog ref="MappingsDialogRef" /> -->
  </div>
</template>

<script setup lang="ts">
// 正确引入SVG图标，避免部署后图片加载不出�?
import tableRefreshIcon from '@/assets/icons/table-refresh.svg'
import tableSearchIcon from '@/assets/icons/table-search.svg'
import userAddIcon from '@/assets/icons/user-add.svg'
import tableDeleteIcon from '@/assets/icons/table-delect.svg'
import detailIcon from '@/assets/icons/button-detail.svg'
import pointIcon from '@/assets/icons/button-point.svg'

import InstanceDetailDialog from './components/InstanceDetailDialog.vue'
import PointsTablesDialog from './components/PointsTablesDialog.vue'
import type { DeviceInstanceBasic, ProductListItem } from '@/types/deviceConfiguration'
import { ElMessageBox, ElMessage } from 'element-plus'
import { getProducts } from '@/api/devicesManagement'
import { useTableData, type TableConfig } from '@/composables/useTableData'

const tableConfig: TableConfig = {
  listUrl: '/modApi/api/instances',
  deleteUrl: '/modApi/api/instances/{id}',
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
} = useTableData<DeviceInstanceBasic>(tableConfig)

filters.product_name = ''

const levelSelectRef = ref<HTMLElement | null>(null)

const productOptions = ref<Array<{ label: string; value: string }>>([])
const getProductOptions = async () => {
  const res = await getProducts()
  productOptions.value = (res?.data?.products || []).map((it: ProductListItem) => ({
    label: it.product_name,
    value: it.product_name,
  }))
}
onMounted(() => getProductOptions())

const instanceDetailDialogRef = ref()
const PointsTablesDialogRef = ref()
// const MappingsDialogRef = ref()

// 配置弹窗相关数据
const configDialogVisible = ref(false)
const currentConfigRow = ref<any>(null)
const currentConfigData = ref({
  channelName: '',
  channelType: '',
  point: '',
})

// 添加规则
const handleAddUser = () => {
  // 设备实例新增：以实例名形式调用 open 并进入编辑态。这里传空字符串代表新建。
  instanceDetailDialogRef.value?.open(null as any)
}

// 查看详情
const handleDetail = (row: DeviceInstanceBasic) => {
  instanceDetailDialogRef.value?.open(row.instance_id as any)
}

// 删除规则
const handleDelete = async (row: DeviceInstanceBasic) => {
  await ElMessageBox.confirm(
    `Are you sure you want to delete rule "${row.instance_name}"?`,
    'Delete Rule',
    {
      confirmButtonText: 'Delete',
      cancelButtonText: 'Cancel',
      type: 'warning',
    },
  )

  // 从模拟数据中删除
  const index = tableData.value.findIndex(
    (item: DeviceInstanceBasic) => item.instance_id === row.instance_id,
  )
  if (index > -1) {
    tableData.value.splice(index, 1)
    pagination.total = tableData.value.length
    ElMessage.success('Device instance deleted successfully')
  }
}

// 打开 Device Points/Mappings 对话框
const openPointsDialog = (row: DeviceInstanceBasic) => {
  PointsTablesDialogRef.value?.open(row.instance_id, row.instance_name)
}
// const openMappingsDialog = (row: DeviceInstanceBasic) => {
//   MappingsDialogRef.value?.open(row.instance_id)
// }

const handleRefresh = () => {
  filters.product_name = ''
  fetchTableData(true)
}
const handleSearch = () => {
  fetchTableData(true)
}

// 详情弹窗提交
const handleDetailSubmit = () => {
  fetchTableData()
}
</script>

<style scoped lang="scss">
.voltage-class .rule-management {
  //   position: relative;
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
        margin: 0;
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
}
</style>
