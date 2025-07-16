const { app, BrowserWindow, ipcMain, Menu, protocol, session } = require('electron');
const path = require('path');
const url = require('url');
const log = require('electron-log');
const { autoUpdater } = require('electron-updater');
const serviceManager = require('./services/service-manager');
const fs = require('fs');

// Configure logging
log.transports.file.level = 'info';
autoUpdater.logger = log;

// Keep a global reference to the window object to prevent it from being garbage collected
let mainWindow;

// Register protocol when app is ready
app.whenReady().then(() => {
  // Register custom protocol for handling local resources
  protocol.registerFileProtocol('app', (request, callback) => {
    const url = request.url.substring(6);
    callback({ path: path.normalize(`${__dirname}/${url}`) });
  });
  
  createWindow();
  
  app.on('activate', function () {
    // On macOS, recreate a window when the dock icon is clicked and no other windows are open
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
  
  // Check for updates
  autoUpdater.checkForUpdatesAndNotify();
});

function createWindow() {
  // Create the browser window
  mainWindow = new BrowserWindow({
    width: 1280,
    height: 800,
    minWidth: 1024,
    minHeight: 768,
    webPreferences: {
      nodeIntegration: false, // Don't integrate Node directly
      contextIsolation: true, // Context isolation
      preload: path.join(__dirname, 'preload.js'), // Use preload script
      webSecurity: false // Allow loading local resources
    },
    icon: path.join(__dirname, '../../build/icons/icon.png')
  });

  // Load the application
  const indexPath = path.join(__dirname, '../dist/index.html');
  log.info(`Loading frontend from: ${indexPath}`);
  
  // Check if the file exists
  if (fs.existsSync(indexPath)) {
    log.info('Frontend index.html file exists');
  } else {
    log.error('Frontend index.html file does not exist');
  }

  // Set CSP headers to allow loading local resources
  session.defaultSession.webRequest.onHeadersReceived((details, callback) => {
    callback({
      responseHeaders: {
        ...details.responseHeaders,
        'Content-Security-Policy': ["default-src 'self' 'unsafe-inline' 'unsafe-eval' data: http: https:"]
      }
    });
  });

  // Use development server URL in development mode
  const startUrl = process.env.NODE_ENV === 'development' 
    ? 'http://localhost:8080' 
    : url.format({
        pathname: indexPath,
        protocol: 'file:',
        slashes: true
      });
  
  log.info(`Loading URL: ${startUrl}`);
  mainWindow.loadURL(startUrl);

  // Open DevTools for debugging
  if (process.env.NODE_ENV === 'development') {
    mainWindow.webContents.openDevTools();
  }

  // Listen for load errors
  mainWindow.webContents.on('did-fail-load', (event, errorCode, errorDescription) => {
    log.error(`Failed to load: ${errorCode} - ${errorDescription}`);
  });

  // When window is closed
  mainWindow.on('closed', function () {
    mainWindow = null;
  });

  // Create application menu
  createMenu();
}

// Create application menu
function createMenu() {
  const template = [
    {
      label: 'File',
      submenu: [
        { role: 'quit' }
      ]
    },
    {
      label: 'View',
      submenu: [
        { role: 'reload' },
        { role: 'forceReload' },
        { role: 'toggleDevTools' },
        { type: 'separator' },
        { role: 'resetZoom' },
        { role: 'zoomIn' },
        { role: 'zoomOut' },
        { type: 'separator' },
        { role: 'togglefullscreen' }
      ]
    },
    {
      label: 'Services',
      submenu: [
        {
          label: 'Start All Services',
          click: () => {
            const results = serviceManager.startAllServices();
            log.info('Start all services results:', results);
          }
        },
        {
          label: 'Stop All Services',
          click: () => {
            const results = serviceManager.stopAllServices();
            log.info('Stop all services results:', results);
          }
        },
        { type: 'separator' },
        {
          label: 'Service Status',
          click: () => {
            const statuses = serviceManager.getAllServicesStatus();
            log.info('Service statuses:', statuses);
            // Status dialog can be shown here
          }
        }
      ]
    },
    {
      label: 'Help',
      submenu: [
        {
          label: 'About',
          click: () => {
            // Show about dialog
          }
        }
      ]
    }
  ];

  const menu = Menu.buildFromTemplate(template);
  Menu.setApplicationMenu(menu);
}

// Quit the application when all windows are closed
app.on('window-all-closed', function () {
  // On macOS, applications and their menu bar stay active until the user quits explicitly with Cmd + Q
  if (process.platform !== 'darwin') app.quit();
});

// Stop all services before the application quits
app.on('will-quit', () => {
  log.info('Application is quitting, stopping all services...');
  serviceManager.stopAllServices();
});

// Handle IPC messages from renderer process
ipcMain.on('app-message', (event, arg) => {
  log.info('Received message from renderer:', arg);
  // Process message
  event.reply('app-reply', 'Message received by main process');
});

// Service control IPC handling
ipcMain.on('service-control', async (event, arg) => {
  log.info('Service control request:', arg);
  
  const { action, service } = arg;
  let result;
  
  switch (action) {
    case 'start':
      result = await serviceManager.startService(service);
      break;
    case 'stop':
      result = await serviceManager.stopService(service);
      break;
    case 'restart':
      result = await serviceManager.restartService(service);
      break;
    case 'status':
      result = serviceManager.getServiceStatus(service);
      break;
    case 'all-status':
      result = serviceManager.getAllServicesStatus();
      break;
    default:
      result = { status: 'error', message: `Unknown action: ${action}` };
  }
  
  // Send result back to renderer process
  event.reply('service-status', result);
  
  // If it's a status request for a specific service, also send a specific reply
  if (action === 'status') {
    event.reply(`service-status-${service}`, result);
  }
}); 