import { describe, it, expect, vi } from 'vitest'
import wsManager from '../websocket'

// Mock WebSocket
const mockWebSocket = {
  send: vi.fn(),
  close: vi.fn(),
  addEventListener: vi.fn(),
  removeEventListener: vi.fn(),
  readyState: WebSocket.CONNECTING,
}

global.WebSocket = vi.fn(() => mockWebSocket) as any

// Mock user store
vi.mock('@/stores/user', () => ({
  useUserStore: vi.fn(() => ({
    isLoggedIn: true,
    token: 'test-token',
  })),
}))

// Mock element-plus
vi.mock('element-plus', () => ({
  ElMessage: {
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
  },
}))

describe('WebSocket Utils', () => {
  describe('WebSocketManager', () => {
    it('should be defined', () => {
      expect(wsManager).toBeDefined()
    })

    it('should have connection status', () => {
      expect(wsManager.status).toBeDefined()
      expect(wsManager.isConnected).toBeDefined()
    })

    it('should have connection stats', () => {
      expect(wsManager.connectionStats).toBeDefined()
      expect(wsManager.connectionStats.messageCount).toBe(0)
      expect(wsManager.connectionStats.errorCount).toBe(0)
    })

    it('should have subscription methods', () => {
      expect(wsManager.subscribe).toBeDefined()
      expect(wsManager.subscribePage).toBeDefined()
      expect(wsManager.unsubscribe).toBeDefined()
      expect(wsManager.unsubscribePage).toBeDefined()
    })

    it('should have control methods', () => {
      expect(wsManager.sendControlCommand).toBeDefined()
      expect(wsManager.setGlobalListeners).toBeDefined()
    })

    it('should have stats method', () => {
      const stats = wsManager.getStats()
      expect(stats).toBeDefined()
      expect(stats.status).toBeDefined()
      expect(stats.isConnected).toBeDefined()
    })
  })
})
