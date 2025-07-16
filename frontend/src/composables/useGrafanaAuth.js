import { ref } from 'vue'
import axios from 'axios'

const isAuthenticated = ref(false)

export function useGrafanaAuth() {
  const ensureGrafanaAuth = async () => {
    if (isAuthenticated.value) return

    try {
      // 检查 sessionStorage 中的 API Key
      const existingKey = sessionStorage.getItem('grafana_api_key')
      if (existingKey) {
        // 验证 key 是否有效
        try {
          await axios.get('/grafana/api/org', {
            headers: { Authorization: `Bearer ${existingKey}` }
          })
          isAuthenticated.value = true
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

      const apiKey = response.data.key
      sessionStorage.setItem('grafana_api_key', apiKey)

      // 设置 Grafana session cookie
      document.cookie = `grafana_session=${apiKey}; path=/grafana; max-age=86400`
      
      isAuthenticated.value = true
    } catch (error) {
      console.error('Grafana auth failed:', error)
      throw new Error('无法建立 Grafana 连接')
    }
  }

  return {
    ensureGrafanaAuth,
    isAuthenticated
  }
}