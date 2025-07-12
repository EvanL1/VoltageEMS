import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { configApi } from '@/api/config';
import type {
  ServiceInfo,
  ServiceConfig,
  ConfigVersion,
  ValidationError,
} from '@/types/config';

export const useConfigStore = defineStore('config', () => {
  // State
  const services = ref<ServiceInfo[]>([]);
  const currentService = ref<ServiceConfig | null>(null);
  const currentServiceName = ref<string>('');
  const configHistory = ref<ConfigVersion[]>([]);
  const validationErrors = ref<ValidationError[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  // Getters
  const servicesByStatus = computed(() => {
    const grouped: Record<string, ServiceInfo[]> = {
      running: [],
      stopped: [],
      error: [],
      unknown: [],
    };

    services.value.forEach((service) => {
      grouped[service.status].push(service);
    });

    return grouped;
  });

  const hasValidationErrors = computed(() => validationErrors.value.length > 0);

  // Actions
  async function fetchAllServices() {
    loading.value = true;
    error.value = null;
    try {
      services.value = await configApi.getAllServices();
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch services';
    } finally {
      loading.value = false;
    }
  }

  async function fetchServiceConfig(serviceName: string) {
    loading.value = true;
    error.value = null;
    try {
      currentService.value = await configApi.getServiceConfig(serviceName);
      currentServiceName.value = serviceName;
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to fetch config';
    } finally {
      loading.value = false;
    }
  }

  async function updateServiceConfig(serviceName: string, config: any) {
    loading.value = true;
    error.value = null;
    try {
      await configApi.updateServiceConfig(serviceName, config);
      await fetchServiceConfig(serviceName);
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to update config';
      throw e;
    } finally {
      loading.value = false;
    }
  }

  async function validateConfig(serviceName: string, config: any) {
    try {
      const result = await configApi.validateConfig(serviceName, config);
      validationErrors.value = result.errors || [];
      return result.valid;
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Failed to validate config';
      return false;
    }
  }

  function clearValidationErrors() {
    validationErrors.value = [];
  }

  return {
    // State
    services,
    currentService,
    currentServiceName,
    configHistory,
    validationErrors,
    loading,
    error,

    // Getters
    servicesByStatus,
    hasValidationErrors,

    // Actions
    fetchAllServices,
    fetchServiceConfig,
    updateServiceConfig,
    validateConfig,
    clearValidationErrors,
  };
});