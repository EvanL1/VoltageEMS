import { invoke } from '@tauri-apps/api/core';
import type {
  ServiceInfo,
  ServiceConfig,
  ValidationResult,
  DiffResult,
} from '@/types/config';

export const configApi = {
  async getAllServices(): Promise<ServiceInfo[]> {
    return await invoke('get_all_services');
  },

  async getServiceConfig(service: string): Promise<ServiceConfig> {
    return await invoke('get_service_config', { service });
  },

  async updateServiceConfig(service: string, config: any): Promise<void> {
    return await invoke('update_service_config', { service, config });
  },

  async validateConfig(service: string, config: any): Promise<ValidationResult> {
    return await invoke('validate_config', { service, config });
  },

  async getServiceStatus(service: string): Promise<ServiceInfo> {
    return await invoke('get_service_status', { service });
  },

  async getConfigDiff(
    service: string,
    version1: string,
    version2: string
  ): Promise<DiffResult> {
    return await invoke('get_config_diff', { service, version1, version2 });
  },

  async importConfig(service: string, filePath: string): Promise<void> {
    return await invoke('import_config', { service, filePath });
  },

  async exportConfig(service: string, filePath: string): Promise<void> {
    return await invoke('export_config', { service, filePath });
  },
};