import { defineStore } from 'pinia'

export const useRealtimeStore = defineStore('realtime', {
  state: () => ({
    // 实时遥测数据 (YC)
    telemetryData: {},
    // 实时遥信数据 (YX)
    signalData: {},
    // 设备状态
    deviceStatus: {},
    // WebSocket连接状态
    wsConnected: false,
    // 数据更新时间戳
    lastUpdate: null,
    // 订阅的测点
    subscribedPoints: []
  }),

  getters: {
    getTelemetryValue: (state) => (pointId) => state.telemetryData[pointId],
    getSignalValue: (state) => (pointId) => state.signalData[pointId],
    getDeviceStatus: (state) => (deviceId) => state.deviceStatus[deviceId],
    isConnected: (state) => state.wsConnected,
    getLastUpdateTime: (state) => state.lastUpdate
  },

  actions: {
    updateTelemetryData(data) {
      Object.assign(this.telemetryData, data)
      this.lastUpdate = Date.now()
    },

    updateSignalData(data) {
      Object.assign(this.signalData, data)
      this.lastUpdate = Date.now()
    },

    updateDeviceStatus(deviceId, status) {
      this.deviceStatus[deviceId] = {
        ...status,
        timestamp: Date.now()
      }
    },

    setWsConnected(connected) {
      this.wsConnected = connected
    },

    subscribePoints(points) {
      this.subscribedPoints = [...new Set([...this.subscribedPoints, ...points])]
    },

    unsubscribePoints(points) {
      this.subscribedPoints = this.subscribedPoints.filter(p => !points.includes(p))
    },

    clearData() {
      this.telemetryData = {}
      this.signalData = {}
      this.deviceStatus = {}
      this.lastUpdate = null
    }
  }
})