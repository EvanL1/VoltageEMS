<template>
  <div class="system-settings-container">
    <!-- 页面头部 -->
    <div class="page-header">
      <h1>{{ $t('systemSettings.title') }}</h1>
      <div class="header-actions">
        <el-button type="primary" @click="handleSave" v-permission="['admin']">
          <el-icon><Document /></el-icon>
          {{ $t('common.save') }}
        </el-button>
        <el-button @click="handleReset" v-permission="['admin']">
          <el-icon><Refresh /></el-icon>
          {{ $t('common.reset') }}
        </el-button>
      </div>
    </div>

    <!-- 设置选项卡 -->
    <el-card>
      <el-tabs v-model="activeTab">
        <!-- 基本设置 -->
        <el-tab-pane :label="$t('systemSettings.basicSettings')" name="basic">
          <el-form :model="basicSettings" label-width="180px">
            <el-form-item :label="$t('systemSettings.systemName')">
              <el-input 
                v-model="basicSettings.systemName" 
                :placeholder="$t('systemSettings.enterSystemName')"
                maxlength="50"
                show-word-limit
                :disabled="!hasPermission(['admin'])"
              />
            </el-form-item>
            <el-form-item :label="$t('systemSettings.systemLogo')">
              <el-upload
                class="logo-uploader"
                :action="uploadUrl"
                :show-file-list="false"
                :on-success="handleLogoSuccess"
                :before-upload="beforeLogoUpload"
                :disabled="!hasPermission(['admin'])"
              >
                <img v-if="basicSettings.logoUrl" :src="basicSettings.logoUrl" class="logo">
                <el-icon v-else class="logo-uploader-icon"><Plus /></el-icon>
              </el-upload>
              <div class="upload-tip">{{ $t('systemSettings.logoTip') }}</div>
            </el-form-item>
            <el-form-item :label="$t('systemSettings.systemVersion')">
              <el-input v-model="basicSettings.version" disabled />
            </el-form-item>
            <el-form-item :label="$t('systemSettings.companyName')">
              <el-input 
                v-model="basicSettings.companyName" 
                :placeholder="$t('systemSettings.enterCompanyName')"
                :disabled="!hasPermission(['admin'])"
              />
            </el-form-item>
            <el-form-item :label="$t('systemSettings.contactEmail')">
              <el-input 
                v-model="basicSettings.contactEmail" 
                :placeholder="$t('systemSettings.enterContactEmail')"
                :disabled="!hasPermission(['admin'])"
              />
            </el-form-item>
            <el-form-item :label="$t('systemSettings.contactPhone')">
              <el-input 
                v-model="basicSettings.contactPhone" 
                :placeholder="$t('systemSettings.enterContactPhone')"
                :disabled="!hasPermission(['admin'])"
              />
            </el-form-item>
          </el-form>
        </el-tab-pane>

        <!-- 系统参数 -->
        <el-tab-pane :label="$t('systemSettings.systemParameters')" name="parameters">
          <el-form :model="systemParams" label-width="180px">
            <el-divider content-position="left">{{ $t('systemSettings.dataRetention') }}</el-divider>
            <el-form-item :label="$t('systemSettings.realtimeDataRetention')">
              <el-input-number 
                v-model="systemParams.realtimeRetention" 
                :min="1" 
                :max="30"
                :disabled="!hasPermission(['admin'])"
              />
              <span class="unit">{{ $t('systemSettings.days') }}</span>
            </el-form-item>
            <el-form-item :label="$t('systemSettings.historyDataRetention')">
              <el-input-number 
                v-model="systemParams.historyRetention" 
                :min="30" 
                :max="3650"
                :disabled="!hasPermission(['admin'])"
              />
              <span class="unit">{{ $t('systemSettings.days') }}</span>
            </el-form-item>
            <el-form-item :label="$t('systemSettings.alarmDataRetention')">
              <el-input-number 
                v-model="systemParams.alarmRetention" 
                :min="30" 
                :max="365"
                :disabled="!hasPermission(['admin'])"
              />
              <span class="unit">{{ $t('systemSettings.days') }}</span>
            </el-form-item>
            
            <el-divider content-position="left">{{ $t('systemSettings.performanceSettings') }}</el-divider>
            <el-form-item :label="$t('systemSettings.dataQueryTimeout')">
              <el-input-number 
                v-model="systemParams.queryTimeout" 
                :min="5" 
                :max="300"
                :disabled="!hasPermission(['admin'])"
              />
              <span class="unit">{{ $t('systemSettings.seconds') }}</span>
            </el-form-item>
            <el-form-item :label="$t('systemSettings.maxConcurrentConnections')">
              <el-input-number 
                v-model="systemParams.maxConnections" 
                :min="10" 
                :max="1000"
                :disabled="!hasPermission(['admin'])"
              />
            </el-form-item>
            <el-form-item :label="$t('systemSettings.dataCacheTime')">
              <el-input-number 
                v-model="systemParams.cacheTime" 
                :min="0" 
                :max="3600"
                :disabled="!hasPermission(['admin'])"
              />
              <span class="unit">{{ $t('systemSettings.seconds') }}</span>
            </el-form-item>
          </el-form>
        </el-tab-pane>

        <!-- 告警设置 -->
        <el-tab-pane :label="$t('systemSettings.alarmSettings')" name="alarm">
          <el-form :model="alarmSettings" label-width="180px">
            <el-form-item :label="$t('systemSettings.enableAlarmSound')">
              <el-switch 
                v-model="alarmSettings.enableSound"
                :disabled="!hasPermission(['admin'])"
              />
            </el-form-item>
            <el-form-item :label="$t('systemSettings.alarmSoundFile')" v-if="alarmSettings.enableSound">
              <el-select 
                v-model="alarmSettings.soundFile" 
                :placeholder="$t('systemSettings.selectSoundFile')"
                :disabled="!hasPermission(['admin'])"
              >
                <el-option label="Beep" value="beep.mp3" />
                <el-option label="Alert" value="alert.mp3" />
                <el-option label="Warning" value="warning.mp3" />
                <el-option label="Siren" value="siren.mp3" />
              </el-select>
              <el-button type="primary" link @click="playSound">
                <el-icon><VideoPlay /></el-icon>
                {{ $t('systemSettings.testSound') }}
              </el-button>
            </el-form-item>
            <el-form-item :label="$t('systemSettings.enableEmailNotification')">
              <el-switch 
                v-model="alarmSettings.enableEmail"
                :disabled="!hasPermission(['admin'])"
              />
            </el-form-item>
            <el-form-item :label="$t('systemSettings.emailRecipients')" v-if="alarmSettings.enableEmail">
              <el-tag
                v-for="email in alarmSettings.emailRecipients"
                :key="email"
                closable
                :disable-transitions="false"
                @close="handleEmailRemove(email)"
                style="margin-right: 8px; margin-bottom: 8px;"
              >
                {{ email }}
              </el-tag>
              <el-input
                v-if="emailInputVisible"
                ref="emailInputRef"
                v-model="emailInputValue"
                class="input-new-email"
                size="small"
                @keyup.enter="handleEmailConfirm"
                @blur="handleEmailConfirm"
                style="width: 200px;"
              />
              <el-button 
                v-else 
                size="small" 
                @click="showEmailInput"
                :disabled="!hasPermission(['admin'])"
              >
                + {{ $t('systemSettings.addEmail') }}
              </el-button>
            </el-form-item>
            <el-form-item :label="$t('systemSettings.alarmLevelFilter')">
              <el-checkbox-group 
                v-model="alarmSettings.levelFilter"
                :disabled="!hasPermission(['admin'])"
              >
                <el-checkbox label="critical">{{ $t('alarmManagement.critical') }}</el-checkbox>
                <el-checkbox label="major">{{ $t('alarmManagement.major') }}</el-checkbox>
                <el-checkbox label="minor">{{ $t('alarmManagement.minor') }}</el-checkbox>
                <el-checkbox label="hint">{{ $t('alarmManagement.hint') }}</el-checkbox>
              </el-checkbox-group>
            </el-form-item>
          </el-form>
        </el-tab-pane>

        <!-- 安全设置 -->
        <el-tab-pane :label="$t('systemSettings.securitySettings')" name="security">
          <el-form :model="securitySettings" label-width="180px">
            <el-form-item :label="$t('systemSettings.passwordPolicy')">
              <el-checkbox 
                v-model="securitySettings.requireUppercase"
                :disabled="!hasPermission(['admin'])"
              >
                {{ $t('systemSettings.requireUppercase') }}
              </el-checkbox>
              <el-checkbox 
                v-model="securitySettings.requireNumber"
                :disabled="!hasPermission(['admin'])"
              >
                {{ $t('systemSettings.requireNumber') }}
              </el-checkbox>
              <el-checkbox 
                v-model="securitySettings.requireSpecial"
                :disabled="!hasPermission(['admin'])"
              >
                {{ $t('systemSettings.requireSpecial') }}
              </el-checkbox>
            </el-form-item>
            <el-form-item :label="$t('systemSettings.minPasswordLength')">
              <el-input-number 
                v-model="securitySettings.minPasswordLength" 
                :min="6" 
                :max="20"
                :disabled="!hasPermission(['admin'])"
              />
            </el-form-item>
            <el-form-item :label="$t('systemSettings.passwordExpiration')">
              <el-input-number 
                v-model="securitySettings.passwordExpiration" 
                :min="0" 
                :max="365"
                :disabled="!hasPermission(['admin'])"
              />
              <span class="unit">{{ $t('systemSettings.days') }}</span>
              <span class="tip">{{ $t('systemSettings.passwordExpirationTip') }}</span>
            </el-form-item>
            <el-form-item :label="$t('systemSettings.sessionTimeout')">
              <el-input-number 
                v-model="securitySettings.sessionTimeout" 
                :min="5" 
                :max="1440"
                :disabled="!hasPermission(['admin'])"
              />
              <span class="unit">{{ $t('systemSettings.minutes') }}</span>
            </el-form-item>
            <el-form-item :label="$t('systemSettings.maxLoginAttempts')">
              <el-input-number 
                v-model="securitySettings.maxLoginAttempts" 
                :min="3" 
                :max="10"
                :disabled="!hasPermission(['admin'])"
              />
              <span class="unit">{{ $t('systemSettings.times') }}</span>
            </el-form-item>
            <el-form-item :label="$t('systemSettings.lockoutDuration')">
              <el-input-number 
                v-model="securitySettings.lockoutDuration" 
                :min="5" 
                :max="60"
                :disabled="!hasPermission(['admin'])"
              />
              <span class="unit">{{ $t('systemSettings.minutes') }}</span>
            </el-form-item>
            <el-form-item :label="$t('systemSettings.enableTwoFactor')">
              <el-switch 
                v-model="securitySettings.enableTwoFactor"
                :disabled="!hasPermission(['admin'])"
              />
            </el-form-item>
          </el-form>
        </el-tab-pane>

        <!-- 备份恢复 -->
        <el-tab-pane :label="$t('systemSettings.backupRestore')" name="backup" v-permission="['admin']">
          <div class="backup-section">
            <h3>{{ $t('systemSettings.dataBackup') }}</h3>
            <el-form :model="backupSettings" label-width="180px">
              <el-form-item :label="$t('systemSettings.autoBackup')">
                <el-switch v-model="backupSettings.autoBackup" />
              </el-form-item>
              <el-form-item :label="$t('systemSettings.backupSchedule')" v-if="backupSettings.autoBackup">
                <el-select v-model="backupSettings.schedule" :placeholder="$t('systemSettings.selectSchedule')">
                  <el-option :label="$t('systemSettings.daily')" value="daily" />
                  <el-option :label="$t('systemSettings.weekly')" value="weekly" />
                  <el-option :label="$t('systemSettings.monthly')" value="monthly" />
                </el-select>
              </el-form-item>
              <el-form-item :label="$t('systemSettings.backupTime')" v-if="backupSettings.autoBackup">
                <el-time-picker 
                  v-model="backupSettings.time" 
                  format="HH:mm"
                  value-format="HH:mm"
                />
              </el-form-item>
              <el-form-item :label="$t('systemSettings.backupRetention')">
                <el-input-number 
                  v-model="backupSettings.retention" 
                  :min="1" 
                  :max="365"
                />
                <span class="unit">{{ $t('systemSettings.days') }}</span>
              </el-form-item>
              <el-form-item>
                <el-button type="primary" @click="handleBackupNow">
                  <el-icon><Download /></el-icon>
                  {{ $t('systemSettings.backupNow') }}
                </el-button>
              </el-form-item>
            </el-form>
            
            <el-divider />
            
            <h3>{{ $t('systemSettings.dataRestore') }}</h3>
            <el-upload
              class="backup-upload"
              drag
              :action="restoreUrl"
              :before-upload="beforeRestoreUpload"
              :on-success="handleRestoreSuccess"
              accept=".zip,.tar.gz"
            >
              <el-icon class="el-icon--upload"><UploadFilled /></el-icon>
              <div class="el-upload__text">
                {{ $t('systemSettings.dropFileHere') }} <em>{{ $t('systemSettings.clickToUpload') }}</em>
              </div>
              <template #tip>
                <div class="el-upload__tip">{{ $t('systemSettings.restoreTip') }}</div>
              </template>
            </el-upload>
            
            <h3 style="margin-top: 30px;">{{ $t('systemSettings.backupHistory') }}</h3>
            <el-table :data="backupHistory" stripe>
              <el-table-column prop="time" :label="$t('systemSettings.backupTime')" width="180">
                <template #default="{ row }">
                  {{ formatTime(row.time) }}
                </template>
              </el-table-column>
              <el-table-column prop="type" :label="$t('systemSettings.backupType')" width="120">
                <template #default="{ row }">
                  <el-tag :type="row.type === 'auto' ? 'info' : 'success'" size="small">
                    {{ row.type === 'auto' ? $t('systemSettings.auto') : $t('systemSettings.manual') }}
                  </el-tag>
                </template>
              </el-table-column>
              <el-table-column prop="size" :label="$t('systemSettings.fileSize')" width="120">
                <template #default="{ row }">
                  {{ formatFileSize(row.size) }}
                </template>
              </el-table-column>
              <el-table-column prop="status" :label="$t('common.status')" width="120">
                <template #default="{ row }">
                  <el-tag :type="getBackupStatusType(row.status)" size="small">
                    {{ getBackupStatusLabel(row.status) }}
                  </el-tag>
                </template>
              </el-table-column>
              <el-table-column :label="$t('common.actions')" width="150">
                <template #default="{ row }">
                  <el-button type="primary" link size="small" @click="handleDownloadBackup(row)">
                    {{ $t('systemSettings.download') }}
                  </el-button>
                  <el-button type="danger" link size="small" @click="handleDeleteBackup(row)">
                    {{ $t('common.delete') }}
                  </el-button>
                </template>
              </el-table-column>
            </el-table>
          </div>
        </el-tab-pane>
      </el-tabs>
    </el-card>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { 
  Document, Refresh, Plus, VideoPlay, Download, UploadFilled
} from '@element-plus/icons-vue'
import { useUserStore } from '@/stores/user'
import dayjs from 'dayjs'

const { t } = useI18n()
const userStore = useUserStore()

// 权限判断
const hasPermission = (roles) => {
  return userStore.hasPermission(roles)
}

// 当前选项卡
const activeTab = ref('basic')

// 基本设置
const basicSettings = reactive({
  systemName: 'Monarch Hub',
  logoUrl: '',
  version: 'v1.0.0',
  companyName: 'Voltage, LLC.',
  contactEmail: 'support@voltage.com',
  contactPhone: '1-800-VOLTAGE'
})

// 系统参数
const systemParams = reactive({
  realtimeRetention: 7,
  historyRetention: 365,
  alarmRetention: 90,
  queryTimeout: 30,
  maxConnections: 100,
  cacheTime: 300
})

// 告警设置
const alarmSettings = reactive({
  enableSound: true,
  soundFile: 'beep.mp3',
  enableEmail: true,
  emailRecipients: ['admin@voltage.com', 'engineer@voltage.com'],
  levelFilter: ['critical', 'major']
})

// 安全设置
const securitySettings = reactive({
  requireUppercase: true,
  requireNumber: true,
  requireSpecial: false,
  minPasswordLength: 8,
  passwordExpiration: 90,
  sessionTimeout: 30,
  maxLoginAttempts: 5,
  lockoutDuration: 15,
  enableTwoFactor: false
})

// 备份设置
const backupSettings = reactive({
  autoBackup: true,
  schedule: 'daily',
  time: '02:00',
  retention: 30
})

// 邮箱输入
const emailInputVisible = ref(false)
const emailInputValue = ref('')
const emailInputRef = ref(null)

// 文件上传
const uploadUrl = '/api/upload/logo'
const restoreUrl = '/api/restore'

// 备份历史
const backupHistory = ref([
  {
    id: 1,
    time: new Date('2024-01-03 02:00:00'),
    type: 'auto',
    size: 1024 * 1024 * 256, // 256MB
    status: 'success'
  },
  {
    id: 2,
    time: new Date('2024-01-02 02:00:00'),
    type: 'auto',
    size: 1024 * 1024 * 255,
    status: 'success'
  },
  {
    id: 3,
    time: new Date('2024-01-01 14:30:00'),
    type: 'manual',
    size: 1024 * 1024 * 258,
    status: 'success'
  }
])

// 格式化时间
const formatTime = (time) => {
  return dayjs(time).format('YYYY-MM-DD HH:mm:ss')
}

// 格式化文件大小
const formatFileSize = (size) => {
  if (size < 1024) return size + ' B'
  if (size < 1024 * 1024) return (size / 1024).toFixed(1) + ' KB'
  if (size < 1024 * 1024 * 1024) return (size / 1024 / 1024).toFixed(1) + ' MB'
  return (size / 1024 / 1024 / 1024).toFixed(1) + ' GB'
}

// 获取备份状态类型
const getBackupStatusType = (status) => {
  const typeMap = {
    success: 'success',
    failed: 'danger',
    pending: 'warning'
  }
  return typeMap[status] || 'info'
}

// 获取备份状态标签
const getBackupStatusLabel = (status) => {
  const labelMap = {
    success: t('common.success'),
    failed: t('common.failed'),
    pending: t('systemSettings.pending')
  }
  return labelMap[status] || status
}

// 保存设置
const handleSave = async () => {
  try {
    // 模拟保存
    await new Promise(resolve => setTimeout(resolve, 1000))
    ElMessage.success(t('systemSettings.saveSuccess'))
  } catch (error) {
    ElMessage.error(t('systemSettings.saveFailed'))
  }
}

// 重置设置
const handleReset = () => {
  ElMessageBox.confirm(
    t('systemSettings.resetConfirm'),
    t('common.warning'),
    {
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
      type: 'warning'
    }
  ).then(() => {
    // 重置为默认值
    loadSettings()
    ElMessage.success(t('systemSettings.resetSuccess'))
  })
}

// Logo上传成功
const handleLogoSuccess = (response) => {
  basicSettings.logoUrl = response.url
  ElMessage.success(t('systemSettings.uploadSuccess'))
}

// Logo上传前检查
const beforeLogoUpload = (file) => {
  const isImage = file.type.startsWith('image/')
  const isLt2M = file.size / 1024 / 1024 < 2

  if (!isImage) {
    ElMessage.error(t('systemSettings.onlyImage'))
    return false
  }
  if (!isLt2M) {
    ElMessage.error(t('systemSettings.imageSizeLimit'))
    return false
  }
  return true
}

// 播放声音
const playSound = () => {
  // 模拟播放声音
  ElMessage.info(t('systemSettings.playingSound', { file: alarmSettings.soundFile }))
}

// 显示邮箱输入
const showEmailInput = () => {
  emailInputVisible.value = true
  nextTick(() => {
    emailInputRef.value?.focus()
  })
}

// 确认添加邮箱
const handleEmailConfirm = () => {
  const email = emailInputValue.value.trim()
  if (email && /^[\w.-]+@[\w.-]+\.\w+$/.test(email)) {
    if (!alarmSettings.emailRecipients.includes(email)) {
      alarmSettings.emailRecipients.push(email)
    }
  }
  emailInputVisible.value = false
  emailInputValue.value = ''
}

// 移除邮箱
const handleEmailRemove = (email) => {
  const index = alarmSettings.emailRecipients.indexOf(email)
  if (index > -1) {
    alarmSettings.emailRecipients.splice(index, 1)
  }
}

// 立即备份
const handleBackupNow = () => {
  ElMessageBox.confirm(
    t('systemSettings.backupConfirm'),
    t('common.confirm'),
    {
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
      type: 'info'
    }
  ).then(async () => {
    const loading = ElMessage.info({
      message: t('systemSettings.backingUp'),
      duration: 0
    })
    
    // 模拟备份
    await new Promise(resolve => setTimeout(resolve, 3000))
    loading.close()
    
    // 添加备份记录
    backupHistory.value.unshift({
      id: Date.now(),
      time: new Date(),
      type: 'manual',
      size: 1024 * 1024 * (250 + Math.random() * 20),
      status: 'success'
    })
    
    ElMessage.success(t('systemSettings.backupSuccess'))
  })
}

// 恢复前检查
const beforeRestoreUpload = (file) => {
  const isValid = file.name.endsWith('.zip') || file.name.endsWith('.tar.gz')
  if (!isValid) {
    ElMessage.error(t('systemSettings.invalidBackupFile'))
    return false
  }
  
  return ElMessageBox.confirm(
    t('systemSettings.restoreConfirm'),
    t('common.warning'),
    {
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
      type: 'warning'
    }
  )
}

// 恢复成功
const handleRestoreSuccess = () => {
  ElMessage.success(t('systemSettings.restoreSuccess'))
}

// 下载备份
const handleDownloadBackup = (row) => {
  ElMessage.info(t('systemSettings.downloadingBackup'))
  // 模拟下载
  setTimeout(() => {
    const link = document.createElement('a')
    link.href = '#'
    link.download = `backup_${dayjs(row.time).format('YYYYMMDD_HHmmss')}.zip`
    link.click()
  }, 1000)
}

// 删除备份
const handleDeleteBackup = async (row) => {
  try {
    await ElMessageBox.confirm(
      t('systemSettings.deleteBackupConfirm'),
      t('common.warning'),
      {
        confirmButtonText: t('common.confirm'),
        cancelButtonText: t('common.cancel'),
        type: 'warning'
      }
    )
    
    const index = backupHistory.value.findIndex(b => b.id === row.id)
    if (index > -1) {
      backupHistory.value.splice(index, 1)
    }
    
    ElMessage.success(t('common.deleteSuccess'))
  } catch (error) {
    // 用户取消
  }
}

// 加载设置
const loadSettings = () => {
  // 从本地存储或API加载设置
  // 这里使用默认值
}

onMounted(() => {
  loadSettings()
})
</script>

<style lang="scss" scoped>
@import '@/styles/design-tokens.scss';

.system-settings-container {
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

/* Tesla Style Card */
.el-card {
  background: var(--color-background-elevated);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  border: 1px solid var(--color-border);
  transition: all 0.3s ease;
  
  &:hover {
    box-shadow: var(--shadow-md);
  }
  
  :deep(.el-card__body) {
    padding: 0;
  }
}

/* Enhanced Tabs - Fix for proper content display */
.el-card :deep(.el-tabs) {
  display: flex;
  flex-direction: column;
  height: 100%;
  
  .el-tabs__header {
    background: var(--color-surface-hover);
    border-bottom: 1px solid var(--color-border);
    margin: 0;
    padding: 0 var(--spacing-xl);
    border-radius: var(--radius-lg) var(--radius-lg) 0 0;
    flex-shrink: 0;
  }
  
  .el-tabs__nav-wrap {
    &::after {
      display: none;
    }
  }
  
  .el-tabs__nav-scroll {
    display: flex;
    align-items: center;
  }
  
  .el-tabs__item {
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
    padding: var(--spacing-md) var(--spacing-xl);
    transition: all 0.3s ease;
    position: relative;
    height: auto;
    line-height: normal;
    
    &:hover {
      color: var(--color-secondary);
    }
    
    &.is-active {
      color: var(--color-secondary);
      font-weight: var(--font-weight-semibold);
      
      &::after {
        content: '';
        position: absolute;
        bottom: 0;
        left: 0;
        right: 0;
        height: 3px;
        background: var(--gradient-accent);
      }
    }
  }
  
  .el-tabs__active-bar {
    display: none;
  }
  
  .el-tabs__content {
    flex: 1;
    padding: var(--spacing-xxl);
    background: var(--color-background-elevated);
    border-radius: 0 0 var(--radius-lg) var(--radius-lg);
    overflow-y: auto;
  }
  
  .el-tab-pane {
    height: 100%;
  }
}

/* Enhanced Form */
.el-form {
  max-width: 800px;
  
  .unit {
    margin-left: var(--spacing-sm);
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
  }
  
  .tip {
    margin-left: var(--spacing-md);
    color: var(--color-text-secondary);
    font-size: var(--font-size-sm);
  }
}

:deep(.el-form-item) {
  margin-bottom: var(--spacing-xl);
  
  .el-form-item__label {
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
    margin-bottom: var(--spacing-sm);
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

/* Enhanced Input Number */
:deep(.el-input-number) {
  .el-input-number__increase,
  .el-input-number__decrease {
    background: var(--color-surface-hover);
    border-left: 1px solid var(--color-border);
    
    &:hover {
      background: var(--color-primary-light);
      color: var(--color-primary);
    }
  }
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

/* Enhanced Checkbox */
:deep(.el-checkbox) {
  margin-right: var(--spacing-xl);
  
  .el-checkbox__inner {
    border-radius: var(--radius-sm);
    border-color: var(--color-border);
  }
  
  .el-checkbox__input.is-checked .el-checkbox__inner {
    background: var(--color-primary);
    border-color: var(--color-primary);
  }
  
  .el-checkbox__label {
    color: var(--color-text-primary);
    font-weight: var(--font-weight-medium);
  }
}

/* Enhanced Time Picker */
:deep(.el-time-picker) {
  width: 100%;
}

/* Enhanced Tags */
:deep(.el-tag) {
  border-radius: var(--radius-full);
  padding: var(--spacing-xs) var(--spacing-md);
  font-weight: var(--font-weight-medium);
  border: none;
  margin-right: var(--spacing-sm);
  margin-bottom: var(--spacing-sm);
}

:deep(.el-tag--info) {
  background: linear-gradient(135deg, #e3f2fd, #bbdefb);
  color: #1565c0;
}

:deep(.el-tag--success) {
  background: linear-gradient(135deg, #e8f5e9, #c8e6c9);
  color: #2e7d32;
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
  margin-left: var(--spacing-md);
  
  &:hover {
    opacity: 0.7;
  }
}

/* Enhanced Divider */
:deep(.el-divider) {
  margin: var(--spacing-xxl) 0;
  border-color: var(--color-border);
  
  .el-divider__text {
    background: var(--color-surface);
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-semibold);
    padding: 0 var(--spacing-lg);
  }
}

/* Logo Uploader */
.logo-uploader {
  :deep(.el-upload) {
    border: 2px dashed var(--color-border);
    border-radius: var(--radius-lg);
    cursor: pointer;
    position: relative;
    overflow: hidden;
    transition: all 0.3s ease;
    background: var(--color-surface-hover);
    
    &:hover {
      border-color: var(--color-primary);
      background: var(--color-primary-light);
    }
  }
  
  .logo {
    width: 150px;
    height: 60px;
    object-fit: contain;
    border-radius: var(--radius-md);
    padding: var(--spacing-sm);
  }
  
  .logo-uploader-icon {
    font-size: 28px;
    color: var(--color-text-secondary);
    width: 150px;
    height: 60px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
}

.upload-tip {
  margin-top: var(--spacing-sm);
  font-size: var(--font-size-sm);
  color: var(--color-text-secondary);
}

/* Email Input */
.input-new-email {
  margin-left: var(--spacing-sm);
}

/* Backup Section */
.backup-section {
  h3 {
    margin: 0 0 var(--spacing-xl) 0;
    font-size: var(--font-size-xl);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }
}

/* Backup Upload */
.backup-upload {
  width: 100%;
  max-width: 600px;
  
  :deep(.el-upload-dragger) {
    padding: var(--spacing-xxl);
    border-radius: var(--radius-lg);
    border: 2px dashed var(--color-border);
    background: var(--color-surface-hover);
    transition: all 0.3s ease;
    
    &:hover {
      border-color: var(--color-primary);
      background: var(--color-primary-light);
    }
  }
  
  :deep(.el-icon--upload) {
    font-size: 67px;
    color: var(--color-text-secondary);
    margin-bottom: var(--spacing-lg);
  }
  
  :deep(.el-upload__text) {
    font-size: var(--font-size-base);
    color: var(--color-text-primary);
    
    em {
      color: var(--color-primary);
      font-style: normal;
      font-weight: var(--font-weight-semibold);
    }
  }
  
  :deep(.el-upload__tip) {
    margin-top: var(--spacing-md);
    color: var(--color-text-secondary);
  }
}

/* Enhanced Table */
:deep(.el-table) {
  background: transparent;
  margin-top: var(--spacing-xl);
  
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

/* Loading States */
:deep(.el-loading-mask) {
  background-color: rgba(255, 255, 255, 0.8);
  backdrop-filter: blur(5px);
}

/* Responsive Design */
@media (max-width: 768px) {
  .system-settings-container {
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
  
  :deep(.el-tabs__item) {
    padding: var(--spacing-sm) var(--spacing-md);
    font-size: var(--font-size-sm);
  }
  
  :deep(.el-form-item__label) {
    font-size: var(--font-size-sm);
  }
}

/* Icon Styling */
:deep(.el-icon) {
  margin-right: var(--spacing-xs);
}
</style>