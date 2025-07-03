import axios from 'axios'
import grafanaConfig from '@/config/grafana'

class UnifiedGrafanaService {
  constructor() {
    this.config = grafanaConfig
    this.apiClient = axios.create({
      baseURL: this.config.baseUrl,
      timeout: 10000,
      headers: {
        'Content-Type': 'application/json'
      }
    })
    
    // 如果有预配置的 API Key，直接使用
    const preConfiguredApiKey = process.env.VUE_APP_GRAFANA_API_KEY
    if (preConfiguredApiKey) {
      this.setApiKey(preConfiguredApiKey)
    }
    
    // 请求拦截器
    this.apiClient.interceptors.request.use(
      config => {
        const apiKey = this.getApiKey()
        if (apiKey) {
          config.headers[this.config.auth.apiKeyHeader] = `Bearer ${apiKey}`
        }
        return config
      },
      error => {
        return Promise.reject(error)
      }
    )
    
    // 响应拦截器
    this.apiClient.interceptors.response.use(
      response => response,
      error => {
        if (error.response?.status === 401) {
          this.clearAuth()
          window.dispatchEvent(new CustomEvent('grafana-auth-failed'))
        }
        return Promise.reject(error)
      }
    )
  }
  
  // 认证相关方法
  async authenticate(username, password) {
    try {
      const response = await this.apiClient.post('/api/auth/login', {
        username,
        password
      })
      
      if (response.data.token) {
        this.setApiKey(response.data.token)
        return { success: true, token: response.data.token }
      }
      
      return { success: false, error: 'No token received' }
    } catch (error) {
      console.error('Grafana authentication failed:', error)
      return { 
        success: false, 
        error: error.response?.data?.message || error.message 
      }
    }
  }
  
  setApiKey(apiKey) {
    sessionStorage.setItem('grafana_api_key', apiKey)
  }
  
  getApiKey() {
    return sessionStorage.getItem('grafana_api_key')
  }
  
  clearAuth() {
    sessionStorage.removeItem('grafana_api_key')
    // 清除 cookie
    document.cookie = `${this.config.auth.cookieName}=; Path=/; Expires=Thu, 01 Jan 1970 00:00:01 GMT;`
  }
  
  isAuthenticated() {
    return !!this.getApiKey()
  }
  
  // 仪表板相关方法
  async getDashboards() {
    try {
      const response = await this.apiClient.get('/api/search?type=dash-db')
      return response.data
    } catch (error) {
      console.error('Failed to fetch dashboards:', error)
      throw error
    }
  }
  
  async getDashboardByUid(uid) {
    try {
      const response = await this.apiClient.get(`/api/dashboards/uid/${uid}`)
      return response.data
    } catch (error) {
      console.error(`Failed to fetch dashboard ${uid}:`, error)
      throw error
    }
  }
  
  // 构建嵌入 URL
  buildEmbedUrl(dashboardUid, options = {}) {
    const params = new URLSearchParams()
    
    // 合并默认配置和自定义选项
    const embedOptions = { ...this.config.embed, ...options }
    
    // 主题
    params.append('theme', embedOptions.theme)
    
    // 时间范围
    params.append('from', embedOptions.timeRange?.from || embedOptions.from || 'now-6h')
    params.append('to', embedOptions.timeRange?.to || embedOptions.to || 'now')
    
    // 刷新间隔
    if (embedOptions.refresh) {
      params.append('refresh', embedOptions.refresh)
    }
    
    // UI 控制选项
    const kioskParams = []
    if (embedOptions.hideNav) kioskParams.push('tv')
    if (embedOptions.hideSidebar) params.append('kiosk', kioskParams.join('&'))
    
    // 变量
    if (embedOptions.vars) {
      Object.entries(embedOptions.vars).forEach(([key, value]) => {
        params.append(`var-${key}`, value)
      })
    }
    
    // 面板 ID
    if (embedOptions.panelId) {
      params.append('viewPanel', embedOptions.panelId)
    }
    
    return `${this.config.baseUrl}/d/${dashboardUid}?${params.toString()}`
  }
  
  // 数据源相关方法
  async getDataSources() {
    try {
      const response = await this.apiClient.get('/api/datasources')
      return response.data
    } catch (error) {
      console.error('Failed to fetch data sources:', error)
      throw error
    }
  }
  
  // 查询数据
  async query(datasourceId, query) {
    try {
      const response = await this.apiClient.post('/api/ds/query', {
        queries: [{
          datasourceId,
          ...query
        }]
      })
      return response.data
    } catch (error) {
      console.error('Query failed:', error)
      throw error
    }
  }
  
  // 健康检查
  async healthCheck() {
    try {
      const response = await this.apiClient.get('/api/health')
      return response.data
    } catch (error) {
      console.error('Health check failed:', error)
      return { status: 'error', message: error.message }
    }
  }
  
  // 获取组织信息
  async getOrganization() {
    try {
      const response = await this.apiClient.get('/api/org')
      return response.data
    } catch (error) {
      console.error('Failed to fetch organization:', error)
      throw error
    }
  }
  
  // 获取用户信息
  async getCurrentUser() {
    try {
      const response = await this.apiClient.get('/api/user')
      return response.data
    } catch (error) {
      console.error('Failed to fetch user info:', error)
      throw error
    }
  }
  
  // 重试机制
  async retryOperation(operation, maxRetries = this.config.performance.maxRetries) {
    let lastError
    
    for (let i = 0; i < maxRetries; i++) {
      try {
        return await operation()
      } catch (error) {
        lastError = error
        if (i < maxRetries - 1) {
          await new Promise(resolve => 
            setTimeout(resolve, this.config.performance.retryDelay * (i + 1))
          )
        }
      }
    }
    
    throw lastError
  }
  
  // 获取配置的仪表板
  getConfiguredDashboards() {
    return Object.entries(this.config.dashboards).map(([key, dashboard]) => ({
      key,
      uid: dashboard.uid,
      name: dashboard.name,
      refreshInterval: dashboard.refreshInterval
    }))
  }
  
  // 获取本地化消息
  getMessage(key, locale = 'zh') {
    const message = this.config.messages[key]
    if (typeof message === 'object') {
      return message[locale] || message.en || key
    }
    return message || key
  }
}

// 导出单例
export default new UnifiedGrafanaService()