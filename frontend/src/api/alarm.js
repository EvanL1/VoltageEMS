import request from './index'

// 获取告警统计
export function getAlarmStatistics() {
  return request({
    url: '/alarms/stats',
    method: 'get'
  })
}

// 获取告警列表
export function getAlarmList(params) {
  return request({
    url: '/alarms',
    method: 'get',
    params
  })
}

// 创建告警
export function createAlarm(data) {
  return request({
    url: '/alarms',
    method: 'post',
    data
  })
}

// 确认告警
export function acknowledgeAlarm(id) {
  return request({
    url: `/alarms/${id}/ack`,
    method: 'post'
  })
}

// 解决告警
export function resolveAlarm(id) {
  return request({
    url: `/alarms/${id}/resolve`,
    method: 'post'
  })
}

// 获取告警类别
export function getAlarmCategories() {
  return request({
    url: '/alarms/categories',
    method: 'get'
  })
}

// 获取服务状态
export function getServiceStatus() {
  return request({
    url: '/status',
    method: 'get'
  })
}