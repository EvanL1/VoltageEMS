export default {
  // Navigation
  nav: {
    home: '首页',
    services: '服务',
    devices: '设备',
    alarms: '报警',
    historyAnalysis: '历史分析',
    liveDashboard: '实时面板',
    realtimeMonitoring: '实时监控',
    system: '系统',
    activity: '活动'
  },

  // Common
  common: {
    refresh: '刷新',
    save: '保存',
    cancel: '取消',
    delete: '删除',
    edit: '编辑',
    add: '添加',
    search: '搜索',
    status: '状态',
    name: '名称',
    type: '类型',
    value: '值',
    time: '时间',
    actions: '操作',
    loading: '加载中...',
    noData: '暂无数据',
    confirm: '确认',
    warning: '警告',
    error: '错误',
    success: '成功',
    info: '信息',
    active: '活动'
  },

  // Header
  header: {
    userDropdown: {
      accountSettings: '账户设置',
      logout: '退出登录'
    }
  },

  // Footer
  footer: {
    copyright: 'Voltage, LLC. © 2025 - 保留所有权利。'
  },

  // Dashboard
  dashboard: {
    title: 'VoltageEMS 仪表板',
    welcomeMessage: '欢迎使用 VoltageEMS 实时监控系统',
    systemOverview: '系统概览',
    recentAlarms: '最近报警',
    deviceStatus: '设备状态',
    dataStatistics: '数据统计'
  },

  // Grafana
  grafana: {
    title: '实时监控系统',
    selectDashboard: '选择仪表板',
    loading: '正在加载仪表板...',
    error: '仪表板加载失败',
    retry: '重试',
    authError: '认证失败，请重新登录',
    networkError: '网络连接失败',
    timeout: '加载超时',
    notFound: '仪表板不存在',
    refresh: '刷新',
    fullscreen: '全屏',
    exitFullscreen: '退出全屏',
    timeRange: {
      '5m': '最近5分钟',
      '15m': '最近15分钟',
      '1h': '最近1小时',
      '6h': '最近6小时',
      '24h': '最近24小时',
      '7d': '最近7天',
      custom: '自定义'
    },
    dashboards: {
      realtime: '实时监控',
      historical: '历史分析',
      system: '系统概览',
      energy: '能源管理',
      temperatureMonitoring: '温度监控',
      comprehensiveMonitoring: '综合监控'
    },
    status: {
      dataSource: '数据源',
      connected: '已连接',
      disconnected: '未连接',
      autoRefresh: '自动刷新',
      lastUpdate: '最后更新',
      seconds: '秒'
    },
    controls: {
      timeRange: '时间范围',
      refresh: '刷新间隔',
      variables: '变量',
      theme: '主题'
    },
    openInGrafana: '在 Grafana 中打开',
    dataInfo: '数据信息',
    tabs: {
      overview: '概览',
      metrics: '指标'
    },
    info: {
      simulatedData: '模拟数据说明',
      randomFluctuation: '范围内随机波动',
      fluctuation: '范围内波动',
      variation: '范围内变化',
      higherDaytime: '白天功率更高'
    }
  },

  // Devices
  devices: {
    title: '设备管理',
    deviceList: '设备列表',
    deviceType: '设备类型',
    deviceId: '设备ID',
    deviceName: '设备名称',
    connectionStatus: '连接状态',
    lastCommunication: '最后通信',
    addDevice: '添加设备',
    editDevice: '编辑设备',
    deleteDevice: '删除设备',
    types: {
      temperatureSensor: '温度传感器',
      powerMeter: '电表',
      voltageMeter: '电压表',
      currentMeter: '电流表'
    },
    status: {
      online: '在线',
      offline: '离线',
      error: '错误',
      maintenance: '维护中'
    }
  },

  // Services
  services: {
    title: '服务管理',
    serviceList: '服务列表',
    serviceName: '服务名称',
    serviceStatus: '服务状态',
    servicePort: '服务端口',
    startTime: '启动时间',
    startService: '启动服务',
    stopService: '停止服务',
    restartService: '重启服务',
    viewLogs: '查看日志',
    services: {
      comsrv: '通信服务',
      modsrv: '模型服务',
      hissrv: '历史服务',
      netsrv: '网络服务',
      alarmsrv: '报警服务'
    },
    status: {
      running: '运行中',
      stopped: '已停止',
      starting: '启动中',
      stopping: '停止中',
      error: '错误'
    }
  },

  // Alarms
  alarms: {
    title: '报警管理',
    alarmList: '报警列表',
    alarmLevel: '报警级别',
    alarmMessage: '报警信息',
    alarmTime: '报警时间',
    alarmSource: '报警源',
    acknowledgeAlarm: '确认',
    clearAlarm: '清除',
    levels: {
      critical: '严重',
      high: '高',
      medium: '中',
      low: '低',
      info: '信息'
    },
    status: {
      active: '活动',
      acknowledged: '已确认',
      cleared: '已清除'
    }
  },

  // System
  system: {
    title: '系统配置',
    generalSettings: '常规设置',
    networkSettings: '网络设置',
    databaseSettings: '数据库设置',
    securitySettings: '安全设置',
    backupRestore: '备份与恢复',
    systemInfo: '系统信息',
    version: '版本',
    uptime: '运行时间',
    memory: '内存使用',
    cpu: 'CPU使用',
    disk: '磁盘使用'
  },

  // History Analysis
  history: {
    title: '历史数据分析',
    timeRange: '时间范围',
    dataType: '数据类型',
    devices: '设备',
    generateReport: '生成报告',
    exportData: '导出数据',
    chartTypes: {
      line: '折线图',
      bar: '柱状图',
      pie: '饼图',
      scatter: '散点图'
    },
    dataTypes: {
      temperature: '温度',
      voltage: '电压',
      current: '电流',
      power: '功率',
      energy: '能量'
    }
  },

  // Activity
  activity: {
    title: '活动日志',
    userActions: '用户操作',
    systemEvents: '系统事件',
    auditTrail: '审计跟踪',
    eventType: '事件类型',
    eventTime: '事件时间',
    eventDescription: '事件描述',
    user: '用户',
    ipAddress: 'IP地址',
    types: {
      login: '登录',
      logout: '退出',
      configuration: '配置更改',
      deviceControl: '设备控制',
      dataExport: '数据导出',
      systemMaintenance: '系统维护'
    }
  },

  // Messages
  messages: {
    saveSuccess: '保存成功',
    deleteSuccess: '删除成功',
    operationSuccess: '操作成功完成',
    operationFailed: '操作失败',
    networkError: '网络错误，请重试',
    validationError: '请检查您的输入',
    confirmDelete: '确定要删除此项吗？',
    unsavedChanges: '您有未保存的更改。确定要离开吗？'
  }
}