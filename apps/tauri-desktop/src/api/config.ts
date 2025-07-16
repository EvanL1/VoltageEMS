// API Configuration
export const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || 'http://localhost:8080'
export const WS_URL = import.meta.env.VITE_WS_URL || 'ws://localhost:8080/ws'

// API Endpoints
export const API_ENDPOINTS = {
  // Auth
  login: '/api/auth/login',
  logout: '/api/auth/logout',
  refresh: '/api/auth/refresh',
  
  // Channels
  channels: '/api/channels',
  channelById: (id: number) => `/api/channels/${id}`,
  
  // Points
  points: '/api/points',
  pointsByChannel: (channelId: number) => `/api/channels/${channelId}/points`,
  
  // Real-time data
  subscribe: '/api/realtime/subscribe',
  unsubscribe: '/api/realtime/unsubscribe',
  
  // Historical data
  history: '/api/history/query',
  export: '/api/history/export',
  
  // Control
  control: '/api/control/execute',
  batchControl: '/api/control/batch',
  
  // Rules
  rules: '/api/rules',
  ruleById: (id: string) => `/api/rules/${id}`,
  testRule: '/api/rules/test',
  
  // Alarms
  alarms: '/api/alarms',
  acknowledgeAlarm: (id: string) => `/api/alarms/${id}/acknowledge`,
  
  // Services
  services: '/api/services',
  serviceAction: (name: string, action: string) => `/api/services/${name}/${action}`,
  
  // Users
  users: '/api/users',
  userById: (id: number) => `/api/users/${id}`,
  roles: '/api/roles',
  permissions: '/api/permissions',
  
  // System
  logs: '/api/system/logs',
  metrics: '/api/system/metrics',
}

// Request timeout
export const REQUEST_TIMEOUT = 30000

// WebSocket reconnect settings
export const WS_RECONNECT_INTERVAL = 5000
export const WS_MAX_RECONNECT_ATTEMPTS = 10