import { Request } from '@/utils/request'

export const getMqttConfig = () => {
  return Request.get('/netApi/mqtt/config')
}

export const updateMqttConfig = (params: any) => {
  return Request.post('/netApi/mqtt/config', params)
}

export const disconnectMqtt = () => {
  return Request.post('/netApi/mqtt/disconnect')
}

export const reconnectMqtt = () => {
  return Request.post('/netApi/mqtt/reconnect')
}
export const getMqttStatus = () => {
  return Request.get('/netApi/mqtt/status')
}
