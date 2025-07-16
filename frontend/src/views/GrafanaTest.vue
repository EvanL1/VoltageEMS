<template>
  <div class="grafana-test">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>Grafana 测试页面</span>
          <el-button type="primary" size="small" @click="openGrafana">
            在新窗口打开 Grafana
          </el-button>
        </div>
      </template>

      <el-alert
        title="Grafana 已启动"
        type="success"
        :closable="false"
        show-icon
        style="margin-bottom: 20px"
      >
        <template #default>
          <p>Grafana 服务运行在: http://localhost:3000</p>
          <p>用户名: admin / 密码: admin</p>
        </template>
      </el-alert>

      <div class="iframe-container">
        <iframe
          :src="grafanaUrl"
          width="100%"
          height="600px"
          frameborder="0"
        ></iframe>
      </div>

      <el-divider />

      <h3>Grafana 功能测试</h3>
      <el-space direction="vertical" style="width: 100%">
        <el-button @click="testGrafanaApi">测试 Grafana API</el-button>
        <el-button @click="createTestDashboard">创建测试仪表板</el-button>
        <el-button @click="listDashboards">列出所有仪表板</el-button>
      </el-space>

      <div v-if="apiResponse" class="api-response">
        <h4>API 响应：</h4>
        <pre>{{ JSON.stringify(apiResponse, null, 2) }}</pre>
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import axios from 'axios'
import { ElMessage } from 'element-plus'

// Grafana URL - 直接访问，绕过认证
const grafanaUrl = ref('http://localhost:3000/login')
const apiResponse = ref(null)

const openGrafana = () => {
  window.open('http://localhost:3000', '_blank')
}

const testGrafanaApi = async () => {
  try {
    // 使用基础认证测试 API
    const response = await axios.get('http://localhost:3000/api/health', {
      auth: {
        username: 'admin',
        password: 'admin'
      }
    })
    apiResponse.value = response.data
    ElMessage.success('Grafana API 连接成功！')
  } catch (error) {
    console.error('API test failed:', error)
    ElMessage.error('API 测试失败：' + error.message)
  }
}

const createTestDashboard = async () => {
  try {
    const dashboard = {
      dashboard: {
        title: 'VoltageEMS 测试仪表板',
        panels: [
          {
            id: 1,
            title: '示例面板',
            type: 'text',
            gridPos: { x: 0, y: 0, w: 24, h: 8 },
            options: {
              content: '# VoltageEMS Grafana 集成测试\n\n这是一个测试仪表板，用于验证 Grafana 集成功能。'
            }
          }
        ],
        schemaVersion: 30,
        version: 0
      },
      overwrite: false
    }

    const response = await axios.post(
      'http://localhost:3000/api/dashboards/db',
      dashboard,
      {
        auth: {
          username: 'admin',
          password: 'admin'
        }
      }
    )
    
    apiResponse.value = response.data
    ElMessage.success('测试仪表板创建成功！')
    
    // 更新 iframe 显示新创建的仪表板
    if (response.data.uid) {
      grafanaUrl.value = `http://localhost:3000/d/${response.data.uid}`
    }
  } catch (error) {
    console.error('Create dashboard failed:', error)
    ElMessage.error('创建仪表板失败：' + error.message)
  }
}

const listDashboards = async () => {
  try {
    const response = await axios.get('http://localhost:3000/api/search?type=dash-db', {
      auth: {
        username: 'admin',
        password: 'admin'
      }
    })
    apiResponse.value = response.data
    ElMessage.success(`找到 ${response.data.length} 个仪表板`)
  } catch (error) {
    console.error('List dashboards failed:', error)
    ElMessage.error('列出仪表板失败：' + error.message)
  }
}
</script>

<style scoped>
.grafana-test {
  height: 100%;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.iframe-container {
  border: 1px solid #dcdfe6;
  border-radius: 4px;
  overflow: hidden;
}

.api-response {
  margin-top: 20px;
  padding: 16px;
  background-color: #f5f7fa;
  border-radius: 4px;
}

.api-response pre {
  margin: 0;
  font-size: 12px;
  overflow-x: auto;
}

h3 {
  margin: 20px 0 16px 0;
  color: #303133;
}
</style>