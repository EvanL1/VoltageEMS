/**
 * Environment variable type definitions that provide typed access across the project.
 */

/// <reference types="vite/client" />

interface ImportMetaEnv {
  // Application metadata.
  readonly VITE_APP_NAME: string // Application name.
  readonly VITE_APP_VERSION: string // Application version.
  readonly VITE_APP_DESCRIPTION: string // Application description.
  readonly VITE_APP_LOCALE: string // Default locale.

  // Environment marker.
  readonly VITE_NODE_ENV: 'development' | 'production' | 'test'
  
  // API configuration.
  readonly VITE_API_BASE_URL: string // API base URL.
  readonly VITE_WS_URL: string // WebSocket endpoint.
  readonly VITE_UPLOAD_URL: string // File upload URL.

  // Feature toggles.
  readonly VITE_USE_MOCK: string // Whether mock data is enabled.
  readonly VITE_DEBUG: string // Whether debug mode is enabled.
  readonly VITE_VUE_DEVTOOLS: string // Whether Vue DevTools is enabled.
  readonly VITE_ERROR_MONITORING: string // Whether error monitoring is enabled.
  readonly VITE_ENABLE_TESTING_TOOLS: string // Whether testing tools are enabled.

  // Development environment configuration.
  readonly VITE_DEV_PORT: string // Development server port.
  readonly VITE_OPEN_BROWSER: string // Whether to open the browser automatically.

  // Asset configuration.
  readonly VITE_CDN_URL: string // CDN address.
  readonly VITE_BASE_PATH: string // Application base path.
  readonly VITE_ASSETS_PATH: string // Static asset path.

  // Logging configuration.
  readonly VITE_LOG_LEVEL: 'debug' | 'info' | 'warn' | 'error'

  // Security configuration.
  readonly VITE_ENABLE_HTTPS: string // Whether HTTPS is enabled.
  readonly VITE_SECURE_COOKIES: string // Whether secure cookies are enabled.

  // Performance monitoring.
  readonly VITE_ENABLE_PERFORMANCE_MONITORING: string

  // Developer settings (optional for local environments).
  readonly VITE_DEVELOPER_NAME?: string // Developer name.
  readonly VITE_DEVELOPER_EMAIL?: string // Developer email.

  // Debug settings (optional for local environments).
  readonly VITE_DEBUG_COMPONENTS?: string // Whether to debug components.
  readonly VITE_DEBUG_API?: string // Whether to debug APIs.
  readonly VITE_DEBUG_ROUTER?: string // Whether to debug routing.

  // Testing settings (optional for local environments).
  readonly VITE_TEST_USER_TOKEN?: string // Test user token.
  readonly VITE_TEST_API_DELAY?: string // Test API delay.

  // Mock settings (optional for local environments).
  readonly VITE_MOCK_DELAY?: string // Mock delay.
  readonly VITE_MOCK_ERROR_RATE?: string // Mock error rate.

  // Experimental feature toggles (optional for local environments).
  readonly VITE_ENABLE_EXPERIMENTAL_FEATURES?: string

  // Database configuration (optional for local environments).
  readonly VITE_DB_HOST?: string // Database host.
  readonly VITE_DB_PORT?: string // Database port.
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}

/**
 * Environment variable utilities that provide type-safe access helpers.
 */
export class EnvUtils {
  /**
   * Retrieve a boolean environment variable.
   * @param key Environment variable key.
   * @param defaultValue Default value when the setting is absent.
   * @returns Boolean value.
   */
  static getBoolean(key: keyof ImportMetaEnv, defaultValue = false): boolean {
    const value = import.meta.env[key]
    if (typeof value !== 'string') return defaultValue
    return value.toLowerCase() === 'true'
  }

  /**
   * Retrieve a numeric environment variable.
   * @param key Environment variable key.
   * @param defaultValue Default value when parsing fails.
   * @returns Numeric value.
   */
  static getNumber(key: keyof ImportMetaEnv, defaultValue = 0): number {
    const value = import.meta.env[key]
    if (typeof value !== 'string') return defaultValue
    const num = parseInt(value, 10)
    return isNaN(num) ? defaultValue : num
  }

  /**
   * Retrieve a string environment variable.
   * @param key Environment variable key.
   * @param defaultValue Default value when the setting is absent.
   * @returns String value.
   */
  static getString(key: keyof ImportMetaEnv, defaultValue = ''): string {
    const value = import.meta.env[key]
    return typeof value === 'string' ? value : defaultValue
  }

  /**
   * Determine whether the current environment is development.
   * @returns True when running in development.
   */
  static isDevelopment(): boolean {
    return this.getString('VITE_NODE_ENV') === 'development'
  }

  /**
   * Determine whether the current environment is production.
   * @returns True when running in production.
   */
  static isProduction(): boolean {
    return this.getString('VITE_NODE_ENV') === 'production'
  }

  /**
   * Determine whether debug mode is enabled.
   * @returns True when debug mode is enabled.
   */
  static isDebugMode(): boolean {
    return this.getBoolean('VITE_DEBUG')
  }

  /**
   * Determine whether mock data is enabled.
   * @returns True when mock data is enabled.
   */
  static isMockEnabled(): boolean {
    return this.getBoolean('VITE_USE_MOCK')
  }

  /**
   * Retrieve the API base URL.
   * @returns API base URL.
   */
  static getApiBaseUrl(): string {
    return this.getString('VITE_API_BASE_URL', '/api')
  }

  /**
   * Retrieve key application information.
   * @returns Application information object.
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
