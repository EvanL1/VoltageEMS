/**
 * WebSocket 连接管理 Composable
 * 提供连接状态、统一订阅/退订操作，封装页面级订阅的挂载/卸载时机。
 */

import { computed, onMounted, onUnmounted } from 'vue'
import wsManager from '@/utils/websocket'
import type { ListenerConfig, SubscriptionConfig, CommandType } from '@/types/websocket'

/**
 * 创建 WebSocket 订阅上下文
 * @param pageId 页面唯一标识
 * @param config 订阅配置
 * @param listeners 监听配置
 */
export default function useWebSocket(
  pageId: string,
  config: SubscriptionConfig,
  listeners: ListenerConfig,
) {
  const status = computed(() => wsManager.status.value)
  const isConnected = computed(() => wsManager.isConnected.value)
  const isConnecting = computed(() => wsManager.isConnecting.value)

  const stats = computed(() => wsManager.getStats())

  /** 设置全局监听 */
  const setGlobalListeners = (listeners: ListenerConfig) => {
    wsManager.setGlobalListeners(listeners)
  }

  /** 发送控制命令 */
  const sendControlCommand = (
    channelId: number,
    pointId: number,
    commandType: CommandType,
    value?: number,
    operator?: string,
    reason?: string,
  ) => {
    wsManager.sendControlCommand(channelId, pointId, commandType, value, operator, reason)
  }

  /** 全局订阅（默认 pageId=global） */
  const subscribe = (customConfig?: SubscriptionConfig, customListeners?: ListenerConfig) => {
    const finalConfig = customConfig || config
    const finalListeners = customListeners || listeners
    return wsManager.subscribe(finalConfig, 'global', finalListeners)
  }

  /** 取消全局订阅 */
  const unsubscribe = (
    customChannels?: number[],
    customSource: 'inst' | 'comsrv' = (config.source as 'inst' | 'comsrv') || 'inst',
  ) => {
    wsManager.unsubscribe('global', customChannels, customSource)
  }

  /** 页面订阅 */
  const subscribePage = (
    customPageId?: string,
    customConfig?: SubscriptionConfig,
    customListeners?: ListenerConfig,
  ) => {
    const finalPageId = customPageId || pageId
    const finalConfig = customConfig || config
    const finalListeners = customListeners || listeners
    return wsManager.subscribe(finalConfig, finalPageId, finalListeners)
  }

  /** 取消页面订阅 */
  const unsubscribePage = (
    customPageId?: string,
    customChannels?: number[],
    customSource: 'inst' | 'comsrv' = 'inst',
  ) => {
    const finalPageId = customPageId || pageId
    const finalChannels = customChannels || config.channels
    wsManager.unsubscribe(finalPageId, finalChannels, customSource)
  }

  onMounted(() => {
    // 先记录订阅，再建立连接，方便断线重连时自动恢复
    subscribePage(pageId, config, listeners)
    wsManager.connect().catch(() => {
      // 重连机制在 wsManager 内部处理
    })
  })

  onUnmounted(() => {
    // Clear current page subscription to avoid resubscribing after navigation
    unsubscribePage(pageId, config.channels, config.source as 'inst' | 'comsrv')
  })

  return {
    status,
    isConnected,
    isConnecting,
    stats,
    setGlobalListeners,
    sendControlCommand,
    subscribe,
    unsubscribe,
    subscribePage,
    unsubscribePage,
  }
}
