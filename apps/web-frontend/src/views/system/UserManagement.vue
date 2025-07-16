<template>
  <div class="user-management-container">
    <!-- 页面头部 -->
    <div class="page-header">
      <h1>{{ $t('userManagement.title') }}</h1>
      <div class="header-actions">
        <el-button 
          v-permission="PERMISSIONS.SYSTEM.USER_CREATE"
          type="primary" 
          @click="handleAdd"
        >
          <el-icon><Plus /></el-icon>
          {{ $t('userManagement.addUser') }}
        </el-button>
      </div>
    </div>

    <!-- 搜索栏 -->
    <el-card class="search-card">
      <el-form :inline="true" :model="searchForm">
        <el-form-item :label="$t('userManagement.username')">
          <el-input 
            v-model="searchForm.username" 
            :placeholder="$t('userManagement.enterUsername')"
            clearable
            @keyup.enter="handleSearch"
          />
        </el-form-item>
        <el-form-item :label="$t('userManagement.role')">
          <el-select 
            v-model="searchForm.role" 
            :placeholder="$t('userManagement.selectRole')"
            clearable
          >
            <el-option 
              v-for="(name, role) in ROLE_NAMES" 
              :key="role"
              :label="name" 
              :value="role" 
            />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('common.status')">
          <el-select 
            v-model="searchForm.status" 
            :placeholder="$t('userManagement.selectStatus')"
            clearable
          >
            <el-option :label="$t('userManagement.active')" value="active" />
            <el-option :label="$t('userManagement.inactive')" value="inactive" />
          </el-select>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="handleSearch">
            <el-icon><Search /></el-icon>
            {{ $t('common.search') }}
          </el-button>
          <el-button @click="handleReset">
            <el-icon><Refresh /></el-icon>
            {{ $t('common.reset') }}
          </el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <!-- 用户列表 -->
    <el-card>
      <el-table 
        :data="userList" 
        stripe 
        v-loading="loading"
        @selection-change="handleSelectionChange"
      >
        <el-table-column type="selection" width="55" />
        <el-table-column prop="id" label="ID" width="80" />
        <el-table-column prop="username" :label="$t('userManagement.username')" min-width="120" />
        <el-table-column prop="realName" :label="$t('userManagement.realName')" min-width="120" />
        <el-table-column prop="role" :label="$t('userManagement.role')" width="120">
          <template #default="{ row }">
            <el-tag :type="getRoleType(row.role)">
              {{ row.role }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="email" :label="$t('userManagement.email')" min-width="180" />
        <el-table-column prop="phone" :label="$t('userManagement.phone')" width="140" />
        <el-table-column prop="status" :label="$t('common.status')" width="100">
          <template #default="{ row }">
            <el-switch 
              v-model="row.status" 
              active-value="active"
              inactive-value="inactive"
              @change="handleStatusChange(row)"
              :disabled="!can.editUser.value || row.role === ROLES.SUPER_ADMIN"
            />
          </template>
        </el-table-column>
        <el-table-column prop="lastLogin" :label="$t('userManagement.lastLogin')" width="180">
          <template #default="{ row }">
            {{ formatTime(row.lastLogin) }}
          </template>
        </el-table-column>
        <el-table-column prop="createdAt" :label="$t('userManagement.createdAt')" width="180">
          <template #default="{ row }">
            {{ formatTime(row.createdAt) }}
          </template>
        </el-table-column>
        <el-table-column :label="$t('common.actions')" width="200" fixed="right">
          <template #default="{ row }">
            <el-button 
              v-if="can.editUser.value"
              type="primary" 
              link 
              size="small" 
              @click="handleEdit(row)"
            >
              {{ $t('common.edit') }}
            </el-button>
            <el-button 
              v-if="canResetPassword(row)"
              type="primary" 
              link 
              size="small" 
              @click="handleResetPassword(row)"
            >
              {{ $t('userManagement.resetPassword') }}
            </el-button>
            <el-button 
              v-if="canDeleteUser(row)"
              type="danger" 
              link 
              size="small" 
              @click="handleDelete(row)"
            >
              {{ $t('common.delete') }}
            </el-button>
          </template>
        </el-table-column>
      </el-table>

      <!-- 分页 -->
      <div class="pagination-container">
        <el-pagination
          v-model:current-page="currentPage"
          v-model:page-size="pageSize"
          :page-sizes="[10, 20, 50, 100]"
          :total="total"
          layout="total, sizes, prev, pager, next, jumper"
          @size-change="handleSizeChange"
          @current-change="handlePageChange"
        />
      </div>
    </el-card>

    <!-- 新增/编辑对话框 -->
    <el-dialog 
      v-model="dialogVisible" 
      :title="dialogTitle"
      width="600px"
      :close-on-click-modal="false"
    >
      <el-form 
        ref="userFormRef" 
        :model="userForm" 
        :rules="userRules" 
        label-width="100px"
      >
        <el-form-item :label="$t('userManagement.username')" prop="username">
          <el-input 
            v-model="userForm.username" 
            :placeholder="$t('userManagement.enterUsername')"
            :disabled="isEdit"
          />
        </el-form-item>
        <el-form-item v-if="!isEdit" :label="$t('userManagement.password')" prop="password">
          <el-input 
            v-model="userForm.password" 
            type="password"
            :placeholder="$t('userManagement.enterPassword')"
            show-password
          />
        </el-form-item>
        <el-form-item :label="$t('userManagement.realName')" prop="realName">
          <el-input 
            v-model="userForm.realName" 
            :placeholder="$t('userManagement.enterRealName')"
          />
        </el-form-item>
        <el-form-item :label="$t('userManagement.role')" prop="role">
          <el-select v-model="userForm.role" :placeholder="$t('userManagement.selectRole')">
            <el-option 
              v-for="(name, role) in ROLE_NAMES" 
              :key="role"
              :label="name" 
              :value="role" 
            />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('userManagement.email')" prop="email">
          <el-input 
            v-model="userForm.email" 
            :placeholder="$t('userManagement.enterEmail')"
          />
        </el-form-item>
        <el-form-item :label="$t('userManagement.phone')" prop="phone">
          <el-input 
            v-model="userForm.phone" 
            :placeholder="$t('userManagement.enterPhone')"
          />
        </el-form-item>
        <el-form-item :label="$t('userManagement.permissions')">
          <el-tree
            ref="permissionTreeRef"
            :data="permissionTree"
            :props="{ label: 'name', children: 'children' }"
            show-checkbox
            node-key="id"
            :default-expand-all="true"
            :default-checked-keys="userForm.permissions"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">{{ $t('common.cancel') }}</el-button>
        <el-button type="primary" @click="handleSubmit">{{ $t('common.confirm') }}</el-button>
      </template>
    </el-dialog>

    <!-- 重置密码对话框 -->
    <el-dialog 
      v-model="resetPasswordDialog.visible" 
      :title="$t('userManagement.resetPassword')"
      width="400px"
    >
      <el-form 
        ref="resetPasswordFormRef" 
        :model="resetPasswordForm" 
        :rules="resetPasswordRules" 
        label-width="100px"
      >
        <el-form-item :label="$t('userManagement.newPassword')" prop="password">
          <el-input 
            v-model="resetPasswordForm.password" 
            type="password"
            :placeholder="$t('userManagement.enterNewPassword')"
            show-password
          />
        </el-form-item>
        <el-form-item :label="$t('userManagement.confirmPassword')" prop="confirmPassword">
          <el-input 
            v-model="resetPasswordForm.confirmPassword" 
            type="password"
            :placeholder="$t('userManagement.enterConfirmPassword')"
            show-password
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="resetPasswordDialog.visible = false">{{ $t('common.cancel') }}</el-button>
        <el-button type="primary" @click="handleResetPasswordSubmit">{{ $t('common.confirm') }}</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Plus, Search, Refresh } from '@element-plus/icons-vue'
import { useUserStore } from '@/stores/user'
import { usePermission } from '@/composables/usePermission'
import dayjs from 'dayjs'

const { t } = useI18n()
const userStore = useUserStore()
const { 
  PERMISSIONS, 
  ROLES, 
  ROLE_NAMES,
  can,
  checkPermission,
  isSystemAdmin,
  isSuperAdmin 
} = usePermission()

// 判断是否可以重置密码
const canResetPassword = (user) => {
  if (!checkPermission(PERMISSIONS.SYSTEM.USER_EDIT)) return false
  // 系统管理员可以重置普通用户密码
  // 超级管理员可以重置所有用户密码（除了自己）
  if (isSuperAdmin.value) {
    return user.id !== userStore.userInfo.id
  }
  if (isSystemAdmin.value) {
    return user.role !== ROLES.SUPER_ADMIN && user.role !== ROLES.SYSTEM_ADMIN
  }
  return false
}

// 判断是否可以删除用户
const canDeleteUser = (user) => {
  if (!checkPermission(PERMISSIONS.SYSTEM.USER_DELETE)) return false
  // 不能删除超级管理员
  if (user.role === ROLES.SUPER_ADMIN) return false
  // 不能删除自己
  if (user.id === userStore.userInfo.id) return false
  // 系统管理员不能删除其他系统管理员
  if (!isSuperAdmin.value && user.role === ROLES.SYSTEM_ADMIN) return false
  return true
}

// 角色选项根据权限动态生成，在具体的表单中实现

// 搜索表单
const searchForm = reactive({
  username: '',
  role: '',
  status: ''
})

// 用户列表
const userList = ref([])
const loading = ref(false)
const total = ref(0)
const currentPage = ref(1)
const pageSize = ref(20)
const selectedUsers = ref([])

// 对话框
const dialogVisible = ref(false)
const isEdit = ref(false)
const dialogTitle = computed(() => 
  isEdit.value ? t('userManagement.editUser') : t('userManagement.addUser')
)

// 用户表单
const userFormRef = ref(null)
const userForm = reactive({
  id: null,
  username: '',
  password: '',
  realName: '',
  role: 'operator',
  email: '',
  phone: '',
  permissions: []
})

// 表单验证规则
const userRules = {
  username: [
    { required: true, message: t('userManagement.usernameRequired'), trigger: 'blur' },
    { min: 3, max: 20, message: t('userManagement.usernameLengthError'), trigger: 'blur' }
  ],
  password: [
    { required: true, message: t('userManagement.passwordRequired'), trigger: 'blur' },
    { min: 6, message: t('userManagement.passwordLengthError'), trigger: 'blur' }
  ],
  realName: [
    { required: true, message: t('userManagement.realNameRequired'), trigger: 'blur' }
  ],
  role: [
    { required: true, message: t('userManagement.roleRequired'), trigger: 'change' }
  ],
  email: [
    { type: 'email', message: t('userManagement.emailFormatError'), trigger: 'blur' }
  ]
}

// 重置密码
const resetPasswordDialog = ref({ visible: false, userId: null })
const resetPasswordFormRef = ref(null)
const resetPasswordForm = reactive({
  password: '',
  confirmPassword: ''
})

// 重置密码验证规则
const resetPasswordRules = {
  password: [
    { required: true, message: t('userManagement.passwordRequired'), trigger: 'blur' },
    { min: 6, message: t('userManagement.passwordLengthError'), trigger: 'blur' }
  ],
  confirmPassword: [
    { required: true, message: t('userManagement.confirmPasswordRequired'), trigger: 'blur' },
    { 
      validator: (rule, value, callback) => {
        if (value !== resetPasswordForm.password) {
          callback(new Error(t('userManagement.passwordMismatch')))
        } else {
          callback()
        }
      }, 
      trigger: 'blur' 
    }
  ]
}

// 权限树
const permissionTreeRef = ref(null)
const permissionTree = ref([
  {
    id: 'monitoring',
    name: t('userManagement.permissions.monitoring'),
    children: [
      { id: 'dashboard', name: t('userManagement.permissions.dashboard') },
      { id: 'realtime', name: t('userManagement.permissions.realtime') },
      { id: 'grafana', name: t('userManagement.permissions.grafana') }
    ]
  },
  {
    id: 'control',
    name: t('userManagement.permissions.control'),
    children: [
      { id: 'device_control', name: t('userManagement.permissions.deviceControl') },
      { id: 'alarm_management', name: t('userManagement.permissions.alarmManagement') },
      { id: 'batch_control', name: t('userManagement.permissions.batchControl') }
    ]
  },
  {
    id: 'config',
    name: t('userManagement.permissions.config'),
    children: [
      { id: 'channel_config', name: t('userManagement.permissions.channelConfig') },
      { id: 'point_table', name: t('userManagement.permissions.pointTable') },
      { id: 'model_config', name: t('userManagement.permissions.modelConfig') }
    ]
  },
  {
    id: 'system',
    name: t('userManagement.permissions.system'),
    children: [
      { id: 'user_management', name: t('userManagement.permissions.userManagement') },
      { id: 'system_settings', name: t('userManagement.permissions.systemSettings') },
      { id: 'audit_logs', name: t('userManagement.permissions.auditLogs') }
    ]
  }
])

// 获取角色类型
const getRoleType = (role) => {
  const typeMap = {
    [ROLES.SUPER_ADMIN]: 'danger',
    [ROLES.SYSTEM_ADMIN]: 'warning',
    [ROLES.OPS_ENGINEER]: 'primary',
    [ROLES.MONITOR]: 'info',
    [ROLES.GUEST]: 'default'
  }
  return typeMap[role] || 'info'
}

// 格式化时间
const formatTime = (time) => {
  return time ? dayjs(time).format('YYYY-MM-DD HH:mm:ss') : '-'
}

// 搜索
const handleSearch = () => {
  currentPage.value = 1
  loadUsers()
}

// 重置搜索
const handleReset = () => {
  searchForm.username = ''
  searchForm.role = ''
  searchForm.status = ''
  handleSearch()
}

// 新增用户
const handleAdd = () => {
  isEdit.value = false
  resetForm()
  dialogVisible.value = true
}

// 编辑用户
const handleEdit = (row) => {
  isEdit.value = true
  Object.assign(userForm, {
    id: row.id,
    username: row.username,
    realName: row.realName,
    role: row.role,
    email: row.email,
    phone: row.phone,
    permissions: row.permissions || []
  })
  dialogVisible.value = true
}

// 删除用户
const handleDelete = async (row) => {
  try {
    await ElMessageBox.confirm(
      t('userManagement.deleteConfirm', { name: row.username }),
      t('common.warning'),
      {
        confirmButtonText: t('common.confirm'),
        cancelButtonText: t('common.cancel'),
        type: 'warning'
      }
    )
    
    // 模拟删除
    const index = userList.value.findIndex(u => u.id === row.id)
    if (index > -1) {
      userList.value.splice(index, 1)
      total.value--
    }
    
    ElMessage.success(t('userManagement.deleteSuccess'))
  } catch (error) {
    // 用户取消
  }
}

// 状态变更
const handleStatusChange = (row) => {
  const action = row.status === 'active' ? 
    t('userManagement.enable') : t('userManagement.disable')
  ElMessage.success(t('userManagement.statusChangeSuccess', { action, name: row.username }))
}

// 重置密码
const handleResetPassword = (row) => {
  resetPasswordDialog.value = { visible: true, userId: row.id }
  resetPasswordForm.password = ''
  resetPasswordForm.confirmPassword = ''
}

// 提交重置密码
const handleResetPasswordSubmit = async () => {
  try {
    await resetPasswordFormRef.value.validate()
    
    // 模拟重置密码
    ElMessage.success(t('userManagement.resetPasswordSuccess'))
    resetPasswordDialog.value.visible = false
  } catch (error) {
    // 验证失败
  }
}

// 选择变更
const handleSelectionChange = (val) => {
  selectedUsers.value = val
}

// 分页变更
const handleSizeChange = () => {
  currentPage.value = 1
  loadUsers()
}

const handlePageChange = () => {
  loadUsers()
}

// 提交表单
const handleSubmit = async () => {
  try {
    await userFormRef.value.validate()
    
    // 获取选中的权限
    const checkedKeys = permissionTreeRef.value.getCheckedKeys()
    userForm.permissions = checkedKeys
    
    if (isEdit.value) {
      // 更新用户
      const index = userList.value.findIndex(u => u.id === userForm.id)
      if (index > -1) {
        userList.value[index] = { ...userList.value[index], ...userForm }
      }
      ElMessage.success(t('userManagement.updateSuccess'))
    } else {
      // 新增用户
      const newUser = {
        ...userForm,
        id: Date.now(),
        status: 'active',
        lastLogin: null,
        createdAt: new Date()
      }
      userList.value.unshift(newUser)
      total.value++
      ElMessage.success(t('userManagement.createSuccess'))
    }
    
    dialogVisible.value = false
  } catch (error) {
    // 验证失败
  }
}

// 重置表单
const resetForm = () => {
  userForm.id = null
  userForm.username = ''
  userForm.password = ''
  userForm.realName = ''
  userForm.role = 'operator'
  userForm.email = ''
  userForm.phone = ''
  userForm.permissions = []
  
  userFormRef.value?.resetFields()
}

// 加载用户数据
const loadUsers = () => {
  loading.value = true
  
  // 模拟加载数据
  setTimeout(() => {
    const roles = ['operator', 'engineer', 'admin']
    const statuses = ['active', 'inactive']
    
    // 生成模拟数据
    const allUsers = [
      {
        id: 1,
        username: 'admin',
        realName: 'Administrator',
        role: 'admin',
        email: 'admin@voltage.com',
        phone: '13800138000',
        status: 'active',
        lastLogin: new Date('2024-01-03 09:00:00'),
        createdAt: new Date('2023-01-01'),
        permissions: ['monitoring', 'control', 'config', 'system']
      },
      {
        id: 2,
        username: 'engineer1',
        realName: 'John Engineer',
        role: 'engineer',
        email: 'engineer1@voltage.com',
        phone: '13800138001',
        status: 'active',
        lastLogin: new Date('2024-01-03 10:30:00'),
        createdAt: new Date('2023-06-15'),
        permissions: ['monitoring', 'control', 'config']
      },
      {
        id: 3,
        username: 'operator1',
        realName: 'Mike Operator',
        role: 'operator',
        email: 'operator1@voltage.com',
        phone: '13800138002',
        status: 'active',
        lastLogin: new Date('2024-01-03 14:00:00'),
        createdAt: new Date('2023-08-20'),
        permissions: ['monitoring']
      }
    ]
    
    // 生成更多模拟用户
    for (let i = 4; i <= 50; i++) {
      const role = roles[Math.floor(Math.random() * roles.length)]
      const status = statuses[Math.floor(Math.random() * statuses.length)]
      
      allUsers.push({
        id: i,
        username: `user${i}`,
        realName: `User ${i}`,
        role: role,
        email: `user${i}@voltage.com`,
        phone: `138${String(Math.floor(Math.random() * 100000000)).padStart(8, '0')}`,
        status: status,
        lastLogin: status === 'active' ? 
          new Date(Date.now() - Math.random() * 7 * 24 * 3600000) : null,
        createdAt: new Date(Date.now() - Math.random() * 365 * 24 * 3600000),
        permissions: role === 'admin' ? ['monitoring', 'control', 'config', 'system'] :
                     role === 'engineer' ? ['monitoring', 'control', 'config'] :
                     ['monitoring']
      })
    }
    
    // 筛选
    let filtered = allUsers
    if (searchForm.username) {
      filtered = filtered.filter(u => 
        u.username.toLowerCase().includes(searchForm.username.toLowerCase()) ||
        u.realName.toLowerCase().includes(searchForm.username.toLowerCase())
      )
    }
    if (searchForm.role) {
      filtered = filtered.filter(u => u.role === searchForm.role)
    }
    if (searchForm.status) {
      filtered = filtered.filter(u => u.status === searchForm.status)
    }
    
    // 分页
    total.value = filtered.length
    const start = (currentPage.value - 1) * pageSize.value
    const end = start + pageSize.value
    userList.value = filtered.slice(start, end)
    
    loading.value = false
  }, 500)
}

onMounted(() => {
  loadUsers()
})
</script>

<style lang="scss" scoped>
@import '@/styles/design-tokens.scss';

.user-management-container {
  padding: var(--page-padding);
  background: var(--color-background);
  min-height: 100vh;
}

/* Apple Style Page Header */
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--spacing-xxl);
  
  h1 {
    font-size: var(--font-size-4xl);
    font-weight: var(--font-weight-bold);
    color: var(--color-text-primary);
    margin: 0;
    letter-spacing: -0.02em;
    
    // Apple's distinctive text rendering
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }
}

.header-actions {
  display: flex;
  gap: var(--spacing-md);
}

/* Tesla Style Cards */
.search-card {
  margin-bottom: var(--spacing-xl);
  background: var(--color-surface);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  border: 1px solid var(--color-border);
  position: relative;
  overflow: hidden;
  transition: all 0.3s ease;
  
  // Tesla's signature gradient top bar
  &::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 3px;
    background: var(--gradient-primary);
    opacity: 0;
    transition: opacity 0.3s ease;
  }
  
  &:hover {
    transform: translateY(-2px);
    box-shadow: var(--shadow-lg);
    
    &::before {
      opacity: 1;
    }
  }
}

/* Main Content Card */
:deep(.el-card) {
  background: var(--color-surface);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  border: 1px solid var(--color-border);
  transition: all 0.3s ease;
  
  &:hover {
    box-shadow: var(--shadow-md);
  }
}

:deep(.el-card__body) {
  padding: var(--spacing-xl);
}

/* Enhanced Search Form */
:deep(.el-form) {
  .el-form-item {
    margin-bottom: var(--spacing-lg);
    
    &:last-child {
      margin-bottom: 0;
    }
  }
  
  .el-form-item__label {
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
  }
}

/* Enhanced Inputs */
:deep(.el-input__inner) {
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border);
  padding: var(--spacing-sm) var(--spacing-md);
  font-size: var(--font-size-base);
  transition: all 0.3s ease;
  
  &:hover {
    border-color: var(--color-border-hover);
  }
  
  &:focus {
    border-color: var(--color-primary);
    box-shadow: 0 0 0 3px var(--color-primary-light);
  }
}

/* Enhanced Select */
:deep(.el-select) {
  width: 100%;
  
  .el-input__inner {
    cursor: pointer;
  }
}

/* Enhanced Table */
:deep(.el-table) {
  background: transparent;
  
  .el-table__header-wrapper {
    th {
      background: var(--color-surface-hover);
      color: var(--color-text-secondary);
      font-weight: var(--font-weight-semibold);
      border-bottom: 2px solid var(--color-border);
    }
  }
  
  .el-table__row {
    transition: background-color 0.3s ease;
    
    &:hover {
      background-color: var(--color-surface-hover);
    }
  }
  
  .el-table__cell {
    border-bottom: 1px solid var(--color-border);
    padding: var(--spacing-md) var(--spacing-sm);
  }
}

/* Enhanced Tags */
:deep(.el-tag) {
  border-radius: var(--radius-full);
  padding: var(--spacing-xs) var(--spacing-md);
  font-weight: var(--font-weight-medium);
  border: none;
}

:deep(.el-tag--info) {
  background: linear-gradient(135deg, #e3f2fd, #bbdefb);
  color: #1565c0;
}

:deep(.el-tag--warning) {
  background: linear-gradient(135deg, #fff3e0, #ffe0b2);
  color: #e65100;
}

:deep(.el-tag--danger) {
  background: linear-gradient(135deg, #ffebee, #ffcdd2);
  color: #c62828;
}

/* Enhanced Switch */
:deep(.el-switch) {
  .el-switch__core {
    border-radius: var(--radius-full);
    background: var(--color-border);
  }
  
  &.is-checked .el-switch__core {
    background: var(--gradient-primary);
  }
}

/* Enhanced Buttons */
:deep(.el-button) {
  border-radius: var(--radius-md);
  font-weight: var(--font-weight-medium);
  padding: var(--spacing-sm) var(--spacing-lg);
  transition: all 0.3s ease;
  border: none;
  box-shadow: var(--shadow-sm);
}

:deep(.el-button--primary) {
  background: var(--gradient-primary);
  color: white;
  
  &:hover {
    transform: translateY(-1px);
    box-shadow: var(--shadow-md);
    opacity: 0.9;
  }
  
  &:active {
    transform: translateY(0);
  }
}

:deep(.el-button--default) {
  background: var(--color-surface);
  color: var(--color-text-primary);
  border: 1px solid var(--color-border);
  
  &:hover {
    background: var(--color-surface-hover);
    border-color: var(--color-primary);
  }
}

:deep(.el-button.is-link) {
  box-shadow: none;
  padding: 0;
  margin: 0 var(--spacing-sm);
  
  &:first-child {
    margin-left: 0;
  }
  
  &:hover {
    opacity: 0.7;
  }
}

/* Enhanced Pagination */
.pagination-container {
  margin-top: var(--spacing-xl);
  display: flex;
  justify-content: flex-end;
  padding: var(--spacing-md) 0;
}

:deep(.el-pagination) {
  .el-pager li {
    border-radius: var(--radius-md);
    margin: 0 var(--spacing-xs);
    font-weight: var(--font-weight-medium);
    transition: all 0.3s ease;
    
    &:hover {
      color: var(--color-primary);
    }
    
    &.active {
      background: var(--gradient-primary);
      color: white;
    }
  }
  
  .btn-prev, .btn-next {
    border-radius: var(--radius-md);
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    
    &:hover {
      border-color: var(--color-primary);
      color: var(--color-primary);
    }
  }
  
  .el-pagination__total,
  .el-pagination__jump {
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
  }
}

/* Enhanced Dialog */
:deep(.el-dialog) {
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-2xl);
  
  .el-dialog__header {
    padding: var(--spacing-xl);
    border-bottom: 1px solid var(--color-border);
    
    .el-dialog__title {
      font-size: var(--font-size-xl);
      font-weight: var(--font-weight-semibold);
      color: var(--color-text-primary);
      letter-spacing: -0.5px;
    }
  }
  
  .el-dialog__body {
    padding: var(--spacing-xl);
  }
  
  .el-dialog__footer {
    padding: var(--spacing-lg) var(--spacing-xl);
    border-top: 1px solid var(--color-border);
    background: var(--color-surface-hover);
  }
}

/* Enhanced Tree */
:deep(.el-tree) {
  background: transparent;
  
  .el-tree-node__content {
    padding: var(--spacing-sm) 0;
    border-radius: var(--radius-md);
    transition: all 0.3s ease;
    
    &:hover {
      background: var(--color-surface-hover);
    }
  }
  
  .el-checkbox__inner {
    border-radius: var(--radius-sm);
  }
  
  .el-checkbox__input.is-checked .el-checkbox__inner {
    background: var(--color-primary);
    border-color: var(--color-primary);
  }
}

/* Loading States */
:deep(.el-loading-mask) {
  background-color: rgba(255, 255, 255, 0.8);
  backdrop-filter: blur(5px);
}

/* Responsive Design */
@media (max-width: 768px) {
  .user-management-container {
    padding: var(--spacing-md);
  }
  
  .page-header {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--spacing-md);
    
    h1 {
      font-size: var(--font-size-2xl);
    }
  }
  
  .header-actions {
    width: 100%;
    
    .el-button {
      flex: 1;
    }
  }
  
  :deep(.el-table) {
    font-size: var(--font-size-sm);
  }
  
  :deep(.el-dialog) {
    width: 90% !important;
    margin: var(--spacing-md) !important;
  }
}

/* Icon Styling */
:deep(.el-icon) {
  margin-right: var(--spacing-xs);
}
</style>