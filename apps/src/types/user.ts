// 用户信息接口
export interface UserInfo {
  id: number
  username: string
  email: string
  nickname: string
  avatar?: string
  role: string
  status: 'active' | 'inactive'
  createdAt: string
  updatedAt: string
}

// 用户管理表格用户信息接口
export interface UserManagementInfo {
  id: number
  userName: string
  realName: string
  role: string
  email: string
  phone: string
  status: 'active' | 'inactive' | 'banned'
  lastLogin: string
  createdAt: string
}

// 登录请求参数
export interface LoginParams {
  username: string
  password: string
  role: string
  remember?: boolean
}

// 登录响应
export interface LoginResponse {
  token: string
  refreshToken: string
  expiresIn: number
  user: UserInfo
}

// 注册请求参数
export interface RegisterParams {
  username: string
  email: string
  password: string
  nickname?: string
}

// 更新用户信息参数
export interface UpdateUserParams {
  nickname?: string
  email?: string
  avatar?: string
}

// 修改密码参数
export interface ChangePasswordParams {
  oldPassword: string
  newPassword: string
}

// API 响应格式
export interface ApiResponse<T = any> {
  code: number
  message: string
  data: T
  success: boolean
}
