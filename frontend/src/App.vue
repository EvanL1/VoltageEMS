<template>
  <div class="app-container flex min-h-screen bg-gray-50 dark:bg-gray-900">
    <!-- Mobile menu button -->
    <div class="lg:hidden fixed top-4 left-4 z-50">
      <button
        @click="isSidebarOpen = !isSidebarOpen"
        class="p-2 rounded-lg bg-white dark:bg-gray-800 shadow-lg hover:shadow-xl transition-all duration-200"
      >
        <svg class="w-6 h-6 text-gray-600 dark:text-gray-300" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                :d="isSidebarOpen ? 'M6 18L18 6M6 6l12 12' : 'M4 6h16M4 12h16M4 18h16'" />
        </svg>
      </button>
    </div>

    <!-- Overlay for mobile -->
    <div
      v-if="isSidebarOpen"
      @click="isSidebarOpen = false"
      class="lg:hidden fixed inset-0 bg-black bg-opacity-50 z-40 transition-opacity duration-300"
    ></div>

    <!-- Sidebar -->
    <aside
      :class="[
        'fixed lg:relative inset-y-0 left-0 z-40 w-64 transform transition-transform duration-300 ease-in-out flex-shrink-0',
        'bg-gradient-to-b from-gray-800 to-gray-900 shadow-2xl',
        isSidebarOpen ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'
      ]"
    >
      <div class="flex flex-col h-full">
        <!-- Logo -->
        <div class="flex items-center justify-center h-20 bg-gray-900/50 backdrop-blur">
          <img :src="logoSrc" alt="Voltage Logo" class="h-12 w-auto animate-fade-in" />
          <span class="ml-3 text-xl font-bold text-white">Voltage</span>
        </div>

        <!-- Navigation -->
        <nav class="flex-1 px-4 py-6 space-y-2 overflow-y-auto">
          <router-link
            v-for="item in menuItems"
            :key="item.path"
            :to="item.path"
            @click="isSidebarOpen = false"
            class="flex items-center px-4 py-3 text-gray-300 rounded-lg transition-all duration-200 hover:bg-gray-700/50 hover:text-white group"
            :class="{ 'bg-primary-600 text-white': $route.path === item.path }"
          >
            <component :is="item.icon" class="w-5 h-5 mr-3 transition-transform group-hover:scale-110" />
            <span class="font-medium">{{ item.name }}</span>
            <span
              v-if="$route.path === item.path"
              class="ml-auto w-1 h-8 bg-white rounded-full"
            ></span>
          </router-link>
        </nav>

        <!-- Footer -->
        <div class="p-4 text-center text-xs text-gray-500 border-t border-gray-700">
          <p>Voltage, LLC. © 2025</p>
          <p class="mt-1">All Rights Reserved</p>
        </div>
      </div>
    </aside>

    <!-- Main Content -->
    <div class="flex-1 flex flex-col transition-all duration-300">
      <!-- Header -->
      <header class="sticky top-0 z-30 bg-white/80 dark:bg-gray-800/80 backdrop-blur-lg shadow-sm">
        <div class="flex items-center justify-between h-16 px-4 sm:px-6 lg:px-8">
          <div class="flex items-center">
            <h1 class="text-xl sm:text-2xl font-bold text-gray-800 dark:text-white animate-fade-in">
              {{ pageTitle }}
            </h1>
          </div>

          <!-- User Menu -->
          <div class="flex items-center space-x-4">
            <!-- Theme Toggle -->
            <button
              @click="toggleTheme"
              class="p-2 rounded-lg bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
            >
              <svg v-if="isDark" class="w-5 h-5 text-gray-600 dark:text-gray-300" fill="currentColor" viewBox="0 0 20 20">
                <path d="M10 2a1 1 0 011 1v1a1 1 0 11-2 0V3a1 1 0 011-1zm4 8a4 4 0 11-8 0 4 4 0 018 0zm-.464 4.95l.707.707a1 1 0 001.414-1.414l-.707-.707a1 1 0 00-1.414 1.414zm2.12-10.607a1 1 0 010 1.414l-.706.707a1 1 0 11-1.414-1.414l.707-.707a1 1 0 011.414 0zM17 11a1 1 0 100-2h-1a1 1 0 100 2h1zm-7 4a1 1 0 011 1v1a1 1 0 11-2 0v-1a1 1 0 011-1zM5.05 6.464A1 1 0 106.465 5.05l-.708-.707a1 1 0 00-1.414 1.414l.707.707zm1.414 8.486l-.707.707a1 1 0 01-1.414-1.414l.707-.707a1 1 0 011.414 1.414zM4 11a1 1 0 100-2H3a1 1 0 000 2h1z" />
              </svg>
              <svg v-else class="w-5 h-5 text-gray-600 dark:text-gray-300" fill="currentColor" viewBox="0 0 20 20">
                <path d="M17.293 13.293A8 8 0 016.707 2.707a8.001 8.001 0 1010.586 10.586z" />
              </svg>
            </button>

            <!-- User Dropdown -->
            <el-dropdown trigger="click" class="user-dropdown">
              <button class="flex items-center space-x-2 px-3 py-2 rounded-lg bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors">
                <div class="w-8 h-8 rounded-full bg-gradient-to-br from-primary-400 to-primary-600 flex items-center justify-center">
                  <span class="text-white font-semibold">V</span>
                </div>
                <span class="hidden sm:block text-sm font-medium text-gray-700 dark:text-gray-300">Voltage</span>
                <svg class="w-4 h-4 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                </svg>
              </button>
              <template #dropdown>
                <el-dropdown-menu>
                  <el-dropdown-item>
                    <span class="flex items-center">
                      <svg class="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                      </svg>
                      账户设置
                    </span>
                  </el-dropdown-item>
                  <el-dropdown-item divided>
                    <span class="flex items-center text-red-600">
                      <svg class="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
                      </svg>
                      退出登录
                    </span>
                  </el-dropdown-item>
                </el-dropdown-menu>
              </template>
            </el-dropdown>
          </div>
        </div>
      </header>

      <!-- Page Content -->
      <main class="flex-1 p-4 sm:p-6 lg:p-8 animate-fade-in overflow-auto">
        <router-view v-slot="{ Component }">
          <transition name="page" mode="out-in">
            <component :is="Component" />
          </transition>
        </router-view>
      </main>
    </div>
  </div>
</template>

<script>
import { ref, computed, watch, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { logoBase64 } from './assets/logo'
import { 
  House as IconHouse,
  Setting as IconSetting,
  DataLine as IconDataLine,
  Edit as IconEdit,
  Connection as IconConnection
} from '@element-plus/icons-vue'

export default {
  name: 'AppMain',
  setup() {
    const route = useRoute()
    const isSidebarOpen = ref(false)
    const isDark = ref(false)
    const logoSrc = logoBase64

    const menuItems = [
      { path: '/', name: '首页', icon: IconHouse },
      { path: '/system', name: '系统', icon: IconSetting },
      { path: '/activity', name: '活动', icon: IconDataLine },
      { path: '/template-builder', name: '模板构建器', icon: IconEdit },
      { path: '/dag-editor', name: 'DAG 编辑器', icon: IconConnection }
    ]

    const pageTitle = computed(() => {
      const currentItem = menuItems.find(item => item.path === route.path)
      if (currentItem) return currentItem.name
      
      if (route.path.startsWith('/system/')) {
        return '系统配置'
      }
      
      return '首页'
    })

    const toggleTheme = () => {
      isDark.value = !isDark.value
      if (isDark.value) {
        document.documentElement.classList.add('dark')
        localStorage.setItem('theme', 'dark')
      } else {
        document.documentElement.classList.remove('dark')
        localStorage.setItem('theme', 'light')
      }
    }

    onMounted(() => {
      // Initialize theme
      const savedTheme = localStorage.getItem('theme')
      if (savedTheme === 'dark' || (!savedTheme && window.matchMedia('(prefers-color-scheme: dark)').matches)) {
        isDark.value = true
        document.documentElement.classList.add('dark')
      }

      // Close sidebar on route change for mobile
      watch(route, () => {
        if (window.innerWidth < 1024) {
          isSidebarOpen.value = false
        }
      })
    })

    return {
      isSidebarOpen,
      isDark,
      logoSrc,
      menuItems,
      pageTitle,
      toggleTheme
    }
  }
}
</script>

<style>
/* Page transitions */
.page-enter-active,
.page-leave-active {
  transition: all 0.3s ease;
}

.page-enter-from {
  opacity: 0;
  transform: translateX(30px);
}

.page-leave-to {
  opacity: 0;
  transform: translateX(-30px);
}

/* Custom scrollbar */
::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

::-webkit-scrollbar-track {
  @apply bg-gray-100 dark:bg-gray-800;
}

::-webkit-scrollbar-thumb {
  @apply bg-gray-400 dark:bg-gray-600 rounded-full;
}

::-webkit-scrollbar-thumb:hover {
  @apply bg-gray-500 dark:bg-gray-500;
}

/* Element Plus overrides for better integration */
.el-dropdown-menu {
  @apply border-0 shadow-xl;
}

.dark .el-dropdown-menu {
  @apply bg-gray-800;
}

.dark .el-dropdown-menu__item {
  @apply text-gray-300 hover:bg-gray-700;
}

.dark .el-dropdown-menu__item--divided::before {
  @apply bg-gray-700;
}
</style>