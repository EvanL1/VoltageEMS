import axios from 'axios'
import type { AxiosInstance } from 'axios'
import { ElMessage } from 'element-plus'

// Create axios instance
const api: AxiosInstance = axios.create({
  baseURL: 'http://localhost:8080/api/v1',
  timeout: 10000,
  headers: {
    'Content-Type': 'application/json'
  }
})

// Request interceptor
api.interceptors.request.use(
  config => {
    // Add auth token if available
    const token = localStorage.getItem('token')
    if (token) {
      config.headers.Authorization = `Bearer ${token}`
    }
    return config
  },
  error => {
    console.error('Request error:', error)
    return Promise.reject(error)
  }
)

// Response interceptor
api.interceptors.response.use(
  response => response.data,
  error => {
    const message = error.response?.data?.message || error.message || 'Network error'
    ElMessage.error(message)
    return Promise.reject(error)
  }
)

// API methods
export const realtimeApi = {
  // Get all channel statuses
  getChannels: () => api.get('/realtime/channels'),
  
  // Get points for a specific channel
  getPoints: (channelId: number, params?: {
    point_types?: string
    limit?: number
  }) => api.get(`/realtime/channels/${channelId}/points`, { params }),
  
  // Get aggregated statistics
  getStatistics: () => api.get('/realtime/statistics')
}

export default api