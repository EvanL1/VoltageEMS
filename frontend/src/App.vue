<template>
  <div class="app-container">
    <!-- Left Navigation Sidebar -->
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
          <el-icon><el-icon-house /></el-icon>
          <span>Home</span>
        </el-menu-item>
        
        <el-menu-item index="/system">
          <el-icon><el-icon-setting /></el-icon>
          <span>System</span>
        </el-menu-item>
        
        <el-menu-item index="/activity">
          <el-icon><el-icon-data-line /></el-icon>
          <span>Activity</span>
        </el-menu-item>
      </el-menu>
      
      <div class="sidebar-footer">
        <p>Voltage, LLC. © 2025 - All Rights Reserved.</p>
      </div>
    </div>
    
    <!-- Main Content Area -->
    <div class="main-container">
      <!-- Header Title Bar -->
      <header class="main-header">
        <div class="header-left">
          <el-icon class="menu-toggle"><el-icon-menu /></el-icon>
          <h2>{{ pageTitle }}</h2>
        </div>
        <div class="header-right">
          <el-dropdown>
            <span class="user-info">
              <el-icon><el-icon-user /></el-icon>
              User: Voltage <el-icon><el-icon-arrow-down /></el-icon>
            </span>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item>Account Settings</el-dropdown-item>
                <el-dropdown-item>Logout</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>
      </header>
      
      <!-- Content Area -->
      <main class="main-content">
        <router-view />
      </main>
    </div>
  </div>
</template>

<script>
import { logoBase64 } from './assets/logo'

export default {
  name: 'App',
  data() {
    return {
      pageTitle: 'Home',
      logoSrc: logoBase64
    }
  },
  watch: {
    $route(to) {
      // Update page title based on route
      const routeMap = {
        '/': 'Home',
        '/system': 'System',
        '/activity': 'Activity'
      };
      
      if (to.path.startsWith('/system/')) {
        this.pageTitle = 'System';
      } else {
        this.pageTitle = routeMap[to.path] || 'Home';
      }
    }
  }
}
</script>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family: Arial, sans-serif;
}

.app-container {
  display: flex;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
}

/* Sidebar Styles */
.sidebar {
  width: 200px;
  height: 100%;
  display: flex;
  flex-direction: column;
  background-color: #3a4654;
  color: #bfcbd9;
}

.logo {
  padding: 20px;
  text-align: center;
}

.logo img {
  width: 80px;
  height: auto;
}

.el-menu-vertical {
  border-right: none;
  flex: 1;
}

.sidebar-footer {
  padding: 10px;
  font-size: 12px;
  text-align: center;
  color: #6c7983;
}

/* Main Content Area Styles */
.main-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  background-color: #f5f7fa;
}

.main-header {
  height: 60px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0 20px;
  background-color: #fff;
  box-shadow: 0 1px 4px rgba(0, 21, 41, 0.08);
}

.header-left {
  display: flex;
  align-items: center;
}

.header-left h2 {
  margin-left: 15px;
  font-size: 18px;
  font-weight: 500;
  color: #303133;
}

.menu-toggle {
  font-size: 20px;
  cursor: pointer;
  color: #606266;
}

.user-info {
  display: flex;
  align-items: center;
  cursor: pointer;
  color: #606266;
}

.main-content {
  flex: 1;
  padding: 20px;
  overflow-y: auto;
}

/* Global dropdown style fix to ensure text is fully visible */
.el-select-dropdown__item {
  white-space: normal !important;
  height: auto !important;
  padding: 8px 20px !important;
  line-height: 1.5 !important;
}

.el-dropdown-menu__item {
  white-space: normal !important;
  line-height: 1.5 !important;
  padding: 8px 20px !important;
}
</style> 