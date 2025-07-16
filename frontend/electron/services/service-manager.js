const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');
const log = require('electron-log');
const os = require('os');

class ServiceManager {
  constructor() {
    this.services = {};
    
    // Get application root directory
    const appRoot = path.join(__dirname, '../../..');
    
    this.serviceConfigs = {
      modsrv: {
        name: 'Model Service',
        executable: os.platform() === 'win32' ? 'modsrv.exe' : 'modsrv',
        args: ['--config', 'modsrv.toml'],
        path: path.join(appRoot, 'services/modsrv/target/release'),
        configPath: path.join(appRoot, 'services/modsrv/modsrv.toml')
      },
      comsrv: {
        name: 'Communication Service',
        executable: os.platform() === 'win32' ? 'comsrv.exe' : 'comsrv',
        args: ['--config', 'comsrv.json'],
        path: path.join(appRoot, 'services/Comsrv/target/release'),
        configPath: path.join(appRoot, 'services/Comsrv/comsrv.json')
      },
      hissrv: {
        name: 'History Service',
        executable: os.platform() === 'win32' ? 'hissrv.exe' : 'hissrv',
        args: ['--config', 'hissrv.yaml'],
        path: path.join(appRoot, 'services/Hissrv/target/release'),
        configPath: path.join(appRoot, 'services/Hissrv/hissrv.yaml')
      },
      netsrv: {
        name: 'Network Service',
        executable: os.platform() === 'win32' ? 'netsrv.exe' : 'netsrv',
        args: ['--config', 'netsrv.json'],
        path: path.join(appRoot, 'services/netsrv/target/release'),
        configPath: path.join(appRoot, 'services/netsrv/netsrv.json')
      }
    };
    
    // Log service configurations
    log.info('Service configurations:', this.serviceConfigs);
  }

  // Start a service
  startService(serviceName) {
    if (this.services[serviceName]) {
      log.info(`Service ${serviceName} is already running`);
      return { status: 'running', message: `Service ${serviceName} is already running` };
    }

    const config = this.serviceConfigs[serviceName];
    if (!config) {
      log.error(`Unknown service: ${serviceName}`);
      return { status: 'error', message: `Unknown service: ${serviceName}` };
    }

    try {
      // Check if executable exists
      const execPath = path.join(config.path, config.executable);
      log.info(`Looking for executable at: ${execPath}`);
      
      if (!fs.existsSync(execPath)) {
        log.error(`Executable not found: ${execPath}`);
        return { status: 'error', message: `Executable not found: ${execPath}` };
      }

      // Start service process
      const process = spawn(execPath, config.args, {
        cwd: config.path,
        stdio: ['ignore', 'pipe', 'pipe']
      });

      // Handle output
      process.stdout.on('data', (data) => {
        log.info(`[${serviceName}] ${data.toString().trim()}`);
      });

      process.stderr.on('data', (data) => {
        log.error(`[${serviceName}] ${data.toString().trim()}`);
      });

      // Handle process exit
      process.on('close', (code) => {
        log.info(`Service ${serviceName} exited with code ${code}`);
        delete this.services[serviceName];
      });

      // Store process reference
      this.services[serviceName] = {
        process,
        startTime: new Date(),
        config
      };

      log.info(`Started service: ${serviceName}`);
      return { status: 'started', message: `Service ${serviceName} started successfully` };
    } catch (error) {
      log.error(`Failed to start service ${serviceName}: ${error.message}`);
      return { status: 'error', message: `Failed to start service: ${error.message}` };
    }
  }

  // Stop a service
  stopService(serviceName) {
    const service = this.services[serviceName];
    if (!service) {
      log.info(`Service ${serviceName} is not running`);
      return { status: 'not_running', message: `Service ${serviceName} is not running` };
    }

    try {
      // On Windows, use taskkill to forcefully terminate the process
      if (os.platform() === 'win32') {
        spawn('taskkill', ['/pid', service.process.pid, '/f', '/t']);
      } else {
        service.process.kill('SIGTERM');
      }

      delete this.services[serviceName];
      log.info(`Stopped service: ${serviceName}`);
      return { status: 'stopped', message: `Service ${serviceName} stopped successfully` };
    } catch (error) {
      log.error(`Failed to stop service ${serviceName}: ${error.message}`);
      return { status: 'error', message: `Failed to stop service: ${error.message}` };
    }
  }

  // Restart a service
  restartService(serviceName) {
    const stopResult = this.stopService(serviceName);
    if (stopResult.status === 'error') {
      return stopResult;
    }

    // Wait a short time to ensure the service is completely stopped
    return new Promise((resolve) => {
      setTimeout(() => {
        resolve(this.startService(serviceName));
      }, 1000);
    });
  }

  // Get service status
  getServiceStatus(serviceName) {
    const service = this.services[serviceName];
    if (!service) {
      return { 
        service: serviceName,
        status: 'stopped', 
        message: `Service ${serviceName} is not running` 
      };
    }

    const uptime = Math.floor((new Date() - service.startTime) / 1000);
    return {
      service: serviceName,
      status: 'running',
      uptime,
      pid: service.process.pid,
      name: service.config.name
    };
  }

  // Get status of all services
  getAllServicesStatus() {
    const statuses = {};
    Object.keys(this.serviceConfigs).forEach(serviceName => {
      statuses[serviceName] = this.getServiceStatus(serviceName);
    });
    return statuses;
  }

  // Start all services
  startAllServices() {
    const results = {};
    Object.keys(this.serviceConfigs).forEach(serviceName => {
      results[serviceName] = this.startService(serviceName);
    });
    return results;
  }

  // Stop all services
  stopAllServices() {
    const results = {};
    Object.keys(this.services).forEach(serviceName => {
      results[serviceName] = this.stopService(serviceName);
    });
    return results;
  }
}

module.exports = new ServiceManager(); 