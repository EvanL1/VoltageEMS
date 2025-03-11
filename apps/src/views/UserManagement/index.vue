<template>
  <div class="voltage-class user-management">
    <div class="user-management__header">
      <el-button type="primary" @click="handleAddUser" class="user-management__add-btn">
        <img :src="userAddIcon" class="user-management__add-btn-icon" />
        Add User
      </el-button>
    </div>

    <!-- 用户操作表单 -->
    <UserOperationForm ref="userFormRef" @submit="handleUserSubmit" @cancel="handleUserCancel" />
    <div class="user-management__table">
      <el-table
        :data="tableData"
        v-loading="loading"
        class="user-management__table-content"
        align="left"
        table-layout="fixed"
      >
        <el-table-column prop="id" label="ID" show-overflow-tooltip />
        <el-table-column prop="userName" label="UserName" show-overflow-tooltip />
        <el-table-column prop="realName" label="RealName" show-overflow-tooltip />
        <!-- 角色列改为纯文字类型 -->
        <el-table-column prop="role" label="Role" show-overflow-tooltip>
          <template #default="{ row }">
            {{ row.role }}
          </template>
        </el-table-column>
        <el-table-column prop="email" label="Email" show-overflow-tooltip />
        <el-table-column prop="phone" label="Phone" show-overflow-tooltip />
        <el-table-column prop="status" label="Status" show-overflow-tooltip>
          <template #default="{ row }">
            <el-switch :model-value="row.status === 'active'" disabled />
          </template>
        </el-table-column>
        <el-table-column prop="lastLogin" label="Last Login" show-overflow-tooltip>
          <template #default="{ row }">
            {{ formatDateTime(row.lastLogin) }}
          </template>
        </el-table-column>
        <el-table-column prop="createdAt" label="Created At" show-overflow-tooltip>
          <template #default="{ row }">
            {{ formatDateTime(row.createdAt) }}
          </template>
        </el-table-column>
        <el-table-column label="Operation" fixed="right" show-overflow-tooltip>
          <template #default="{ row }">
            <div class="user-management__operation">
              <div class="user-management__operation-item" @click="handleEdit(row)">
                <img :src="tableEditIcon" />
                <span class="user-management__operation-text">Edit</span>
              </div>
              <div class="user-management__operation-item" @click="handleDelete(row)">
                <img :src="tableDeleteIcon" />
                <span class="user-management__operation-text">Delete</span>
              </div>
            </div>
          </template>
        </el-table-column>
      </el-table>

      <div class="user-management__pagination">
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
// 正确引入SVG图标，避免部署后图片加载不出来
import userAddIcon from '@/assets/icons/user-add.svg'
import tableEditIcon from '@/assets/icons/table-edit.svg'
import tableDeleteIcon from '@/assets/icons/table-delect.svg'

import type { UserManagementInfo } from '@/types/user'
import { Plus, Edit, Delete } from '@element-plus/icons-vue'
import { useTableData, type TableConfig } from '@/composables/useTableData'
import UserOperationForm from './UserOperationForm.vue'

// 表格配置
const tableConfig: TableConfig = {
  listUrl: '/api/users',
  deleteUrl: '/api/users/{id}',
  batchDeleteUrl: '/api/users/batch-delete',
  enableDelete: true,
  enableBatchDelete: true,
  defaultPageSize: 20,
  deleteConfirmMessage: 'Are you sure you want to delete this user?',
  batchDeleteConfirmMessage: 'Are you sure you want to delete selected users?',
}

// 使用表格数据管理 composable
const {
  loading,
  tableData,
  pagination: paginationData,
  handlePageChange,
} = useTableData<UserManagementInfo>(tableConfig)

// 创建可写的分页数据
const pagination = reactive({
  page: 1,
  pageSize: 20,
  total: 0,
})

// 同步分页数据
watch(
  paginationData,
  (newPagination) => {
    pagination.page = newPagination.page
    pagination.pageSize = newPagination.pageSize
    pagination.total = newPagination.total
  },
  { immediate: true },
)

// 模拟数据 - 在实际项目中这应该来自API
const mockData: UserManagementInfo[] = [
  {
    id: 1,
    userName: 'john_doe',
    realName: 'John Doe',
    role: 'Admin',
    email: 'john.doe@example.com',
    phone: '+1234567890',
    status: 'active',
    lastLogin: '2024-01-15T10:30:00Z',
    createdAt: '2023-12-01T08:00:00Z',
  },
  {
    id: 2,
    userName: 'jane_smith',
    realName: 'Jane Smith',
    role: 'User',
    email: 'jane.smith@example.com',
    phone: '+1234567891',
    status: 'active',
    lastLogin: '2024-01-14T16:45:00Z',
    createdAt: '2023-12-05T09:15:00Z',
  },
  {
    id: 3,
    userName: 'mike_wilson',
    realName: 'Mike Wilson',
    role: 'Moderator',
    email: 'mike.wilson@example.com',
    phone: '+1234567892',
    status: 'inactive',
    lastLogin: '2024-01-10T14:20:00Z',
    createdAt: '2023-11-28T11:30:00Z',
  },
  {
    id: 4,
    userName: 'sarah_johnson',
    realName: 'Sarah Johnson',
    role: 'User',
    email: 'sarah.johnson@example.com',
    phone: '+1234567893',
    status: 'banned',
    lastLogin: '2024-01-05T12:10:00Z',
    createdAt: '2023-11-20T15:45:00Z',
  },
]

// 初始化数据
onMounted(async () => {
  // 临时使用模拟数据，实际项目中应该调用 fetchTableData()
  await loadMockData()
  // await fetchTableData() // 真实API调用
})

// 加载模拟数据 - 临时方法
const loadMockData = async () => {
  // 模拟API延迟
  await new Promise((resolve) => setTimeout(resolve, 500))
  // 直接设置模拟数据
  tableData.value = mockData
  // 设置分页总数
  pagination.total = mockData.length
}

// 分页大小改变
const handlePageSizeChange = (newPageSize: number) => {
  handlePageChange(1, newPageSize)
}

// 当前页改变
const handleCurrentPageChange = (newPage: number) => {
  handlePageChange(newPage)
}

// 获取状态标签类型
const getStatusTagType = (status: string) => {
  const statusTypeMap: Record<string, string> = {
    active: 'success',
    inactive: 'warning',
    banned: 'danger',
  }
  return statusTypeMap[status] || 'info'
}

// 格式化日期时间，输出格式如 2025/07/08 12:40:32
const formatDateTime = (dateTime: string) => {
  if (!dateTime) return '-'
  const date = new Date(dateTime)
  const year = date.getFullYear()
  // 补零函数
  const pad = (n: number) => (n < 10 ? '0' + n : n)
  const month = pad(date.getMonth() + 1) // 月份补零
  const day = pad(date.getDate())
  const hour = pad(date.getHours())
  const minute = pad(date.getMinutes())
  const second = pad(date.getSeconds())
  return `${year}/${month}/${day} ${hour}:${minute}:${second}`
}

// 表单引用
const userFormRef = ref()

// 添加用户
const handleAddUser = () => {
  userFormRef.value?.open()
}

// 当前编辑的用户ID
const editingUserId = ref<number | null>(null)

// 处理用户表单提交
const handleUserSubmit = (formData: any) => {
  console.log('User form submitted:', formData)

  if (editingUserId.value) {
    // 编辑模式：更新现有用户
    const userIndex = tableData.value.findIndex((user) => user.id === editingUserId.value)
    if (userIndex > -1) {
      tableData.value[userIndex] = {
        ...tableData.value[userIndex],
        userName: formData.userName,
        realName: formData.userName, // 暂时使用用户名作为真实姓名
        role: formData.role,
        email: formData.email,
        phone: formData.phone,
        status: formData.status,
      }
      ElMessage.success('User updated successfully')
    }
    editingUserId.value = null
  } else {
    // 创建模式：添加新用户
    const newUser: UserManagementInfo = {
      id: Date.now(), // 临时ID，实际项目中应该由后端生成
      userName: formData.userName,
      realName: formData.userName, // 暂时使用用户名作为真实姓名
      role: formData.role,
      email: formData.email,
      phone: formData.phone,
      status: formData.status,
      lastLogin: '-',
      createdAt: new Date().toISOString(),
    }

    // 添加到表格数据
    tableData.value.unshift(newUser)

    // 更新分页总数
    pagination.total = tableData.value.length

    ElMessage.success('User added successfully')
  }
}

// 处理用户表单取消
const handleUserCancel = () => {
  console.log('User form cancelled')
  // 清除编辑状态
  editingUserId.value = null
}

// 编辑用户
const handleEdit = (row: UserManagementInfo) => {
  // 设置当前编辑的用户ID
  editingUserId.value = row.id

  // 准备编辑数据（不包含密码字段）
  const editData = {
    userName: row.userName,
    role: row.role,
    email: row.email,
    phone: row.phone,
    status: row.status,
  }

  userFormRef.value?.open(editData, 'edit')
}

// 删除用户
const handleDelete = async (row: UserManagementInfo) => {
  // 使用 composable 的删除方法
  // const success = await deleteRow(row.id, `Are you sure you want to delete user "${row.userName}"?`)

  if (true) {
    // 如果 composable 删除失败（比如API未配置），则使用本地删除
    try {
      await ElMessageBox.confirm(
        `Are you sure you want to delete user "${row.userName}"?`,
        'Delete User',
        {
          confirmButtonText: 'Delete',
          cancelButtonText: 'Cancel',
          type: 'warning',
        },
      )

      // 本地删除操作
      const index = tableData.value.findIndex((item: UserManagementInfo) => item.id === row.id)
      if (index > -1) {
        tableData.value.splice(index, 1)
        ElMessage.success('User deleted successfully')
      }
    } catch (error) {
      if (error !== 'cancel') {
        ElMessage.info('Delete cancelled')
      }
    }
  }
}
</script>

<style scoped lang="scss">
.voltage-class .user-management {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  .user-management__header {
    display: flex;
    align-items: center;
    margin-bottom: 20px;

    .user-management__add-btn {
      display: flex;
      align-items: center;
      gap: 8px;
      .user-management__add-btn-icon {
        width: 14px;
        height: 14px;
        margin-right: 8px;
      }
    }
  }

  .user-management__table {
    flex: 1;
    .user-management__table-content {
      width: 100%;
      height: calc(100% - 72px);
      overflow-y: auto;
      .user-management__operation {
        display: flex;
        align-items: center;
        gap: 20px;
        .user-management__operation-item {
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

    .user-management__pagination {
      display: flex;
      justify-content: flex-end;
      margin-top: 20px;
    }
  }
}
</style>
