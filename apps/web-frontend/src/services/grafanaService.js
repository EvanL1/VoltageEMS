import axios from 'axios'

class GrafanaService {
  constructor() {
    this.grafanaBaseUrl = '/grafana'
    this.apiKey = null
  }

  // 确保认证
  async ensureAuth() {
    // 检查是否已有有效的 API Key
    const existingKey = sessionStorage.getItem('grafana_api_key')
    if (existingKey) {
      this.apiKey = existingKey
      try {
        // 验证 key 是否仍然有效
        await this.grafanaRequest('/api/org')
        return
      } catch {
        // Key 无效，继续创建新的
      }
    }

    // 创建新的 API Key
    const response = await axios.post('/api/grafana/auth/create-key', {
      name: `web-session-${Date.now()}`,
      role: 'Viewer',
      secondsToLive: 86400 // 24小时
    })

    this.apiKey = response.data.key
    sessionStorage.setItem('grafana_api_key', this.apiKey)

    // 设置 Grafana session cookie
    document.cookie = `grafana_session=${this.apiKey}; path=/grafana; max-age=86400`
  }

  // 发送 Grafana API 请求
  async grafanaRequest(path, options = {}) {
    if (!this.apiKey) {
      throw new Error('Grafana API Key not initialized')
    }

    const config = {
      ...options,
      headers: {
        'Authorization': `Bearer ${this.apiKey}`,
        'Content-Type': 'application/json',
        ...(options.headers || {})
      }
    }

    const response = await axios({
      url: `${this.grafanaBaseUrl}${path}`,
      ...config
    })

    return response.data
  }

  // 获取所有仪表板
  async getDashboards() {
    return this.grafanaRequest('/api/search?type=dash-db')
  }

  // 获取仪表板详情
  async getDashboard(uid) {
    const data = await this.grafanaRequest(`/api/dashboards/uid/${uid}`)
    return data.dashboard
  }

  // 创建仪表板
  async createDashboard(dashboard, folderUid) {
    const payload = {
      dashboard,
      folderUid,
      overwrite: false,
      message: 'Created from VoltageEMS'
    }

    return this.grafanaRequest('/api/dashboards/db', {
      method: 'POST',
      data: payload
    })
  }

  // 创建快照
  async createSnapshot(dashboardUid, name) {
    const dashboard = await this.getDashboard(dashboardUid)
    
    const payload = {
      dashboard,
      name,
      expires: 3600 // 1小时后过期
    }

    return this.grafanaRequest('/api/snapshots', {
      method: 'POST',
      data: payload
    })
  }

  // 构建仪表板 URL
  buildDashboardUrl(uid, params = {}) {
    const searchParams = new URLSearchParams()

    if (params.orgId) searchParams.append('orgId', params.orgId)
    if (params.from) searchParams.append('from', params.from)
    if (params.to) searchParams.append('to', params.to)
    if (params.theme) searchParams.append('theme', params.theme)
    if (params.refresh) searchParams.append('refresh', params.refresh)
    if (params.kiosk !== undefined) {
      searchParams.append('kiosk', params.kiosk === true ? '1' : params.kiosk)
    }

    // 添加变量
    if (params.variables) {
      Object.entries(params.variables).forEach(([key, value]) => {
        searchParams.append(`var-${key}`, value)
      })
    }

    const queryString = searchParams.toString()
    return `${this.grafanaBaseUrl}/d/${uid}${queryString ? '?' + queryString : ''}`
  }
}

export const grafanaService = new GrafanaService()