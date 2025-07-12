<script setup lang="ts">
import { ref, reactive, computed, watch, onMounted } from 'vue';
import { ElMessage } from 'element-plus';
import { invoke } from '@tauri-apps/api/core';
import type { Channel, TransportConfig, ProtocolConfig } from '@/types/channel';
import type { CsvType, ValidationResult } from '@/types/point-table';
import ProtocolMappingTable from './ProtocolMappingTable.vue';

const props = defineProps<{
  channelId?: number | null;
}>();

const emit = defineEmits<{
  saved: [];
  cancel: [];
}>();

// 通道表单数据
const channelForm = reactive<Channel>({
  id: 0,
  name: '',
  protocol: 'modbus',
  protocol_type: 'modbus_tcp',
  enabled: true,
  transport_config: {
    transport_type: 'tcp',
    tcp: {
      host: '127.0.0.1',
      port: 502,
      timeout: 5000,
      retry_count: 3,
    }
  },
  protocol_config: {
    modbus: {
      mode: 'tcp',
      timeout_ms: 1000,
      retry_count: 3,
    }
  },
  polling_config: {
    interval_ms: 1000,
    batch_size: 100,
    priority: 5,
  },
  point_table: {
    telemetry: [],
    signal: [],
    control: [],
    adjustment: [],
    telemetry_mapping: [],
    signal_mapping: [],
    control_mapping: [],
    adjustment_mapping: [],
  },
  logging: {
    level: 'info',
    file: '',
    max_size: '100MB',
    max_backups: 5,
  }
});

const loading = ref(false);
const activeTab = ref('basic');
const pointTableTab = ref('telemetry');

// 协议选项
const protocolOptions = [
  { label: 'Modbus TCP', value: 'modbus_tcp', protocol: 'modbus' },
  { label: 'Modbus RTU', value: 'modbus_rtu', protocol: 'modbus' },
  { label: 'IEC 60870-5-104', value: 'iec104', protocol: 'iec60870' },
  { label: 'IEC 60870-5-101', value: 'iec101', protocol: 'iec60870' },
  { label: 'CAN Bus', value: 'can', protocol: 'can' },
];

// 监听协议类型变化
watch(() => channelForm.protocol_type, (newType) => {
  const option = protocolOptions.find(p => p.value === newType);
  if (option) {
    channelForm.protocol = option.protocol;
    updateTransportConfig(newType);
    updateProtocolConfig(option.protocol);
  }
});

// 更新传输配置
function updateTransportConfig(protocolType: string) {
  switch (protocolType) {
    case 'modbus_tcp':
    case 'iec104':
      channelForm.transport_config.transport_type = 'tcp';
      break;
    case 'modbus_rtu':
    case 'iec101':
      channelForm.transport_config.transport_type = 'serial';
      if (!channelForm.transport_config.serial) {
        channelForm.transport_config.serial = {
          port_name: '/dev/ttyUSB0',
          baud_rate: 9600,
          data_bits: 8,
          stop_bits: 1,
          parity: 'none',
          flow_control: 'none',
        };
      }
      break;
    case 'can':
      channelForm.transport_config.transport_type = 'can';
      if (!channelForm.transport_config.can) {
        channelForm.transport_config.can = {
          interface: 'can0',
          bitrate: 500000,
          loopback: false,
          recv_own_msgs: false,
        };
      }
      break;
  }
}

// 更新协议配置
function updateProtocolConfig(protocol: string) {
  channelForm.protocol_config = {};
  switch (protocol) {
    case 'modbus':
      channelForm.protocol_config.modbus = {
        mode: channelForm.protocol_type === 'modbus_tcp' ? 'tcp' : 'rtu',
        timeout_ms: 1000,
        retry_count: 3,
      };
      break;
    case 'iec60870':
      channelForm.protocol_config.iec60870 = {
        mode: channelForm.protocol_type === 'iec104' ? '104' : '101',
        link_address: 1,
        cot_size: 2,
        ioa_size: 3,
        k: 12,
        w: 8,
        t1: 15,
        t2: 10,
        t3: 20,
      };
      break;
    case 'can':
      channelForm.protocol_config.can = {
        dbc_file: '',
        filters: [],
      };
      break;
  }
}

// 加载通道数据
async function loadChannel() {
  if (!props.channelId) return;
  
  loading.value = true;
  try {
    // TODO: 实际API调用
    // const channel = await invoke<Channel>('get_channel', { id: props.channelId });
    // Object.assign(channelForm, channel);
    ElMessage.info('加载通道配置...');
  } catch (error) {
    console.error('加载通道失败:', error);
    ElMessage.error('加载通道配置失败');
  } finally {
    loading.value = false;
  }
}

// 保存通道
async function saveChannel() {
  if (!channelForm.name) {
    ElMessage.warning('请输入通道名称');
    return;
  }
  
  loading.value = true;
  try {
    if (props.channelId) {
      // 更新通道
      // await invoke('update_channel', { id: props.channelId, channel: channelForm });
      ElMessage.success('更新通道成功');
    } else {
      // 创建通道
      // await invoke('create_channel', { channel: channelForm });
      ElMessage.success('创建通道成功');
    }
    emit('saved');
  } catch (error) {
    console.error('保存通道失败:', error);
    ElMessage.error('保存通道失败');
  } finally {
    loading.value = false;
  }
}

// CSV文件上传处理
async function handleCsvUpload(csvType: CsvType, file: File) {
  try {
    const content = await file.text();
    // TODO: 实际API调用
    // const result = await invoke<ValidationResult>('upload_channel_csv', {
    //   channelId: channelForm.id,
    //   csvType,
    //   content
    // });
    
    // 模拟成功
    ElMessage.success(`上传${csvType}文件成功`);
    
    // 更新本地数据
    if (csvType === 'telemetry') {
      // 解析CSV并更新点表...
    }
  } catch (error) {
    console.error('上传CSV失败:', error);
    ElMessage.error('上传CSV文件失败');
  }
}

// 导出CSV文件
async function exportCsv(csvType: CsvType) {
  try {
    // TODO: 实际API调用
    // const content = await invoke<string>('export_channel_csv', {
    //   channelId: channelForm.id,
    //   csvType
    // });
    
    const content = 'point_id,signal_name,chinese_name\n1001,test,测试';
    const blob = new Blob([content], { type: 'text/csv;charset=utf-8' });
    const url = window.URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${channelForm.name}_${csvType}.csv`;
    a.click();
    window.URL.revokeObjectURL(url);
    ElMessage.success('导出成功');
  } catch (error) {
    console.error('导出CSV失败:', error);
    ElMessage.error('导出CSV文件失败');
  }
}

// 下载CSV模板
async function downloadTemplate(csvType: CsvType) {
  try {
    // TODO: 实际API调用
    // const content = await invoke<string>('get_channel_protocol_template', {
    //   protocolType: channelForm.protocol_type,
    //   csvType
    // });
    
    const content = 'point_id,signal_name,chinese_name\n';
    const blob = new Blob([content], { type: 'text/csv;charset=utf-8' });
    const url = window.URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `template_${channelForm.protocol_type}_${csvType}.csv`;
    a.click();
    window.URL.revokeObjectURL(url);
    ElMessage.success('下载模板成功');
  } catch (error) {
    console.error('下载模板失败:', error);
    ElMessage.error('下载模板文件失败');
  }
}

// 计算点数统计
const pointCounts = computed(() => ({
  telemetry: channelForm.point_table.telemetry.length,
  signal: channelForm.point_table.signal.length,
  control: channelForm.point_table.control.length,
  adjustment: channelForm.point_table.adjustment.length,
  total: channelForm.point_table.telemetry.length +
         channelForm.point_table.signal.length +
         channelForm.point_table.control.length +
         channelForm.point_table.adjustment.length
}));

onMounted(() => {
  loadChannel();
});
</script>

<template>
  <div class="channel-editor" v-loading="loading">
    <el-tabs v-model="activeTab">
      <!-- 基本配置 -->
      <el-tab-pane label="基本配置" name="basic">
        <el-form :model="channelForm" label-width="120px">
          <el-form-item label="通道名称" required>
            <el-input v-model="channelForm.name" placeholder="请输入通道名称" />
          </el-form-item>
          
          <el-form-item label="协议类型" required>
            <el-select v-model="channelForm.protocol_type" placeholder="请选择协议">
              <el-option
                v-for="option in protocolOptions"
                :key="option.value"
                :label="option.label"
                :value="option.value"
              />
            </el-select>
          </el-form-item>
          
          <el-form-item label="启用状态">
            <el-switch v-model="channelForm.enabled" />
          </el-form-item>
          
          <!-- 传输配置 -->
          <el-divider>传输配置</el-divider>
          
          <!-- TCP配置 -->
          <template v-if="channelForm.transport_config.transport_type === 'tcp'">
            <el-form-item label="主机地址">
              <el-input v-model="channelForm.transport_config.tcp!.host" placeholder="127.0.0.1" />
            </el-form-item>
            <el-form-item label="端口">
              <el-input-number v-model="channelForm.transport_config.tcp!.port" :min="1" :max="65535" />
            </el-form-item>
            <el-form-item label="超时时间(ms)">
              <el-input-number v-model="channelForm.transport_config.tcp!.timeout" :min="100" :max="60000" />
            </el-form-item>
          </template>
          
          <!-- 串口配置 -->
          <template v-else-if="channelForm.transport_config.transport_type === 'serial'">
            <el-form-item label="串口">
              <el-input v-model="channelForm.transport_config.serial!.port_name" placeholder="/dev/ttyUSB0" />
            </el-form-item>
            <el-form-item label="波特率">
              <el-select v-model="channelForm.transport_config.serial!.baud_rate">
                <el-option :value="9600" label="9600" />
                <el-option :value="19200" label="19200" />
                <el-option :value="38400" label="38400" />
                <el-option :value="57600" label="57600" />
                <el-option :value="115200" label="115200" />
              </el-select>
            </el-form-item>
            <el-form-item label="数据位">
              <el-select v-model="channelForm.transport_config.serial!.data_bits">
                <el-option :value="7" label="7" />
                <el-option :value="8" label="8" />
              </el-select>
            </el-form-item>
            <el-form-item label="停止位">
              <el-select v-model="channelForm.transport_config.serial!.stop_bits">
                <el-option :value="1" label="1" />
                <el-option :value="2" label="2" />
              </el-select>
            </el-form-item>
            <el-form-item label="校验">
              <el-select v-model="channelForm.transport_config.serial!.parity">
                <el-option value="none" label="无" />
                <el-option value="even" label="偶校验" />
                <el-option value="odd" label="奇校验" />
              </el-select>
            </el-form-item>
          </template>
          
          <!-- CAN配置 -->
          <template v-else-if="channelForm.transport_config.transport_type === 'can'">
            <el-form-item label="接口名称">
              <el-input v-model="channelForm.transport_config.can!.interface" placeholder="can0" />
            </el-form-item>
            <el-form-item label="波特率">
              <el-select v-model="channelForm.transport_config.can!.bitrate">
                <el-option :value="125000" label="125 Kbps" />
                <el-option :value="250000" label="250 Kbps" />
                <el-option :value="500000" label="500 Kbps" />
                <el-option :value="1000000" label="1 Mbps" />
              </el-select>
            </el-form-item>
          </template>
          
          <!-- 轮询配置 -->
          <el-divider>轮询配置</el-divider>
          
          <el-form-item label="轮询间隔(ms)">
            <el-input-number v-model="channelForm.polling_config.interval_ms" :min="100" :max="60000" />
          </el-form-item>
          <el-form-item label="批量大小">
            <el-input-number v-model="channelForm.polling_config.batch_size" :min="1" :max="1000" />
          </el-form-item>
          <el-form-item label="优先级">
            <el-input-number v-model="channelForm.polling_config.priority" :min="1" :max="10" />
          </el-form-item>
        </el-form>
      </el-tab-pane>
      
      <!-- 点表配置 -->
      <el-tab-pane label="点表配置" name="point-table">
        <div class="point-table-stats">
          <el-statistic title="遥测点" :value="pointCounts.telemetry" />
          <el-statistic title="遥信点" :value="pointCounts.signal" />
          <el-statistic title="遥控点" :value="pointCounts.control" />
          <el-statistic title="遥调点" :value="pointCounts.adjustment" />
          <el-statistic title="总点数" :value="pointCounts.total" />
        </div>
        
        <el-tabs v-model="pointTableTab">
          <!-- 遥测 -->
          <el-tab-pane label="遥测 (YC)" name="telemetry">
            <div class="table-toolbar">
              <el-upload
                :show-file-list="false"
                accept=".csv"
                :before-upload="(file) => { handleCsvUpload('telemetry', file); return false; }"
              >
                <el-button>上传CSV</el-button>
              </el-upload>
              <el-button @click="exportCsv('telemetry')">导出CSV</el-button>
              <el-button @click="downloadTemplate('telemetry')">下载模板</el-button>
            </div>
            
            <el-table :data="channelForm.point_table.telemetry" style="width: 100%">
              <el-table-column prop="point_id" label="点号" width="80" />
              <el-table-column prop="signal_name" label="信号名称" />
              <el-table-column prop="chinese_name" label="中文名称" />
              <el-table-column prop="data_type" label="数据类型" width="100" />
              <el-table-column prop="unit" label="单位" width="80" />
              <el-table-column prop="scale" label="系数" width="80" />
              <el-table-column prop="offset" label="偏移" width="80" />
            </el-table>
            
            <!-- 映射配置 -->
            <el-divider>映射配置</el-divider>
            <div class="table-toolbar">
              <el-upload
                :show-file-list="false"
                accept=".csv"
                :before-upload="(file) => { handleCsvUpload('telemetry_mapping', file); return false; }"
              >
                <el-button>上传映射CSV</el-button>
              </el-upload>
              <el-button @click="exportCsv('telemetry_mapping')">导出映射</el-button>
              <el-button @click="downloadTemplate('telemetry_mapping')">下载映射模板</el-button>
            </div>
            
            <ProtocolMappingTable
              :data="channelForm.point_table.telemetry_mapping"
              :protocol-type="channelForm.protocol_type"
            />
          </el-tab-pane>
          
          <!-- 遥信 -->
          <el-tab-pane label="遥信 (YX)" name="signal">
            <div class="table-toolbar">
              <el-upload
                :show-file-list="false"
                accept=".csv"
                :before-upload="(file) => { handleCsvUpload('signal', file); return false; }"
              >
                <el-button>上传CSV</el-button>
              </el-upload>
              <el-button @click="exportCsv('signal')">导出CSV</el-button>
              <el-button @click="downloadTemplate('signal')">下载模板</el-button>
            </div>
            
            <el-table :data="channelForm.point_table.signal" style="width: 100%">
              <el-table-column prop="point_id" label="点号" width="80" />
              <el-table-column prop="signal_name" label="信号名称" />
              <el-table-column prop="chinese_name" label="中文名称" />
              <el-table-column prop="reverse" label="取反" width="80">
                <template #default="{ row }">
                  <el-tag :type="row.reverse ? 'warning' : 'info'" size="small">
                    {{ row.reverse ? '是' : '否' }}
                  </el-tag>
                </template>
              </el-table-column>
            </el-table>
            
            <!-- 映射配置 -->
            <el-divider>映射配置</el-divider>
            <div class="table-toolbar">
              <el-upload
                :show-file-list="false"
                accept=".csv"
                :before-upload="(file) => { handleCsvUpload('signal_mapping', file); return false; }"
              >
                <el-button>上传映射CSV</el-button>
              </el-upload>
              <el-button @click="exportCsv('signal_mapping')">导出映射</el-button>
              <el-button @click="downloadTemplate('signal_mapping')">下载映射模板</el-button>
            </div>
            
            <ProtocolMappingTable
              :data="channelForm.point_table.signal_mapping"
              :protocol-type="channelForm.protocol_type"
            />
          </el-tab-pane>
          
          <!-- 遥控 -->
          <el-tab-pane label="遥控 (YK)" name="control">
            <div class="table-toolbar">
              <el-upload
                :show-file-list="false"
                accept=".csv"
                :before-upload="(file) => { handleCsvUpload('control', file); return false; }"
              >
                <el-button>上传CSV</el-button>
              </el-upload>
              <el-button @click="exportCsv('control')">导出CSV</el-button>
              <el-button @click="downloadTemplate('control')">下载模板</el-button>
            </div>
            
            <el-table :data="channelForm.point_table.control" style="width: 100%">
              <el-table-column prop="point_id" label="点号" width="80" />
              <el-table-column prop="signal_name" label="信号名称" />
              <el-table-column prop="chinese_name" label="中文名称" />
            </el-table>
          </el-tab-pane>
          
          <!-- 遥调 -->
          <el-tab-pane label="遥调 (YT)" name="adjustment">
            <div class="table-toolbar">
              <el-upload
                :show-file-list="false"
                accept=".csv"
                :before-upload="(file) => { handleCsvUpload('adjustment', file); return false; }"
              >
                <el-button>上传CSV</el-button>
              </el-upload>
              <el-button @click="exportCsv('adjustment')">导出CSV</el-button>
              <el-button @click="downloadTemplate('adjustment')">下载模板</el-button>
            </div>
            
            <el-table :data="channelForm.point_table.adjustment" style="width: 100%">
              <el-table-column prop="point_id" label="点号" width="80" />
              <el-table-column prop="signal_name" label="信号名称" />
              <el-table-column prop="chinese_name" label="中文名称" />
              <el-table-column prop="data_type" label="数据类型" width="100" />
              <el-table-column prop="unit" label="单位" width="80" />
              <el-table-column prop="scale" label="系数" width="80" />
              <el-table-column prop="offset" label="偏移" width="80" />
            </el-table>
          </el-tab-pane>
        </el-tabs>
      </el-tab-pane>
      
      <!-- 日志配置 -->
      <el-tab-pane label="日志配置" name="logging">
        <el-form :model="channelForm.logging" label-width="120px">
          <el-form-item label="日志级别">
            <el-select v-model="channelForm.logging.level">
              <el-option value="error" label="Error" />
              <el-option value="warn" label="Warn" />
              <el-option value="info" label="Info" />
              <el-option value="debug" label="Debug" />
              <el-option value="trace" label="Trace" />
            </el-select>
          </el-form-item>
          
          <el-form-item label="日志文件">
            <el-input 
              v-model="channelForm.logging.file" 
              placeholder="留空使用默认路径"
            />
          </el-form-item>
          
          <el-form-item label="最大文件大小">
            <el-input v-model="channelForm.logging.max_size" placeholder="100MB" />
          </el-form-item>
          
          <el-form-item label="最大备份数">
            <el-input-number v-model="channelForm.logging.max_backups" :min="1" :max="100" />
          </el-form-item>
        </el-form>
      </el-tab-pane>
    </el-tabs>
    
    <!-- 底部操作按钮 -->
    <div class="form-actions">
      <el-button @click="emit('cancel')">取消</el-button>
      <el-button type="primary" @click="saveChannel" :loading="loading">
        {{ channelId ? '更新' : '创建' }}
      </el-button>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.channel-editor {
  .point-table-stats {
    display: flex;
    gap: 40px;
    margin-bottom: 20px;
    padding: 20px;
    background: #f5f7fa;
    border-radius: 8px;
    
    :deep(.el-statistic) {
      text-align: center;
    }
  }
  
  .table-toolbar {
    display: flex;
    gap: 12px;
    margin-bottom: 16px;
  }
  
  .form-actions {
    display: flex;
    justify-content: flex-end;
    gap: 12px;
    margin-top: 24px;
    padding-top: 24px;
    border-top: 1px solid var(--glass-border);
  }
  
  .el-divider {
    margin: 24px 0 16px;
    border-top-color: var(--glass-border);
  }
  
  // 深色主题覆盖
  :deep(.el-form) {
    .el-form-item__label {
      color: var(--text-primary);
    }
    
    .el-input__wrapper {
      background: var(--glass-bg);
      border-color: var(--glass-border);
    }
    
    .el-select__wrapper {
      background: var(--glass-bg);
      border-color: var(--glass-border);
    }
    
    .el-input-number__decrease,
    .el-input-number__increase {
      background: var(--glass-bg);
      border-color: var(--glass-border);
      color: var(--text-primary);
      
      &:hover {
        background: rgba(98, 106, 239, 0.2);
        color: var(--primary-color);
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
      
      &.is-active {
        color: var(--primary-color);
        background: rgba(98, 106, 239, 0.2);
      }
    }
  }
}
</style>