/**
 * Electron环境检测和API工具
 */

/**
 * Check if the application is running in Electron environment
 * @returns {boolean} True if running in Electron, false otherwise
 */
export function isElectron() {
  // Renderer process
  if (typeof window !== 'undefined' && typeof window.process === 'object' && window.process.type === 'renderer') {
    return true;
  }

  // Main process
  if (typeof process !== 'undefined' && typeof process.versions === 'object' && !!process.versions.electron) {
    return true;
  }

  // Detect if running in Electron with exposed API
  if (typeof window !== 'undefined' && typeof window.electronAPI !== 'undefined') {
    return true;
  }

  return false;
}

/**
 * Get the Electron API if available
 * @returns {object|null} The Electron API object or null if not in Electron environment
 */
export function getElectronAPI() {
  if (isElectron() && typeof window !== 'undefined' && window.electronAPI) {
    return window.electronAPI;
  }
  return null;
}

/**
 * Safely call an Electron API method
 * @param {string} method - The method name to call
 * @param {any} args - Arguments to pass to the method
 * @returns {any} The result of the method call or null if not available
 */
export function callElectronAPI(method, ...args) {
  const api = getElectronAPI();
  if (api && typeof api[method] === 'function') {
    return api[method](...args);
  }
  console.warn(`Electron API method ${method} is not available`);
  return null;
}

// 获取应用版本
export const getAppVersion = () => {
  if (isElectron()) {
    return window.electronAPI.getAppVersion();
  }
  return process.env.VUE_APP_VERSION || '1.0.0';
};

// 获取平台信息
export const getPlatform = () => {
  if (isElectron()) {
    return window.electronAPI.getPlatform();
  }
  return 'web';
};

// 服务管理API
export const serviceAPI = {
  // 启动服务
  startService: (serviceId) => {
    if (isElectron()) {
      return window.electronAPI.startService(serviceId);
    }
    console.warn('Not running in Electron environment');
    return Promise.resolve({ status: 'error', message: 'Not running in Electron environment' });
  },
  
  // 停止服务
  stopService: (serviceId) => {
    if (isElectron()) {
      return window.electronAPI.stopService(serviceId);
    }
    console.warn('Not running in Electron environment');
    return Promise.resolve({ status: 'error', message: 'Not running in Electron environment' });
  },
  
  // 重启服务
  restartService: (serviceId) => {
    if (isElectron()) {
      return window.electronAPI.restartService(serviceId);
    }
    console.warn('Not running in Electron environment');
    return Promise.resolve({ status: 'error', message: 'Not running in Electron environment' });
  },
  
  // 获取服务状态
  getServiceStatus: (serviceId) => {
    if (isElectron()) {
      return window.electronAPI.getServiceStatus(serviceId);
    }
    console.warn('Not running in Electron environment');
    return Promise.resolve({ status: 'unknown', message: 'Not running in Electron environment' });
  }
};

// 发送消息到主进程
export const sendToMain = (channel, data) => {
  if (isElectron()) {
    window.electronAPI.sendMessage(channel, data);
    return true;
  }
  console.warn('Not running in Electron environment');
  return false;
};

// 监听来自主进程的消息
export const listenFromMain = (channel, callback) => {
  if (isElectron()) {
    window.electronAPI.onMessage(channel, callback);
    return true;
  }
  console.warn('Not running in Electron environment');
  return false;
}; 