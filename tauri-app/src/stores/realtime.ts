import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { realtimeApi } from '@/api'
import type { ChannelStatus, PointData } from '@/types/realtime'

export const useRealtimeStore = defineStore('realtime', () => {
  // State
  const channels = ref<ChannelStatus[]>([])
  const channelPoints = ref<Map<number, PointData[]>>(new Map())
  const statistics = ref({
    total_channels: 0,
    online_channels: 0,
    offline_channels: 0,
    total_points: 0,
    total_errors: 0,
    timestamp: new Date()
  })
  const loading = ref(false)
  const error = ref<string | null>(null)
  
  // Getters
  const onlineChannels = computed(() => 
    channels.value.filter(ch => ch.status === 'online')
  )
  
  const offlineChannels = computed(() => 
    channels.value.filter(ch => ch.status === 'offline')
  )
  
  const errorChannels = computed(() => 
    channels.value.filter(ch => ch.status === 'error')
  )
  
  const getChannelPoints = computed(() => (channelId: number) => {
    return channelPoints.value.get(channelId) || []
  })
  
  // Actions
  async function fetchChannels() {
    loading.value = true
    error.value = null
    
    try {
      const response = await realtimeApi.getChannels()
      channels.value = response.channels
    } catch (err) {
      error.value = 'Failed to fetch channels'
      console.error('Error fetching channels:', err)
    } finally {
      loading.value = false
    }
  }
  
  async function fetchChannelPoints(channelId: number, pointTypes?: string) {
    loading.value = true
    error.value = null
    
    try {
      const response = await realtimeApi.getPoints(channelId, { point_types: pointTypes })
      channelPoints.value.set(channelId, response.points)
    } catch (err) {
      error.value = `Failed to fetch points for channel ${channelId}`
      console.error('Error fetching points:', err)
    } finally {
      loading.value = false
    }
  }
  
  async function fetchStatistics() {
    try {
      const response = await realtimeApi.getStatistics()
      statistics.value = {
        ...response,
        timestamp: new Date(response.timestamp)
      }
    } catch (err) {
      console.error('Error fetching statistics:', err)
    }
  }
  
  function updateChannelData(channelId: string, data: any) {
    // Update channel status if it's a status update
    if (data.status) {
      const channel = channels.value.find(ch => ch.channel_id === parseInt(channelId))
      if (channel) {
        channel.status = data.status
        channel.last_update = new Date()
      }
    }
    
    // Update point data if it's point data
    if (data.point_id && data.value !== undefined) {
      const id = parseInt(channelId.split(':')[0])
      const points = channelPoints.value.get(id) || []
      const pointIndex = points.findIndex(p => p.point_id === data.point_id)
      
      if (pointIndex >= 0) {
        points[pointIndex] = {
          ...points[pointIndex],
          value: data.value,
          quality: data.quality || points[pointIndex].quality,
          timestamp: new Date()
        }
      } else {
        points.push({
          point_id: data.point_id,
          point_type: data.point_type || 'YC',
          value: data.value,
          quality: data.quality || 0,
          timestamp: new Date(),
          description: data.description
        })
      }
      
      channelPoints.value.set(id, points)
    }
  }
  
  function clearData() {
    channels.value = []
    channelPoints.value.clear()
    error.value = null
  }
  
  return {
    // State
    channels,
    channelPoints,
    statistics,
    loading,
    error,
    
    // Getters
    onlineChannels,
    offlineChannels,
    errorChannels,
    getChannelPoints,
    
    // Actions
    fetchChannels,
    fetchChannelPoints,
    fetchStatistics,
    updateChannelData,
    clearData
  }
})