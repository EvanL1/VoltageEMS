const { contextBridge, ipcRenderer } = require('electron');

// Safely expose APIs to the renderer process
contextBridge.exposeInMainWorld('electronAPI', {
  // System information
  getAppVersion: () => process.env.npm_package_version,
  getPlatform: () => process.platform,
  
  // IPC communication
  sendMessage: (channel, data) => {
    // Whitelist channels
    const validChannels = ['app-message', 'service-control'];
    if (validChannels.includes(channel)) {
      ipcRenderer.send(channel, data);
    }
  },
  
  // Receive messages
  onMessage: (channel, callback) => {
    const validChannels = ['app-reply', 'service-status'];
    if (validChannels.includes(channel)) {
      // Remove old listeners to avoid duplicates
      ipcRenderer.removeAllListeners(channel);
      // Add new listener
      ipcRenderer.on(channel, (event, ...args) => callback(...args));
    }
  },
  
  // Service control
  startService: (serviceName) => {
    ipcRenderer.send('service-control', { action: 'start', service: serviceName });
  },
  
  stopService: (serviceName) => {
    ipcRenderer.send('service-control', { action: 'stop', service: serviceName });
  },
  
  restartService: (serviceName) => {
    ipcRenderer.send('service-control', { action: 'restart', service: serviceName });
  },
  
  getServiceStatus: (serviceName) => {
    ipcRenderer.send('service-control', { action: 'status', service: serviceName });
    return new Promise((resolve) => {
      ipcRenderer.once(`service-status-${serviceName}`, (event, status) => {
        resolve(status);
      });
    });
  }
}); 