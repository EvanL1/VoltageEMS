/**
 * 环境变量类型定义
 * 为项目中使用的环境变量提供类型支持
 */

/// <reference types="vite/client" />

interface ImportMetaEnv {
  // 应用基础信息
  readonly VITE_APP_NAME: string // 应用名称
  readonly VITE_APP_VERSION: string // 应用版本
  readonly VITE_APP_DESCRIPTION: string // 应用描述
  readonly VITE_APP_LOCALE: string // 默认语言

  // 环境标识
  readonly VITE_NODE_ENV: 'development' | 'production' | 'test'

  // API配置
  readonly VITE_API_BASE_URL: string // API基础地址
  readonly VITE_WS_URL: string // WebSocket地址
  readonly VITE_UPLOAD_URL: string // 文件上传地址

  // 功能开关
  readonly VITE_USE_MOCK: string // 是否启用Mock数据
  readonly VITE_DEBUG: string // 是否启用调试模式
  readonly VITE_VUE_DEVTOOLS: string // 是否启用Vue DevTools
  readonly VITE_ERROR_MONITORING: string // 是否启用错误监控
  readonly VITE_ENABLE_TESTING_TOOLS: string // 是否启用测试工具

  // 开发环境配置
  readonly VITE_DEV_PORT: string // 开发服务器端口
  readonly VITE_OPEN_BROWSER: string // 是否自动打开浏览器

  // 资源配置
  readonly VITE_CDN_URL: string // CDN地址
  readonly VITE_BASE_PATH: string // 应用部署路径
  readonly VITE_ASSETS_PATH: string // 静态资源路径

  // 日志配置
  readonly VITE_LOG_LEVEL: 'debug' | 'info' | 'warn' | 'error'

  // 安全配置
  readonly VITE_ENABLE_HTTPS: string // 是否启用HTTPS
  readonly VITE_SECURE_COOKIES: string // 是否启用安全Cookie

  // 性能监控
  readonly VITE_ENABLE_PERFORMANCE_MONITORING: string

  // 开发者配置（本地环境可选）
  readonly VITE_DEVELOPER_NAME?: string // 开发者名称
  readonly VITE_DEVELOPER_EMAIL?: string // 开发者邮箱

  // 调试配置（本地环境可选）
  readonly VITE_DEBUG_COMPONENTS?: string // 是否调试组件
  readonly VITE_DEBUG_API?: string // 是否调试API
  readonly VITE_DEBUG_ROUTER?: string // 是否调试路由

  // 测试配置（本地环境可选）
  readonly VITE_TEST_USER_TOKEN?: string // 测试用户Token
  readonly VITE_TEST_API_DELAY?: string // 测试API延迟

  // Mock配置（本地环境可选）
  readonly VITE_MOCK_DELAY?: string // Mock延迟时间
  readonly VITE_MOCK_ERROR_RATE?: string // Mock错误率

  // 实验性功能开关（本地环境可选）
  readonly VITE_ENABLE_EXPERIMENTAL_FEATURES?: string

  // 数据库配置（本地环境可选）
  readonly VITE_DB_HOST?: string // 数据库主机
  readonly VITE_DB_PORT?: string // 数据库端口
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}

/**
 * 环境变量工具类
 * 提供类型安全的环境变量访问方法
 */
export class EnvUtils {
  /**
   * 获取布尔类型的环境变量
   * @param key 环境变量键名
   * @param defaultValue 默认值
   * @returns 布尔值
   */
  static getBoolean(key: keyof ImportMetaEnv, defaultValue = false): boolean {
    const value = import.meta.env[key]
    if (typeof value !== 'string') return defaultValue
    return value.toLowerCase() === 'true'
  }

  /**
   * 获取数字类型的环境变量
   * @param key 环境变量键名
   * @param defaultValue 默认值
   * @returns 数字值
   */
  static getNumber(key: keyof ImportMetaEnv, defaultValue = 0): number {
    const value = import.meta.env[key]
    if (typeof value !== 'string') return defaultValue
    const num = parseInt(value, 10)
    return isNaN(num) ? defaultValue : num
  }

  /**
   * 获取字符串类型的环境变量
   * @param key 环境变量键名
   * @param defaultValue 默认值
   * @returns 字符串值
   */
  static getString(key: keyof ImportMetaEnv, defaultValue = ''): string {
    const value = import.meta.env[key]
    return typeof value === 'string' ? value : defaultValue
  }

  /**
   * 检查是否为开发环境
   * @returns 是否为开发环境
   */
  static isDevelopment(): boolean {
    return this.getString('VITE_NODE_ENV') === 'development'
  }

  /**
   * 检查是否为生产环境
   * @returns 是否为生产环境
   */
  static isProduction(): boolean {
    return this.getString('VITE_NODE_ENV') === 'production'
  }

  /**
   * 检查是否启用调试模式
   * @returns 是否启用调试模式
   */
  static isDebugMode(): boolean {
    return this.getBoolean('VITE_DEBUG')
  }

  /**
   * 检查是否启用Mock数据
   * @returns 是否启用Mock数据
   */
  static isMockEnabled(): boolean {
    return this.getBoolean('VITE_USE_MOCK')
  }

  /**
   * 获取API基础地址
   * @returns API基础地址
   */
  static getApiBaseUrl(): string {
    return this.getString('VITE_API_BASE_URL', '/api')
  }

  /**
   * 获取应用信息
   * @returns 应用信息对象
   */
  static getAppInfo() {
    return {
      name: this.getString('VITE_APP_NAME', 'Volotage-EMS'),
      version: this.getString('VITE_APP_VERSION', '1.0.0'),
      description: this.getString('VITE_APP_DESCRIPTION', 'Energy Management System'),
      locale: this.getString('VITE_APP_LOCALE', 'zh-CN'),
    }
  }
}
