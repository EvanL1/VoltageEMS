<template>
  <div class="app-container">
    <el-container>
      <el-aside width="200px">
        <el-menu
          router
          :default-active="$route.path"
          class="el-menu-vertical"
          background-color="#304156"
          text-color="#bfcbd9"
          active-text-color="#409EFF">
          <el-menu-item index="/">
            <el-icon><el-icon-menu /></el-icon>
            <span>首页</span>
          </el-menu-item>
          <el-sub-menu index="/config">
            <template #title>
              <el-icon><el-icon-setting /></el-icon>
              <span>配置管理</span>
            </template>
            <el-menu-item index="/config/modsrv">modsrv 配置</el-menu-item>
            <el-menu-item index="/config/netsrv">netsrv 配置</el-menu-item>
            <el-menu-item index="/config/comsrv">comsrv 配置</el-menu-item>
            <el-menu-item index="/config/hissrv">hissrv 配置</el-menu-item>
            <el-menu-item index="/config/mosquitto">Mosquitto 配置</el-menu-item>
          </el-sub-menu>
          <el-menu-item index="/dashboard">
            <el-icon><el-icon-data-line /></el-icon>
            <span>数据看板</span>
          </el-menu-item>
        </el-menu>
      </el-aside>
      <el-container>
        <el-header>
          <h2>能源管理系统 (EMS) 配置管理平台</h2>
        </el-header>
        <el-main>
          <router-view />
        </el-main>
        <el-footer>
          <p>© 2025 Voltage, LLC. All rights reserved.</p>
        </el-footer>
      </el-container>
    </el-container>
  </div>
</template>

<script>
export default {
  name: 'App'
}
</script>

<style>
.app-container {
  height: 100vh;
  width: 100vw;
}

.el-header {
  background-color: #409EFF;
  color: white;
  display: flex;
  align-items: center;
}

.el-footer {
  background-color: #f5f7fa;
  color: #606266;
  text-align: center;
}

.el-aside {
  background-color: #304156;
  color: #bfcbd9;
}

.el-menu-vertical {
  height: 100%;
  border-right: none;
}
</style> 