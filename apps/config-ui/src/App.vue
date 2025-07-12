<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { useRouter, useRoute } from 'vue-router';
import { useConfigStore } from '@/stores/config';
import { ElMessage, ElMessageBox } from 'element-plus';
import LogViewer from '@/components/LogViewer.vue';
import { 
  Setting, 
  Monitor, 
  Connection, 
  Document, 
  Tools, 
  Upload, 
  Download,
  CircleCheck,
  Warning,
  Bell,
  User,
  ArrowDown
} from '@element-plus/icons-vue';

const configStore = useConfigStore();
const router = useRouter();
const route = useRoute();

// 当前激活的菜单
const activeMenu = computed(() => route.path);

// 创建粒子效果
onMounted(() => {
  createParticles();
  createScanLine();
});

function createParticles() {
  const container = document.createElement('div');
  container.className = 'particle-container';
  
  // 创建更少的粒子，减少视觉干扰
  for (let i = 0; i < 10; i++) {
    const particle = document.createElement('div');
    particle.className = 'particle';
    particle.style.left = Math.random() * 100 + '%';
    particle.style.animationDelay = Math.random() * 20 + 's';
    particle.style.animationDuration = (25 + Math.random() * 15) + 's';
    container.appendChild(particle);
  }
  
  document.body.appendChild(container);
}

function createScanLine() {
  const scanLine = document.createElement('div');
  scanLine.className = 'scan-line';
  document.body.appendChild(scanLine);
}

// 配置分类
const configCategories = [
  {
    id: 'monitor',
    name: '实时监控',
    icon: Monitor,
    children: [
      { id: 'realtime', name: '实时数据', path: '/monitor/realtime' },
      { id: 'channels-status', name: '通道状态', path: '/monitor/channels' },
      { id: 'alarms', name: '告警监控', path: '/monitor/alarms' },
      { id: 'trends', name: '历史趋势', path: '/monitor/trends' },
    ]
  },
  {
    id: 'data',
    name: '数据配置',
    icon: Document,
    children: [
      { id: 'channels', name: '通道管理', path: '/channels' },
      { id: 'data-flow', name: '数据流配置', path: '/data-flow' },
      { id: 'calculations', name: '计算配置', path: '/calculations' },
    ]
  },
  {
    id: 'services',
    name: '服务配置',
    icon: Setting,
    children: [
      { id: 'comsrv', name: '通信服务 (comsrv)', path: '/service/comsrv' },
      { id: 'modsrv', name: '计算服务 (modsrv)', path: '/service/modsrv' },
      { id: 'hissrv', name: '历史服务 (hissrv)', path: '/service/hissrv' },
      { id: 'netsrv', name: '网络服务 (netsrv)', path: '/service/netsrv' },
      { id: 'alarmsrv', name: '告警服务 (alarmsrv)', path: '/service/alarmsrv' },
      { id: 'rulesrv', name: '规则服务 (rulesrv)', path: '/service/rulesrv' },
    ]
  },
  {
    id: 'alarm',
    name: '告警配置',
    icon: Bell,
    children: [
      { id: 'alarm-rules', name: '告警规则', path: '/alarm-rules' },
      { id: 'alarm-levels', name: '告警等级', path: '/alarm-levels' },
      { id: 'alarm-notifications', name: '通知配置', path: '/alarm-notifications' },
    ]
  },
  {
    id: 'system',
    name: '系统配置',
    icon: Tools,
    children: [
      { id: 'global', name: '全局设置', path: '/settings/global' },
      { id: 'redis', name: 'Redis配置', path: '/settings/redis' },
      { id: 'influxdb', name: 'InfluxDB配置', path: '/settings/influxdb' },
      { id: 'backup', name: '备份恢复', path: '/settings/backup' },
      { id: 'templates', name: '配置模板', path: '/settings/templates' },
    ]
  }
];

// 顶部操作
const handleImportConfig = () => {
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = '.json,.yaml,.yml';
  input.onchange = async (e) => {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (!file) return;
    // TODO: 实现配置导入
    ElMessage.info('配置导入功能开发中...');
  };
  input.click();
};

const handleExportConfig = () => {
  // TODO: 实现配置导出
  ElMessage.info('配置导出功能开发中...');
};

const handleValidateAll = () => {
  // TODO: 实现全部验证
  ElMessage.info('配置验证功能开发中...');
};

// 用户菜单
const showUserMenu = ref(false);
const showLogViewer = ref(false);

const handleLogout = () => {
  ElMessageBox.confirm('确定要退出系统吗？', '提示', {
    confirmButtonText: '确定',
    cancelButtonText: '取消',
    type: 'warning',
  }).then(() => {
    // TODO: 实现退出逻辑
    ElMessage.success('已退出');
  });
};

onMounted(() => {
  configStore.fetchAllServices();
});

// 页面过渡动画钩子
function onBeforeEnter(el: Element) {
  (el as HTMLElement).style.opacity = '0';
}

function onEnter(el: Element, done: () => void) {
  setTimeout(() => {
    (el as HTMLElement).style.opacity = '1';
    done();
  }, 100);
}

function onLeave(el: Element, done: () => void) {
  (el as HTMLElement).style.opacity = '0';
  setTimeout(done, 300);
}
</script>

<template>
  <el-container class="app-container">
    <!-- 顶部导航栏 -->
    <el-header class="app-header">
      <div class="header-left">
        <div class="logo">
          <svg width="32" height="32" viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg" class="logo-icon">
            <defs>
              <linearGradient id="logoGradient" x1="0%" y1="0%" x2="100%" y2="100%">
                <stop offset="0%" style="stop-color:var(--primary-color);stop-opacity:1" />
                <stop offset="100%" style="stop-color:var(--accent-cyan);stop-opacity:1" />
              </linearGradient>
            </defs>
            <rect width="32" height="32" rx="6" fill="url(#logoGradient)"/>
            <path d="M8 12L16 20L24 12" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="logo-path"/>
            <path d="M16 12V20" stroke="white" stroke-width="2" stroke-linecap="round" class="logo-path"/>
          </svg>
          <h1>VoltageEMS 配置中心</h1>
        </div>
      </div>

      <div class="header-center">
        <!-- 主导航菜单 -->
        <el-menu 
          mode="horizontal" 
          :default-active="activeMenu"
          :router="true"
          class="top-menu"
        >
          <el-menu-item index="/">
            <el-icon><Monitor /></el-icon>
            <span>仪表盘</span>
          </el-menu-item>
          
          <el-sub-menu v-for="category in configCategories" :key="category.id" :index="category.id">
            <template #title>
              <el-icon><component :is="category.icon" /></el-icon>
              <span>{{ category.name }}</span>
            </template>
            <el-menu-item 
              v-for="item in category.children" 
              :key="item.id"
              :index="item.path"
            >
              {{ item.name }}
            </el-menu-item>
          </el-sub-menu>
        </el-menu>
      </div>

      <div class="header-right">
        <!-- 全局操作按钮 -->
        <el-button-group class="action-group">
          <el-tooltip content="导入配置">
            <el-button :icon="Upload" @click="handleImportConfig" />
          </el-tooltip>
          <el-tooltip content="导出配置">
            <el-button :icon="Download" @click="handleExportConfig" />
          </el-tooltip>
          <el-tooltip content="验证所有配置">
            <el-button :icon="CircleCheck" @click="handleValidateAll" />
          </el-tooltip>
        </el-button-group>

        <!-- 通知图标 -->
        <el-badge :value="3" class="notification-badge">
          <el-button :icon="Bell" circle text />
        </el-badge>

        <!-- 用户菜单 -->
        <el-dropdown @visible-change="(val) => showUserMenu = val">
          <el-button circle text class="user-button">
            <el-avatar :icon="User" :size="32" />
            <el-icon class="el-icon--right">
              <ArrowDown />
            </el-icon>
          </el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item>个人设置</el-dropdown-item>
              <el-dropdown-item>系统信息</el-dropdown-item>
              <el-dropdown-item @click="showLogViewer = true">查看日志</el-dropdown-item>
              <el-dropdown-item divided @click="handleLogout">退出系统</el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
      </div>
    </el-header>

    <!-- 主内容区 -->
    <el-main class="app-main">
      <!-- 页面工具栏 -->
      <div class="page-toolbar">
        <div class="toolbar-left">
          <h2>{{ $route.meta.title || '仪表盘' }}</h2>
          <span class="page-description" v-if="$route.meta.description">
            {{ $route.meta.description }}
          </span>
        </div>
        <div class="toolbar-right">
          <slot name="toolbar-actions"></slot>
        </div>
      </div>

      <!-- 路由内容 -->
      <div class="page-content">
        <router-view v-slot="{ Component, route }">
          <transition 
            :name="route.meta.transition || 'slide-fade'" 
            mode="out-in"
            @before-enter="onBeforeEnter"
            @enter="onEnter"
            @leave="onLeave"
          >
            <keep-alive>
              <component :is="Component" :key="route.path" />
            </keep-alive>
          </transition>
        </router-view>
      </div>
    </el-main>
  </el-container>
  
  <!-- 日志查看器对话框 -->
  <el-dialog 
    v-model="showLogViewer" 
    title="系统日志" 
    width="80%" 
    :close-on-click-modal="false"
  >
    <LogViewer style="height: 600px;" />
  </el-dialog>
</template>

<style lang="scss" scoped>
.app-container {
  height: 100vh;
  background: var(--bg-primary);
  display: flex;
  flex-direction: column;
  position: relative;
}

// 顶部导航栏
.app-header {
  height: 60px;
  background: var(--glass-bg);
  backdrop-filter: var(--glass-blur);
  border-bottom: 1px solid var(--glass-border);
  box-shadow: 0 2px 16px rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 24px;
  position: relative;
  z-index: 100;
  flex-shrink: 0;

  .header-left {
    display: flex;
    align-items: center;
    min-width: 240px;

    .logo {
      display: flex;
      align-items: center;
      gap: 12px;
      cursor: pointer;
      transition: transform 0.3s ease;
      
      &:hover {
        transform: scale(1.05);
        
        .logo-icon {
          filter: drop-shadow(0 0 20px var(--primary-color));
          animation: rotateLogo 0.6s ease;
        }
      }
      
      .logo-icon {
        transition: all 0.3s ease;
        
        .logo-path {
          animation: drawPath 2s ease-out;
        }
      }

      h1 {
        margin: 0;
        font-size: 18px;
        font-weight: 600;
        background: var(--primary-gradient);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        background-clip: text;
        white-space: nowrap;
        position: relative;
        
        &::after {
          content: attr(data-text);
          position: absolute;
          top: 0;
          left: 0;
          background: linear-gradient(90deg, var(--accent-cyan) 0%, var(--accent-purple) 100%);
          -webkit-background-clip: text;
          -webkit-text-fill-color: transparent;
          background-clip: text;
          opacity: 0;
          transition: opacity 0.3s ease;
        }
        
        &:hover::after {
          opacity: 1;
        }
      }
    }
    
    @keyframes rotateLogo {
      0% { transform: rotate(0deg); }
      100% { transform: rotate(360deg); }
    }
    
    @keyframes drawPath {
      from {
        stroke-dasharray: 100;
        stroke-dashoffset: 100;
      }
      to {
        stroke-dasharray: 100;
        stroke-dashoffset: 0;
      }
    }
  }

  .header-center {
    flex: 1;
    display: flex;
    justify-content: center;
    overflow: hidden;

    .top-menu {
      border: none;
      height: 60px;
      background: transparent;
      
      --el-menu-bg-color: transparent;
      --el-menu-text-color: var(--text-secondary);
      --el-menu-hover-text-color: var(--text-primary);
      --el-menu-active-color: var(--primary-color);

      :deep(.el-menu-item) {
        height: 60px;
        line-height: 60px;
        border-bottom: 3px solid transparent;
        transition: all 0.3s;
        padding: 0 20px;
        color: var(--text-secondary);

        &.is-active {
          border-bottom-color: var(--primary-color);
          color: var(--primary-color);
          background: rgba(98, 106, 239, 0.1);
          text-shadow: 0 0 10px rgba(98, 106, 239, 0.5);
        }

        &:hover {
          background: rgba(255, 255, 255, 0.05);
          color: var(--text-primary);
        }
      }

      :deep(.el-sub-menu__title) {
        height: 60px;
        line-height: 60px;
        border-bottom: 3px solid transparent;
        padding: 0 20px;
        color: var(--text-secondary);
        transition: all 0.3s ease;
        position: relative;
        overflow: hidden;
        
        &::before {
          content: '';
          position: absolute;
          top: 0;
          left: -100%;
          width: 100%;
          height: 100%;
          background: linear-gradient(90deg, transparent 0%, rgba(98, 106, 239, 0.2) 50%, transparent 100%);
          transition: left 0.5s ease;
        }

        &:hover {
          background: rgba(255, 255, 255, 0.05);
          color: var(--text-primary);
          
          &::before {
            left: 100%;
          }
        }
      }

      :deep(.el-sub-menu.is-active > .el-sub-menu__title) {
        color: var(--el-color-primary);
        border-bottom-color: var(--el-color-primary);
      }
    }
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 16px;
    min-width: 280px;
    justify-content: flex-end;

    .action-group {
      :deep(.el-button) {
        border: 1px solid var(--glass-border);
        background: var(--glass-bg);
        backdrop-filter: var(--glass-blur);
        color: var(--text-primary);
        
        &:hover {
          background: rgba(98, 106, 239, 0.2);
          border-color: var(--primary-color);
          transform: translateY(-2px);
          box-shadow: 0 4px 16px rgba(98, 106, 239, 0.3);
        }
      }
    }

    .notification-badge {
      :deep(.el-badge__content) {
        top: 8px;
        right: 8px;
      }
    }

    .user-button {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 4px 8px;
      
      &:hover {
        background: rgba(98, 106, 239, 0.1);
        transform: scale(1.05);
      }
    }
  }
}

// 主内容区
.app-main {
  flex: 1;
  padding: 0;
  overflow-y: auto;
  background: var(--bg-primary);
  position: relative;

  .page-toolbar {
    background: var(--glass-bg);
    backdrop-filter: var(--glass-blur);
    padding: 20px 32px;
    border-bottom: 1px solid var(--glass-border);
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    position: sticky;
    top: 0;
    z-index: 10;

    .toolbar-left {
      flex: 1;

      h2 {
        margin: 0 0 4px 0;
        font-size: 24px;
        font-weight: 600;
        color: var(--text-primary);
        line-height: 32px;
        text-shadow: 0 0 20px rgba(98, 106, 239, 0.3);
      }

      .page-description {
        display: block;
        font-size: 14px;
        color: var(--text-secondary);
        line-height: 20px;
      }
    }

    .toolbar-right {
      display: flex;
      gap: 12px;
      align-items: center;
      margin-left: 32px;
    }
  }

  .page-content {
    padding: 32px;
    max-width: 1400px;
    margin: 0 auto;
    width: 100%;
    box-sizing: border-box;
    position: relative;
    
    // 页面入场动画
    > * {
      animation: pageEnter 0.5s ease-out;
    }
  }
}

@keyframes pageEnter {
  from {
    opacity: 0;
    transform: translateY(30px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

// 过渡动画
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

// 滑动淡入淡出
.slide-fade-enter-active,
.slide-fade-leave-active {
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.slide-fade-enter-from {
  opacity: 0;
  transform: translateY(20px);
  filter: blur(4px);
}

.slide-fade-leave-to {
  opacity: 0;
  transform: translateY(-20px);
  filter: blur(4px);
}

// 缩放淡入淡出
.scale-fade-enter-active,
.scale-fade-leave-active {
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.scale-fade-enter-from {
  opacity: 0;
  transform: scale(0.95);
  filter: blur(2px);
}

.scale-fade-leave-to {
  opacity: 0;
  transform: scale(1.05);
  filter: blur(2px);
}

// 旋转淡入淡出
.rotate-fade-enter-active,
.rotate-fade-leave-active {
  transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1);
  transform-origin: center center;
}

.rotate-fade-enter-from {
  opacity: 0;
  transform: scale(0.9) rotateY(-90deg);
}

.rotate-fade-leave-to {
  opacity: 0;
  transform: scale(0.9) rotateY(90deg);
}
</style>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Hiragino Sans GB',
    'Microsoft YaHei', 'Helvetica Neue', Helvetica, Arial, sans-serif;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

html {
  color-scheme: dark;
}

#app {
  height: 100vh;
  background: var(--bg-primary);
}
</style>