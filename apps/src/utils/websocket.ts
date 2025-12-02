/**
 * WebSocket 连接管理器
 * 负责连接生命周期、订阅管理、重连恢复、监听分发。
 * 统一管理全局与页面级订阅，断线自动恢复被动断开前的订阅。
 */

import { ref, reactive, computed } from 'vue'
import { ElMessage } from 'element-plus'
import { useUserStore } from '@/stores/user'
import type {
  ConnectionStatus,
  ClientMessage,
  ServerMessage,
  SubscriptionConfig,
  ListenerConfig,
  SubscribeMessage,
  UnsubscribeMessage,
  ControlMessage,
  PingMessage,
  CommandType,
  UnsubscribeAckMessage,
} from '@/types/websocket'

/** WebSocket 连接配置 */
interface WebSocketConfig {
  url: string
  reconnectInterval?: number
  maxReconnectAttempts?: number
  heartbeatInterval?: number
  heartbeatTimeout?: number
}

/** 待订阅信息记录，用于重连或等待 ACK */
interface PendingSubscription {
  id: string
  pageId: string
  config: SubscriptionConfig
  listeners: any
  timestamp: number
}

class WebSocketManager {
  private ws: WebSocket | null = null
  private config: WebSocketConfig
  private reconnectAttempts = 0
  private heartbeatTimer: number | null = null
  private heartbeatTimeoutTimer: number | null = null
  private reconnectTimer: number | null = null
  private messageIdCounter = 0
  private isManualDisconnect = false

  public readonly status = ref<ConnectionStatus>('disconnected')
  public readonly isConnected = computed(() => this.status.value === 'connected')
  public readonly isConnecting = computed(() => this.status.value === 'connecting')

  private globalListeners: ListenerConfig = {}

  private pageSubscriptions = new Map<
    string,
    {
      id: string
      config: SubscriptionConfig
      listeners: Partial<ListenerConfig>
    }
  >()

  private pendingSubscriptionsMap = new Map<string, PendingSubscription>()

  public readonly connectionStats = reactive({
    connectTime: 0,
    disconnectTime: 0,
    messageCount: 0,
    errorCount: 0,
    latency: 0,
  })

  /** 构造函数，合并默认配置 */
  constructor(config: WebSocketConfig) {
    this.config = {
      reconnectInterval: 2000,
      maxReconnectAttempts: Infinity,
      heartbeatInterval: 5000,
      heartbeatTimeout: 3000,
      ...config,
    }
  }

  /** 生成唯一消息 ID */
  private generateMessageId(): string {
    return `${Date.now()}-${Math.random().toString(36).substring(2, 15)}`
  }

  /** 拷贝订阅配置，避免引用被修改 */
  private cloneConfig(config: SubscriptionConfig): SubscriptionConfig {
    return {
      ...config,
      channels: [...config.channels],
      dataTypes: [...config.dataTypes],
    }
  }

  /** 判断两个订阅配置是否等价（用于去重复用） */
  private isSameSubscription(config1: SubscriptionConfig, config2: SubscriptionConfig): boolean {
    return (
      JSON.stringify([...config1.channels].sort()) ===
        JSON.stringify([...config2.channels].sort()) &&
      JSON.stringify([...config1.dataTypes].sort()) ===
        JSON.stringify([...config2.dataTypes].sort()) &&
      config1.interval === config2.interval &&
      config1.source === config2.source
    )
  }

  /** 发送订阅消息 */
  private sendSubscribeMessage(config: SubscriptionConfig, messageId: string): void {
    const subscribeMessage: SubscribeMessage = {
      id: messageId,
      type: 'subscribe',
      timestamp: '',
      data: {
        channels: config.channels,
        data_types: config.dataTypes,
        interval: config.interval,
        source: config.source,
      },
    }
    this.sendMessage(subscribeMessage)
    console.log('[WebSocket] 发送订阅', messageId, config)
  }

  /** 发送取消订阅消息 */
  private sendUnsubscribeMessage(
    channels: number[],
    source: 'inst' | 'comsrv' = 'inst',
    messageId: string,
  ): void {
    const unsubscribeMessage: UnsubscribeMessage = {
      id: messageId,
      type: 'unsubscribe',
      timestamp: '',
      data: {
        channels: channels,
        source: source,
      },
    }

    this.sendMessage(unsubscribeMessage)
    console.log('[WebSocket] 发送取消订阅', messageId, channels)
  }

  /** WebSocket 发送统一入口，补充时间戳 */
  private sendMessage(message: ClientMessage): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new Error('WebSocket is not connected')
    }
    message.timestamp = new Date().toISOString()
    this.ws.send(JSON.stringify(message))
    console.log('[WebSocket] 发送消息', message)
  }

  /** 处理待订阅队列，批量发送并记录 */
  private processPendingSubscriptions(): void {
    if (this.pendingSubscriptionsMap.size === 0) return

    console.log('[WebSocket] 处理待订阅队列:', this.pendingSubscriptionsMap.size)
    for (const [id, subInfo] of this.pendingSubscriptionsMap) {
      this.sendSubscribeMessage(subInfo.config, id)
      this.pageSubscriptions.set(subInfo.pageId, {
        id: subInfo.id,
        config: subInfo.config,
        listeners: subInfo.listeners,
      })
    }
  }

  /** 重连前将已有订阅补回待订阅队列 */
  private enqueueExistingSubscriptionsForReconnect(): void {
    for (const [pageId, record] of this.pageSubscriptions) {
      const exists = Array.from(this.pendingSubscriptionsMap.values()).some(
        (pending) =>
          pending.pageId === pageId && this.isSameSubscription(pending.config, record.config),
      )

      if (!exists) {
        const messageId = this.generateMessageId()
        this.pendingSubscriptionsMap.set(messageId, {
          id: record.id,
          pageId,
          config: this.cloneConfig(record.config),
          listeners: record.listeners,
          timestamp: Date.now(),
        })
      }
    }
  }

  /** 统一订阅入口，支持全局/页面订阅 */
  public subscribe(
    config: SubscriptionConfig,
    pageId = 'global',
    listeners: Partial<ListenerConfig> = {},
  ): string {
    const normalizedConfig = this.cloneConfig(config)
    const subscriptionId = this.generateMessageId()

    const existing = this.pageSubscriptions.get(pageId)
    if (existing && this.isSameSubscription(existing.config, normalizedConfig)) {
      existing.listeners = { ...existing.listeners, ...listeners }
      return existing.id
    }

    for (const [pendingId, subInfo] of this.pendingSubscriptionsMap) {
      if (subInfo.pageId === pageId && this.isSameSubscription(subInfo.config, normalizedConfig)) {
        this.pendingSubscriptionsMap.set(pendingId, {
          ...subInfo,
          listeners: { ...subInfo.listeners, ...listeners },
        })
        return subInfo.id
      }
    }

    this.pageSubscriptions.set(pageId, {
      id: subscriptionId,
      config: normalizedConfig,
      listeners,
    })

    const messageId = this.generateMessageId()
    this.pendingSubscriptionsMap.set(messageId, {
      id: subscriptionId,
      pageId,
      config: normalizedConfig,
      listeners,
      timestamp: Date.now(),
    })

    if (this.isConnected.value) {
      this.sendSubscribeMessage(normalizedConfig, messageId)
    }

    return subscriptionId
  }

  /** 从待订阅队列删除/截断指定页面的频道 */
  private trimPendingSubscriptions(pageId: string, channels?: number[]): void {
    for (const [pendingId, subInfo] of Array.from(this.pendingSubscriptionsMap.entries())) {
      if (subInfo.pageId !== pageId) continue

      if (!channels || channels.length === 0) {
        this.pendingSubscriptionsMap.delete(pendingId)
        continue
      }

      const remainingChannels = subInfo.config.channels.filter(
        (channel) => !channels.includes(channel),
      )
      if (remainingChannels.length === 0) {
        this.pendingSubscriptionsMap.delete(pendingId)
      } else {
        this.pendingSubscriptionsMap.set(pendingId, {
          ...subInfo,
          config: {
            ...subInfo.config,
            channels: remainingChannels,
          },
        })
      }
    }
  }

  /** 统一取消订阅入口，支持全局/页面取消 */
  public unsubscribe(
    pageId = 'global',
    channels?: number[],
    source: 'inst' | 'comsrv' = 'inst',
  ): void {
    const record = this.pageSubscriptions.get(pageId)
    const targetChannels = channels || record?.config.channels || []

    if (targetChannels.length === 0) return

    this.trimPendingSubscriptions(pageId, targetChannels)

    if (record) {
      if (channels && channels.length > 0) {
        record.config.channels = record.config.channels.filter(
          (channel) => !targetChannels.includes(channel),
        )
        if (record.config.channels.length === 0) {
          this.pageSubscriptions.delete(pageId)
        }
      } else {
        this.pageSubscriptions.delete(pageId)
      }
    }

    if (this.isConnected.value) {
      const messageId = this.generateMessageId()
      this.sendUnsubscribeMessage(targetChannels, source, messageId)
    }
  }

  /** 绑定 WebSocket 事件 */
  private setupEventHandlers(onConnect: () => void, onError: (error: Error) => void): void {
    if (!this.ws) return

    this.ws.onopen = () => {
      console.log('[WebSocket] 连接成功')
      this.status.value = 'connected'
      this.reconnectAttempts = 0
      this.connectionStats.connectTime = Date.now()
      this.startHeartbeat()
      this.globalListeners.onConnect?.()

      this.enqueueExistingSubscriptionsForReconnect()
      this.processPendingSubscriptions()

      onConnect()
    }

    this.ws.onmessage = (event) => {
      try {
        const message: ServerMessage = JSON.parse(event.data)
        this.handleMessage(message)
        this.connectionStats.messageCount++
      } catch (error) {
        console.error('[WebSocket] 消息解析失败:', error)
        this.connectionStats.errorCount++
      }
    }

    this.ws.onclose = (event) => {
      console.log('[WebSocket] 连接关闭:', event.code, event.reason)
      this.status.value = 'disconnected'
      this.connectionStats.disconnectTime = Date.now()
      this.stopHeartbeat()
      this.globalListeners.onDisconnect?.()

      if (
        !this.isManualDisconnect &&
        this.reconnectAttempts < (this.config.maxReconnectAttempts ?? 0)
      ) {
        this.scheduleReconnect()
      }
    }

    this.ws.onerror = (error) => {
      console.error('[WebSocket] 连接出错:', error)
      this.status.value = 'error'
      this.connectionStats.errorCount++
      const wsError = new Error('WebSocket connection error')
      this.handleError(wsError)
      onError(wsError)
    }
  }

  /** 建立连接（含登录校验） */
  public connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      const userStore = useUserStore()
      if (!userStore.isLoggedIn || !userStore.token) {
        console.log('[WebSocket] 未登录或无 token，跳过连接')
        reject(new Error('User not logged in or token invalid'))
        return
      }

      if (this.ws && this.ws.readyState === WebSocket.OPEN) {
        resolve()
        return
      }

      this.isManualDisconnect = false
      this.status.value = 'connecting'
      this.connectionStats.connectTime = Date.now()

      try {
        this.ws = new WebSocket(this.config.url)
        this.setupEventHandlers(resolve, reject)
      } catch (error) {
        this.handleError(error as Error)
        reject(error)
      }
    })
  }

  /** 服务端消息分发 */
  private handleMessage(message: ServerMessage): void {
    console.log('[WebSocket] 收到消息:', message)

    switch (message.type) {
      case 'connection_established':
        break
      case 'data_update':
        this.handleDataUpdate((message as any).data)
        break
      case 'data_batch':
        this.handleBatchDataUpdate((message as any).data, message.timestamp)
        break
      case 'alarm':
        this.handleAlarm((message as any).data)
        break
      case 'subscribe_ack':
        this.handleSubscribeAck((message as any).data)
        break
      case 'unsubscribe_ack':
        this.handleUnsubscribeAck((message as any).data)
        break
      case 'control_ack':
        this.handleControlAck((message as any).data)
        break
      case 'error':
        this.handleServerError((message as any).data)
        break
      case 'pong':
        this.handlePong((message as any).data)
        break
      case 'alarm_num':
        this.handleAlarmNum((message as any).data)
        break
      default:
        console.warn('[WebSocket] 未知消息类型:', (message as any).type)
    }
  }

  /** 处理单条数据更新 */
  private handleDataUpdate(data: any): void {
    this.globalListeners.onDataUpdate?.(data)

    this.pageSubscriptions.forEach((record) => {
      if (record.config.channels.includes(data.channel_id)) {
        record.listeners.onDataUpdate?.(data)
      }
    })
  }

  /** 处理批量数据更新 */
  private handleBatchDataUpdate(data: any, timestamp: string): void {
    this.globalListeners.onBatchDataUpdate?.(data, timestamp)

    this.pageSubscriptions.forEach((record) => {
      const relevantUpdates = data.updates.filter((update: any) =>
        record.config.channels.includes(update.channel_id),
      )
      if (relevantUpdates.length > 0) {
        record.listeners.onBatchDataUpdate?.({ updates: relevantUpdates }, timestamp)
      }
    })
  }

  /** 处理告警 */
  private handleAlarm(alarm: any): void {
    this.globalListeners.onAlarm?.(alarm)

    this.pageSubscriptions.forEach((record) => {
      record.listeners.onAlarm?.(alarm)
    })
  }

  /** 处理订阅确认 */
  private handleSubscribeAck(data: any): void {
    console.log('[WebSocket] 订阅确认:', data)

    const requestId = data.request_id
    if (requestId) {
      this.pendingSubscriptionsMap.delete(requestId)
    }

    if (data.failed && data.failed.length > 0) {
      ElMessage.warning(`部分频道订阅失败: ${data.failed.join(', ')}`)
    }
  }

  /** 处理取消订阅确认 */
  private handleUnsubscribeAck(data: UnsubscribeAckMessage['data']): void {
    console.log('[WebSocket] 取消订阅确认:', data)

    if (data.failed && data.failed.length > 0) {
      ElMessage.warning(`部分频道取消订阅失败: ${data.failed.join(', ')}`)
    }
  }

  /** 处理控制指令确认 */
  private handleControlAck(data: any): void {
    console.log('[WebSocket] 控制指令确认:', data)
    if (!data.result.success) {
      ElMessage.error(`控制命令执行失败: ${data.result.message}`)
    }
  }

  /** 处理告警数量更新 */
  private handleAlarmNum(data: any): void {
    console.log('[WebSocket] 告警数量更新:', data)
    this.globalListeners.onAlarmNum?.(data)
  }

  /** 处理服务端错误 */
  private handleServerError(error: any): void {
    console.error('[WebSocket] 服务端错误', error)
    this.globalListeners.onError?.(error)
    ElMessage.error(`WebSocket错误: ${error.message}`)
  }

  /** 处理心跳响应 */
  private handlePong(data: any): void {
    this.connectionStats.latency = data.latency
    if (this.heartbeatTimeoutTimer) {
      clearTimeout(this.heartbeatTimeoutTimer)
      this.heartbeatTimeoutTimer = null
    }
  }

  /** 发送控制指令 */
  public sendControlCommand(
    channelId: number,
    pointId: number,
    commandType: CommandType,
    value?: number,
    operator: string = 'system',
    reason?: string,
  ): void {
    const controlMessage: ControlMessage = {
      id: this.generateMessageId(),
      type: 'control',
      timestamp: '',
      data: {
        channel_id: channelId,
        point_id: pointId,
        command_type: commandType as any,
        value,
        operator,
        reason,
      },
    }

    this.sendMessage(controlMessage)
  }

  /** 发送 ping */
  public sendPing(): void {
    const pingMessage: PingMessage = {
      id: this.generateMessageId(),
      type: 'ping',
      timestamp: '',
    }

    this.sendMessage(pingMessage)
  }

  /** 设置/合并全局监听 */
  public setGlobalListeners(listeners: ListenerConfig): void {
    this.globalListeners = { ...this.globalListeners, ...listeners }
  }

  /** 启动心跳与超时检测 */
  private startHeartbeat(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer)
    }

    this.heartbeatTimer = setInterval(() => {
      try {
        this.sendPing()
        console.log('[WebSocket] 发送心跳')
      } catch (err) {
        console.warn('[WebSocket] 心跳发送失败:', err)
      }

      if (this.heartbeatTimeoutTimer) {
        clearTimeout(this.heartbeatTimeoutTimer)
      }

      this.heartbeatTimeoutTimer = setTimeout(() => {
        console.warn('[WebSocket] 心跳超时，关闭等待重连')
        this.closeForReconnect('heartbeat timeout')
      }, this.config.heartbeatTimeout)
    }, this.config.heartbeatInterval)
  }

  /** 为重连关闭连接，保留订阅记录 */
  private closeForReconnect(reason: string): void {
    this.isManualDisconnect = false
    this.stopHeartbeat()
    if (this.ws) {
      this.ws.close(4000, reason)
      this.ws = null
    }
    this.status.value = 'disconnected'
  }

  /** 停止心跳与超时计时器 */
  private stopHeartbeat(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer)
      this.heartbeatTimer = null
    }
    if (this.heartbeatTimeoutTimer) {
      clearTimeout(this.heartbeatTimeoutTimer)
      this.heartbeatTimeoutTimer = null
    }
  }

  /** 安排重连（指数退避可按需扩展） */
  private scheduleReconnect(): void {
    if (this.isManualDisconnect) return

    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer)
    }

    this.reconnectAttempts++
    const delay = this.config.reconnectInterval

    console.log(`[WebSocket] ${delay}ms后尝试重连（第${this.reconnectAttempts}次）`)

    this.reconnectTimer = setTimeout(() => {
      this.globalListeners.onReconnect?.()
      this.connect().catch((error) => {
        console.error('[WebSocket] 重连失败:', error)
      })
    }, delay)
  }

  /** 统一错误处理回调 */
  private handleError(error: Error): void {
    console.error('[WebSocket] 连接出错:', error)
    this.globalListeners.onError?.({ code: 'CONNECTION_ERROR', message: error.message })
  }

  /** 主动断开连接并清理所有状态 */
  public disconnect(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer)
      this.reconnectTimer = null
    }

    this.isManualDisconnect = true
    this.stopHeartbeat()

    if (this.ws) {
      this.ws.close(1000, 'manual disconnect')
      this.ws = null
    }

    this.globalListeners = {}
    this.pageSubscriptions.clear()
    this.pendingSubscriptionsMap.clear()
    this.status.value = 'disconnected'
  }

  /** 获取当前连接统计信息 */
  public getStats() {
    return {
      status: this.status.value,
      isConnected: this.isConnected.value,
      pageSubscriptions: this.pageSubscriptions.size,
      ...this.connectionStats,
    }
  }

  public subscribePage(
    pageId: string,
    config: SubscriptionConfig,
    listeners: Partial<ListenerConfig>,
  ): string {
    return this.subscribe(config, pageId, listeners)
  }

  public unsubscribePage(
    pageId: string,
    channels?: number[],
    source: 'inst' | 'comsrv' = 'inst',
  ): void {
    this.unsubscribe(pageId, channels, source)
  }
}

const wsManager = new WebSocketManager({
  url: '/ws',
})

export default wsManager
