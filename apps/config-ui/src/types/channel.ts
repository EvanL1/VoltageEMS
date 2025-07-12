import type { PointDefinition, ProtocolMappingEnum } from './point-table';

// 通道信息（列表用）
export interface ChannelInfo {
  id: number;
  name: string;
  protocol: string;
  protocol_type: string;
  enabled: boolean;
  point_counts: {
    telemetry: number;
    signal: number;
    control: number;
    adjustment: number;
  };
  last_updated: string;
}

// 传输配置
export interface TransportConfig {
  transport_type: 'tcp' | 'serial' | 'can';
  tcp?: {
    host: string;
    port: number;
    timeout: number;
    retry_count: number;
  };
  serial?: {
    port_name: string;
    baud_rate: number;
    data_bits: number;
    stop_bits: number;
    parity: 'none' | 'even' | 'odd';
    flow_control: 'none' | 'software' | 'hardware';
  };
  can?: {
    interface: string;
    bitrate: number;
    loopback: boolean;
    recv_own_msgs: boolean;
  };
}

// 协议配置
export interface ProtocolConfig {
  modbus?: {
    mode: 'tcp' | 'rtu';
    timeout_ms: number;
    retry_count: number;
  };
  iec60870?: {
    mode: '104' | '101';
    link_address: number;
    cot_size: number;
    ioa_size: number;
    k: number;
    w: number;
    t1: number;
    t2: number;
    t3: number;
  };
  can?: {
    dbc_file: string;
    filters: Array<{
      id: number;
      mask: number;
    }>;
  };
}

// 轮询配置
export interface PollingConfig {
  interval_ms: number;
  batch_size: number;
  priority: number;
}

// 日志配置
export interface LoggingConfig {
  level: string;
  file: string;
  max_size: string;
  max_backups: number;
}

// 点表配置
export interface PointTableConfig {
  telemetry: PointDefinition[];
  signal: PointDefinition[];
  control: PointDefinition[];
  adjustment: PointDefinition[];
  telemetry_mapping: ProtocolMappingEnum[];
  signal_mapping: ProtocolMappingEnum[];
  control_mapping: ProtocolMappingEnum[];
  adjustment_mapping: ProtocolMappingEnum[];
}

// 完整通道配置
export interface Channel {
  id: number;
  name: string;
  protocol: string;
  protocol_type: string;
  enabled: boolean;
  transport_config: TransportConfig;
  protocol_config: ProtocolConfig;
  polling_config: PollingConfig;
  point_table: PointTableConfig;
  logging: LoggingConfig;
}