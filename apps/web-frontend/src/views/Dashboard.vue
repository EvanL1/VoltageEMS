<template>
  <div class="dashboard">
    <el-row :gutter="20">
      <el-col :span="24">
        <el-card class="dashboard-card">
          <template #header>
            <div class="card-header">
              <h2>系统数据看板</h2>
            </div>
          </template>
          <div class="grafana-container">
            <iframe
              :src="grafanaUrl"
              width="100%"
              height="800"
              frameborder="0"
              allowfullscreen
            ></iframe>
          </div>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script>
export default {
  name: 'DashboardView',
  data() {
    return {
      // 这里可以根据实际部署环境配置 Grafana 的 URL
      grafanaUrl: 'http://localhost:3000/d/ems-dashboard/ems-system-overview?orgId=1&refresh=5s&kiosk'
    }
  }
}
</script>

<style scoped>
.dashboard {
  padding: 20px;
}

.dashboard-card {
  margin-bottom: 20px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.grafana-container {
  height: 800px;
  overflow: hidden;
}
</style> 