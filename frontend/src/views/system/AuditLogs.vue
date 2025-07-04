<template>
  <div class="audit-logs-container">
    <!-- 页面头部 -->
    <div class="page-header">
      <h1>{{ $t('auditLogs.title') }}</h1>
      <div class="header-actions">
        <el-button @click="handleExport" v-permission="['admin']">
          <el-icon><Download /></el-icon>
          {{ $t('auditLogs.export') }}
        </el-button>
        <el-button type="primary" @click="handleRefresh">
          <el-icon><Refresh /></el-icon>
          {{ $t('common.refresh') }}
        </el-button>
      </div>
    </div>

    <!-- 搜索栏 -->
    <el-card class="search-card">
      <el-form :inline="true" :model="searchForm">
        <el-form-item :label="$t('auditLogs.user')">
          <el-input 
            v-model="searchForm.user" 
            :placeholder="$t('auditLogs.enterUser')"
            clearable
            @keyup.enter="handleSearch"
          />
        </el-form-item>
        <el-form-item :label="$t('auditLogs.module')">
          <el-select 
            v-model="searchForm.module" 
            :placeholder="$t('auditLogs.selectModule')"
            clearable
          >
            <el-option :label="$t('auditLogs.modules.all')" value="" />
            <el-option :label="$t('auditLogs.modules.auth')" value="auth" />
            <el-option :label="$t('auditLogs.modules.config')" value="config" />
            <el-option :label="$t('auditLogs.modules.control')" value="control" />
            <el-option :label="$t('auditLogs.modules.monitoring')" value="monitoring" />
            <el-option :label="$t('auditLogs.modules.system')" value="system" />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('auditLogs.action')">
          <el-select 
            v-model="searchForm.action" 
            :placeholder="$t('auditLogs.selectAction')"
            clearable
          >
            <el-option :label="$t('auditLogs.actions.all')" value="" />
            <el-option :label="$t('auditLogs.actions.create')" value="create" />
            <el-option :label="$t('auditLogs.actions.update')" value="update" />
            <el-option :label="$t('auditLogs.actions.delete')" value="delete" />
            <el-option :label="$t('auditLogs.actions.login')" value="login" />
            <el-option :label="$t('auditLogs.actions.logout')" value="logout" />
            <el-option :label="$t('auditLogs.actions.control')" value="control" />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('auditLogs.result')">
          <el-select 
            v-model="searchForm.result" 
            :placeholder="$t('auditLogs.selectResult')"
            clearable
          >
            <el-option :label="$t('auditLogs.results.all')" value="" />
            <el-option :label="$t('auditLogs.results.success')" value="success" />
            <el-option :label="$t('auditLogs.results.failed')" value="failed" />
          </el-select>
        </el-form-item>
        <el-form-item :label="$t('common.timeRange')">
          <el-date-picker
            v-model="searchForm.timeRange"
            type="datetimerange"
            :shortcuts="timeShortcuts"
            :range-separator="$t('common.to')"
            :start-placeholder="$t('common.startTime')"
            :end-placeholder="$t('common.endTime')"
            format="YYYY-MM-DD HH:mm:ss"
            value-format="YYYY-MM-DD HH:mm:ss"
          />
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

    <!-- 统计信息 -->
    <el-row :gutter="20" class="stats-row">
      <el-col :xs="24" :sm="12" :md="6">
        <el-card class="stat-card">
          <div class="stat-item">
            <div class="stat-icon" style="background-color: #409eff;">
              <el-icon><Document /></el-icon>
            </div>
            <div class="stat-content">
              <div class="stat-value">{{ stats.total.toLocaleString() }}</div>
              <div class="stat-label">{{ $t('auditLogs.totalLogs') }}</div>
            </div>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="24" :sm="12" :md="6">
        <el-card class="stat-card">
          <div class="stat-item">
            <div class="stat-icon" style="background-color: #67c23a;">
              <el-icon><SuccessFilled /></el-icon>
            </div>
            <div class="stat-content">
              <div class="stat-value">{{ stats.success.toLocaleString() }}</div>
              <div class="stat-label">{{ $t('auditLogs.successOperations') }}</div>
            </div>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="24" :sm="12" :md="6">
        <el-card class="stat-card">
          <div class="stat-item">
            <div class="stat-icon" style="background-color: #f56c6c;">
              <el-icon><CircleCloseFilled /></el-icon>
            </div>
            <div class="stat-content">
              <div class="stat-value">{{ stats.failed.toLocaleString() }}</div>
              <div class="stat-label">{{ $t('auditLogs.failedOperations') }}</div>
            </div>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="24" :sm="12" :md="6">
        <el-card class="stat-card">
          <div class="stat-item">
            <div class="stat-icon" style="background-color: #e6a23c;">
              <el-icon><User /></el-icon>
            </div>
            <div class="stat-content">
              <div class="stat-value">{{ stats.uniqueUsers }}</div>
              <div class="stat-label">{{ $t('auditLogs.activeUsers') }}</div>
            </div>
          </div>
        </el-card>
      </el-col>
    </el-row>

    <!-- 日志列表 -->
    <el-card>
      <el-table 
        :data="logList" 
        stripe 
        v-loading="loading"
        @row-click="handleRowClick"
        style="cursor: pointer;"
      >
        <el-table-column prop="timestamp" :label="$t('auditLogs.timestamp')" width="180">
          <template #default="{ row }">
            {{ formatTime(row.timestamp) }}
          </template>
        </el-table-column>
        <el-table-column prop="user" :label="$t('auditLogs.user')" width="120" />
        <el-table-column prop="ip" :label="$t('auditLogs.ipAddress')" width="140" />
        <el-table-column prop="module" :label="$t('auditLogs.module')" width="120">
          <template #default="{ row }">
            <el-tag size="small">{{ getModuleLabel(row.module) }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="action" :label="$t('auditLogs.action')" width="120">
          <template #default="{ row }">
            <el-tag :type="getActionType(row.action)" size="small">
              {{ getActionLabel(row.action) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="target" :label="$t('auditLogs.target')" min-width="200" show-overflow-tooltip />
        <el-table-column prop="result" :label="$t('auditLogs.result')" width="100">
          <template #default="{ row }">
            <el-tag :type="row.result === 'success' ? 'success' : 'danger'" size="small">
              {{ row.result === 'success' ? $t('auditLogs.results.success') : $t('auditLogs.results.failed') }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="duration" :label="$t('auditLogs.duration')" width="100">
          <template #default="{ row }">
            {{ row.duration }}ms
          </template>
        </el-table-column>
      </el-table>

      <!-- 分页 -->
      <div class="pagination-container">
        <el-pagination
          v-model:current-page="currentPage"
          v-model:page-size="pageSize"
          :page-sizes="[20, 50, 100, 200]"
          :total="total"
          layout="total, sizes, prev, pager, next, jumper"
          @size-change="handleSizeChange"
          @current-change="handlePageChange"
        />
      </div>
    </el-card>

    <!-- 详情对话框 -->
    <el-dialog 
      v-model="detailDialog.visible" 
      :title="$t('auditLogs.logDetail')"
      width="800px"
    >
      <el-descriptions :column="2" border>
        <el-descriptions-item :label="$t('auditLogs.timestamp')">
          {{ formatTime(detailDialog.data.timestamp) }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('auditLogs.user')">
          {{ detailDialog.data.user }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('auditLogs.ipAddress')">
          {{ detailDialog.data.ip }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('auditLogs.userAgent')">
          {{ detailDialog.data.userAgent }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('auditLogs.module')">
          <el-tag size="small">{{ getModuleLabel(detailDialog.data.module) }}</el-tag>
        </el-descriptions-item>
        <el-descriptions-item :label="$t('auditLogs.action')">
          <el-tag :type="getActionType(detailDialog.data.action)" size="small">
            {{ getActionLabel(detailDialog.data.action) }}
          </el-tag>
        </el-descriptions-item>
        <el-descriptions-item :label="$t('auditLogs.target')" :span="2">
          {{ detailDialog.data.target }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('auditLogs.result')">
          <el-tag :type="detailDialog.data.result === 'success' ? 'success' : 'danger'" size="small">
            {{ detailDialog.data.result === 'success' ? $t('auditLogs.results.success') : $t('auditLogs.results.failed') }}
          </el-tag>
        </el-descriptions-item>
        <el-descriptions-item :label="$t('auditLogs.duration')">
          {{ detailDialog.data.duration }}ms
        </el-descriptions-item>
        <el-descriptions-item :label="$t('auditLogs.requestMethod')">
          <el-tag>{{ detailDialog.data.method }}</el-tag>
        </el-descriptions-item>
        <el-descriptions-item :label="$t('auditLogs.requestUrl')">
          {{ detailDialog.data.url }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('auditLogs.requestParams')" :span="2">
          <pre class="json-display">{{ formatJson(detailDialog.data.params) }}</pre>
        </el-descriptions-item>
        <el-descriptions-item :label="$t('auditLogs.responseData')" :span="2" v-if="detailDialog.data.response">
          <pre class="json-display">{{ formatJson(detailDialog.data.response) }}</pre>
        </el-descriptions-item>
        <el-descriptions-item :label="$t('auditLogs.errorMessage')" :span="2" v-if="detailDialog.data.error">
          <el-alert :title="detailDialog.data.error" type="error" :closable="false" />
        </el-descriptions-item>
      </el-descriptions>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { 
  Download, Refresh, Search, Document, User,
  SuccessFilled, CircleCloseFilled
} from '@element-plus/icons-vue'
import dayjs from 'dayjs'

const { t } = useI18n()

// 搜索表单
const searchForm = reactive({
  user: '',
  module: '',
  action: '',
  result: '',
  timeRange: []
})

// 时间快捷选项
const timeShortcuts = [
  {
    text: t('auditLogs.today'),
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setHours(0, 0, 0, 0)
      return [start, end]
    }
  },
  {
    text: t('auditLogs.yesterday'),
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setDate(start.getDate() - 1)
      start.setHours(0, 0, 0, 0)
      end.setDate(end.getDate() - 1)
      end.setHours(23, 59, 59, 999)
      return [start, end]
    }
  },
  {
    text: t('auditLogs.lastWeek'),
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setDate(start.getDate() - 7)
      return [start, end]
    }
  },
  {
    text: t('auditLogs.lastMonth'),
    value: () => {
      const end = new Date()
      const start = new Date()
      start.setMonth(start.getMonth() - 1)
      return [start, end]
    }
  }
]

// 统计信息
const stats = reactive({
  total: 0,
  success: 0,
  failed: 0,
  uniqueUsers: 0
})

// 日志列表
const logList = ref([])
const loading = ref(false)
const total = ref(0)
const currentPage = ref(1)
const pageSize = ref(50)

// 详情对话框
const detailDialog = ref({
  visible: false,
  data: {}
})

// 格式化时间
const formatTime = (time) => {
  return dayjs(time).format('YYYY-MM-DD HH:mm:ss')
}

// 格式化JSON
const formatJson = (data) => {
  if (!data) return ''
  if (typeof data === 'string') {
    try {
      data = JSON.parse(data)
    } catch (e) {
      return data
    }
  }
  return JSON.stringify(data, null, 2)
}

// 获取模块标签
const getModuleLabel = (module) => {
  const labels = {
    auth: t('auditLogs.modules.auth'),
    config: t('auditLogs.modules.config'),
    control: t('auditLogs.modules.control'),
    monitoring: t('auditLogs.modules.monitoring'),
    system: t('auditLogs.modules.system')
  }
  return labels[module] || module
}

// 获取操作标签
const getActionLabel = (action) => {
  const labels = {
    create: t('auditLogs.actions.create'),
    update: t('auditLogs.actions.update'),
    delete: t('auditLogs.actions.delete'),
    login: t('auditLogs.actions.login'),
    logout: t('auditLogs.actions.logout'),
    control: t('auditLogs.actions.control')
  }
  return labels[action] || action
}

// 获取操作类型
const getActionType = (action) => {
  const typeMap = {
    create: 'success',
    update: 'warning',
    delete: 'danger',
    login: 'primary',
    logout: 'info',
    control: 'warning'
  }
  return typeMap[action] || 'info'
}

// 搜索
const handleSearch = () => {
  currentPage.value = 1
  loadLogs()
}

// 重置搜索
const handleReset = () => {
  searchForm.user = ''
  searchForm.module = ''
  searchForm.action = ''
  searchForm.result = ''
  searchForm.timeRange = []
  handleSearch()
}

// 刷新
const handleRefresh = () => {
  loadLogs()
  ElMessage.success(t('common.refreshSuccess'))
}

// 导出
const handleExport = async () => {
  try {
    await ElMessageBox.confirm(
      t('auditLogs.exportConfirm'),
      t('common.confirm'),
      {
        confirmButtonText: t('common.confirm'),
        cancelButtonText: t('common.cancel'),
        type: 'info'
      }
    )
    
    ElMessage.info(t('auditLogs.exporting'))
    // 模拟导出
    setTimeout(() => {
      const link = document.createElement('a')
      link.href = '#'
      link.download = `audit_logs_${dayjs().format('YYYYMMDD_HHmmss')}.csv`
      link.click()
      ElMessage.success(t('auditLogs.exportSuccess'))
    }, 2000)
  } catch (error) {
    // 用户取消
  }
}

// 查看详情
const handleRowClick = (row) => {
  detailDialog.value = {
    visible: true,
    data: { ...row }
  }
}

// 分页变更
const handleSizeChange = () => {
  currentPage.value = 1
  loadLogs()
}

const handlePageChange = () => {
  loadLogs()
}

// 加载日志数据
const loadLogs = () => {
  loading.value = true
  
  // 模拟加载数据
  setTimeout(() => {
    // 生成模拟数据
    const modules = ['auth', 'config', 'control', 'monitoring', 'system']
    const actions = ['create', 'update', 'delete', 'login', 'logout', 'control']
    const users = ['admin', 'engineer1', 'operator1', 'engineer2', 'operator2']
    const targets = [
      'Channel: Modbus_TCP_Demo',
      'Point: YC001',
      'Model: Energy_Calculation',
      'Device: PLC_01',
      'User: operator1',
      'Alarm Rule: High_Temperature',
      'Storage Policy: Realtime_Data',
      'System Settings'
    ]
    
    const allLogs = []
    const now = Date.now()
    
    // 生成1000条模拟日志
    for (let i = 0; i < 1000; i++) {
      const module = modules[Math.floor(Math.random() * modules.length)]
      const action = actions[Math.floor(Math.random() * actions.length)]
      const result = Math.random() > 0.1 ? 'success' : 'failed'
      
      allLogs.push({
        id: i + 1,
        timestamp: new Date(now - Math.random() * 30 * 24 * 3600000), // 过去30天内
        user: users[Math.floor(Math.random() * users.length)],
        ip: `192.168.1.${Math.floor(Math.random() * 255)}`,
        userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/96.0',
        module: module,
        action: action,
        target: targets[Math.floor(Math.random() * targets.length)],
        result: result,
        duration: Math.floor(Math.random() * 1000) + 50,
        method: ['GET', 'POST', 'PUT', 'DELETE'][Math.floor(Math.random() * 4)],
        url: `/api/${module}/${action}`,
        params: {
          id: Math.floor(Math.random() * 100),
          name: `test_${i}`,
          value: Math.random() * 100
        },
        response: result === 'success' ? { code: 200, message: 'Success' } : null,
        error: result === 'failed' ? 'Permission denied or resource not found' : null
      })
    }
    
    // 筛选
    let filtered = allLogs
    if (searchForm.user) {
      filtered = filtered.filter(log => 
        log.user.toLowerCase().includes(searchForm.user.toLowerCase())
      )
    }
    if (searchForm.module) {
      filtered = filtered.filter(log => log.module === searchForm.module)
    }
    if (searchForm.action) {
      filtered = filtered.filter(log => log.action === searchForm.action)
    }
    if (searchForm.result) {
      filtered = filtered.filter(log => log.result === searchForm.result)
    }
    if (searchForm.timeRange && searchForm.timeRange.length === 2) {
      const [start, end] = searchForm.timeRange
      filtered = filtered.filter(log => {
        const time = log.timestamp.getTime()
        return time >= new Date(start).getTime() && time <= new Date(end).getTime()
      })
    }
    
    // 排序（按时间倒序）
    filtered.sort((a, b) => b.timestamp - a.timestamp)
    
    // 统计
    stats.total = filtered.length
    stats.success = filtered.filter(log => log.result === 'success').length
    stats.failed = filtered.filter(log => log.result === 'failed').length
    stats.uniqueUsers = new Set(filtered.map(log => log.user)).size
    
    // 分页
    total.value = filtered.length
    const start = (currentPage.value - 1) * pageSize.value
    const end = start + pageSize.value
    logList.value = filtered.slice(start, end)
    
    loading.value = false
  }, 500)
}

onMounted(() => {
  loadLogs()
})
</script>

<style lang="scss" scoped>
@import '@/styles/design-tokens.scss';

.audit-logs-container {
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
  
  .header-actions {
    display: flex;
    gap: var(--spacing-md);
  }
}

/* Tesla Style Search Card */
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

/* Stats Row */
.stats-row {
  margin-bottom: var(--spacing-xl);
}

/* Tesla Style Stat Cards */
.stat-card {
  background: var(--color-surface);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  border: 1px solid var(--color-border);
  position: relative;
  overflow: hidden;
  transition: all 0.3s ease;
  
  &::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 3px;
    background: linear-gradient(90deg, var(--color-primary), var(--color-secondary));
    transform: scaleX(0);
    transform-origin: left;
    transition: transform 0.3s ease;
  }
  
  &:hover {
    transform: translateY(-4px);
    box-shadow: var(--shadow-lg);
    
    &::before {
      transform: scaleX(1);
    }
  }
  
  .stat-item {
    display: flex;
    align-items: center;
    padding: var(--spacing-xl);
    
    .stat-icon {
      width: 60px;
      height: 60px;
      border-radius: var(--radius-lg);
      display: flex;
      align-items: center;
      justify-content: center;
      margin-right: var(--spacing-lg);
      background: var(--gradient-primary);
      box-shadow: var(--shadow-md);
      
      .el-icon {
        font-size: 28px;
        color: white;
      }
    }
    
    .stat-content {
      flex: 1;
      
      .stat-value {
        font-size: var(--font-size-2xl);
        font-weight: var(--font-weight-bold);
        color: var(--color-text-primary);
        margin-bottom: var(--spacing-xs);
        letter-spacing: -0.5px;
      }
      
      .stat-label {
        font-size: var(--font-size-sm);
        color: var(--color-text-secondary);
        font-weight: var(--font-weight-medium);
      }
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

/* Enhanced Form */
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

/* Enhanced Date Picker */
:deep(.el-date-editor) {
  width: 100%;
  
  .el-input__inner {
    padding-left: var(--spacing-md);
  }
}

/* Enhanced Table */
:deep(.el-table) {
  background: transparent;
  cursor: pointer;
  
  .el-table__header-wrapper {
    th {
      background: var(--color-surface-hover);
      color: var(--color-text-secondary);
      font-weight: var(--font-weight-semibold);
      border-bottom: 2px solid var(--color-border);
    }
  }
  
  .el-table__row {
    transition: all 0.3s ease;
    
    &:hover {
      background-color: var(--color-surface-hover);
      transform: scale(1.01);
      box-shadow: var(--shadow-sm);
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

:deep(.el-tag--primary) {
  background: linear-gradient(135deg, #e3f2fd, #bbdefb);
  color: #1565c0;
}

:deep(.el-tag--success) {
  background: linear-gradient(135deg, #e8f5e9, #c8e6c9);
  color: #2e7d32;
}

:deep(.el-tag--warning) {
  background: linear-gradient(135deg, #fff3e0, #ffe0b2);
  color: #e65100;
}

:deep(.el-tag--danger) {
  background: linear-gradient(135deg, #ffebee, #ffcdd2);
  color: #c62828;
}

:deep(.el-tag--info) {
  background: linear-gradient(135deg, #f5f5f5, #e0e0e0);
  color: #616161;
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
}

/* Enhanced Descriptions */
:deep(.el-descriptions) {
  .el-descriptions__header {
    margin-bottom: var(--spacing-lg);
  }
  
  .el-descriptions__title {
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }
  
  .el-descriptions__label {
    background: var(--color-surface-hover);
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
  }
  
  .el-descriptions__content {
    color: var(--color-text-primary);
  }
}

/* JSON Display */
.json-display {
  background-color: var(--color-surface-hover);
  padding: var(--spacing-md);
  border-radius: var(--radius-md);
  font-family: 'SF Mono', Monaco, 'Cascadia Code', 'Roboto Mono', Consolas, 'Courier New', monospace;
  font-size: var(--font-size-sm);
  max-height: 200px;
  overflow: auto;
  margin: 0;
  white-space: pre-wrap;
  word-break: break-all;
  border: 1px solid var(--color-border);
  
  &::-webkit-scrollbar {
    width: 8px;
    height: 8px;
  }
  
  &::-webkit-scrollbar-track {
    background: var(--color-surface);
    border-radius: var(--radius-sm);
  }
  
  &::-webkit-scrollbar-thumb {
    background: var(--color-border);
    border-radius: var(--radius-sm);
    
    &:hover {
      background: var(--color-border-hover);
    }
  }
}

/* Enhanced Alert */
:deep(.el-alert) {
  border-radius: var(--radius-md);
  border: none;
  box-shadow: var(--shadow-sm);
}

:deep(.el-alert--error) {
  background: linear-gradient(135deg, #fee, #fdd);
}

/* Loading States */
:deep(.el-loading-mask) {
  background-color: rgba(255, 255, 255, 0.8);
  backdrop-filter: blur(5px);
}

/* Responsive Design */
@media (max-width: 768px) {
  .audit-logs-container {
    padding: var(--spacing-md);
  }
  
  .page-header {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--spacing-md);
    
    h1 {
      font-size: var(--font-size-2xl);
    }
    
    .header-actions {
      width: 100%;
      
      .el-button {
        flex: 1;
      }
    }
  }
  
  .stat-card {
    margin-bottom: var(--spacing-md);
    
    .stat-item {
      padding: var(--spacing-md);
      
      .stat-icon {
        width: 48px;
        height: 48px;
        
        .el-icon {
          font-size: 24px;
        }
      }
      
      .stat-value {
        font-size: var(--font-size-xl);
      }
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