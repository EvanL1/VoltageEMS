/**
 * Axios 请求封装
 * 提供统一的HTTP请求配置、拦截器和错误处理
 */

import axios, { type AxiosRequestConfig } from 'axios'
import { ElMessage } from 'element-plus'

// 定义响应数据的通用接口
export interface ApiResponse<T = any> {
  code: number // 响应状态码
  message: string // 响应消息
  data: T // 响应数据
  success: boolean // 请求是否成功
}

// 定义请求配置的扩展接口
export interface RequestConfig extends AxiosRequestConfig {
  showErrorMessage?: boolean // 是否显示错误消息，默认true
  showSuccessMessage?: boolean // 是否显示成功消息，默认false
  loading?: boolean // 是否显示loading，默认false
}

/**
 * 创建axios实例
 * 设置基础配置项
 */
const service = axios.create({
  // API的base_url，可以通过环境变量配置
  baseURL: import.meta.env.VITE_API_BASE_URL || '/api',
  // 请求超时时间 (毫秒)
  timeout: 15000,
  // 默认请求头
  headers: {
    'Content-Type': 'application/json;charset=UTF-8',
  },
  // 跨域请求时是否需要使用凭证
  withCredentials: false,
})

/**
 * 请求拦截器
 * 在发送请求之前做一些统一处理
 */
service.interceptors.request.use(
  (config: any) => {
    // 从localStorage获取token并添加到请求头
    const token = localStorage.getItem('token')
    if (token && config.headers) {
      config.headers.Authorization = `Bearer ${token}`
    }

    // 如果是POST/PUT/PATCH请求且没有设置Content-Type，设置为json
    if (['post', 'put', 'patch'].includes(config.method?.toLowerCase() || '')) {
      if (!config.headers['Content-Type']) {
        config.headers['Content-Type'] = 'application/json'
      }
    }

    // 添加时间戳防止缓存 (GET请求)
    if (config.method?.toLowerCase() === 'get') {
      config.params = {
        ...config.params,
        _t: Date.now(),
      }
    }

    console.log(`[请求] ${config.method?.toUpperCase()} ${config.url}`, config)
    return config
  },
  (error: any) => {
    console.error('[请求错误]', error)
    ElMessage.error('请求配置错误')
    return Promise.reject(error)
  },
)

/**
 * 响应拦截器
 * 对响应数据进行统一处理
 */
service.interceptors.response.use(
  (response: any) => {
    console.log(`[响应] ${response.config.method?.toUpperCase()} ${response.config.url}`, response)

    const { data } = response
    const customConfig = response.config as any

    // 检查业务状态码
    if (data.code === 200 || data.success) {
      // 请求成功
      if (customConfig.showSuccessMessage && data.message) {
        ElMessage.success(data.message)
      }
      return response
    } else {
      // 业务逻辑错误
      const errorMessage = data.message || '请求失败'

      // 根据不同的业务状态码进行处理
      switch (data.code) {
        case 401:
          // 未授权，清除token并跳转到登录页
          localStorage.removeItem('token')
          ElMessage.error('登录已过期，请重新登录')
          // 这里可以添加路由跳转到登录页的逻辑
          // router.push('/login')
          break
        case 403:
          ElMessage.error('没有权限访问该资源')
          break
        case 404:
          ElMessage.error('请求的资源不存在')
          break
        case 500:
          ElMessage.error('服务器内部错误')
          break
        default:
          if (customConfig.showErrorMessage !== false) {
            ElMessage.error(errorMessage)
          }
      }

      return Promise.reject(new Error(errorMessage))
    }
  },
  (error: any) => {
    console.error('[响应错误]', error)

    let errorMessage = '网络请求失败'

    if (error.response) {
      // 服务器返回错误状态码
      const { status, statusText } = error.response

      switch (status) {
        case 400:
          errorMessage = '请求参数错误'
          break
        case 401:
          errorMessage = '未授权访问'
          localStorage.removeItem('token')
          break
        case 403:
          errorMessage = '拒绝访问'
          break
        case 404:
          errorMessage = '请求地址不存在'
          break
        case 408:
          errorMessage = '请求超时'
          break
        case 500:
          errorMessage = '服务器内部错误'
          break
        case 502:
          errorMessage = '网关错误'
          break
        case 503:
          errorMessage = '服务不可用'
          break
        case 504:
          errorMessage = '网关超时'
          break
        default:
          errorMessage = `连接错误${status}: ${statusText}`
      }
    } else if (error.request) {
      // 请求已发出但没有收到响应
      if (error.code === 'ECONNABORTED') {
        errorMessage = '请求超时'
      } else {
        errorMessage = '网络连接异常'
      }
    } else {
      // 请求配置出错
      errorMessage = error.message || '请求配置错误'
    }

    // 显示错误消息
    const config = error.config as any
    if (config?.showErrorMessage !== false) {
      ElMessage.error(errorMessage)
    }

    return Promise.reject(error)
  },
)

/**
 * 封装的请求方法
 */
class Request {
  /**
   * GET请求
   * @param url 请求地址
   * @param params 请求参数
   * @param config 请求配置
   */
  static async get<T = any>(
    url: string,
    params?: any,
    config?: RequestConfig,
  ): Promise<ApiResponse<T>> {
    const response = await service.get(url, { params, ...config })
    return response.data
  }

  /**
   * POST请求
   * @param url 请求地址
   * @param data 请求数据
   * @param config 请求配置
   */
  static async post<T = any>(
    url: string,
    data?: any,
    config?: RequestConfig,
  ): Promise<ApiResponse<T>> {
    const response = await service.post(url, data, config)
    return response.data
  }

  /**
   * PUT请求
   * @param url 请求地址
   * @param data 请求数据
   * @param config 请求配置
   */
  static async put<T = any>(
    url: string,
    data?: any,
    config?: RequestConfig,
  ): Promise<ApiResponse<T>> {
    const response = await service.put(url, data, config)
    return response.data
  }

  /**
   * DELETE请求
   * @param url 请求地址
   * @param config 请求配置
   */
  static async delete<T = any>(url: string, config?: RequestConfig): Promise<ApiResponse<T>> {
    const response = await service.delete(url, config)
    return response.data
  }

  /**
   * PATCH请求
   * @param url 请求地址
   * @param data 请求数据
   * @param config 请求配置
   */
  static async patch<T = any>(
    url: string,
    data?: any,
    config?: RequestConfig,
  ): Promise<ApiResponse<T>> {
    const response = await service.patch(url, data, config)
    return response.data
  }

  /**
   * 文件上传
   * @param url 上传地址
   * @param file 文件对象
   * @param data 额外数据
   * @param config 请求配置
   */
  static async upload<T = any>(
    url: string,
    file: File,
    data?: any,
    config?: RequestConfig,
  ): Promise<ApiResponse<T>> {
    const formData = new FormData()
    formData.append('file', file)

    // 添加额外数据
    if (data) {
      Object.keys(data).forEach((key) => {
        formData.append(key, data[key])
      })
    }

    const response = await service.post(url, formData, {
      ...config,
      headers: {
        'Content-Type': 'multipart/form-data',
      },
    })
    return response.data
  }

  /**
   * 下载文件
   * @param url 下载地址
   * @param params 请求参数
   * @param filename 文件名
   */
  static async download(url: string, params?: any, filename?: string): Promise<void> {
    try {
      const response = await service.get(url, {
        params,
        responseType: 'blob',
      })

      // 创建blob链接
      const blob = new Blob([response.data])
      const downloadUrl = window.URL.createObjectURL(blob)

      // 创建下载链接
      const link = document.createElement('a')
      link.href = downloadUrl
      link.download = filename || `download_${Date.now()}`

      // 触发下载
      document.body.appendChild(link)
      link.click()

      // 清理
      document.body.removeChild(link)
      window.URL.revokeObjectURL(downloadUrl)

      ElMessage.success('文件下载成功')
    } catch (error) {
      console.error('文件下载失败:', error)
      ElMessage.error('文件下载失败')
    }
  }
}

// 导出请求实例和封装的方法
export { service }
export default Request
