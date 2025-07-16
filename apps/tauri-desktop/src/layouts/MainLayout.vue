<template>
  <el-container class="main-layout">
    <!-- Sidebar -->
    <el-aside :width="sidebarCollapsed ? '64px' : '240px'" class="sidebar">
      <div class="logo-container">
        <el-icon :size="32" color="#409EFF"><Lightning /></el-icon>
        <transition name="fade">
          <h2 v-if="!sidebarCollapsed" class="logo-text">VoltageEMS</h2>
        </transition>
      </div>
      
      <el-menu
        :default-active="activeMenu"
        :collapse="sidebarCollapsed"
        :collapse-transition="false"
        background-color="#304156"
        text-color="#bfcbd9"
        active-text-color="#409EFF"
        router
      >
        <template v-for="item in menuItems" :key="item.path">
          <el-sub-menu v-if="item.children" :index="item.path">
            <template #title>
              <el-icon><component :is="item.icon" /></el-icon>
              <span>{{ item.title }}</span>
            </template>
            <el-menu-item
              v-for="child in item.children"
              :key="child.path"
              :index="child.path"
            >
              <el-icon><component :is="child.icon" /></el-icon>
              <span>{{ child.title }}</span>
            </el-menu-item>
          </el-sub-menu>
          
          <el-menu-item v-else :index="item.path">
            <el-icon><component :is="item.icon" /></el-icon>
            <span>{{ item.title }}</span>
          </el-menu-item>
        </template>
      </el-menu>
      
      <div class="sidebar-toggle" @click="toggleSidebar">
        <el-icon :size="20">
          <Fold v-if="!sidebarCollapsed" />
          <Expand v-else />
        </el-icon>
      </div>
    </el-aside>
    
    <!-- Main Container -->
    <el-container>
      <!-- Header -->
      <el-header class="main-header">
        <div class="header-left">
          <el-breadcrumb separator="/">
            <el-breadcrumb-item :to="{ path: '/' }">Home</el-breadcrumb-item>
            <el-breadcrumb-item v-for="item in breadcrumbs" :key="item.path">
              {{ item.title }}
            </el-breadcrumb-item>
          </el-breadcrumb>
        </div>
        
        <div class="header-right">
          <!-- Language Switcher -->
          <el-dropdown @command="changeLanguage" class="header-item">
            <span class="el-dropdown-link">
              <el-icon><Location /></el-icon>
              {{ currentLanguage }}
            </span>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="en">English</el-dropdown-item>
                <el-dropdown-item command="zh">中文</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
          
          <!-- Theme Switcher -->
          <div class="header-item" @click="toggleTheme">
            <el-icon :size="18">
              <Sunny v-if="isDark" />
              <Moon v-else />
            </el-icon>
          </div>
          
          <!-- Notifications -->
          <el-badge :value="unreadNotifications" class="header-item">
            <el-icon :size="18"><Bell /></el-icon>
          </el-badge>
          
          <!-- User Menu -->
          <el-dropdown class="header-item user-menu">
            <span class="el-dropdown-link">
              <el-avatar :size="32" src="/avatar.png" />
              <span class="username">{{ username }}</span>
            </span>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item @click="goToProfile">
                  <el-icon><User /></el-icon>
                  Profile
                </el-dropdown-item>
                <el-dropdown-item @click="goToSettings">
                  <el-icon><Setting /></el-icon>
                  Settings
                </el-dropdown-item>
                <el-dropdown-item divided @click="logout">
                  <el-icon><SwitchButton /></el-icon>
                  Logout
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>
      </el-header>
      
      <!-- Main Content -->
      <el-main class="main-content">
        <router-view v-slot="{ Component }">
          <transition name="fade-transform" mode="out-in">
            <component :is="Component" />
          </transition>
        </router-view>
      </el-main>
      
      <!-- Footer Status Bar -->
      <el-footer class="status-bar" height="30px">
        <div class="status-left">
          <span class="status-item">
            <el-icon :color="wsConnected ? '#67C23A' : '#F56C6C'">
              <Connection />
            </el-icon>
            {{ wsConnected ? 'Connected' : 'Disconnected' }}
          </span>
          <span class="status-item">
            <el-icon><Cpu /></el-icon>
            Services: {{ onlineServices }}/{{ totalServices }}
          </span>
        </div>
        
        <div class="status-right">
          <span class="status-item">{{ currentTime }}</span>
          <span class="status-item">v{{ appVersion }}</span>
        </div>
      </el-footer>
    </el-container>
  </el-container>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { 
  Fold, 
  Expand, 
  Location, 
  Sunny, 
  Moon, 
  Bell, 
  User,
  Setting,
  SwitchButton,
  Connection,
  Cpu,
  Monitor,
  Clock,
  Operation,
  Grid,
  Document,
  Tools,
  Warning,
  Avatar,
  Notebook,
  DataAnalysis,
  Upload,
  Refresh,
  Box,
  Lightning,
  Odometer,
  SetUp
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import { wsManager } from '@/api/websocket'
import dayjs from 'dayjs'

const route = useRoute()
const router = useRouter()

// Sidebar state
const sidebarCollapsed = ref(false)
const activeMenu = computed(() => route.path)

// User state
const username = ref('Admin')
const unreadNotifications = ref(5)

// Theme state
const isDark = ref(false)

// Language state
const currentLanguage = ref('en')

// Connection state
const wsConnected = computed(() => wsManager.connected.value)
const onlineServices = ref(5)
const totalServices = ref(6)

// Time
const currentTime = ref(dayjs().format('HH:mm:ss'))
const appVersion = ref('1.0.0')

// Menu items
const menuItems = ref([
  {
    path: '/dashboard',
    title: 'Dashboard',
    icon: 'DataAnalysis'
  },
  {
    path: '/monitor',
    title: 'Real-time Monitor',
    icon: 'Monitor'
  },
  {
    path: '/history',
    title: 'History Data',
    icon: 'Clock'
  },
  {
    path: '/control',
    title: 'Device Control',
    icon: 'Operation'
  },
  {
    path: '/config',
    title: 'Configuration',
    icon: 'Setting',
    children: [
      {
        path: '/config/channel',
        title: 'Channel Config',
        icon: 'Grid'
      },
      {
        path: '/config/points',
        title: 'Point Table',
        icon: 'Document'
      }
    ]
  },
  {
    path: '/rules',
    title: 'Rule Engine',
    icon: 'Tools'
  },
  {
    path: '/service',
    title: 'Service Status',
    icon: 'Cpu'
  },
  {
    path: '/alarm',
    title: 'Alarm Management',
    icon: 'Warning'
  },
  {
    path: '/user',
    title: 'User Management',
    icon: 'Avatar'
  },
  {
    path: '/log',
    title: 'System Logs',
    icon: 'Notebook'
  },
  {
    path: '/system',
    title: 'System',
    icon: 'Box',
    children: [
      {
        path: '/system/config',
        title: 'Config Management',
        icon: 'Upload'
      },
      {
        path: '/system/update',
        title: 'System Update',
        icon: 'Refresh'
      }
    ]
  }
])

// Breadcrumbs
const breadcrumbs = computed(() => {
  const matched = route.matched.filter(item => item.meta && item.meta.title)
  return matched.map(item => ({
    path: item.path,
    title: item.meta.title
  }))
})

// Timer
let timer: number | null = null

onMounted(() => {
  // Update time every second
  timer = window.setInterval(() => {
    currentTime.value = dayjs().format('HH:mm:ss')
  }, 1000)
})

onUnmounted(() => {
  if (timer) {
    clearInterval(timer)
  }
})

// Methods
const toggleSidebar = () => {
  sidebarCollapsed.value = !sidebarCollapsed.value
}

const toggleTheme = () => {
  isDark.value = !isDark.value
  document.documentElement.classList.toggle('dark', isDark.value)
}

const changeLanguage = (lang: string) => {
  currentLanguage.value = lang
  // TODO: Implement i18n
  ElMessage.success(`Language changed to ${lang}`)
}

const goToProfile = () => {
  router.push('/user/profile')
}

const goToSettings = () => {
  router.push('/system/settings')
}

const logout = () => {
  ElMessage.success('Logged out successfully')
  router.push('/login')
}
</script>

<style lang="scss" scoped>
.main-layout {
  height: 100vh;
  
  .sidebar {
    background-color: #304156;
    transition: width 0.3s;
    overflow: hidden;
    
    .logo-container {
      height: 60px;
      display: flex;
      align-items: center;
      padding: 0 20px;
      border-bottom: 1px solid rgba(255, 255, 255, 0.1);
      
      .logo {
        width: 32px;
        height: 32px;
      }
      
      .logo-text {
        margin-left: 12px;
        color: #fff;
        font-size: 18px;
        font-weight: 600;
        white-space: nowrap;
      }
    }
    
    .el-menu {
      border: none;
      height: calc(100% - 100px);
      overflow-y: auto;
      
      &::-webkit-scrollbar {
        width: 4px;
      }
      
      &::-webkit-scrollbar-thumb {
        background: rgba(255, 255, 255, 0.2);
        border-radius: 2px;
      }
    }
    
    .sidebar-toggle {
      position: absolute;
      bottom: 0;
      width: 100%;
      height: 40px;
      display: flex;
      align-items: center;
      justify-content: center;
      cursor: pointer;
      border-top: 1px solid rgba(255, 255, 255, 0.1);
      color: #bfcbd9;
      
      &:hover {
        background-color: rgba(255, 255, 255, 0.05);
      }
    }
  }
  
  .main-header {
    background: #fff;
    box-shadow: 0 1px 4px rgba(0, 21, 41, 0.08);
    padding: 0 20px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    
    .header-left {
      display: flex;
      align-items: center;
    }
    
    .header-right {
      display: flex;
      align-items: center;
      gap: 20px;
      
      .header-item {
        cursor: pointer;
        display: flex;
        align-items: center;
        color: #606266;
        
        &:hover {
          color: #409EFF;
        }
      }
      
      .user-menu {
        .el-dropdown-link {
          display: flex;
          align-items: center;
          gap: 8px;
          
          .username {
            font-size: 14px;
          }
        }
      }
    }
  }
  
  .main-content {
    background-color: #f5f7fa;
    padding: 20px;
    overflow-y: auto;
  }
  
  .status-bar {
    background: #fff;
    border-top: 1px solid #e6e6e6;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0 20px;
    font-size: 12px;
    color: #606266;
    
    .status-left,
    .status-right {
      display: flex;
      align-items: center;
      gap: 20px;
    }
    
    .status-item {
      display: flex;
      align-items: center;
      gap: 5px;
    }
  }
}

// Transitions
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.fade-transform-enter-active,
.fade-transform-leave-active {
  transition: all 0.3s;
}

.fade-transform-enter-from {
  opacity: 0;
  transform: translateX(-30px);
}

.fade-transform-leave-to {
  opacity: 0;
  transform: translateX(30px);
}

// Dark mode support
:global(.dark) {
  .main-header {
    background: #1e1e1e;
    color: #fff;
    
    .header-right .header-item {
      color: #bfcbd9;
      
      &:hover {
        color: #409EFF;
      }
    }
  }
  
  .main-content {
    background-color: #151515;
  }
  
  .status-bar {
    background: #1e1e1e;
    border-top-color: #333;
    color: #bfcbd9;
  }
}
</style>