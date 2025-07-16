import { defineStore } from 'pinia'
import axios from 'axios'

// 模拟数据，用于前端开发
const mockConfigs = {
  modsrv: {
    service_name: 'modsrv',
    log_level: 'info',
    log_path: '/var/log/ems',
    redis: {
      host: 'redis',
      port: 6379,
      db: 0
    },
    model: {
      update_interval_ms: 1000,
      enabled: true,
      params: {
        param1: 'value1',
        param2: 'value2'
      }
    }
  },
  netsrv: {
    service_name: 'netsrv',
    log_level: 'info',
    log_path: '/var/log/ems',
    redis: {
      host: 'redis',
      port: 6379,
      db: 0
    },
    mqtt: {
      broker: 'mosquitto',
      port: 1883,
      topic_prefix: 'ems/'
    }
  },
  comsrv: {
    service_name: 'comsrv',
    log_level: 'info',
    log_path: '/var/log/ems',
    devices: [
      {
        id: 'device1',
        type: 'modbus',
        address: '192.168.1.100',
        port: 502
      },
      {
        id: 'device2',
        type: 'modbus',
        address: '192.168.1.101',
        port: 502
      }
    ]
  },
  hissrv: {
    service_name: 'hissrv',
    log_level: 'info',
    log_path: '/var/log/ems',
    influxdb: {
      url: 'http://influxdb:8086',
      token: 'your-token',
      org: 'voltage',
      bucket: 'ems'
    }
  },
  mosquitto: `# Mosquitto 配置文件

# 基本配置
listener 1883
allow_anonymous true

# 持久化设置
persistence true
persistence_location /mosquitto/data/

# 日志设置
log_dest file /mosquitto/log/mosquitto.log
log_type all`
}

export const useConfigStore = defineStore('config', {
  state: () => ({
    configs: {
      modsrv: null,
      netsrv: null,
      comsrv: null,
      hissrv: null,
      mosquitto: null
    },
    loading: false,
    error: null
  }),

  getters: {
    getConfig: (state) => (service) => state.configs[service],
    isLoading: (state) => state.loading,
    hasError: (state) => state.error !== null
  },

  actions: {
    async fetchConfig(service) {
      this.loading = true
      this.error = null
      
      try {
        // 检查是否有后端 API
        const useBackend = false // 设置为 false 使用模拟数据
        
        if (useBackend) {
          const response = await axios.get(`/api/config/${service}`)
          this.configs[service] = response.data
        } else {
          // 使用模拟数据
          await new Promise(resolve => setTimeout(resolve, 500)) // 模拟网络延迟
          this.configs[service] = mockConfigs[service]
        }
      } catch (error) {
        this.error = error.message || 'Failed to fetch config'
        console.error(error)
      } finally {
        this.loading = false
      }
    },

    async saveConfig(service, config) {
      this.loading = true
      this.error = null
      
      try {
        // 检查是否有后端 API
        const useBackend = false // 设置为 false 使用模拟数据
        
        if (useBackend) {
          await axios.post(`/api/config/${service}`, { config })
        } else {
          // 模拟保存
          await new Promise(resolve => setTimeout(resolve, 500)) // 模拟网络延迟
          mockConfigs[service] = config
        }
        
        this.configs[service] = config
      } catch (error) {
        this.error = error.message || 'Failed to save config'
        console.error(error)
        throw error
      } finally {
        this.loading = false
      }
    }
  }
})