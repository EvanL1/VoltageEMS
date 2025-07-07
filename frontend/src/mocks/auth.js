// 模拟认证服务
import { ROLES } from '@/utils/permission'

// 模拟用户数据
const mockUsers = [
  {
    id: 1,
    username: 'admin',
    password: 'admin123',
    realName: '超级管理员',
    role: ROLES.SUPER_ADMIN,
    email: 'admin@monarchhub.com',
    phone: '13800138000',
    status: 'active'
  },
  {
    id: 2,
    username: 'sysadmin',
    password: 'sys123',
    realName: '系统管理员',
    role: ROLES.SYSTEM_ADMIN,
    email: 'sysadmin@monarchhub.com',
    phone: '13800138001',
    status: 'active'
  },
  {
    id: 3,
    username: 'engineer',
    password: 'eng123',
    realName: '运维工程师',
    role: ROLES.OPS_ENGINEER,
    email: 'engineer@monarchhub.com',
    phone: '13800138002',
    status: 'active'
  },
  {
    id: 4,
    username: 'monitor',
    password: 'mon123',
    realName: '监控人员',
    role: ROLES.MONITOR,
    email: 'monitor@monarchhub.com',
    phone: '13800138003',
    status: 'active'
  },
  {
    id: 5,
    username: 'guest',
    password: 'guest123',
    realName: '访客用户',
    role: ROLES.GUEST,
    email: 'guest@monarchhub.com',
    phone: '13800138004',
    status: 'active'
  }
]

// 模拟延迟
const delay = (ms) => new Promise(resolve => setTimeout(resolve, ms))

// 模拟登录
export async function mockLogin(credentials) {
  await delay(500)
  
  const user = mockUsers.find(u => 
    u.username === credentials.username && 
    u.password === credentials.password &&
    u.status === 'active'
  )
  
  if (!user) {
    throw new Error('用户名或密码错误')
  }
  
  // 生成模拟token
  const token = btoa(`${user.username}:${Date.now()}`)
  
  // 保存当前用户到 localStorage
  localStorage.setItem('mockCurrentUser', JSON.stringify(user))
  
  return {
    data: {
      token,
      userInfo: {
        id: user.id,
        username: user.username,
        realName: user.realName,
        role: user.role,
        email: user.email,
        phone: user.phone
      }
    }
  }
}

// 模拟获取用户信息
export async function mockGetUserInfo() {
  await delay(300)
  
  const token = localStorage.getItem('token')
  if (!token) {
    throw new Error('未登录')
  }
  
  const currentUser = JSON.parse(localStorage.getItem('mockCurrentUser') || '{}')
  if (!currentUser.id) {
    throw new Error('用户信息无效')
  }
  
  return {
    data: {
      userInfo: {
        id: currentUser.id,
        username: currentUser.username,
        realName: currentUser.realName,
        role: currentUser.role,
        email: currentUser.email,
        phone: currentUser.phone
      }
    }
  }
}

// 模拟登出
export async function mockLogout() {
  await delay(200)
  localStorage.removeItem('mockCurrentUser')
  return { data: { success: true } }
}