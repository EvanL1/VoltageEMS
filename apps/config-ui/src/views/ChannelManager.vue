<script setup lang="ts">
import { ref, reactive, onMounted, computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { ElMessage, ElMessageBox } from 'element-plus';
import { Plus, Upload, Download, Delete, Document, Search, Filter, Refresh, CopyDocument } from '@element-plus/icons-vue';
import ChannelEditor from '@/components/ChannelEditor.vue';
import type { Channel, ChannelInfo } from '@/types/channel';

const channels = ref<ChannelInfo[]>([]);
const loading = ref(false);
const showEditDialog = ref(false);
const currentChannelId = ref<number | null>(null);

// 搜索和过滤
const searchText = ref('');
const filterProtocol = ref('');
const filterStatus = ref<boolean | undefined>();

// 计算过滤后的通道列表
const filteredChannels = computed(() => {
  return channels.value.filter(channel => {
    // 文本搜索
    if (searchText.value && !channel.name.toLowerCase().includes(searchText.value.toLowerCase()) 
        && !channel.protocol_type.toLowerCase().includes(searchText.value.toLowerCase())) {
      return false;
    }
    // 协议过滤
    if (filterProtocol.value && channel.protocol_type !== filterProtocol.value) {
      return false;
    }
    // 状态过滤
    if (filterStatus.value !== undefined && channel.enabled !== filterStatus.value) {
      return false;
    }
    return true;
  });
});

// 模拟数据 - 将被实际API调用替换
const mockChannels: ChannelInfo[] = [
  {
    id: 1,
    name: 'Modbus TCP 主站',
    protocol: 'modbus',
    protocol_type: 'modbus_tcp',
    enabled: true,
    point_counts: {
      telemetry: 50,
      signal: 30,
      control: 10,
      adjustment: 5
    },
    last_updated: new Date().toISOString()
  },
  {
    id: 2,
    name: 'IEC 104 通道',
    protocol: 'iec60870',
    protocol_type: 'iec104',
    enabled: true,
    point_counts: {
      telemetry: 40,
      signal: 20,
      control: 8,
      adjustment: 4
    },
    last_updated: new Date().toISOString()
  },
  {
    id: 3,
    name: 'CAN 总线',
    protocol: 'can',
    protocol_type: 'can',
    enabled: false,
    point_counts: {
      telemetry: 20,
      signal: 10,
      control: 0,
      adjustment: 0
    },
    last_updated: new Date().toISOString()
  },
];

// 加载通道列表
async function loadChannels() {
  loading.value = true;
  try {
    // 暂时使用模拟数据
    channels.value = mockChannels;
    // TODO: 实际API调用
    // channels.value = await invoke<ChannelInfo[]>('get_all_channels');
  } catch (error) {
    console.error('加载通道失败:', error);
    ElMessage.error('加载通道列表失败');
  } finally {
    loading.value = false;
  }
}

const protocolOptions = [
  { label: 'Modbus TCP', value: 'modbus_tcp' },
  { label: 'Modbus RTU', value: 'modbus_rtu' },
  { label: 'IEC 60870-5-104', value: 'iec104' },
  { label: 'IEC 60870-5-101', value: 'iec101' },
  { label: 'CAN Bus', value: 'can' },
];

// 计算通道总点数
function getTotalPoints(channel: ChannelInfo): number {
  const counts = channel.point_counts;
  return counts.telemetry + counts.signal + counts.control + counts.adjustment;
}

function getStatusType(enabled: boolean) {
  return enabled ? 'success' : 'info';
}

function editChannel(channel: ChannelInfo) {
  currentChannelId.value = channel.id;
  showEditDialog.value = true;
}

function addChannel() {
  currentChannelId.value = null;
  showEditDialog.value = true;
}

async function deleteChannel(channel: ChannelInfo) {
  ElMessageBox.confirm(
    `确定要删除通道 "${channel.name}" 吗？这将同时删除该通道的所有点表配置。`,
    '删除确认',
    {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      type: 'warning',
    }
  ).then(async () => {
    try {
      // TODO: 实际API调用
      // await invoke('delete_channel', { id: channel.id });
      const index = channels.value.findIndex(c => c.id === channel.id);
      if (index > -1) {
        channels.value.splice(index, 1);
      }
      ElMessage.success('删除成功');
    } catch (error) {
      console.error('删除通道失败:', error);
      ElMessage.error('删除通道失败');
    }
  });
}

function handleChannelSaved() {
  showEditDialog.value = false;
  loadChannels();
}

// 导出通道配置
async function exportChannel(channel: ChannelInfo) {
  try {
    // TODO: 实际API调用
    // const yamlContent = await invoke<string>('export_channel_config', { channelId: channel.id });
    const yamlContent = `# 通道配置导出\nid: ${channel.id}\nname: ${channel.name}\n`;
    const blob = new Blob([yamlContent], { type: 'text/yaml' });
    const url = window.URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `channel_${channel.id}_${channel.name}.yaml`;
    a.click();
    window.URL.revokeObjectURL(url);
    ElMessage.success('导出成功');
  } catch (error) {
    console.error('导出通道配置失败:', error);
    ElMessage.error('导出通道配置失败');
  }
}

// 导出所有通道配置
async function exportAllChannels() {
  try {
    const yamlContent = `# VoltageEMS 通道配置导出\nchannels:\n${channels.value.map(ch => `  - id: ${ch.id}\n    name: ${ch.name}`).join('\n')}`;
    const blob = new Blob([yamlContent], { type: 'text/yaml' });
    const url = window.URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `all_channels_${new Date().toISOString().split('T')[0]}.yaml`;
    a.click();
    window.URL.revokeObjectURL(url);
    ElMessage.success('导出所有通道配置成功');
  } catch (error) {
    console.error('导出所有通道失败:', error);
    ElMessage.error('导出失败');
  }
}

// 导入通道配置
async function importChannel() {
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = '.yaml,.yml';
  input.onchange = async (e) => {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (!file) return;
    
    try {
      const content = await file.text();
      // TODO: 实际API调用
      // await invoke('import_channel_config', { yamlContent: content });
      ElMessage.success('导入成功');
      loadChannels();
    } catch (error) {
      console.error('导入通道配置失败:', error);
      ElMessage.error('导入通道配置失败');
    }
  };
  input.click();
}

// 复制通道
function duplicateChannel(channel: ChannelInfo) {
  const newChannel = {
    ...channel,
    id: Math.max(...channels.value.map(c => c.id)) + 1,
    name: `${channel.name}_副本`,
    enabled: false
  };
  channels.value.push(newChannel);
  ElMessage.success('通道复制成功');
}

onMounted(() => {
  loadChannels();
});
</script>

<template>
  <div class="channel-manager">
    <!-- 顶部工具栏 -->
    <div class="toolbar">
      <div class="toolbar-left">
        <el-input 
          v-model="searchText" 
          placeholder="搜索通道名称或协议类型"
          :prefix-icon="Search"
          clearable
          style="width: 300px"
        />
        <el-select v-model="filterProtocol" placeholder="所有协议" clearable style="width: 150px">
          <el-option
            v-for="option in protocolOptions"
            :key="option.value"
            :label="option.label"
            :value="option.value"
          />
        </el-select>
        <el-select v-model="filterStatus" placeholder="所有状态" clearable style="width: 120px">
          <el-option label="启用" :value="true" />
          <el-option label="禁用" :value="false" />
        </el-select>
      </div>
      <div class="toolbar-right">
        <el-button @click="importChannel">
          <el-icon><Upload /></el-icon>
          导入配置
        </el-button>
        <el-button @click="exportAllChannels">
          <el-icon><Download /></el-icon>
          导出全部
        </el-button>
        <el-button type="primary" @click="addChannel">
          <el-icon><Plus /></el-icon>
          新建通道
        </el-button>
      </div>
    </div>
    
    <!-- 统计栏 -->
    <div class="stats-bar">
      <div class="stat-item">
        <div class="stat-value">{{ channels.length }}</div>
        <div class="stat-label">总通道数</div>
      </div>
      <div class="stat-divider"></div>
      <div class="stat-item">
        <div class="stat-value">{{ channels.filter(c => c.enabled).length }}</div>
        <div class="stat-label">启用通道</div>
      </div>
      <div class="stat-divider"></div>
      <div class="stat-item">
        <div class="stat-value">{{ channels.reduce((sum, c) => sum + getTotalPoints(c), 0) }}</div>
        <div class="stat-label">总点数</div>
      </div>
      <div class="stat-divider"></div>
      <div class="stat-item">
        <div class="stat-value">{{ new Set(channels.map(c => c.protocol)).size }}</div>
        <div class="stat-label">协议类型</div>
      </div>
    </div>

    <!-- 通道列表表格 -->
    <el-table 
      :data="filteredChannels" 
      style="width: 100%" 
      height="calc(100vh - 300px)"
      v-loading="loading"
      stripe
      :default-sort="{ prop: 'id', order: 'ascending' }"
    >
      <el-table-column prop="id" label="ID" width="60" sortable />
      <el-table-column prop="name" label="通道名称" min-width="150" />
      <el-table-column prop="protocol_type" label="协议类型" width="120">
        <template #default="{ row }">
          {{ protocolOptions.find(p => p.value === row.protocol_type)?.label || row.protocol_type }}
        </template>
      </el-table-column>
      <el-table-column prop="enabled" label="状态" width="80">
        <template #default="{ row }">
          <el-tag :type="getStatusType(row.enabled)" size="small">
            {{ row.enabled ? '启用' : '禁用' }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column label="点表统计" width="180">
        <template #default="{ row }">
          <div class="point-counts">
            <el-tooltip content="遥测(YC)点数">
              <span class="count-item">YC: {{ row.point_counts.telemetry }}</span>
            </el-tooltip>
            <el-tooltip content="遥信(YX)点数">
              <span class="count-item">YX: {{ row.point_counts.signal }}</span>
            </el-tooltip>
            <el-tooltip content="遥控(YK)点数">
              <span class="count-item">YK: {{ row.point_counts.control }}</span>
            </el-tooltip>
            <el-tooltip content="遥调(YT)点数">
              <span class="count-item">YT: {{ row.point_counts.adjustment }}</span>
            </el-tooltip>
          </div>
        </template>
      </el-table-column>
      <el-table-column label="总点数" width="80">
        <template #default="{ row }">
          <span class="total-points" :class="{ 'high-count': getTotalPoints(row) > 100 }">
            {{ getTotalPoints(row) }}
          </span>
        </template>
      </el-table-column>
      <el-table-column prop="last_updated" label="更新时间" width="180" sortable>
        <template #default="{ row }">
          {{ new Date(row.last_updated).toLocaleString() }}
        </template>
      </el-table-column>
      <el-table-column label="操作" width="300" fixed="right">
        <template #default="{ row }">
          <div class="action-buttons">
            <el-button size="small" class="action-btn" @click="editChannel(row)">
              <el-icon><Document /></el-icon>
              配置
            </el-button>
            <el-button size="small" class="action-btn" @click="exportChannel(row)">
              <el-icon><Download /></el-icon>
              导出
            </el-button>
            <el-button size="small" class="action-btn" :disabled="row.enabled" @click="duplicateChannel(row)">
              <el-icon><CopyDocument /></el-icon>
              复制
            </el-button>
            <el-button size="small" class="action-btn danger" @click="deleteChannel(row)">
              <el-icon><Delete /></el-icon>
              删除
            </el-button>
          </div>
        </template>
      </el-table-column>
    </el-table>
    
    <!-- 通道编辑对话框 -->
    <el-dialog
      v-model="showEditDialog"
      :title="currentChannelId ? '编辑通道' : '添加通道'"
      width="90%"
      :close-on-click-modal="false"
      destroy-on-close
    >
      <ChannelEditor
        :channel-id="currentChannelId"
        @saved="handleChannelSaved"
        @cancel="showEditDialog = false"
      />
    </el-dialog>
  </div>
</template>

<style lang="scss" scoped>
.channel-manager {
  padding: 0;
  
  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px 32px;
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-light);
    gap: 16px;
    
    .toolbar-left {
      display: flex;
      align-items: center;
      gap: 12px;
      flex: 1;
    }
    
    .toolbar-right {
      display: flex;
      align-items: center;
      gap: 12px;
    }
  }
  
  .stats-bar {
    display: flex;
    align-items: center;
    padding: 20px 32px;
    background: var(--glass-bg);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--glass-border);
    border-radius: 12px;
    margin: 20px 32px;
    gap: 32px;
    box-shadow: var(--shadow-md);
    position: relative;
    overflow: hidden;
    
    // 背景动画效果
    &::before {
      content: '';
      position: absolute;
      top: -50%;
      left: -50%;
      width: 200%;
      height: 200%;
      background: radial-gradient(circle, rgba(98, 106, 239, 0.1) 0%, transparent 70%);
      animation: rotate 20s linear infinite;
    }
    
    .stat-item {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 8px;
      position: relative;
      z-index: 1;
      transition: transform 0.3s ease;
      
      &:hover {
        transform: translateY(-2px);
        
        .stat-value {
          text-shadow: 0 0 20px var(--primary-color);
        }
      }
      
      .stat-value {
        font-size: 32px;
        font-weight: 700;
        background: var(--primary-gradient);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        background-clip: text;
        line-height: 1;
        transition: all 0.3s ease;
      }
      
      .stat-label {
        font-size: 14px;
        color: var(--text-secondary);
        text-transform: uppercase;
        letter-spacing: 1px;
        font-weight: 500;
      }
    }
    
    .stat-divider {
      width: 1px;
      height: 40px;
      background: linear-gradient(180deg, transparent 0%, var(--primary-color) 50%, transparent 100%);
      opacity: 0.3;
    }
  }
  
  @keyframes rotate {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
  
  .point-counts {
    display: flex;
    gap: 8px;
    font-size: 12px;
    
    .count-item {
      padding: 4px 8px;
      background: var(--glass-bg);
      backdrop-filter: var(--glass-blur);
      border: 1px solid var(--glass-border);
      border-radius: 6px;
      white-space: nowrap;
      transition: all 0.3s ease;
      position: relative;
      overflow: hidden;
      
      &::before {
        content: '';
        position: absolute;
        top: 0;
        left: -100%;
        width: 100%;
        height: 100%;
        background: linear-gradient(90deg, transparent 0%, rgba(98, 106, 239, 0.3) 50%, transparent 100%);
        transition: left 0.5s ease;
      }
      
      &:hover {
        background: rgba(98, 106, 239, 0.2);
        border-color: var(--primary-color);
        color: var(--primary-color);
        transform: scale(1.05);
        
        &::before {
          left: 100%;
        }
      }
      
      // 不同类型的颜色
      &:nth-child(1) { border-color: var(--info-color); }
      &:nth-child(2) { border-color: var(--success-color); }
      &:nth-child(3) { border-color: var(--warning-color); }
      &:nth-child(4) { border-color: var(--accent-purple); }
    }
  }
  
  :deep(.el-table) {
    border: none;
    font-size: 14px;
    background: transparent;
    
    th {
      background: var(--glass-bg);
      backdrop-filter: var(--glass-blur);
      font-weight: 600;
      color: var(--text-primary);
      border-bottom: 1px solid var(--border-light);
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }
    
    td {
      border-bottom: 1px solid var(--border-light);
      color: var(--text-primary);
    }
    
    .el-table__row {
      transition: all 0.3s ease;
      
      &:hover {
        background: rgba(98, 106, 239, 0.1);
        
        td {
          color: var(--text-primary);
        }
      }
    }
  }
  
  .total-points {
    font-weight: 700;
    font-size: 16px;
    color: var(--primary-color);
    transition: all 0.3s ease;
    
    &.high-count {
      color: var(--accent-cyan);
      text-shadow: 0 0 10px rgba(0, 212, 255, 0.5);
    }
  }
  
  .action-buttons {
    display: flex;
    gap: 8px;
    
    .action-btn {
      background: var(--glass-bg);
      backdrop-filter: var(--glass-blur);
      border: 1px solid var(--glass-border);
      color: var(--text-primary);
      transition: all 0.3s ease;
      
      &:hover:not(:disabled) {
        background: rgba(98, 106, 239, 0.2);
        border-color: var(--primary-color);
        color: var(--primary-color);
        transform: translateY(-2px);
        box-shadow: 0 4px 16px rgba(98, 106, 239, 0.3);
      }
      
      &.danger:hover:not(:disabled) {
        background: rgba(255, 56, 56, 0.2);
        border-color: var(--danger-color);
        color: var(--danger-color);
        box-shadow: 0 4px 16px rgba(255, 56, 56, 0.3);
      }
      
      &:disabled {
        opacity: 0.5;
        cursor: not-allowed;
      }
    }
  }
}
</style>