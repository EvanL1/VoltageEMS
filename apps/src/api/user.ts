import request from '@/utils/request'
import type {
  LoginParams,
  LoginResponse,
  RegisterParams,
  UpdateUserParams,
  ChangePasswordParams,
  UserInfo,
  ApiResponse,
} from '@/types/user'

// 用户相关API接口
export const userApi = {
  /**
   * 用户登录
   * @param params 登录参数
   * @returns 登录响应
   */
  login(params: LoginParams): Promise<ApiResponse<LoginResponse>> {
    return request.post('/api/auth/login', params)
  },

  /**
   * 用户注册
   * @param params 注册参数
   * @returns 注册响应
   */
  register(params: RegisterParams): Promise<ApiResponse<UserInfo>> {
    return request.post('/api/auth/register', params)
  },

  /**
   * 用户登出
   * @returns 登出响应
   */
  logout(): Promise<ApiResponse<null>> {
    return request.post('/api/auth/logout')
  },

  /**
   * 获取当前用户信息
   * @returns 用户信息
   */
  getUserInfo(): Promise<ApiResponse<UserInfo>> {
    return request.get('/api/user/info')
  },

  /**
   * 更新用户信息
   * @param params 更新参数
   * @returns 更新响应
   */
  updateUserInfo(params: UpdateUserParams): Promise<ApiResponse<UserInfo>> {
    return request.put('/api/user/info', params)
  },

  /**
   * 修改密码
   * @param params 密码参数
   * @returns 修改响应
   */
  changePassword(params: ChangePasswordParams): Promise<ApiResponse<null>> {
    return request.put('/api/user/password', params)
  },

  /**
   * 刷新Token
   * @param refreshToken 刷新令牌
   * @returns 新的Token
   */
  refreshToken(
    refreshToken: string,
  ): Promise<ApiResponse<{ token: string; refreshToken: string }>> {
    return request.post('/api/auth/refresh', { refreshToken })
  },

  /**
   * 上传头像
   * @param file 头像文件
   * @returns 上传响应
   */
  uploadAvatar(file: File): Promise<ApiResponse<{ url: string }>> {
    const formData = new FormData()
    formData.append('avatar', file)
    return request.post('/api/user/avatar', formData, {
      headers: {
        'Content-Type': 'multipart/form-data',
      },
    })
  },
}

// 虚拟数据生成函数
const generateMockUserInfo = (): UserInfo => ({
  id: 1,
  username: 'admin',
  email: 'admin@example.com',
  nickname: '管理员',
  avatar: 'https://avatars.githubusercontent.com/u/1?v=4',
  role: 'admin',
  status: 'active',
  createdAt: '2024-01-01T00:00:00Z',
  updatedAt: '2024-01-01T00:00:00Z',
})

// 模拟API响应延迟
const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))

// 虚拟API实现
export const mockUserApi = {
  /**
   * 模拟用户登录
   */
  async login(params: LoginParams): Promise<ApiResponse<LoginResponse>> {
    await delay(1000) // 模拟网络延迟

    // 模拟登录验证
    if (params.username === 'admin' && params.password === '123456') {
      const userInfo = generateMockUserInfo()
      return {
        code: 200,
        message: '登录成功',
        success: true,
        data: {
          token: 'mock-jwt-token-' + Date.now(),
          refreshToken: 'mock-refresh-token-' + Date.now(),
          expiresIn: 7200, // 2小时
          user: userInfo,
        },
      }
    } else {
      throw new Error('用户名或密码错误')
    }
  },

  /**
   * 模拟用户注册
   */
  async register(params: RegisterParams): Promise<ApiResponse<UserInfo>> {
    await delay(1000)

    return {
      code: 200,
      message: '注册成功',
      success: true,
      data: {
        id: Date.now(),
        username: params.username,
        email: params.email,
        nickname: params.nickname || params.username,
        avatar: '',
        role: 'user',
        status: 'active',
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      },
    }
  },

  /**
   * 模拟用户登出
   */
  async logout(): Promise<ApiResponse<null>> {
    await delay(500)

    return {
      code: 200,
      message: '登出成功',
      success: true,
      data: null,
    }
  },

  /**
   * 模拟获取用户信息
   */
  async getUserInfo(): Promise<ApiResponse<UserInfo>> {
    await delay(500)

    return {
      code: 200,
      message: '获取成功',
      success: true,
      data: generateMockUserInfo(),
    }
  },

  /**
   * 模拟更新用户信息
   */
  async updateUserInfo(params: UpdateUserParams): Promise<ApiResponse<UserInfo>> {
    await delay(1000)

    const userInfo = generateMockUserInfo()
    const updatedUser = { ...userInfo, ...params, updatedAt: new Date().toISOString() }

    return {
      code: 200,
      message: '更新成功',
      success: true,
      data: updatedUser,
    }
  },

  /**
   * 模拟修改密码
   */
  async changePassword(params: ChangePasswordParams): Promise<ApiResponse<null>> {
    await delay(1000)

    return {
      code: 200,
      message: '密码修改成功',
      success: true,
      data: null,
    }
  },

  /**
   * 模拟刷新Token
   */
  async refreshToken(
    refreshToken: string,
  ): Promise<ApiResponse<{ token: string; refreshToken: string }>> {
    await delay(500)

    return {
      code: 200,
      message: 'Token刷新成功',
      success: true,
      data: {
        token: 'mock-jwt-token-' + Date.now(),
        refreshToken: 'mock-refresh-token-' + Date.now(),
      },
    }
  },

  /**
   * 模拟上传头像
   */
  async uploadAvatar(file: File): Promise<ApiResponse<{ url: string }>> {
    await delay(2000) // 模拟上传时间

    return {
      code: 200,
      message: '头像上传成功',
      success: true,
      data: {
        url: 'https://avatars.githubusercontent.com/u/1?v=4',
      },
    }
  },
}
