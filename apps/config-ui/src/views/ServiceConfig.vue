<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue';
import { useRoute } from 'vue-router';
import { useConfigStore } from '@/stores/config';
import ConfigEditor from '@/components/ConfigEditor.vue';
import * as yaml from 'js-yaml';

const route = useRoute();
const configStore = useConfigStore();

const props = defineProps<{
  name: string;
}>();

const activeTab = ref('basic');
const configText = ref('');
const editorMode = ref<'yaml' | 'json'>('yaml');
const hasChanges = ref(false);

const currentConfig = computed(() => configStore.currentService);

watch(() => props.name, async (newName) => {
  if (newName) {
    await loadServiceConfig(newName);
  }
});

onMounted(() => {
  if (props.name) {
    loadServiceConfig(props.name);
  }
});

async function loadServiceConfig(serviceName: string) {
  await configStore.fetchServiceConfig(serviceName);
  if (currentConfig.value) {
    configText.value = yaml.dump(currentConfig.value);
  }
}

async function saveConfig() {
  try {
    let configObj;
    if (editorMode.value === 'yaml') {
      configObj = yaml.load(configText.value);
    } else {
      configObj = JSON.parse(configText.value);
    }
    
    // 验证配置
    const isValid = await configStore.validateConfig(props.name, configObj);
    if (!isValid) {
      ElMessage.error('配置验证失败，请检查错误信息');
      return;
    }
    
    await configStore.updateServiceConfig(props.name, configObj);
    ElMessage.success('配置保存成功');
    hasChanges.value = false;
  } catch (error) {
    ElMessage.error('保存失败：' + (error as Error).message);
  }
}

function resetConfig() {
  loadServiceConfig(props.name);
  hasChanges.value = false;
}

function exportConfig() {
  const blob = new Blob([configText.value], { type: 'text/plain' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `${props.name}-config.${editorMode.value}`;
  a.click();
  URL.revokeObjectURL(url);
}
</script>

<template>
  <div class="service-config">
    <div class="config-header">
      <h2>{{ name }} 配置管理</h2>
      <div class="header-actions">
        <el-tag v-if="hasChanges" type="warning">有未保存的更改</el-tag>
      </div>
    </div>
    
    <el-tabs v-model="activeTab">
      <el-tab-pane label="配置编辑" name="basic">
        <div class="editor-toolbar">
          <el-radio-group v-model="editorMode" size="small">
            <el-radio-button label="yaml">YAML</el-radio-button>
            <el-radio-button label="json">JSON</el-radio-button>
          </el-radio-group>
          
          <el-button-group>
            <el-button size="small" @click="resetConfig">
              <el-icon><RefreshLeft /></el-icon>
              重置
            </el-button>
            <el-button size="small" @click="exportConfig">
              <el-icon><Download /></el-icon>
              导出
            </el-button>
          </el-button-group>
        </div>
        
        <ConfigEditor
          v-model="configText"
          :language="editorMode"
          @change="hasChanges = true"
        />
        
        <div class="validation-errors" v-if="configStore.hasValidationErrors">
          <el-alert
            v-for="(error, index) in configStore.validationErrors"
            :key="index"
            :title="`${error.field}: ${error.message}`"
            type="error"
            :closable="false"
          />
        </div>
      </el-tab-pane>
      
      <el-tab-pane label="配置历史" name="history">
        <el-empty description="暂无历史记录" />
      </el-tab-pane>
      
      <el-tab-pane label="配置对比" name="diff">
        <el-empty description="选择两个版本进行对比" />
      </el-tab-pane>
    </el-tabs>
    
    <div class="config-actions">
      <el-button @click="resetConfig" :disabled="!hasChanges">
        取消更改
      </el-button>
      <el-button
        type="primary"
        @click="saveConfig"
        :loading="configStore.loading"
        :disabled="!hasChanges"
      >
        保存配置
      </el-button>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.service-config {
  padding: 32px;
  background: var(--bg-secondary);
  border-radius: 12px;
  box-shadow: var(--shadow-lg);
  
  .config-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 24px;
    padding-bottom: 16px;
    border-bottom: 1px solid var(--glass-border);
    
    h2 {
      margin: 0;
      font-size: 28px;
      background: var(--primary-gradient);
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
      background-clip: text;
      font-weight: 700;
      text-shadow: 0 0 30px rgba(98, 106, 239, 0.3);
    }
    
    .header-actions {
      :deep(.el-tag) {
        background: rgba(255, 156, 0, 0.2);
        border: 1px solid var(--warning-color);
        color: var(--warning-color);
        font-weight: 500;
      }
    }
  }
  
  :deep(.el-tabs) {
    .el-tabs__header {
      background: var(--glass-bg);
      backdrop-filter: var(--glass-blur);
      border-radius: 8px;
      padding: 4px;
      margin-bottom: 20px;
      border: 1px solid var(--glass-border);
    }
    
    .el-tabs__nav {
      border: none;
    }
    
    .el-tabs__item {
      color: var(--text-secondary);
      font-weight: 500;
      transition: all 0.3s ease;
      padding: 8px 20px;
      border-radius: 6px;
      
      &:hover {
        color: var(--primary-color);
        background: rgba(98, 106, 239, 0.1);
      }
      
      &.is-active {
        color: var(--primary-color);
        background: rgba(98, 106, 239, 0.2);
        text-shadow: 0 0 10px rgba(98, 106, 239, 0.5);
      }
    }
    
    .el-tabs__active-bar {
      display: none;
    }
  }
  
  .editor-toolbar {
    display: flex;
    justify-content: space-between;
    margin-bottom: 16px;
    padding: 12px 16px;
    background: var(--glass-bg);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--glass-border);
    border-radius: 8px;
    
    :deep(.el-radio-group) {
      .el-radio-button__inner {
        background: transparent;
        border-color: var(--glass-border);
        color: var(--text-primary);
        
        &:hover {
          color: var(--primary-color);
          border-color: var(--primary-color);
        }
      }
      
      .el-radio-button__orig-radio:checked + .el-radio-button__inner {
        background: var(--primary-color);
        border-color: var(--primary-color);
        color: #fff;
        box-shadow: 0 0 10px rgba(98, 106, 239, 0.5);
      }
    }
    
    :deep(.el-button) {
      background: var(--glass-bg);
      border-color: var(--glass-border);
      color: var(--text-primary);
      
      &:hover {
        background: rgba(98, 106, 239, 0.2);
        border-color: var(--primary-color);
        color: var(--primary-color);
        transform: translateY(-2px);
      }
    }
  }
  
  .validation-errors {
    margin-top: 16px;
    
    .el-alert {
      margin-bottom: 12px;
      background: rgba(255, 56, 56, 0.1);
      border: 1px solid rgba(255, 56, 56, 0.3);
      color: var(--danger-color);
      
      :deep(.el-alert__title) {
        color: var(--danger-color);
      }
    }
  }
  
  .config-actions {
    margin-top: 24px;
    padding-top: 20px;
    border-top: 1px solid var(--glass-border);
    text-align: right;
    display: flex;
    justify-content: flex-end;
    gap: 12px;
    
    .el-button {
      min-width: 120px;
      background: var(--glass-bg);
      border: 1px solid var(--glass-border);
      color: var(--text-primary);
      
      &:hover:not(:disabled) {
        background: rgba(98, 106, 239, 0.1);
        border-color: var(--primary-color);
        transform: translateY(-2px);
      }
      
      &.el-button--primary {
        background: var(--primary-gradient);
        border: none;
        color: #fff;
        font-weight: 500;
        
        &:hover:not(:disabled) {
          transform: translateY(-2px);
          box-shadow: 0 8px 24px rgba(98, 106, 239, 0.4);
        }
      }
      
      &:disabled {
        opacity: 0.5;
      }
    }
  }
  
  // 配置编辑器的深色主题
  :deep(.config-editor) {
    background: var(--bg-primary);
    border: 1px solid var(--glass-border);
    border-radius: 8px;
    overflow: hidden;
    
    .monaco-editor {
      background: var(--bg-primary) !important;
      
      .margin {
        background: var(--bg-primary) !important;
      }
    }
  }
}
</style>