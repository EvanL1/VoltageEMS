import { invoke } from '@tauri-apps/api/core';

// 日志级别
export enum LogLevel {
  DEBUG = 'DEBUG',
  INFO = 'INFO',
  WARN = 'WARN',
  ERROR = 'ERROR',
  CRITICAL = 'CRITICAL'
}

// 日志条目接口
interface LogEntry {
  timestamp: string;
  level: LogLevel;
  message: string;
  context?: any;
  stack?: string;
}

// 日志缓冲区
const logBuffer: LogEntry[] = [];
const MAX_BUFFER_SIZE = 1000;

// 创建日志条目
function createLogEntry(level: LogLevel, message: string, context?: any, stack?: string): LogEntry {
  return {
    timestamp: new Date().toISOString(),
    level,
    message,
    context,
    stack
  };
}

// 添加日志到缓冲区
function addToBuffer(entry: LogEntry) {
  logBuffer.push(entry);
  if (logBuffer.length > MAX_BUFFER_SIZE) {
    logBuffer.shift();
  }
}

// 写入日志到文件（通过 Tauri 命令）
async function writeToFile(entry: LogEntry) {
  try {
    await invoke('write_log', { entry: JSON.stringify(entry) });
  } catch (error) {
    console.error('Failed to write log to file:', error);
  }
}

// 日志记录器类
class Logger {
  private static instance: Logger;

  private constructor() {
    this.setupGlobalErrorHandlers();
  }

  static getInstance(): Logger {
    if (!Logger.instance) {
      Logger.instance = new Logger();
    }
    return Logger.instance;
  }

  // 设置全局错误处理器
  private setupGlobalErrorHandlers() {
    // 捕获未处理的错误
    window.addEventListener('error', (event) => {
      this.error('Uncaught error', {
        message: event.message,
        filename: event.filename,
        lineno: event.lineno,
        colno: event.colno,
        error: event.error?.toString()
      }, event.error?.stack);
    });

    // 捕获未处理的 Promise 拒绝
    window.addEventListener('unhandledrejection', (event) => {
      this.error('Unhandled promise rejection', {
        reason: event.reason,
        promise: event.promise
      });
    });

    // Vue 错误处理将在 main.ts 中设置
  }

  // 日志方法
  debug(message: string, context?: any) {
    this.log(LogLevel.DEBUG, message, context);
  }

  info(message: string, context?: any) {
    this.log(LogLevel.INFO, message, context);
  }

  warn(message: string, context?: any) {
    this.log(LogLevel.WARN, message, context);
  }

  error(message: string, context?: any, stack?: string) {
    this.log(LogLevel.ERROR, message, context, stack);
  }

  critical(message: string, context?: any, stack?: string) {
    this.log(LogLevel.CRITICAL, message, context, stack);
  }

  // 核心日志方法
  private log(level: LogLevel, message: string, context?: any, stack?: string) {
    const entry = createLogEntry(level, message, context, stack);
    
    // 添加到缓冲区
    addToBuffer(entry);
    
    // 控制台输出
    const consoleMethod = this.getConsoleMethod(level);
    if (context) {
      consoleMethod(`[${entry.timestamp}] [${level}] ${message}`, context);
    } else {
      consoleMethod(`[${entry.timestamp}] [${level}] ${message}`);
    }
    
    if (stack) {
      console.error('Stack trace:', stack);
    }
    
    // 异步写入文件（只记录警告及以上级别）
    if ([LogLevel.WARN, LogLevel.ERROR, LogLevel.CRITICAL].includes(level)) {
      writeToFile(entry);
    }
  }

  // 获取对应的控制台方法
  private getConsoleMethod(level: LogLevel) {
    switch (level) {
      case LogLevel.DEBUG:
        return console.debug;
      case LogLevel.INFO:
        return console.info;
      case LogLevel.WARN:
        return console.warn;
      case LogLevel.ERROR:
      case LogLevel.CRITICAL:
        return console.error;
      default:
        return console.log;
    }
  }

  // 获取日志缓冲区
  getLogBuffer(): LogEntry[] {
    return [...logBuffer];
  }

  // 清空日志缓冲区
  clearLogBuffer() {
    logBuffer.length = 0;
  }

  // 导出日志
  async exportLogs(): Promise<string> {
    const logs = this.getLogBuffer();
    const content = logs.map(entry => 
      `[${entry.timestamp}] [${entry.level}] ${entry.message}${
        entry.context ? '\nContext: ' + JSON.stringify(entry.context, null, 2) : ''
      }${
        entry.stack ? '\nStack: ' + entry.stack : ''
      }`
    ).join('\n\n' + '='.repeat(80) + '\n\n');
    
    return content;
  }
}

// 导出单例实例
export const logger = Logger.getInstance();

// 导出便捷方法
export const debug = (message: string, context?: any) => logger.debug(message, context);
export const info = (message: string, context?: any) => logger.info(message, context);
export const warn = (message: string, context?: any) => logger.warn(message, context);
export const error = (message: string, context?: any, stack?: string) => logger.error(message, context, stack);
export const critical = (message: string, context?: any, stack?: string) => logger.critical(message, context, stack);