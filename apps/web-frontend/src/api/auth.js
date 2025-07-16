import request from '@/utils/request'
import { mockLogin, mockGetUserInfo, mockLogout } from '@/mocks/auth'

// 判断是否使用模拟数据（当后端服务不可用时）
const USE_MOCK = true // 暂时使用模拟数据

// 用户登录
export function login(data) {
  if (USE_MOCK) {
    return mockLogin(data)
  }
  return request({
    url: '/api/auth/login',
    method: 'post',
    data
  })
}

// 获取用户信息
export function getUserInfo() {
  if (USE_MOCK) {
    return mockGetUserInfo()
  }
  return request({
    url: '/api/auth/userinfo',
    method: 'get'
  })
}

// 用户登出
export function logout() {
  if (USE_MOCK) {
    return mockLogout()
  }
  return request({
    url: '/api/auth/logout',
    method: 'post'
  })
}

// 刷新Token
export function refreshToken() {
  return request({
    url: '/api/auth/refresh',
    method: 'post'
  })
}

// 修改密码
export function changePassword(data) {
  return request({
    url: '/api/auth/change-password',
    method: 'post',
    data
  })
}