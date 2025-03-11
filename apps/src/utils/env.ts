/**
 * 环境变量使用示例
 * 演示如何在Vue项目中正确使用环境变量
 */

import { EnvUtils } from '@/types/env.d'

/**
 * 应用配置类
 * 基于环境变量提供应用配置
 */
export class AppConfig {
  /**
   * 获取应用基础信息
   */
  static getAppInfo() {
    return {
      name: import.meta.env.VITE_APP_NAME || 'Volotage-EMS',
      version: import.meta.env.VITE_APP_VERSION || '1.0.0',
      description: import.meta.env.VITE_APP_DESCRIPTION || 'Energy Management System',
      locale: import.meta.env.VITE_APP_LOCALE || 'zh-CN',
    }
  }

  /**
   * 获取API配置
   */
  static getApiConfig() {
    return {
      baseURL: import.meta.env.VITE_API_BASE_URL || '/api',
      wsURL: import.meta.env.VITE_WS_URL || 'ws://localhost:3000/ws',
      uploadURL: import.meta.env.VITE_UPLOAD_URL || '/api/upload',
      timeout: 15000, // 15秒超时
    }
  }

  /**
   * 获取功能开关配置
   */
  static getFeatureFlags() {
    return {
      useMock: import.meta.env.VITE_USE_MOCK === 'true',
      debug: import.meta.env.VITE_DEBUG === 'true',
      vueDevtools: import.meta.env.VITE_VUE_DEVTOOLS === 'true',
      errorMonitoring: import.meta.env.VITE_ERROR_MONITORING === 'true',
      testingTools: import.meta.env.VITE_ENABLE_TESTING_TOOLS === 'true',
      performanceMonitoring: import.meta.env.VITE_ENABLE_PERFORMANCE_MONITORING === 'true',
    }
  }

  /**
   * 获取资源配置
   */
  static getResourceConfig() {
    return {
      cdnURL: import.meta.env.VITE_CDN_URL || '',
      basePath: import.meta.env.VITE_BASE_PATH || '/',
      assetsPath: import.meta.env.VITE_ASSETS_PATH || '/assets/',
    }
  }

  /**
   * 获取日志配置
   */
  static getLogConfig() {
    return {
      level: import.meta.env.VITE_LOG_LEVEL || 'info',
      enableConsole: import.meta.env.VITE_DEBUG === 'true',
      enableRemote: import.meta.env.VITE_ERROR_MONITORING === 'true',
    }
  }

  /**
   * 检查是否为开发环境
   */
  static isDevelopment(): boolean {
    return import.meta.env.DEV || import.meta.env.VITE_NODE_ENV === 'development'
  }

  /**
   * 检查是否为生产环境
   */
  static isProduction(): boolean {
    return import.meta.env.PROD || import.meta.env.VITE_NODE_ENV === 'production'
  }

  /**
   * 获取完整的应用配置
   */
  static getFullConfig() {
    return {
      app: this.getAppInfo(),
      api: this.getApiConfig(),
      features: this.getFeatureFlags(),
      resources: this.getResourceConfig(),
      logging: this.getLogConfig(),
      environment: {
        isDev: this.isDevelopment(),
        isProd: this.isProduction(),
        nodeEnv: import.meta.env.VITE_NODE_ENV || 'development',
      },
    }
  }
}

/**
 * 环境变量验证器
 * 在应用启动时验证必需的环境变量
 */
export class EnvValidator {
  /**
   * 必需的环境变量列表
   */
  private static readonly REQUIRED_VARS = ['VITE_APP_NAME', 'VITE_APP_VERSION', 'VITE_API_BASE_URL']

  /**
   * 验证必需的环境变量
   * @returns 验证结果
   */
  static validate(): { valid: boolean; missing: string[] } {
    const missing: string[] = []

    for (const varName of this.REQUIRED_VARS) {
      const value = import.meta.env[varName]
      if (!value || value.trim() === '') {
        missing.push(varName)
      }
    }

    return {
      valid: missing.length === 0,
      missing,
    }
  }

  /**
   * 打印环境变量验证报告
   */
  static printValidationReport(): void {
    const result = this.validate()

    if (result.valid) {
      console.log('✅ 环境变量验证通过')
    } else {
      console.error('❌ 环境变量验证失败，缺少以下变量:')
      result.missing.forEach((varName) => {
        console.error(`  - ${varName}`)
      })
    }

    // 打印当前环境信息
    console.log('📊 当前环境信息:')
    console.log(`  - 环境: ${import.meta.env.VITE_NODE_ENV || 'development'}`)
    console.log(`  - 应用名称: ${import.meta.env.VITE_APP_NAME || 'Unknown'}`)
    console.log(`  - 应用版本: ${import.meta.env.VITE_APP_VERSION || 'Unknown'}`)
    console.log(`  - API地址: ${import.meta.env.VITE_API_BASE_URL || 'Unknown'}`)
    console.log(`  - 调试模式: ${import.meta.env.VITE_DEBUG === 'true' ? '开启' : '关闭'}`)
    console.log(`  - Mock数据: ${import.meta.env.VITE_USE_MOCK === 'true' ? '开启' : '关闭'}`)
  }
}

// 在开发环境下自动执行验证
if (AppConfig.isDevelopment()) {
  EnvValidator.printValidationReport()
}

// 导出默认配置
export default AppConfig
