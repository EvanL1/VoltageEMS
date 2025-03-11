import { createStore } from 'vuex'
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

export default createStore({
  state: {
    configs: {
      modsrv: null,
      netsrv: null,
      comsrv: null,
      hissrv: null,
      mosquitto: null
    },
    loading: false,
    error: null
  },
  getters: {
    getConfig: (state) => (service) => {
      return state.configs[service]
    },
    isLoading: (state) => state.loading,
    hasError: (state) => state.error !== null
  },
  mutations: {
    SET_CONFIG(state, { service, config }) {
      state.configs[service] = config
    },
    SET_LOADING(state, loading) {
      state.loading = loading
    },
    SET_ERROR(state, error) {
      state.error = error
    }
  },
  actions: {
    async fetchConfig({ commit }, service) {
      commit('SET_LOADING', true)
      commit('SET_ERROR', null)
      
      try {
        // 检查是否有后端 API
        const useBackend = false // 设置为 false 使用模拟数据
        
        if (useBackend) {
          const response = await axios.get(`/api/config/${service}`)
          commit('SET_CONFIG', { service, config: response.data })
        } else {
          // 使用模拟数据
          setTimeout(() => {
            commit('SET_CONFIG', { service, config: mockConfigs[service] })
          }, 500) // 模拟网络延迟
        }
      } catch (error) {
        commit('SET_ERROR', error.message || 'Failed to fetch config')
        console.error(error)
      } finally {
        setTimeout(() => {
          commit('SET_LOADING', false)
        }, 500) // 模拟网络延迟
      }
    },
    async saveConfig({ commit }, { service, config }) {
      commit('SET_LOADING', true)
      commit('SET_ERROR', null)
      
      try {
        // 检查是否有后端 API
        const useBackend = false // 设置为 false 使用模拟数据
        
        if (useBackend) {
          await axios.post(`/api/config/${service}`, { config })
        } else {
          // 模拟保存
          setTimeout(() => {
            mockConfigs[service] = config
          }, 500) // 模拟网络延迟
        }
        
        commit('SET_CONFIG', { service, config })
      } catch (error) {
        commit('SET_ERROR', error.message || 'Failed to save config')
        console.error(error)
      } finally {
        setTimeout(() => {
          commit('SET_LOADING', false)
        }, 500) // 模拟网络延迟
      }
    }
  }
}) 