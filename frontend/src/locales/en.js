export default {
  // Navigation
  nav: {
    home: 'Home',
    services: 'Services',
    devices: 'Devices',
    alarms: 'Alarms',
    historyAnalysis: 'History Analysis',
    liveDashboard: 'Live Dashboard',
    realtimeMonitoring: 'Real-time Monitoring',
    system: 'System',
    activity: 'Activity'
  },

  // Common
  common: {
    refresh: 'Refresh',
    save: 'Save',
    cancel: 'Cancel',
    delete: 'Delete',
    edit: 'Edit',
    add: 'Add',
    search: 'Search',
    status: 'Status',
    name: 'Name',
    type: 'Type',
    value: 'Value',
    time: 'Time',
    actions: 'Actions',
    loading: 'Loading...',
    noData: 'No Data',
    confirm: 'Confirm',
    warning: 'Warning',
    error: 'Error',
    success: 'Success',
    info: 'Information'
  },

  // Header
  header: {
    userDropdown: {
      accountSettings: 'Account Settings',
      logout: 'Logout'
    }
  },

  // Footer
  footer: {
    copyright: 'Voltage, LLC. © 2025 - All Rights Reserved.'
  },

  // Dashboard
  dashboard: {
    title: 'VoltageEMS Dashboard',
    welcomeMessage: 'Welcome to VoltageEMS Real-time Monitoring System',
    systemOverview: 'System Overview',
    recentAlarms: 'Recent Alarms',
    deviceStatus: 'Device Status',
    dataStatistics: 'Data Statistics'
  },

  // Grafana
  grafana: {
    title: 'Real-time Monitoring System',
    selectDashboard: 'Select Dashboard',
    timeRange: {
      '5min': '5 Minutes',
      '15min': '15 Minutes',
      '30min': '30 Minutes',
      '1hour': '1 Hour'
    },
    dashboards: {
      temperatureMonitoring: 'Temperature Monitoring',
      comprehensiveMonitoring: 'Comprehensive Monitoring'
    },
    status: {
      dataSource: 'Data Source',
      connected: 'Connected',
      autoRefresh: 'Auto Refresh',
      lastUpdate: 'Last Update',
      seconds: 'seconds'
    }
  },

  // Devices
  devices: {
    title: 'Device Management',
    deviceList: 'Device List',
    deviceType: 'Device Type',
    deviceId: 'Device ID',
    deviceName: 'Device Name',
    connectionStatus: 'Connection Status',
    lastCommunication: 'Last Communication',
    addDevice: 'Add Device',
    editDevice: 'Edit Device',
    deleteDevice: 'Delete Device',
    types: {
      temperatureSensor: 'Temperature Sensor',
      powerMeter: 'Power Meter',
      voltageMeter: 'Voltage Meter',
      currentMeter: 'Current Meter'
    },
    status: {
      online: 'Online',
      offline: 'Offline',
      error: 'Error',
      maintenance: 'Maintenance'
    }
  },

  // Services
  services: {
    title: 'Service Management',
    serviceList: 'Service List',
    serviceName: 'Service Name',
    serviceStatus: 'Service Status',
    servicePort: 'Service Port',
    startTime: 'Start Time',
    startService: 'Start Service',
    stopService: 'Stop Service',
    restartService: 'Restart Service',
    viewLogs: 'View Logs',
    services: {
      comsrv: 'Communication Service',
      modsrv: 'Model Service',
      hissrv: 'History Service',
      netsrv: 'Network Service',
      alarmsrv: 'Alarm Service'
    },
    status: {
      running: 'Running',
      stopped: 'Stopped',
      starting: 'Starting',
      stopping: 'Stopping',
      error: 'Error'
    }
  },

  // Alarms
  alarms: {
    title: 'Alarm Management',
    alarmList: 'Alarm List',
    alarmLevel: 'Alarm Level',
    alarmMessage: 'Alarm Message',
    alarmTime: 'Alarm Time',
    alarmSource: 'Alarm Source',
    acknowledgeAlarm: 'Acknowledge',
    clearAlarm: 'Clear',
    levels: {
      critical: 'Critical',
      high: 'High',
      medium: 'Medium',
      low: 'Low',
      info: 'Information'
    },
    status: {
      active: 'Active',
      acknowledged: 'Acknowledged',
      cleared: 'Cleared'
    }
  },

  // System
  system: {
    title: 'System Configuration',
    generalSettings: 'General Settings',
    networkSettings: 'Network Settings',
    databaseSettings: 'Database Settings',
    securitySettings: 'Security Settings',
    backupRestore: 'Backup & Restore',
    systemInfo: 'System Information',
    version: 'Version',
    uptime: 'Uptime',
    memory: 'Memory Usage',
    cpu: 'CPU Usage',
    disk: 'Disk Usage'
  },

  // History Analysis
  history: {
    title: 'Historical Data Analysis',
    timeRange: 'Time Range',
    dataType: 'Data Type',
    devices: 'Devices',
    generateReport: 'Generate Report',
    exportData: 'Export Data',
    chartTypes: {
      line: 'Line Chart',
      bar: 'Bar Chart',
      pie: 'Pie Chart',
      scatter: 'Scatter Plot'
    },
    dataTypes: {
      temperature: 'Temperature',
      voltage: 'Voltage',
      current: 'Current',
      power: 'Power',
      energy: 'Energy'
    }
  },

  // Activity
  activity: {
    title: 'Activity Log',
    userActions: 'User Actions',
    systemEvents: 'System Events',
    auditTrail: 'Audit Trail',
    eventType: 'Event Type',
    eventTime: 'Event Time',
    eventDescription: 'Event Description',
    user: 'User',
    ipAddress: 'IP Address',
    types: {
      login: 'Login',
      logout: 'Logout',
      configuration: 'Configuration Change',
      deviceControl: 'Device Control',
      dataExport: 'Data Export',
      systemMaintenance: 'System Maintenance'
    }
  },

  // Messages
  messages: {
    saveSuccess: 'Saved successfully',
    deleteSuccess: 'Deleted successfully',
    operationSuccess: 'Operation completed successfully',
    operationFailed: 'Operation failed',
    networkError: 'Network error, please try again',
    validationError: 'Please check your input',
    confirmDelete: 'Are you sure you want to delete this item?',
    unsavedChanges: 'You have unsaved changes. Are you sure you want to leave?'
  }
}