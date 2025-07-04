<template>
  <el-container class="main-layout">
    <!-- 侧边栏 -->
    <el-aside :width="isCollapse ? '64px' : '220px'" class="sidebar">
      <div class="logo-container">
        <img src="/logo.png" alt="Monarch Hub" class="logo" v-show="!isCollapse">
        <img src="/logo-mini.png" alt="M" class="logo-mini" v-show="isCollapse">
      </div>
      
      <el-menu
        :default-active="$route.path"
        :collapse="isCollapse"
        :collapse-transition="false"
        router
        class="sidebar-menu"
      >
        <!-- 监控展示类菜单 -->
        <el-sub-menu index="monitoring" v-if="hasMonitoringAccess">
          <template #title>
            <el-icon><Monitor /></el-icon>
            <span>{{ $t('menu.monitoring') }}</span>
          </template>
          <el-menu-item index="/dashboard">
            <el-icon><DataAnalysis /></el-icon>
            <span>{{ $t('menu.dashboard') }}</span>
          </el-menu-item>
          <el-menu-item index="/monitoring/realtime">
            <el-icon><Timer /></el-icon>
            <span>{{ $t('menu.realtime') }}</span>
          </el-menu-item>
          <el-menu-item index="/monitoring/devices">
            <el-icon><Connection /></el-icon>
            <span>{{ $t('menu.devices') }}</span>
          </el-menu-item>
          <el-menu-item index="/monitoring/energy">
            <el-icon><DataLine /></el-icon>
            <span>{{ $t('menu.energy') }}</span>
          </el-menu-item>
          <el-menu-item index="/monitoring/alarms">
            <el-icon><WarnTriangleFilled /></el-icon>
            <span>{{ $t('menu.alarms') }}</span>
          </el-menu-item>
          <el-menu-item index="/monitoring/topology">
            <el-icon><Share /></el-icon>
            <span>{{ $t('menu.topology') }}</span>
          </el-menu-item>
        </el-sub-menu>

        <!-- 控制操作类菜单 -->
        <el-sub-menu index="control" v-if="hasControlAccess">
          <template #title>
            <el-icon><Operation /></el-icon>
            <span>{{ $t('menu.control') }}</span>
          </template>
          <el-menu-item index="/control/devices">
            <el-icon><SwitchButton /></el-icon>
            <span>{{ $t('menu.deviceControl') }}</span>
          </el-menu-item>
          <el-menu-item index="/control/alarms">
            <el-icon><Bell /></el-icon>
            <span>{{ $t('menu.alarmManagement') }}</span>
          </el-menu-item>
          <el-menu-item index="/control/batch">
            <el-icon><CopyDocument /></el-icon>
            <span>{{ $t('menu.batchControl') }}</span>
          </el-menu-item>
          <el-menu-item index="/control/schedule">
            <el-icon><Calendar /></el-icon>
            <span>{{ $t('menu.scheduledTasks') }}</span>
          </el-menu-item>
        </el-sub-menu>

        <!-- 配置管理类菜单 -->
        <el-sub-menu index="config" v-if="hasConfigAccess">
          <template #title>
            <el-icon><Setting /></el-icon>
            <span>{{ $t('menu.config') }}</span>
          </template>
          <el-menu-item index="/config/channels">
            <el-icon><Link /></el-icon>
            <span>{{ $t('menu.channelConfig') }}</span>
          </el-menu-item>
          <el-menu-item index="/config/points">
            <el-icon><Grid /></el-icon>
            <span>{{ $t('menu.pointTable') }}</span>
          </el-menu-item>
          <el-menu-item index="/config/models">
            <el-icon><DataBoard /></el-icon>
            <span>{{ $t('menu.modelConfig') }}</span>
          </el-menu-item>
          <el-menu-item index="/config/alarms">
            <el-icon><Warning /></el-icon>
            <span>{{ $t('menu.alarmRules') }}</span>
          </el-menu-item>
          <el-menu-item index="/config/storage">
            <el-icon><Files /></el-icon>
            <span>{{ $t('menu.storagePolicy') }}</span>
          </el-menu-item>
          <el-menu-item index="/config/network">
            <el-icon><Upload /></el-icon>
            <span>{{ $t('menu.networkForward') }}</span>
          </el-menu-item>
        </el-sub-menu>

        <!-- 系统管理类菜单 -->
        <el-sub-menu index="system" v-if="hasSystemAccess">
          <template #title>
            <el-icon><Tools /></el-icon>
            <span>{{ $t('menu.system') }}</span>
          </template>
          <el-menu-item index="/system/users" v-if="isAdmin">
            <el-icon><User /></el-icon>
            <span>{{ $t('menu.userManagement') }}</span>
          </el-menu-item>
          <el-menu-item index="/system/settings" v-if="isAdmin">
            <el-icon><SetUp /></el-icon>
            <span>{{ $t('menu.systemSettings') }}</span>
          </el-menu-item>
          <el-menu-item index="/system/audit">
            <el-icon><Document /></el-icon>
            <span>{{ $t('menu.auditLogs') }}</span>
          </el-menu-item>
          <el-menu-item index="/system/services">
            <el-icon><Cpu /></el-icon>
            <span>{{ $t('menu.serviceMonitor') }}</span>
          </el-menu-item>
        </el-sub-menu>
      </el-menu>
    </el-aside>

    <el-container>
      <!-- 顶部导航栏 -->
      <el-header class="header">
        <div class="header-left">
          <el-icon class="collapse-btn" @click="toggleCollapse" :size="20">
            <component :is="isCollapse ? 'Expand' : 'Fold'" />
          </el-icon>
          
          <!-- 面包屑导航 -->
          <el-breadcrumb separator="/" class="breadcrumb">
            <el-breadcrumb-item :to="{ path: '/' }">
              {{ $t('common.home') }}
            </el-breadcrumb-item>
            <el-breadcrumb-item v-for="item in breadcrumbs" :key="item.path">
              {{ item.title }}
            </el-breadcrumb-item>
          </el-breadcrumb>
        </div>

        <div class="header-right">
          <!-- 搜索 -->
          <el-input
            v-model="searchText"
            class="search-input"
            :placeholder="$t('common.search')"
            prefix-icon="Search"
            clearable
          />

          <!-- 通知 -->
          <el-badge :value="unreadCount" :max="99" class="notification-badge">
            <el-icon :size="20"><Bell /></el-icon>
          </el-badge>

          <!-- 语言切换 -->
          <el-dropdown @command="changeLanguage">
            <div class="language-icon">
              {{ currentLang === 'zh' ? '中' : 'EN' }}
            </div>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="zh">中文</el-dropdown-item>
                <el-dropdown-item command="en">English</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>

          <!-- 用户信息 -->
          <el-dropdown>
            <div class="user-info">
              <el-avatar :size="32" :src="userInfo.avatar">
                {{ userInfo.name?.charAt(0) }}
              </el-avatar>
              <span class="username">{{ userInfo.name }}</span>
              <el-tag size="small" :type="roleTagType">{{ $t(`roles.${userInfo.role}`) }}</el-tag>
            </div>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item @click="viewProfile">
                  <el-icon><User /></el-icon>
                  {{ $t('common.profile') }}
                </el-dropdown-item>
                <el-dropdown-item divided @click="logout">
                  <el-icon><SwitchButton /></el-icon>
                  {{ $t('common.logout') }}
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>

          <!-- 设置 -->
          <el-icon :size="20" @click="openSettings" class="settings-icon">
            <Setting />
          </el-icon>
        </div>
      </el-header>

      <!-- 主内容区域 -->
      <el-main class="main-content">
        <router-view v-slot="{ Component }">
          <transition name="fade-transform" mode="out-in">
            <component :is="Component" />
          </transition>
        </router-view>
      </el-main>
    </el-container>
  </el-container>
</template>

<script setup>
import { ref, computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useUserStore } from '@/stores/user'
import { useAlarmStore } from '@/stores/alarm'

const route = useRoute()
const router = useRouter()
const { t, locale } = useI18n()
const userStore = useUserStore()
const alarmStore = useAlarmStore()

// 响应式数据
const isCollapse = ref(false)
const searchText = ref('')
const currentLang = computed(() => locale.value)

// 用户信息
const userInfo = computed(() => userStore.userInfo || {})
const isAdmin = computed(() => userStore.isAdmin)

// 权限控制
const hasMonitoringAccess = computed(() => true) // 所有用户都有监控权限
const hasControlAccess = computed(() => userStore.canControl)
const hasConfigAccess = computed(() => userStore.canConfig)
const hasSystemAccess = computed(() => userStore.isEngineer || userStore.isAdmin)

// 角色标签类型
const roleTagType = computed(() => {
  const roleTypes = {
    operator: 'info',
    engineer: 'warning',
    admin: 'danger'
  }
  return roleTypes[userInfo.value.role] || 'info'
})

// 未读通知数
const unreadCount = computed(() => alarmStore.activeCount)

// 面包屑导航
const breadcrumbs = computed(() => {
  const matched = route.matched.filter(item => item.meta && item.meta.title)
  return matched.map(item => ({
    path: item.path,
    title: t(item.meta.title)
  }))
})

// 方法
const toggleCollapse = () => {
  isCollapse.value = !isCollapse.value
}

const changeLanguage = (lang) => {
  locale.value = lang
  localStorage.setItem('language', lang)
  ElMessage.success(t('common.languageChanged'))
}

const viewProfile = () => {
  router.push('/profile')
}

const logout = async () => {
  try {
    await ElMessageBox.confirm(
      t('common.logoutConfirm'),
      t('common.confirm'),
      {
        confirmButtonText: t('common.ok'),
        cancelButtonText: t('common.cancel'),
        type: 'warning'
      }
    )
    
    await userStore.logout()
    router.push('/login')
    ElMessage.success(t('common.logoutSuccess'))
  } catch {
    // 用户取消
  }
}

const openSettings = () => {
  router.push('/settings')
}

// 监听路由变化，更新面包屑
watch(() => route.path, () => {
  // 可以在这里添加路由变化的逻辑
})
</script>

<style lang="scss" scoped>
@import '@/styles/design-tokens.scss';

.main-layout {
  height: 100vh;
  background: var(--color-background);
  display: flex;
  overflow: hidden;
}

// 深色工业风格侧边栏
.sidebar {
  background: var(--color-background-secondary);
  border-right: 1px solid var(--color-border);
  transition: width var(--duration-normal) var(--ease-in-out);
  position: relative;
  z-index: 100;
  display: flex;
  flex-direction: column;
  
  .logo-container {
    height: 72px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 var(--space-4);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    
    .logo {
      height: 36px;
      transition: all var(--duration-normal) var(--ease-in-out);
    }
    
    .logo-mini {
      height: 28px;
    }
  }
  
  .sidebar-menu {
    border: none;
    background: transparent;
    padding: var(--space-3) 0;
    flex: 1;
    overflow-y: auto;
    
    :deep(.el-menu-item),
    :deep(.el-sub-menu__title) {
      height: 44px;
      line-height: 44px;
      margin: 0 var(--space-2);
      padding: 0 var(--space-4) !important;
      border-radius: var(--radius-lg);
      color: var(--color-text-secondary);
      font-weight: var(--font-weight-medium);
      font-size: var(--font-size-base);
      transition: all var(--duration-fast) var(--ease-in-out);
      
      .el-icon {
        font-size: 18px;
        margin-right: var(--space-3);
      }
      
      &:hover {
        background: var(--color-surface-hover);
        color: var(--color-text-primary);
      }
      
      &.is-active {
        background: var(--gradient-accent);
        color: var(--color-text-primary);
        box-shadow: var(--shadow-sm);
        
        .el-icon {
          color: var(--color-text-primary);
        }
      }
    }
    
    :deep(.el-sub-menu) {
      .el-sub-menu__title {
        &:hover {
          background: var(--color-surface-hover);
        }
      }
      
      &.is-opened {
        .el-sub-menu__title {
          color: var(--color-text-primary);
        }
      }
      
      .el-menu-item {
        padding-left: calc(var(--space-4) + var(--space-8)) !important;
        font-size: var(--font-size-sm);
      }
    }
  }
}

// 右侧容器 - 修复嵌套容器布局
.main-layout > .el-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-width: 0;
  position: relative;
}

// 深色工业风格顶部导航
.header {
  height: 72px;
  background: var(--color-background-elevated);
  border-bottom: 1px solid var(--color-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 var(--space-6);
  flex-shrink: 0;
  z-index: 99;
  
  .header-left {
    display: flex;
    align-items: center;
    gap: var(--space-5);
    
    .collapse-btn {
      width: 36px;
      height: 36px;
      display: flex;
      align-items: center;
      justify-content: center;
      border-radius: var(--radius-lg);
      cursor: pointer;
      transition: all var(--duration-fast) var(--ease-in-out);
      color: var(--color-text-secondary);
      
      &:hover {
        background: var(--color-surface-hover);
        color: var(--color-secondary);
      }
      
      &:active {
        transform: scale(0.95);
      }
    }
    
    .breadcrumb {
      :deep(.el-breadcrumb__inner) {
        color: var(--color-text-tertiary);
        font-weight: var(--font-weight-regular);
        transition: color var(--duration-fast) var(--ease-in-out);
        
        &.is-link:hover {
          color: var(--color-primary);
        }
      }
      
      :deep(.el-breadcrumb__item:last-child .el-breadcrumb__inner) {
        color: var(--color-text-primary);
        font-weight: var(--font-weight-medium);
      }
    }
  }
  
  .header-right {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    
    .search-input {
      width: 280px;
      
      :deep(.el-input__wrapper) {
        background: var(--color-surface);
        border: 1px solid var(--color-border);
        border-radius: var(--radius-full);
        padding: 0 var(--space-4);
        
        &:hover {
          background: var(--color-surface-hover);
          border-color: var(--color-border-hover);
        }
        
        &.is-focus {
          background: var(--color-surface-active);
          border-color: var(--color-secondary);
          box-shadow: 0 0 0 1px var(--color-secondary);
        }
      }
    }
    
    .notification-badge {
      width: 40px;
      height: 40px;
      display: flex;
      align-items: center;
      justify-content: center;
      border-radius: var(--radius-lg);
      cursor: pointer;
      transition: all var(--duration-fast) var(--ease-in-out);
      color: var(--color-text-secondary);
      position: relative;
      
      &:hover {
        background: var(--color-surface-hover);
        color: var(--color-secondary);
      }
      
      :deep(.el-badge__content) {
        top: 5px;
        right: 5px;
      }
    }
    
    .language-icon {
      width: 40px;
      height: 40px;
      display: flex;
      align-items: center;
      justify-content: center;
      border-radius: var(--radius-lg);
      cursor: pointer;
      transition: all var(--duration-fast) var(--ease-in-out);
      color: var(--color-text-secondary);
      font-size: var(--font-size-sm);
      font-weight: var(--font-weight-bold);
      
      &:hover {
        background: var(--color-surface-hover);
        color: var(--color-secondary);
      }
    }
    
    .user-info {
      display: flex;
      align-items: center;
      gap: var(--space-3);
      padding: var(--space-2) var(--space-3);
      border-radius: var(--radius-full);
      cursor: pointer;
      transition: all var(--duration-fast) var(--ease-in-out);
      
      &:hover {
        background: var(--color-surface-hover);
      }
      
      :deep(.el-avatar) {
        background: var(--gradient-accent);
        font-weight: var(--font-weight-semibold);
        box-shadow: var(--shadow-sm);
      }
      
      .username {
        color: var(--color-text-primary);
        font-weight: var(--font-weight-medium);
        margin-right: var(--space-2);
      }
      
      :deep(.el-tag) {
        height: 22px;
        line-height: 20px;
        padding: 0 var(--space-2);
        font-size: var(--font-size-xs);
      }
    }
    
    .settings-icon {
      width: 40px;
      height: 40px;
      display: flex;
      align-items: center;
      justify-content: center;
      border-radius: var(--radius-lg);
      cursor: pointer;
      transition: all var(--duration-fast) var(--ease-in-out);
      color: var(--color-text-secondary);
      
      &:hover {
        background: var(--color-surface-hover);
        color: var(--color-secondary);
        transform: rotate(90deg);
      }
    }
  }
}

// 主内容区域
.main-content {
  flex: 1;
  padding: var(--space-6);
  background: var(--color-background);
  overflow-y: auto;
  overflow-x: hidden;
  min-height: 0;
  
  // 自定义滚动条
  &::-webkit-scrollbar {
    width: 10px;
  }
  
  &::-webkit-scrollbar-track {
    background: transparent;
  }
  
  &::-webkit-scrollbar-thumb {
    background: var(--color-border-medium);
    border-radius: var(--radius-full);
    border: 2px solid var(--color-background);
    
    &:hover {
      background: var(--color-border-heavy);
    }
  }
}

// 页面过渡动画
.fade-transform-leave-active,
.fade-transform-enter-active {
  transition: all var(--duration-normal) var(--ease-in-out);
}

.fade-transform-enter-from {
  opacity: 0;
  transform: translateY(20px);
}

.fade-transform-leave-to {
  opacity: 0;
  transform: translateY(-20px) scale(0.98);
}

// 折叠状态优化
.el-aside {
  &[style*="width: 64px"] {
    .logo-container {
      padding: 0;
    }
    
    .sidebar-menu {
      :deep(.el-menu-item),
      :deep(.el-sub-menu__title) {
        padding: 0 !important;
        justify-content: center;
        
        .el-icon {
          margin: 0;
        }
        
        span {
          display: none;
        }
      }
    }
  }
}
</style>