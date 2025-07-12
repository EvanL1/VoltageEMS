export interface ServiceInfo {
  name: string;
  version: string;
  status: ServiceStatus;
  uptime: string;
  memory: string;
  connections: number;
}

export type ServiceStatus = 'running' | 'stopped' | 'error' | 'unknown';

export interface ServiceConfig {
  name: string;
  version: string;
  redis: RedisConfig;
  channels?: ChannelConfig[];
  logging: LoggingConfig;
}

export interface RedisConfig {
  url: string;
  prefix: string;
  poolSize: number;
}

export interface ChannelConfig {
  id: number;
  name: string;
  protocol: string;
  enabled: boolean;
  parameters: Record<string, any>;
}

export interface LoggingConfig {
  level: string;
  file: string;
  rotation: string;
}

export interface ValidationResult {
  valid: boolean;
  errors: ValidationError[];
  warnings: ValidationWarning[];
}

export interface ValidationError {
  field: string;
  message: string;
}

export interface ValidationWarning {
  field: string;
  message: string;
}

export interface ConfigVersion {
  version: number;
  timestamp: string;
  author: string;
  message: string;
}

export interface DiffResult {
  added: DiffItem[];
  removed: DiffItem[];
  modified: DiffItem[];
}

export interface DiffItem {
  path: string;
  oldValue?: any;
  newValue?: any;
}