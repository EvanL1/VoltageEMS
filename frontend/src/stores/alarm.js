import { defineStore } from 'pinia'
import { fetchAlarms, acknowledgeAlarm, clearAlarm } from '@/api/alarm'

export const useAlarmStore = defineStore('alarm', {
  state: () => ({
    // 告警列表
    alarms: [],
    // 告警统计
    statistics: {
      critical: 0,
      major: 0,
      minor: 0,
      warning: 0,
      info: 0
    },
    // 活跃告警数
    activeCount: 0,
    // 加载状态
    loading: false,
    // 筛选条件
    filters: {
      level: null,
      type: null,
      status: null,
      timeRange: null
    }
  }),

  getters: {
    activeAlarms: (state) => state.alarms.filter(a => a.status === 'active'),
    acknowledgedAlarms: (state) => state.alarms.filter(a => a.status === 'acknowledged'),
    clearedAlarms: (state) => state.alarms.filter(a => a.status === 'cleared'),
    
    alarmsByLevel: (state) => (level) => state.alarms.filter(a => a.level === level),
    
    filteredAlarms: (state) => {
      let result = [...state.alarms]
      
      if (state.filters.level) {
        result = result.filter(a => a.level === state.filters.level)
      }
      if (state.filters.type) {
        result = result.filter(a => a.type === state.filters.type)
      }
      if (state.filters.status) {
        result = result.filter(a => a.status === state.filters.status)
      }
      
      return result
    }
  },

  actions: {
    async fetchAlarms() {
      this.loading = true
      try {
        const { data } = await fetchAlarms(this.filters)
        this.alarms = data.alarms
        this.statistics = data.statistics
        this.activeCount = data.activeCount
      } catch (error) {
        console.error('Failed to fetch alarms:', error)
      } finally {
        this.loading = false
      }
    },

    async acknowledgeAlarm(alarmId) {
      try {
        await acknowledgeAlarm(alarmId)
        const alarm = this.alarms.find(a => a.id === alarmId)
        if (alarm) {
          alarm.status = 'acknowledged'
          alarm.acknowledgedAt = new Date().toISOString()
        }
      } catch (error) {
        console.error('Failed to acknowledge alarm:', error)
        throw error
      }
    },

    async clearAlarm(alarmId) {
      try {
        await clearAlarm(alarmId)
        const alarm = this.alarms.find(a => a.id === alarmId)
        if (alarm) {
          alarm.status = 'cleared'
          alarm.clearedAt = new Date().toISOString()
        }
        this.activeCount--
      } catch (error) {
        console.error('Failed to clear alarm:', error)
        throw error
      }
    },

    setFilter(filterType, value) {
      this.filters[filterType] = value
    },

    clearFilters() {
      this.filters = {
        level: null,
        type: null,
        status: null,
        timeRange: null
      }
    },

    addAlarm(alarm) {
      this.alarms.unshift(alarm)
      if (alarm.status === 'active') {
        this.activeCount++
        this.statistics[alarm.level]++
      }
    }
  }
})