<template>
  <div class="home">
    <el-row :gutter="20">
      <el-col :span="24">
        <el-card class="welcome-card">
          <h1>欢迎使用能源管理系统配置平台</h1>
          <p>本平台提供了对 EMS 系统各组件配置文件的集中管理功能。</p>
        </el-card>
      </el-col>
    </el-row>
    
    <el-row :gutter="20" class="mt-20">
      <el-col :span="8" v-for="service in services" :key="service.id">
        <el-card class="service-card">
          <template #header>
            <div class="card-header">
              <h3>{{ service.name }}</h3>
            </div>
          </template>
          <div class="card-content">
            <p>{{ service.description }}</p>
            <el-button type="primary" @click="goToConfig(service.route)">
              管理配置
            </el-button>
          </div>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script>
export default {
  name: 'HomeView',
  data() {
    return {
      services: [
        {
          id: 'modsrv',
          name: 'modsrv 服务',
          description: '模型服务，负责执行实时模型计算和控制策略',
          route: '/config/modsrv'
        },
        {
          id: 'netsrv',
          name: 'netsrv 服务',
          description: '网络服务，负责将数据通过多种协议上送到外部系统',
          route: '/config/netsrv'
        },
        {
          id: 'comsrv',
          name: 'comsrv 服务',
          description: '通信服务，负责与设备通信，采集实时数据',
          route: '/config/comsrv'
        },
        {
          id: 'hissrv',
          name: 'hissrv 服务',
          description: '历史数据服务，负责将实时数据存储到时序数据库',
          route: '/config/hissrv'
        },
        {
          id: 'mosquitto',
          name: 'Mosquitto 服务',
          description: 'MQTT 消息代理，用于设备通信和数据传输',
          route: '/config/mosquitto'
        }
      ]
    }
  },
  methods: {
    goToConfig(route) {
      this.$router.push(route)
    }
  }
}
</script>

<style scoped>
.home {
  padding: 20px;
}

.welcome-card {
  margin-bottom: 20px;
  text-align: center;
}

.mt-20 {
  margin-top: 20px;
}

.service-card {
  height: 200px;
  margin-bottom: 20px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.card-content {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  height: 120px;
}
</style> 