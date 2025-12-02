<script setup lang="ts">
import en from 'element-plus/es/locale/lang/en'
import wsManager from '@/utils/websocket'
import { useRouter } from 'vue-router'
import { useGlobalStore } from '@/stores/global'
import { useUserStore } from '@/stores/user'
const locale = en
const router = useRouter()
const globalStore = useGlobalStore()
const userStore = useUserStore()
// 处理告警详情跳转
const handleAlarmDetail = () => {
  ElMessage.closeAll()
  router.push({ name: 'alarmCurrentRecords' })
}
let idCount = 0
const alarmMap = new Map()
// 初始化WebSocket连接
const initWebSocket = async () => {
  try {
    await wsManager.connect()
    console.log('[main] WebSocket连接成功')
    // 设置全局监听器
    wsManager.setGlobalListeners({
      onConnect: () => {
        console.log('[main] WebSocket已连接')
      },
      onDisconnect: () => {
        console.log('[main] WebSocket连接断开')
      },
      onError: (error) => {
        console.error('[main] WebSocket错误:', error)
      },
      onAlarm: (alarm) => {
        if (alarm.status == 1) {
          // 告警触发状态，显示带跳转按钮的消息
          const currentId = idCount++
          const message = ElMessage({
            type: 'error',
            showClose: true,
            dangerouslyUseHTMLString: true,
            message: `
              <div style="display: flex; align-items: center; gap: 0.08rem;">
                <span>${alarm.message}</span>
                <button id="to-detail-btn-${currentId}" style="padding:0.04rem 0.1rem;background-color: #ff6900; color: #fff; border: none; cursor: pointer; font-size:0.14rem; border-radius: 0.02rem;">to detail</button>
              </div>
            `,
            duration: 0,
            onClose: () => {
              // 通过ID映射表清理事件监听，防止内存泄漏
              const buttonId = `to-detail-btn-${currentId}`

              const eventInfo = document.getElementById(buttonId)
              if (eventInfo) {
                // 移除事件监听器
                eventInfo.removeEventListener('click', handleAlarmDetail)
                console.log(`[App] 通过ID映射表清理告警按钮事件监听: ${buttonId}`)
              }

              // 从告警映射中移除
              alarmMap.delete(alarm.alarm_id)
            },
          })

          // 存储告警消息到映射中
          alarmMap.set(alarm.alarm_id, message)

          // 添加按钮点击事件监听
          nextTick(() => {
            const buttonId = `to-detail-btn-${currentId}`
            const btn = document.getElementById(buttonId)
            if (btn) {
              btn.addEventListener('click', handleAlarmDetail)
            }
          })
        } else {
          // 告警恢复状态，显示成功消息并关闭对应的告警消息
          ElMessage.success({
            message: alarm.message,
            duration: 3000,
          })

          // 关闭对应的告警消息
          const existingMessage = alarmMap.get(alarm.alarm_id)
          if (existingMessage) {
            existingMessage.close()
            alarmMap.delete(alarm.alarm_id)
            console.log(`[App] 关闭告警消息: ${alarm.alarm_id}`)
          }
        }
        console.log('[main] 告警:', alarm)
      },
      onAlarmNum: (alarmNum) => {
        console.log('[main] 告警数量更新:', alarmNum)
        globalStore.alarmNum = alarmNum.current_alarms
      },
    })
  } catch (error) {
    console.error('[main] WebSocket连接失败:', error)
  }
}
// 监听用户登录状态变化
watch(
  () => userStore.isLoggedIn,
  (isLoggedIn) => {
    if (isLoggedIn) {
      // 用户登录成功，连接WebSocket
      console.log('[main] 用户登录成功，尝试连接WebSocket')
      initWebSocket()
    } else {
      // 用户登出，断开WebSocket连接
      console.log('[main] 用户登出，断开WebSocket连接')
      wsManager.disconnect()
    }
  },
  { immediate: true }, // 立即执行一次
)
</script>

<template>
  <el-config-provider :locale="locale">
    <router-view />
  </el-config-provider>
</template>

<style>
body {
  margin: 0;
  padding: 0;
  overflow: hidden;
  box-sizing: border-box;
}

#app {
  height: 100vh;
  width: 100vw;
  margin: 0;
  padding: 0;
  overflow: hidden;
  position: relative;
}
</style>
