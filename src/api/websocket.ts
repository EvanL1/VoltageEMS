import { ref, reactive } from 'vue'
import { ElMessage } from 'element-plus'

export interface WsMessage {
  type: 'Subscribe' | 'Unsubscribe' | 'DataUpdate' | 'Ping' | 'Pong' | 'Error'
  channels?: string[]
  channel?: string
  data?: any
  message?: string
}

export interface ChannelData {
  [key: string]: any
}

class WebSocketManager {
  private ws: WebSocket | null = null
  private reconnectTimer: number | null = null
  private heartbeatTimer: number | null = null
  private reconnectAttempts = 0
  private maxReconnectAttempts = 10
  private reconnectDelay = 1000
  
  // Reactive state
  public connected = ref(false)
  public channelData = reactive<ChannelData>({})
  public subscribedChannels = ref<string[]>([])
  
  private wsUrl: string
  private onDataUpdate?: (channel: string, data: any) => void
  
  constructor(baseUrl: string = 'ws://localhost:8080') {
    this.wsUrl = `${baseUrl}/ws/realtime`
  }
  
  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        this.ws = new WebSocket(this.wsUrl)
        
        this.ws.onopen = () => {
          console.log('WebSocket connected')
          this.connected.value = true
          this.reconnectAttempts = 0
          this.startHeartbeat()
          resolve()
        }
        
        this.ws.onmessage = (event) => {
          try {
            const message: WsMessage = JSON.parse(event.data)
            this.handleMessage(message)
          } catch (error) {
            console.error('Failed to parse WebSocket message:', error)
          }
        }
        
        this.ws.onerror = (error) => {
          console.error('WebSocket error:', error)
          ElMessage.error('WebSocket connection error')
        }
        
        this.ws.onclose = () => {
          console.log('WebSocket disconnected')
          this.connected.value = false
          this.stopHeartbeat()
          this.attemptReconnect()
        }
        
      } catch (error) {
        reject(error)
      }
    })
  }
  
  private handleMessage(message: WsMessage) {
    switch (message.type) {
      case 'DataUpdate':
        if (message.channel && message.data) {
          // Update reactive channel data
          this.channelData[message.channel] = message.data
          
          // Call callback if registered
          if (this.onDataUpdate) {
            this.onDataUpdate(message.channel, message.data)
          }
        }
        break
        
      case 'Pong':
        // Heartbeat response received
        break
        
      case 'Error':
        console.error('WebSocket error:', message.message)
        ElMessage.error(message.message || 'WebSocket error')
        break
    }
  }
  
  subscribe(channels: string[]): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      console.error('WebSocket not connected')
      return
    }
    
    const message: WsMessage = {
      type: 'Subscribe',
      channels
    }
    
    this.ws.send(JSON.stringify(message))
    
    // Update subscribed channels
    channels.forEach(channel => {
      if (!this.subscribedChannels.value.includes(channel)) {
        this.subscribedChannels.value.push(channel)
      }
    })
  }
  
  unsubscribe(channels: string[]): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      console.error('WebSocket not connected')
      return
    }
    
    const message: WsMessage = {
      type: 'Unsubscribe',
      channels
    }
    
    this.ws.send(JSON.stringify(message))
    
    // Update subscribed channels
    this.subscribedChannels.value = this.subscribedChannels.value.filter(
      channel => !channels.includes(channel)
    )
  }
  
  private startHeartbeat(): void {
    this.heartbeatTimer = window.setInterval(() => {
      if (this.ws && this.ws.readyState === WebSocket.OPEN) {
        const message: WsMessage = { type: 'Ping' }
        this.ws.send(JSON.stringify(message))
      }
    }, 30000) // 30 seconds
  }
  
  private stopHeartbeat(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer)
      this.heartbeatTimer = null
    }
  }
  
  private attemptReconnect(): void {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      ElMessage.error('Failed to reconnect to WebSocket')
      return
    }
    
    this.reconnectTimer = window.setTimeout(() => {
      this.reconnectAttempts++
      console.log(`Attempting to reconnect... (${this.reconnectAttempts}/${this.maxReconnectAttempts})`)
      this.connect()
    }, this.reconnectDelay * Math.pow(2, Math.min(this.reconnectAttempts, 5)))
  }
  
  disconnect(): void {
    this.stopHeartbeat()
    
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer)
      this.reconnectTimer = null
    }
    
    if (this.ws) {
      this.ws.close()
      this.ws = null
    }
    
    this.connected.value = false
    this.subscribedChannels.value = []
    Object.keys(this.channelData).forEach(key => delete this.channelData[key])
  }
  
  onDataUpdateCallback(callback: (channel: string, data: any) => void): void {
    this.onDataUpdate = callback
  }
}

// Create singleton instance
export const wsManager = new WebSocketManager()