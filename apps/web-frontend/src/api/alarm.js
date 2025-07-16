import request from '@/utils/request'

// 获取告警统计
export function getAlarmStatistics() {
  return request({
    url: '/alarms/stats',
    method: 'get'
  })
}

// 获取告警列表
export function fetchAlarms(params) {
  return request({
    url: '/alarms',
    method: 'get',
    params
  })
}

// 获取告警列表(兼容旧版本)
export function getAlarmList(params) {
  return fetchAlarms(params)
}

// 获取告警详情
export function getAlarmDetail(id) {
  return request({
    url: `/alarms/${id}`,
    method: 'get'
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
    url: `/alarms/${id}/acknowledge`,
    method: 'put'
  })
}

// 清除告警
export function clearAlarm(id) {
  return request({
    url: `/alarms/${id}/clear`,
    method: 'put'
  })
}

// 解决告警(兼容旧版本)
export function resolveAlarm(id) {
  return clearAlarm(id)
}

// 批量确认告警
export function batchAcknowledgeAlarms(ids) {
  return request({
    url: '/alarms/batch-acknowledge',
    method: 'put',
    data: { ids }
  })
}

// 批量清除告警
export function batchClearAlarms(ids) {
  return request({
    url: '/alarms/batch-clear',
    method: 'put',
    data: { ids }
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

// 获取告警规则
export function getAlarmRules() {
  return request({
    url: '/alarm-rules',
    method: 'get'
  })
}

// 创建告警规则
export function createAlarmRule(data) {
  return request({
    url: '/alarm-rules',
    method: 'post',
    data
  })
}

// 更新告警规则
export function updateAlarmRule(id, data) {
  return request({
    url: `/alarm-rules/${id}`,
    method: 'put',
    data
  })
}

// 删除告警规则
export function deleteAlarmRule(id) {
  return request({
    url: `/alarm-rules/${id}`,
    method: 'delete'
  })
}