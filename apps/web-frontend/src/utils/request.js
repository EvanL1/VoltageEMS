import axios from 'axios'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useUserStore } from '@/stores/user'
import router from '@/router'

// 创建axios实例
const service = axios.create({
  baseURL: process.env.VUE_APP_BASE_API || '/api',
  timeout: 30000
})

// 模拟响应数据（开发模式）
const mockResponses = {
  '/api/auth/login': (config) => {
    const { username, password } = JSON.parse(config.data)
    const users = {
      operator: { role: 'operator', name: 'Operator', id: 1 },
      engineer: { role: 'engineer', name: 'Engineer', id: 2 },
      admin: { role: 'admin', name: 'Admin', id: 3 }
    }
    
    if (users[username] && password === `${username}123`) {
      return {
        data: {
          token: `mock-token-${username}`,
          userInfo: users[username]
        }
      }
    }
    throw new Error('Invalid username or password')
  },
  
  '/api/auth/userinfo': () => {
    const token = localStorage.getItem('token')
    if (!token) throw new Error('Not logged in')
    
    const username = token.replace('mock-token-', '')
    const users = {
      operator: { role: 'operator', name: 'Operator', id: 1, avatar: null },
      engineer: { role: 'engineer', name: 'Engineer', id: 2, avatar: null },
      admin: { role: 'admin', name: 'Admin', id: 3, avatar: null }
    }
    
    return {
      data: {
        userInfo: users[username],
        permissions: []
      }
    }
  },
  
  '/api/alarms': () => ({
    data: {
      alarms: [
        {
          id: 1,
          level: 'critical',
          message: 'Inverter Overload',
          device: 'PCS-01',
          time: new Date().toISOString(),
          status: 'active'
        },
        {
          id: 2,
          level: 'major',
          message: 'Battery Temperature Too High',
          device: 'BMS-01',
          time: new Date(Date.now() - 600000).toISOString(),
          status: 'acknowledged'
        }
      ],
      statistics: {
        critical: 2,
        major: 5,
        minor: 8,
        warning: 0,
        info: 12
      },
      activeCount: 7
    }
  })
}

// 请求拦截器
service.interceptors.request.use(
  config => {
    const userStore = useUserStore()
    if (userStore.token) {
      config.headers['Authorization'] = `Bearer ${userStore.token}`
    }
    return config
  },
  error => {
    console.error('Request error:', error)
    return Promise.reject(error)
  }
)

// 响应拦截器
service.interceptors.response.use(
  response => {
    // 如果是开发模式且有模拟数据，返回模拟数据
    if (process.env.NODE_ENV === 'development') {
      const mockHandler = mockResponses[response.config.url]
      if (mockHandler) {
        try {
          return mockHandler(response.config)
        } catch (error) {
          return Promise.reject(error)
        }
      }
    }
    
    return response.data
  },
  error => {
    // 在开发模式下，如果有模拟数据，返回模拟数据
    if (process.env.NODE_ENV === 'development' && error.config) {
      const mockHandler = mockResponses[error.config.url]
      if (mockHandler) {
        try {
          return mockHandler(error.config)
        } catch (mockError) {
          ElMessage.error(mockError.message || 'Request failed')
          return Promise.reject(mockError)
        }
      }
    }
    
    // 处理HTTP错误
    if (error.response) {
      const { status, data } = error.response
      
      switch (status) {
        case 401:
          // Token过期或无效
          ElMessageBox.confirm(
            'Login expired, please login again',
            'Notice',
            {
              confirmButtonText: 'Login',
              cancelButtonText: 'Cancel',
              type: 'warning'
            }
          ).then(() => {
            const userStore = useUserStore()
            userStore.logout()
            router.push('/login')
          })
          break
          
        case 403:
          ElMessage.error('No permission to access this resource')
          break
          
        case 404:
          ElMessage.error('Requested resource not found')
          break
          
        case 500:
          ElMessage.error('Internal server error')
          break
          
        default:
          ElMessage.error(data?.message || 'Request failed')
      }
    } else if (error.request) {
      ElMessage.error('Network connection failed, please check your network')
    } else {
      ElMessage.error(error.message || 'Request failed')
    }
    
    return Promise.reject(error)
  }
)

export default service