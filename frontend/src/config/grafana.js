export default {
  baseUrl: process.env.VUE_APP_GRAFANA_URL || 'http://localhost:3000',
  auth: {
    apiKeyHeader: 'Authorization',
    storageKey: 'grafana_api_key',
    userStorageKey: 'grafana_user'
  },
  dashboards: {
    default: process.env.VUE_APP_GRAFANA_DEFAULT_DASHBOARD || 'energy-overview',
    energy: {
      uid: 'energy-overview',
      name: '能源概览'
    },
    realtime: {
      uid: 'realtime-monitoring',
      name: '实时监控'
    },
    history: {
      uid: 'history-data',
      name: '历史数据'
    },
    alarm: {
      uid: 'alarm-dashboard',
      name: '告警面板'
    }
  },
  embeddedDefaults: {
    theme: 'light',
    hideControls: true,
    kiosk: 'tv'
  },
  panels: {
    timeSeriesDefaults: {
      type: 'graph',
      refresh: '5s'
    },
    gaugeDefaults: {
      type: 'stat',
      refresh: '1s'
    }
  },
  performance: {
    maxRetries: 3,
    retryDelay: 1000
  },
  messages: {
    loading: {
      zh: '加载中...',
      en: 'Loading...'
    },
    error: {
      zh: '加载失败',
      en: 'Load failed'
    },
    authenticate: {
      zh: '请先认证',
      en: 'Please authenticate first'
    }
  }
}