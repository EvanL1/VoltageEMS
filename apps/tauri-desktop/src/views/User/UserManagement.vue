<template>
  <div class="user-management">
    <!-- User Statistics -->
    <el-row :gutter="20" class="user-stats">
      <el-col :span="6">
        <el-card>
          <el-statistic title="Total Users" :value="totalUsers">
            <template #prefix>
              <el-icon color="#409EFF"><User /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card>
          <el-statistic title="Active Users" :value="activeUsers">
            <template #prefix>
              <el-icon color="#67C23A"><UserFilled /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card>
          <el-statistic title="Total Roles" :value="totalRoles">
            <template #prefix>
              <el-icon color="#E6A23C"><Stamp /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
      
      <el-col :span="6">
        <el-card>
          <el-statistic title="Online Now" :value="onlineUsers">
            <template #prefix>
              <el-icon color="#67C23A"><Connection /></el-icon>
            </template>
          </el-statistic>
        </el-card>
      </el-col>
    </el-row>
    
    <!-- User Management Tabs -->
    <el-card class="user-management-card">
      <el-tabs v-model="activeTab">
        <!-- Users Tab -->
        <el-tab-pane label="Users" name="users">
          <div class="tab-toolbar">
            <el-button type="primary" @click="showAddUserDialog = true">
              <el-icon><Plus /></el-icon>
              Add User
            </el-button>
            
            <el-button @click="importUsers">
              <el-icon><Upload /></el-icon>
              Import
            </el-button>
            
            <el-button @click="exportUsers">
              <el-icon><Download /></el-icon>
              Export
            </el-button>
            
            <div style="flex: 1"></div>
            
            <el-input
              v-model="userSearch"
              placeholder="Search users..."
              :prefix-icon="Search"
              clearable
              style="width: 300px"
            />
          </div>
          
          <el-table :data="filteredUsers" style="width: 100%" v-loading="loading">
            <el-table-column type="selection" width="55" />
            <el-table-column prop="username" label="Username" width="150" />
            <el-table-column prop="fullName" label="Full Name" />
            <el-table-column prop="email" label="Email" />
            <el-table-column prop="role" label="Role" width="120">
              <template #default="{ row }">
                <el-tag>{{ row.role }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="status" label="Status" width="100">
              <template #default="{ row }">
                <el-switch
                  v-model="row.active"
                  @change="toggleUserStatus(row)"
                />
              </template>
            </el-table-column>
            <el-table-column prop="lastLogin" label="Last Login" width="180">
              <template #default="{ row }">
                {{ row.lastLogin ? formatTime(row.lastLogin) : 'Never' }}
              </template>
            </el-table-column>
            <el-table-column label="Actions" width="200" fixed="right">
              <template #default="{ row }">
                <el-button type="primary" size="small" text @click="editUser(row)">
                  Edit
                </el-button>
                <el-button type="info" size="small" text @click="resetPassword(row)">
                  Reset Password
                </el-button>
                <el-button type="danger" size="small" text @click="deleteUser(row)">
                  Delete
                </el-button>
              </template>
            </el-table-column>
          </el-table>
          
          <el-pagination
            v-model:current-page="userPage"
            v-model:page-size="userPageSize"
            :page-sizes="[10, 20, 50, 100]"
            :total="users.length"
            layout="total, sizes, prev, pager, next, jumper"
            style="margin-top: 20px"
          />
        </el-tab-pane>
        
        <!-- Roles Tab -->
        <el-tab-pane label="Roles" name="roles">
          <div class="tab-toolbar">
            <el-button type="primary" @click="showAddRoleDialog = true">
              <el-icon><Plus /></el-icon>
              Add Role
            </el-button>
          </div>
          
          <el-table :data="roles" style="width: 100%">
            <el-table-column prop="name" label="Role Name" width="150" />
            <el-table-column prop="description" label="Description" />
            <el-table-column prop="userCount" label="Users" width="100" />
            <el-table-column prop="createdAt" label="Created" width="180">
              <template #default="{ row }">
                {{ formatTime(row.createdAt) }}
              </template>
            </el-table-column>
            <el-table-column label="Actions" width="150" fixed="right">
              <template #default="{ row }">
                <el-button type="primary" size="small" text @click="editRole(row)">
                  Edit
                </el-button>
                <el-button 
                  type="danger" 
                  size="small" 
                  text 
                  @click="deleteRole(row)"
                  :disabled="row.system"
                >
                  Delete
                </el-button>
              </template>
            </el-table-column>
          </el-table>
        </el-tab-pane>
        
        <!-- Permissions Tab -->
        <el-tab-pane label="Permissions" name="permissions">
          <div class="permissions-matrix">
            <el-table :data="permissionMatrix" style="width: 100%" border>
              <el-table-column prop="module" label="Module" width="200" fixed />
              <el-table-column
                v-for="role in roles"
                :key="role.id"
                :label="role.name"
                width="120"
                align="center"
              >
                <template #default="{ row }">
                  <el-checkbox-group v-model="row.permissions[role.id]">
                    <el-checkbox label="read" />
                    <el-checkbox label="write" />
                    <el-checkbox label="delete" />
                  </el-checkbox-group>
                </template>
              </el-table-column>
            </el-table>
            
            <el-button type="primary" @click="savePermissions" style="margin-top: 20px">
              Save Permissions
            </el-button>
          </div>
        </el-tab-pane>
        
        <!-- Activity Log Tab -->
        <el-tab-pane label="Activity Log" name="activity">
          <div class="tab-toolbar">
            <el-date-picker
              v-model="activityDateRange"
              type="datetimerange"
              range-separator="to"
              start-placeholder="Start date"
              end-placeholder="End date"
              format="YYYY-MM-DD HH:mm:ss"
              value-format="YYYY-MM-DD HH:mm:ss"
            />
            
            <el-select v-model="activityUser" placeholder="All Users" clearable style="width: 200px">
              <el-option
                v-for="user in users"
                :key="user.id"
                :label="user.username"
                :value="user.id"
              />
            </el-select>
            
            <el-button @click="refreshActivity">
              <el-icon><Refresh /></el-icon>
              Refresh
            </el-button>
          </div>
          
          <el-table :data="activities" style="width: 100%">
            <el-table-column prop="timestamp" label="Time" width="180">
              <template #default="{ row }">
                {{ formatTime(row.timestamp) }}
              </template>
            </el-table-column>
            <el-table-column prop="user" label="User" width="120" />
            <el-table-column prop="action" label="Action" width="150">
              <template #default="{ row }">
                <el-tag :type="getActionType(row.action)">{{ row.action }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="resource" label="Resource" />
            <el-table-column prop="ip" label="IP Address" width="150" />
            <el-table-column prop="result" label="Result" width="100">
              <template #default="{ row }">
                <el-tag :type="row.result === 'success' ? 'success' : 'danger'">
                  {{ row.result }}
                </el-tag>
              </template>
            </el-table-column>
          </el-table>
        </el-tab-pane>
      </el-tabs>
    </el-card>
    
    <!-- Add/Edit User Dialog -->
    <el-dialog
      v-model="showAddUserDialog"
      :title="editingUser ? 'Edit User' : 'Add New User'"
      width="600px"
    >
      <el-form :model="userForm" :rules="userRules" ref="userFormRef" label-width="120px">
        <el-form-item label="Username" prop="username">
          <el-input v-model="userForm.username" :disabled="editingUser" />
        </el-form-item>
        
        <el-form-item label="Full Name" prop="fullName">
          <el-input v-model="userForm.fullName" />
        </el-form-item>
        
        <el-form-item label="Email" prop="email">
          <el-input v-model="userForm.email" type="email" />
        </el-form-item>
        
        <el-form-item label="Phone">
          <el-input v-model="userForm.phone" />
        </el-form-item>
        
        <el-form-item label="Role" prop="role">
          <el-select v-model="userForm.role" placeholder="Select role">
            <el-option
              v-for="role in roles"
              :key="role.id"
              :label="role.name"
              :value="role.name"
            />
          </el-select>
        </el-form-item>
        
        <el-form-item label="Password" prop="password" v-if="!editingUser">
          <el-input v-model="userForm.password" type="password" show-password />
        </el-form-item>
        
        <el-form-item label="Confirm Password" prop="confirmPassword" v-if="!editingUser">
          <el-input v-model="userForm.confirmPassword" type="password" show-password />
        </el-form-item>
        
        <el-form-item label="Active">
          <el-switch v-model="userForm.active" />
        </el-form-item>
        
        <el-form-item label="Permissions">
          <el-checkbox-group v-model="userForm.permissions">
            <el-checkbox label="realtime_view">View Realtime Data</el-checkbox>
            <el-checkbox label="history_view">View History Data</el-checkbox>
            <el-checkbox label="control_execute">Execute Controls</el-checkbox>
            <el-checkbox label="config_edit">Edit Configuration</el-checkbox>
            <el-checkbox label="user_manage">Manage Users</el-checkbox>
          </el-checkbox-group>
        </el-form-item>
      </el-form>
      
      <template #footer>
        <el-button @click="cancelUserEdit">Cancel</el-button>
        <el-button type="primary" @click="saveUser">Save</el-button>
      </template>
    </el-dialog>
    
    <!-- Add/Edit Role Dialog -->
    <el-dialog
      v-model="showAddRoleDialog"
      :title="editingRole ? 'Edit Role' : 'Add New Role'"
      width="600px"
    >
      <el-form :model="roleForm" :rules="roleRules" ref="roleFormRef" label-width="120px">
        <el-form-item label="Role Name" prop="name">
          <el-input v-model="roleForm.name" :disabled="editingRole?.system" />
        </el-form-item>
        
        <el-form-item label="Description" prop="description">
          <el-input v-model="roleForm.description" type="textarea" :rows="3" />
        </el-form-item>
        
        <el-form-item label="Base Permissions">
          <el-tree
            :data="permissionTree"
            show-checkbox
            node-key="id"
            :default-checked-keys="roleForm.permissions"
            ref="permissionTreeRef"
          />
        </el-form-item>
      </el-form>
      
      <template #footer>
        <el-button @click="cancelRoleEdit">Cancel</el-button>
        <el-button type="primary" @click="saveRole">Save</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, reactive } from 'vue'
import {
  User,
  UserFilled,
  Stamp,
  Connection,
  Plus,
  Upload,
  Download,
  Search,
  Refresh
} from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import dayjs from 'dayjs'

// Mock data
const users = ref([
  {
    id: 1,
    username: 'admin',
    fullName: 'System Administrator',
    email: 'admin@voltageems.com',
    phone: '+1 234 567 8900',
    role: 'Administrator',
    active: true,
    lastLogin: dayjs().subtract(1, 'hour').toISOString(),
    createdAt: dayjs().subtract(1, 'year').toISOString()
  },
  {
    id: 2,
    username: 'operator1',
    fullName: 'John Doe',
    email: 'john.doe@voltageems.com',
    phone: '+1 234 567 8901',
    role: 'Operator',
    active: true,
    lastLogin: dayjs().subtract(2, 'hour').toISOString(),
    createdAt: dayjs().subtract(6, 'month').toISOString()
  },
  {
    id: 3,
    username: 'viewer1',
    fullName: 'Jane Smith',
    email: 'jane.smith@voltageems.com',
    phone: '+1 234 567 8902',
    role: 'Viewer',
    active: true,
    lastLogin: dayjs().subtract(1, 'day').toISOString(),
    createdAt: dayjs().subtract(3, 'month').toISOString()
  },
  {
    id: 4,
    username: 'engineer1',
    fullName: 'Bob Johnson',
    email: 'bob.johnson@voltageems.com',
    phone: '+1 234 567 8903',
    role: 'Engineer',
    active: false,
    lastLogin: dayjs().subtract(1, 'week').toISOString(),
    createdAt: dayjs().subtract(2, 'month').toISOString()
  }
])

const roles = ref([
  {
    id: 1,
    name: 'Administrator',
    description: 'Full system access',
    userCount: 1,
    system: true,
    createdAt: dayjs().subtract(1, 'year').toISOString()
  },
  {
    id: 2,
    name: 'Operator',
    description: 'Can view and control devices',
    userCount: 2,
    system: false,
    createdAt: dayjs().subtract(6, 'month').toISOString()
  },
  {
    id: 3,
    name: 'Engineer',
    description: 'Can configure system and rules',
    userCount: 1,
    system: false,
    createdAt: dayjs().subtract(3, 'month').toISOString()
  },
  {
    id: 4,
    name: 'Viewer',
    description: 'Read-only access',
    userCount: 3,
    system: false,
    createdAt: dayjs().subtract(1, 'month').toISOString()
  }
])

const activities = ref([
  {
    timestamp: dayjs().subtract(5, 'minute').toISOString(),
    user: 'admin',
    action: 'login',
    resource: 'System',
    ip: '192.168.1.100',
    result: 'success'
  },
  {
    timestamp: dayjs().subtract(10, 'minute').toISOString(),
    user: 'operator1',
    action: 'control',
    resource: 'Channel 1 - Point 30001',
    ip: '192.168.1.101',
    result: 'success'
  },
  {
    timestamp: dayjs().subtract(30, 'minute').toISOString(),
    user: 'engineer1',
    action: 'config_change',
    resource: 'Rule Engine - RULE_001',
    ip: '192.168.1.102',
    result: 'success'
  },
  {
    timestamp: dayjs().subtract(1, 'hour').toISOString(),
    user: 'viewer1',
    action: 'export',
    resource: 'Historical Data',
    ip: '192.168.1.103',
    result: 'success'
  }
])

// Statistics
const totalUsers = computed(() => users.value.length)
const activeUsers = computed(() => users.value.filter(u => u.active).length)
const totalRoles = computed(() => roles.value.length)
const onlineUsers = ref(2)

// State
const activeTab = ref('users')
const loading = ref(false)
const userSearch = ref('')
const userPage = ref(1)
const userPageSize = ref(20)

// User dialog
const showAddUserDialog = ref(false)
const editingUser = ref<any>(null)
const userFormRef = ref<FormInstance>()
const userForm = reactive({
  username: '',
  fullName: '',
  email: '',
  phone: '',
  role: '',
  password: '',
  confirmPassword: '',
  active: true,
  permissions: []
})

const userRules = reactive<FormRules>({
  username: [
    { required: true, message: 'Please enter username', trigger: 'blur' },
    { min: 3, max: 20, message: 'Length should be 3 to 20', trigger: 'blur' }
  ],
  fullName: [
    { required: true, message: 'Please enter full name', trigger: 'blur' }
  ],
  email: [
    { required: true, message: 'Please enter email', trigger: 'blur' },
    { type: 'email', message: 'Please enter valid email', trigger: 'blur' }
  ],
  role: [
    { required: true, message: 'Please select role', trigger: 'change' }
  ],
  password: [
    { required: true, message: 'Please enter password', trigger: 'blur' },
    { min: 8, message: 'Password must be at least 8 characters', trigger: 'blur' }
  ],
  confirmPassword: [
    { required: true, message: 'Please confirm password', trigger: 'blur' },
    {
      validator: (rule: any, value: any, callback: any) => {
        if (value !== userForm.password) {
          callback(new Error('Passwords do not match'))
        } else {
          callback()
        }
      },
      trigger: 'blur'
    }
  ]
})

// Role dialog
const showAddRoleDialog = ref(false)
const editingRole = ref<any>(null)
const roleFormRef = ref<FormInstance>()
const roleForm = reactive({
  name: '',
  description: '',
  permissions: []
})

const roleRules = reactive<FormRules>({
  name: [
    { required: true, message: 'Please enter role name', trigger: 'blur' }
  ],
  description: [
    { required: true, message: 'Please enter description', trigger: 'blur' }
  ]
})

// Permissions
const permissionTree = ref([
  {
    id: 'realtime',
    label: 'Realtime Data',
    children: [
      { id: 'realtime_view', label: 'View' },
      { id: 'realtime_subscribe', label: 'Subscribe' }
    ]
  },
  {
    id: 'history',
    label: 'Historical Data',
    children: [
      { id: 'history_view', label: 'View' },
      { id: 'history_export', label: 'Export' }
    ]
  },
  {
    id: 'control',
    label: 'Device Control',
    children: [
      { id: 'control_view', label: 'View' },
      { id: 'control_execute', label: 'Execute' }
    ]
  },
  {
    id: 'config',
    label: 'Configuration',
    children: [
      { id: 'config_view', label: 'View' },
      { id: 'config_edit', label: 'Edit' }
    ]
  },
  {
    id: 'user',
    label: 'User Management',
    children: [
      { id: 'user_view', label: 'View' },
      { id: 'user_manage', label: 'Manage' }
    ]
  }
])

const permissionMatrix = ref([
  {
    module: 'Realtime Data',
    permissions: {
      1: ['read', 'write'],
      2: ['read'],
      3: ['read', 'write'],
      4: ['read']
    }
  },
  {
    module: 'Historical Data',
    permissions: {
      1: ['read', 'write', 'delete'],
      2: ['read'],
      3: ['read', 'write'],
      4: ['read']
    }
  },
  {
    module: 'Device Control',
    permissions: {
      1: ['read', 'write'],
      2: ['read', 'write'],
      3: ['read'],
      4: []
    }
  },
  {
    module: 'Configuration',
    permissions: {
      1: ['read', 'write', 'delete'],
      2: [],
      3: ['read', 'write'],
      4: []
    }
  },
  {
    module: 'User Management',
    permissions: {
      1: ['read', 'write', 'delete'],
      2: [],
      3: [],
      4: []
    }
  }
])

// Activity
const activityDateRange = ref<string[]>([])
const activityUser = ref('')

// Computed
const filteredUsers = computed(() => {
  if (!userSearch.value) return users.value
  
  const search = userSearch.value.toLowerCase()
  return users.value.filter(user => 
    user.username.toLowerCase().includes(search) ||
    user.fullName.toLowerCase().includes(search) ||
    user.email.toLowerCase().includes(search)
  )
})

// Methods
function toggleUserStatus(user: any) {
  ElMessage.success(`User ${user.active ? 'activated' : 'deactivated'}`)
}

function editUser(user: any) {
  editingUser.value = user
  Object.assign(userForm, {
    ...user,
    password: '',
    confirmPassword: '',
    permissions: []
  })
  showAddUserDialog.value = true
}

async function resetPassword(user: any) {
  await ElMessageBox.confirm(
    `Reset password for ${user.username}?`,
    'Confirm Reset',
    {
      confirmButtonText: 'Reset',
      cancelButtonText: 'Cancel',
      type: 'warning'
    }
  )
  
  ElMessage.success('Password reset email sent')
}

async function deleteUser(user: any) {
  await ElMessageBox.confirm(
    `Delete user ${user.username}?`,
    'Confirm Delete',
    {
      confirmButtonText: 'Delete',
      cancelButtonText: 'Cancel',
      type: 'warning'
    }
  )
  
  const index = users.value.findIndex(u => u.id === user.id)
  if (index > -1) {
    users.value.splice(index, 1)
    ElMessage.success('User deleted')
  }
}

function cancelUserEdit() {
  showAddUserDialog.value = false
  editingUser.value = null
  userFormRef.value?.resetFields()
}

async function saveUser() {
  const valid = await userFormRef.value?.validate()
  if (!valid) return
  
  if (editingUser.value) {
    // Update existing user
    const index = users.value.findIndex(u => u.id === editingUser.value.id)
    if (index > -1) {
      users.value[index] = {
        ...users.value[index],
        ...userForm
      }
    }
    ElMessage.success('User updated')
  } else {
    // Add new user
    users.value.push({
      id: Date.now(),
      ...userForm,
      lastLogin: null,
      createdAt: dayjs().toISOString()
    })
    ElMessage.success('User created')
  }
  
  cancelUserEdit()
}

function editRole(role: any) {
  editingRole.value = role
  Object.assign(roleForm, {
    name: role.name,
    description: role.description,
    permissions: []
  })
  showAddRoleDialog.value = true
}

async function deleteRole(role: any) {
  await ElMessageBox.confirm(
    `Delete role ${role.name}?`,
    'Confirm Delete',
    {
      confirmButtonText: 'Delete',
      cancelButtonText: 'Cancel',
      type: 'warning'
    }
  )
  
  const index = roles.value.findIndex(r => r.id === role.id)
  if (index > -1) {
    roles.value.splice(index, 1)
    ElMessage.success('Role deleted')
  }
}

function cancelRoleEdit() {
  showAddRoleDialog.value = false
  editingRole.value = null
  roleFormRef.value?.resetFields()
}

async function saveRole() {
  const valid = await roleFormRef.value?.validate()
  if (!valid) return
  
  if (editingRole.value) {
    // Update existing role
    const index = roles.value.findIndex(r => r.id === editingRole.value.id)
    if (index > -1) {
      roles.value[index] = {
        ...roles.value[index],
        ...roleForm
      }
    }
    ElMessage.success('Role updated')
  } else {
    // Add new role
    roles.value.push({
      id: Date.now(),
      ...roleForm,
      userCount: 0,
      system: false,
      createdAt: dayjs().toISOString()
    })
    ElMessage.success('Role created')
  }
  
  cancelRoleEdit()
}

function savePermissions() {
  ElMessage.success('Permissions saved')
}

function refreshActivity() {
  // TODO: Fetch activity log from API
  ElMessage.success('Activity log refreshed')
}

function importUsers() {
  // TODO: Implement import
  ElMessage.info('Import users feature coming soon')
}

function exportUsers() {
  // TODO: Implement export
  ElMessage.success('Exporting users...')
}

// Utility
function formatTime(timestamp: string) {
  return dayjs(timestamp).format('YYYY-MM-DD HH:mm:ss')
}

function getActionType(action: string) {
  switch (action) {
    case 'login':
    case 'logout':
      return 'info'
    case 'control':
      return 'warning'
    case 'config_change':
      return 'danger'
    case 'export':
      return 'success'
    default:
      return ''
  }
}
</script>

<style lang="scss" scoped>
.user-management {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 20px;
  
  .user-stats {
    :deep(.el-statistic__number) {
      font-size: 28px;
    }
  }
  
  .user-management-card {
    flex: 1;
    
    .tab-toolbar {
      display: flex;
      align-items: center;
      gap: 10px;
      margin-bottom: 20px;
    }
    
    .permissions-matrix {
      :deep(.el-checkbox-group) {
        display: flex;
        flex-direction: column;
        gap: 5px;
        
        .el-checkbox {
          margin-right: 0;
        }
      }
    }
  }
}
</style>