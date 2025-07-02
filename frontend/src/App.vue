<template>
  <div class="app-container">
    <!-- Sidebar -->
    <div class="sidebar">
      <div class="logo">
        <img :src="logoSrc" alt="Voltage Logo" />
      </div>
      <el-menu
        router
        :default-active="$route.path"
        class="el-menu-vertical"
        background-color="#3a4654"
        text-color="#bfcbd9"
        active-text-color="#409EFF">
        
        <el-menu-item index="/">
          <el-icon><House /></el-icon>
          <span>{{ $t('nav.home') }}</span>
        </el-menu-item>
        
        <el-menu-item index="/services">
          <el-icon><Connection /></el-icon>
          <span>{{ $t('nav.services') }}</span>
        </el-menu-item>
        
        <el-menu-item index="/devices">
          <el-icon><Monitor /></el-icon>
          <span>{{ $t('nav.devices') }}</span>
        </el-menu-item>
        
        <el-menu-item index="/alarms">
          <el-icon><Bell /></el-icon>
          <span>{{ $t('nav.alarms') }}</span>
        </el-menu-item>
        
        <el-menu-item index="/history">
          <el-icon><DataAnalysis /></el-icon>
          <span>{{ $t('nav.historyAnalysis') }}</span>
        </el-menu-item>
        
        <el-menu-item index="/grafana-live">
          <el-icon><DataLine /></el-icon>
          <span>{{ $t('nav.liveDashboard') }}</span>
        </el-menu-item>
        
        <el-menu-item index="/grafana-embedded">
          <el-icon><Monitor /></el-icon>
          <span>{{ $t('nav.realtimeMonitoring') }}</span>
        </el-menu-item>
        
        <el-menu-item index="/system">
          <el-icon><Setting /></el-icon>
          <span>{{ $t('nav.system') }}</span>
        </el-menu-item>
        
        <el-menu-item index="/activity">
          <el-icon><DataLine /></el-icon>
          <span>{{ $t('nav.activity') }}</span>
        </el-menu-item>
      </el-menu>
      
      <div class="sidebar-footer">
        <p>{{ $t('footer.copyright') }}</p>
      </div>
    </div>

    <!-- Main Content -->
    <div class="main-content">
      <!-- Header -->
      <header class="header">
        <div class="header-content">
          <h1 class="page-title">{{ pageTitle }}</h1>
          
          <div class="header-actions">
            <!-- Language Switcher -->
            <el-dropdown trigger="click" class="language-dropdown">
              <span class="language-info">
                <el-icon><Switch /></el-icon>
                <span class="language-text">{{ currentLanguageName }}</span>
                <el-icon><ArrowDown /></el-icon>
              </span>
              <template #dropdown>
                <el-dropdown-menu>
                  <el-dropdown-item 
                    v-for="lang in supportedLanguages" 
                    :key="lang.code"
                    @click="switchLanguage(lang.code)"
                    :class="{ 'is-active': currentLocale === lang.code }"
                  >
                    <span class="language-flag">{{ lang.flag }}</span>
                    {{ lang.name }}
                  </el-dropdown-item>
                </el-dropdown-menu>
              </template>
            </el-dropdown>

            <!-- User Dropdown -->
            <el-dropdown trigger="click" class="user-dropdown">
              <span class="user-info">
                <el-avatar size="small">V</el-avatar>
                <span class="username">Voltage</span>
                <el-icon><ArrowDown /></el-icon>
              </span>
              <template #dropdown>
                <el-dropdown-menu>
                  <el-dropdown-item>
                    <el-icon><User /></el-icon>
                    {{ $t('header.userDropdown.accountSettings') }}
                  </el-dropdown-item>
                  <el-dropdown-item divided>
                    <el-icon><SwitchButton /></el-icon>
                    {{ $t('header.userDropdown.logout') }}
                  </el-dropdown-item>
                </el-dropdown-menu>
              </template>
            </el-dropdown>
          </div>
        </div>
      </header>

      <!-- Page Content -->
      <main class="page-content">
        <router-view v-slot="{ Component }">
          <transition name="fade" mode="out-in">
            <component :is="Component" />
          </transition>
        </router-view>
      </main>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { logoBase64 } from './assets/logo'
import { setLocale, getCurrentLocale, supportedLocales } from './i18n'
import { 
  House,
  Setting,
  DataLine,
  DataAnalysis,
  Connection,
  Monitor,
  Bell,
  User,
  SwitchButton,
  ArrowDown,
  Switch
} from '@element-plus/icons-vue'

const route = useRoute()
const { t: $t } = useI18n()
const logoSrc = logoBase64

// Language switching
const supportedLanguages = supportedLocales
const currentLocale = computed(() => getCurrentLocale())
const currentLanguageName = computed(() => {
  const lang = supportedLanguages.find(l => l.code === currentLocale.value)
  return lang ? lang.name : 'English'
})

const switchLanguage = (langCode) => {
  setLocale(langCode)
}

const pageTitle = computed(() => {
  const titleMap = {
    '/': 'nav.home',
    '/services': 'nav.services',
    '/devices': 'nav.devices', 
    '/alarms': 'nav.alarms',
    '/history': 'nav.historyAnalysis',
    '/grafana-live': 'nav.liveDashboard',
    '/grafana-embedded': 'nav.realtimeMonitoring',
    '/system': 'nav.system',
    '/activity': 'nav.activity'
  }
  
  return titleMap[route.path] ? $t(titleMap[route.path]) : 'Voltage EMS'
})
</script>

<style scoped>
.app-container {
  display: flex;
  height: 100vh;
  background-color: #f5f5f5;
}

/* Sidebar Styles */
.sidebar {
  width: 250px;
  background-color: #3a4654;
  display: flex;
  flex-direction: column;
  box-shadow: 2px 0 6px rgba(0, 0, 0, 0.1);
}

.logo {
  height: 60px;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: #2e3742;
  border-bottom: 1px solid #434a56;
}

.logo img {
  height: 40px;
}

.el-menu-vertical {
  flex: 1;
  border-right: none;
}

.el-menu-item {
  height: 48px;
  line-height: 48px;
}

.el-menu-item span {
  margin-left: 8px;
}

.sidebar-footer {
  padding: 20px;
  text-align: center;
  color: #8492a6;
  font-size: 12px;
  border-top: 1px solid #434a56;
}

/* Main Content Styles */
.main-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* Header Styles */
.header {
  height: 60px;
  background-color: #fff;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.08);
  z-index: 100;
}

.header-content {
  height: 100%;
  padding: 0 24px;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.page-title {
  font-size: 20px;
  font-weight: 500;
  color: #303133;
  margin: 0;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 16px;
}

.language-dropdown .language-info {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 4px;
  transition: background-color 0.3s;
  font-size: 14px;
  color: #606266;
}

.language-dropdown .language-info:hover {
  background-color: #f5f7fa;
}

.language-text {
  font-size: 14px;
}

.language-flag {
  margin-right: 8px;
  font-size: 16px;
}

.el-dropdown-menu .is-active {
  background-color: #ecf5ff;
  color: #409eff;
}

.user-info {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 4px;
  transition: background-color 0.3s;
}

.user-info:hover {
  background-color: #f5f7fa;
}

.username {
  font-size: 14px;
  color: #606266;
}

/* Page Content Styles */
.page-content {
  flex: 1;
  padding: 24px;
  overflow-y: auto;
  background-color: #f5f5f5;
}

/* Transitions */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* Responsive */
@media (max-width: 768px) {
  .sidebar {
    position: fixed;
    left: -250px;
    height: 100vh;
    z-index: 200;
    transition: left 0.3s;
  }
  
  .sidebar.open {
    left: 0;
  }
  
  .main-content {
    margin-left: 0;
  }
  
  .page-content {
    padding: 16px;
  }
}
</style>

<style>
/* Global Element Plus adjustments */
.el-card {
  border-radius: 8px;
  border: none;
  box-shadow: 0 2px 12px 0 rgba(0, 0, 0, 0.1);
}

.el-card__header {
  padding: 16px 20px;
  border-bottom: 1px solid #ebeef5;
}

.el-card__body {
  padding: 20px;
}

/* Fix for select and date picker dropdowns */
.el-select-dropdown,
.el-picker-panel {
  z-index: 2000 !important;
}

/* Scrollbar styling */
::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

::-webkit-scrollbar-track {
  background: #f1f1f1;
}

::-webkit-scrollbar-thumb {
  background: #c1c1c1;
  border-radius: 4px;
}

::-webkit-scrollbar-thumb:hover {
  background: #a8a8a8;
}
</style>