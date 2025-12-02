<template>
  <FormDialog ref="formDialogRef" :title="dialogTitle" width="9.44rem" @close="handleClose">
    <template #dialog-body>
      <div class="voltage-class channel-detail-dialog">
        <el-form
          :model="form"
          :inline="true"
          label-width="1.5rem"
          :disabled="!isEditing"
          :rules="formRules"
          :validate-on-rule-change="false"
          ref="formRef"
        >
          <!-- 基础信息 -->
          <div class="channel-detail__section">
            <h4 class="channel-detail__section-title">Basic Information</h4>
            <el-form-item label="ID:" v-if="!isAdd">
              <span v-if="!isEditing" class="channel-detail__text">{{ form.id }}</span>
              <el-input v-else v-model="form.id" disabled />
            </el-form-item>
            <el-form-item label="Name:" :style="!isAdd ? 'margin-right: 0' : ''" prop="name">
              <span v-if="!isEditing" class="channel-detail__text">{{ form.name }}</span>
              <el-input v-else v-model="form.name" placeholder="Please enter channel name" />
            </el-form-item>
            <el-form-item label="Protocol:" :style="isAdd ? 'margin-right: 0' : ''" prop="protocol">
              <span v-if="!isEditing" class="channel-detail__text">{{
                getProtocolLabel(form.protocol)
              }}</span>
              <div class="channel-detail__protocol-select" v-else>
                <el-select
                  v-model="form.protocol"
                  placeholder="Please select protocol"
                  :teleported="false"
                  append-to=".channel-detail__protocol-select"
                >
                  <el-option
                    v-for="option in PROTOCOL_OPTIONS"
                    :key="option.value"
                    :label="option.label"
                    :value="option.value"
                  />
                </el-select>
              </div>
            </el-form-item>
            <el-form-item v-if="!isEditing || isAdd" label="Enabled:" style="margin-right: 0">
              <template v-if="!isEditing">
                <span
                  class="channel-detail__text"
                  :style="{ color: form.enabled ? '#67C23A' : '#F56C6C', fontWeight: 600 }"
                >
                  {{ form.enabled ? 'Enabled' : 'Disabled' }}
                </span>
              </template>
              <el-switch v-else v-model="form.enabled" />
            </el-form-item>
            <el-form-item label="Description:" style="width: calc(100% - 0.1rem); margin-right: 0">
              <span v-if="!isEditing" class="channel-detail__text">{{
                form.description || '-'
              }}</span>
              <el-input
                v-else
                v-model="form.description"
                type="textarea"
                :rows="2"
                placeholder="Please enter description"
              />
            </el-form-item>
          </div>
          <!-- 参数配置 -->
          <div class="channel-detail__section">
            <h4 class="channel-detail__section-title">Parameters</h4>
            <div class="channel-detail__parameters">
              <!-- Modbus TCP -->
              <template v-if="form.protocol === 'modbus_tcp'">
                <el-form-item
                  label="Host:"
                  class="channel-detail__parameter-item"
                  prop="parameters.host"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).host || '-'
                  }}</span>
                  <el-input
                    v-else
                    v-model="(form.parameters as any).host"
                    placeholder="please enter host"
                  />
                </el-form-item>
                <el-form-item
                  label="Port:"
                  class="channel-detail__parameter-item"
                  style="margin-right: 0"
                  prop="parameters.port"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).port ?? '-'
                  }}</span>
                  <el-input-number
                    v-else
                    v-model="(form.parameters as any).port"
                    :controls="false"
                    :min="0"
                    :precision="0"
                    align="left"
                    placeholder="please enter port"
                  />
                </el-form-item>
                <el-form-item
                  label="Connect Timeout (ms):"
                  class="channel-detail__parameter-item"
                  prop="parameters.connect_timeout_ms"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).connect_timeout_ms ?? '-'
                  }}</span>
                  <el-input-number
                    v-else
                    v-model="(form.parameters as any).connect_timeout_ms"
                    :controls="false"
                    :min="0"
                    :precision="0"
                    align="left"
                    placeholder="please enter connect timeout (ms)"
                  />
                </el-form-item>
                <el-form-item
                  label="Read Timeout (ms):"
                  class="channel-detail__parameter-item"
                  style="margin-right: 0"
                  prop="parameters.read_timeout_ms"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).read_timeout_ms ?? '-'
                  }}</span>
                  <el-input-number
                    v-else
                    v-model="(form.parameters as any).read_timeout_ms"
                    :controls="false"
                    :min="0"
                    :precision="0"
                    align="left"
                    placeholder="please enter read timeout (ms)"
                  />
                </el-form-item>
              </template>

              <template v-else-if="form.protocol === 'can'">
                <el-form-item label="Bitrate:" class="channel-detail__parameter-item">
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).bitrate ?? '-'
                  }}</span>
                  <el-input-number
                    v-else
                    v-model="(form.parameters as any).bitrate"
                    :controls="false"
                    align="left"
                    placeholder="please enter bitrate"
                  />
                </el-form-item>
                <el-form-item
                  label="Data Bitrate:"
                  class="channel-detail__parameter-item"
                  style="margin-right: 0"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).data_bitrate ?? '-'
                  }}</span>
                  <el-input-number
                    v-else
                    v-model="(form.parameters as any).data_bitrate"
                    :controls="false"
                    align="left"
                    placeholder="please enter data bitrate"
                  />
                </el-form-item>
                <el-form-item label="FD Mode:" class="channel-detail__parameter-item">
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).fd_mode ? 'Yes' : 'No'
                  }}</span>
                  <el-switch v-else v-model="(form.parameters as any).fd_mode" />
                </el-form-item>
                <el-form-item
                  label="Interface:"
                  class="channel-detail__parameter-item"
                  style="margin-right: 0"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).interface || '-'
                  }}</span>
                  <el-input
                    v-else
                    v-model="(form.parameters as any).interface"
                    placeholder="please enter interface"
                  />
                </el-form-item>
                <el-form-item label="Listen Only:" class="channel-detail__parameter-item">
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).listen_only ? 'Yes' : 'No'
                  }}</span>
                  <el-switch v-else v-model="(form.parameters as any).listen_only" />
                </el-form-item>
                <el-form-item
                  label="Loopback:"
                  class="channel-detail__parameter-item"
                  style="margin-right: 0"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).loopback ? 'Yes' : 'No'
                  }}</span>
                  <el-switch v-else v-model="(form.parameters as any).loopback" />
                </el-form-item>
                <el-form-item
                  label="Timeout (ms):"
                  class="channel-detail__parameter-item"
                  style="margin-right: 0"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).timeout_ms ?? '-'
                  }}</span>
                  <el-input-number
                    v-else
                    v-model="(form.parameters as any).timeout_ms"
                    :controls="false"
                    align="left"
                    placeholder="please enter timeout (ms)"
                  />
                </el-form-item>
              </template>

              <template v-else-if="form.protocol === 'virt'">
                <el-form-item label="Update Interval (ms):" class="channel-detail__parameter-item">
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).update_interval_ms ?? '-'
                  }}</span>
                  <el-input-number
                    v-else
                    v-model="(form.parameters as any).update_interval_ms"
                    :controls="false"
                    align="left"
                    placeholder="please enter update interval (ms)"
                  />
                </el-form-item>
              </template>

              <template v-else-if="form.protocol === 'modbus_rtu'">
                <el-form-item
                  label="Device:"
                  class="channel-detail__parameter-item"
                  prop="parameters.device"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).device || '-'
                  }}</span>
                  <el-input
                    v-else
                    v-model="(form.parameters as any).device"
                    placeholder="please enter device"
                  />
                </el-form-item>
                <el-form-item
                  label="Baud Rate:"
                  class="channel-detail__parameter-item"
                  style="margin-right: 0"
                  prop="parameters.baud_rate"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).baud_rate ?? '-'
                  }}</span>
                  <el-input-number
                    v-else
                    v-model="(form.parameters as any).baud_rate"
                    :controls="false"
                    :min="0"
                    :precision="0"
                    align="left"
                    placeholder="please enter baud rate"
                  />
                </el-form-item>
                <el-form-item
                  label="Data Bits:"
                  class="channel-detail__parameter-item"
                  prop="parameters.data_bits"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).data_bits ?? '-'
                  }}</span>
                  <el-input-number
                    v-else
                    v-model="(form.parameters as any).data_bits"
                    :controls="false"
                    :min="0"
                    :precision="0"
                    align="left"
                    placeholder="please enter data bits"
                  />
                </el-form-item>
                <el-form-item
                  label="Stop Bits:"
                  class="channel-detail__parameter-item"
                  style="margin-right: 0"
                  prop="parameters.stop_bits"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).stop_bits ?? '-'
                  }}</span>
                  <el-input-number
                    v-else
                    v-model="(form.parameters as any).stop_bits"
                    :controls="false"
                    :min="0"
                    :precision="0"
                    align="left"
                    placeholder="please enter stop bits"
                  />
                </el-form-item>
                <el-form-item
                  label="Parity:"
                  class="channel-detail__parameter-item"
                  prop="parameters.parity"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).parity || '-'
                  }}</span>
                  <el-select
                    v-else
                    v-model="(form.parameters as any).parity"
                    placeholder="please enter parity"
                    :teleported="false"
                    append-to=".channel-detail__parameter-item"
                  >
                    <el-option label="N" value="N" />
                    <el-option label="E" value="E" />
                    <el-option label="O" value="O" />
                  </el-select>
                </el-form-item>
                <el-form-item
                  label="Connect Timeout (ms):"
                  class="channel-detail__parameter-item"
                  style="margin-right: 0"
                  prop="parameters.connect_timeout_ms"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).connect_timeout_ms ?? '-'
                  }}</span>
                  <el-input-number
                    v-else
                    v-model="(form.parameters as any).connect_timeout_ms"
                    :controls="false"
                    :min="0"
                    :precision="0"
                    align="left"
                    placeholder="please enter connect timeout (ms)"
                  />
                </el-form-item>
                <el-form-item
                  label="Read Timeout (ms):"
                  class="channel-detail__parameter-item"
                  prop="parameters.read_timeout_ms"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).read_timeout_ms ?? '-'
                  }}</span>
                  <el-input-number
                    v-else
                    v-model="(form.parameters as any).read_timeout_ms"
                    :controls="false"
                    :min="0"
                    :precision="0"
                    align="left"
                    placeholder="please enter read timeout (ms)"
                  />
                </el-form-item>
                <el-form-item
                  label="Retry Interval (ms):"
                  class="channel-detail__parameter-item"
                  style="margin-right: 0"
                  prop="parameters.retry_interval_ms"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).retry_interval_ms ?? '-'
                  }}</span>
                  <el-input-number
                    v-else
                    v-model="(form.parameters as any).retry_interval_ms"
                    :controls="false"
                    :min="0"
                    :precision="0"
                    align="left"
                    placeholder="please enter retry interval (ms)"
                  />
                </el-form-item>
              </template>

              <template v-else-if="form.protocol === 'di_do'">
                <el-form-item label="Driver:" class="channel-detail__parameter-item">
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).driver || '-'
                  }}</span>
                  <el-input
                    v-else
                    v-model="(form.parameters as any).driver"
                    placeholder="please enter driver"
                  />
                </el-form-item>
                <el-form-item
                  label="GPIO Base Path:"
                  class="channel-detail__parameter-item"
                  style="margin-right: 0"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).gpio_base_path || '-'
                  }}</span>
                  <el-input
                    v-else
                    v-model="(form.parameters as any).gpio_base_path"
                    placeholder="please enter gpio base path"
                  />
                </el-form-item>
                <el-form-item
                  label="DI Poll Interval (ms):"
                  class="channel-detail__parameter-item"
                  style="margin-right: 0"
                >
                  <span v-if="!isEditing" class="channel-detail__text">{{
                    (form.parameters as any).di_poll_interval_ms ?? '-'
                  }}</span>
                  <el-input-number
                    v-else
                    v-model="(form.parameters as any).di_poll_interval_ms"
                    :controls="false"
                    :min="0"
                    :precision="0"
                    align="left"
                    placeholder="please enter di poll interval (ms)"
                  />
                </el-form-item>
              </template>
            </div>
          </div>
          <!-- 运行时状态 -->
          <div class="channel-detail__section" v-if="!isEditing && form.runtime_status">
            <h4 class="channel-detail__section-title">Runtime Status</h4>
            <el-form-item label="Connected:">
              <span
                class="channel-detail__text"
                :style="{
                  color: form.runtime_status.connected ? '#67C23A' : '#F56C6C',
                  fontWeight: 600,
                }"
              >
                {{ form.runtime_status.connected ? 'Connected' : 'Disconnected' }}
              </span>
            </el-form-item>
            <el-form-item label="Running:" v-if="form.runtime_status" style="margin-right: 0">
              <span
                class="channel-detail__text"
                :style="{
                  color: form.runtime_status.running ? '#67C23A' : '#F56C6C',
                  fontWeight: 600,
                }"
              >
                {{ form.runtime_status.running ? 'Running' : 'Stopped' }}
              </span>
            </el-form-item>
            <el-form-item label="Last Update:">
              <span class="channel-detail__text">{{
                formatIsoToDateTime(form.runtime_status?.last_update || '-')
              }}</span>
            </el-form-item>
            <el-form-item label="Error Count:" style="margin-right: 0">
              <span class="channel-detail__text">{{ form.runtime_status?.error_count }}</span>
            </el-form-item>
            <el-form-item label="Last Error:">
              <span class="channel-detail__text">{{
                form.runtime_status?.last_error || 'No errors'
              }}</span>
            </el-form-item>
          </div>

          <!-- 点位统计 -->
          <div v-if="!isEditing" class="channel-detail__section">
            <h4 class="channel-detail__section-title">Point Counts</h4>
            <el-form-item label="Telemetry:">
              <span class="channel-detail__text">{{ form.point_counts?.telemetry }}</span>
            </el-form-item>
            <el-form-item label="Signal:" style="margin-right: 0">
              <span class="channel-detail__text">{{ form.point_counts?.signal }}</span>
            </el-form-item>
            <el-form-item label="Control:">
              <span class="channel-detail__text">{{ form.point_counts?.control }}</span>
            </el-form-item>
            <el-form-item label="Adjustment:" style="margin-right: 0">
              <span class="channel-detail__text">{{ form.point_counts?.adjustment }}</span>
            </el-form-item>
          </div>
        </el-form>
      </div>
    </template>
    <template #dialog-footer>
      <div class="dialog-footer">
        <el-button @click="handleCancel">{{
          isAdd ? 'Cancel Add' : isEditing ? 'Cancel Edit' : 'Cancel'
        }}</el-button>
        <el-button v-if="!isEditing" type="primary" @click="handleEdit">Edit</el-button>
        <el-button v-else type="primary" @click="handleSubmit">Submit</el-button>
      </div>
    </template>
  </FormDialog>
</template>

<script setup lang="ts">
import { ref, watch, computed, nextTick } from 'vue'
import { ElMessage, type FormInstance } from 'element-plus'
import { Plus } from '@element-plus/icons-vue'
import FormDialog from '@/components/dialog/FormDialog.vue'
import type {
  ChannelDetail,
  modbusTcpParams,
  canParams,
  virtualParams,
  modbusRtuParams,
} from '@/types/channelConfiguration'
import { PROTOCOL_OPTIONS } from '@/types/channelConfiguration'
import dayjs from 'dayjs'
import { getChannelDetail, createChannel, updateChannel } from '@/api/channelsManagement'

type diDoParams = {
  di_poll_interval_ms: number
  driver: string
  gpio_base_path: string
}

// Props
interface Props {
  channelData?: ChannelDetail
}

const props = withDefaults(defineProps<Props>(), {
  channelData: () => ({
    id: 0,
    name: '',
    description: '',
    protocol: 'modbus_tcp',
    enabled: false,
    parameters: {
      parameters: {
        host: '',
        port: 502,
        connect_timeout_ms: 3000,
        read_timeout_ms: 4000,
      },
    },
    runtime_status: {
      connected: false,
      running: false,
      last_update: '',
      error_count: 0,
      last_error: null,
      statistics: {},
    },
    point_counts: {
      telemetry: 0,
      signal: 0,
      control: 0,
      adjustment: 0,
    },
  }),
})

// Emits
const emit = defineEmits<{
  submit: []
  cancel: []
}>()
const copyForm = ref<{
  description: string
  protocol: 'modbus_tcp' | 'can' | 'virt' | 'modbus_rtu' | 'di_do'
  name: string
  parameters: modbusTcpParams | canParams | virtualParams | modbusRtuParams | diDoParams
}>()
// 响应式数据
const formDialogRef = ref()
const isEditing = ref(false)
const isAdd = ref(false)
const formRef = ref<FormInstance>()
// 表单数据
const form = ref<ChannelDetail>({
  id: 0,
  name: '',
  description: '',
  protocol: 'modbus_tcp',
  enabled: false,
  parameters: { parameters: { host: '', port: 502, retry_count: 0, timeout_ms: 2000 } as any },
  runtime_status: {
    connected: false,
    running: false,
    last_update: '',
    error_count: 0,
    last_error: null,
    statistics: {},
  },
  point_counts: {
    telemetry: 0,
    signal: 0,
    control: 0,
    adjustment: 0,
  },
})

// 格式化时间
const formatIsoToDateTime = (isoString: string) => {
  return dayjs(isoString).format('YYYY-MM-DD HH:mm:ss')
}
// 获取协议标签
const getProtocolLabel = (protocol: string) => {
  const option = PROTOCOL_OPTIONS.find((opt) => opt.value === protocol)
  return option ? option.label : protocol
}

// 动态标题：新增/编辑/查看
const dialogTitle = computed(() =>
  isAdd.value ? 'Add Channel' : isEditing.value ? 'Edit Channel' : 'Channel Details',
)

// 校验规则：拆分为 TCP 与 RTU，查看模式传空对象；数字限制改由 input-number 控制
const requiredMsg = (name: string) => `${name} is required`
const tcpRules: Record<string, any[]> = {
  name: [{ required: true, message: requiredMsg('Name'), trigger: 'blur' }],
  protocol: [{ required: true, message: requiredMsg('Protocol'), trigger: 'change' }],
  'parameters.host': [{ required: true, message: requiredMsg('Host'), trigger: 'blur' }],
  'parameters.port': [{ required: true, message: requiredMsg('Port'), trigger: 'blur' }],
  'parameters.connect_timeout_ms': [
    { required: true, message: requiredMsg('Connect Timeout (ms)'), trigger: 'blur' },
  ],
  'parameters.read_timeout_ms': [
    { required: true, message: requiredMsg('Read Timeout (ms)'), trigger: 'blur' },
  ],
}
const rtuRules: Record<string, any[]> = {
  name: [{ required: true, message: requiredMsg('Name'), trigger: 'blur' }],
  protocol: [{ required: true, message: requiredMsg('Protocol'), trigger: 'change' }],
  'parameters.device': [{ required: true, message: requiredMsg('Device'), trigger: 'blur' }],
  'parameters.parity': [{ required: true, message: requiredMsg('Parity'), trigger: 'change' }],
  'parameters.baud_rate': [{ required: true, message: requiredMsg('Baud Rate'), trigger: 'blur' }],
  'parameters.data_bits': [{ required: true, message: requiredMsg('Data Bits'), trigger: 'blur' }],
  'parameters.stop_bits': [{ required: true, message: requiredMsg('Stop Bits'), trigger: 'blur' }],
  'parameters.connect_timeout_ms': [
    { required: true, message: requiredMsg('Connect Timeout (ms)'), trigger: 'blur' },
  ],
  'parameters.read_timeout_ms': [
    { required: true, message: requiredMsg('Read Timeout (ms)'), trigger: 'blur' },
  ],
  'parameters.retry_interval_ms': [
    { required: true, message: requiredMsg('Retry Interval (ms)'), trigger: 'blur' },
  ],
}
const diDoRules: Record<string, any[]> = {
  name: [{ required: true, message: requiredMsg('Name'), trigger: 'blur' }],
  protocol: [{ required: true, message: requiredMsg('Protocol'), trigger: 'change' }],
  'parameters.driver': [{ required: true, message: requiredMsg('Driver'), trigger: 'blur' }],
  'parameters.gpio_base_path': [
    { required: true, message: requiredMsg('GPIO Base Path'), trigger: 'blur' },
  ],
  'parameters.di_poll_interval_ms': [
    { required: true, message: requiredMsg('DI Poll Interval (ms)'), trigger: 'blur' },
    {
      validator: (_: any, value: any, callback: any) => {
        const num = Number(value)
        if (!Number.isInteger(num) || num < 0) callback(new Error('Must be an integer >= 0'))
        else callback()
      },
      trigger: 'change',
    },
  ],
}
const formRules = computed<Record<string, any[]>>(() => {
  if (!isEditing.value) return {}
  if (form.value.protocol === 'modbus_tcp') return tcpRules
  if (form.value.protocol === 'modbus_rtu') return rtuRules
  if (form.value.protocol === 'di_do') return diDoRules
  return {}
})

// 当协议变化时，初始化对应的参数模板
watch(
  () => form.value.protocol,
  (protocol, prev) => {
    if (!isEditing.value && !isAdd.value) return
    const protocolChanged = prev !== protocol
    const hasParams = form.value.parameters && Object.keys(form.value.parameters || {}).length > 0
    if (protocolChanged || hasParams) {
      form.value.parameters = {} as any
    }
    nextTick(() => {
      formRef.value?.clearValidate()
    })
  },
  { immediate: false },
)

// 添加参数
const addParameter = () => {
  const key = prompt('Please enter parameter key:')
  if (key && !form.value.parameters[key]) {
    form.value.parameters[key] = ''
  }
}

// 编辑
const handleEdit = () => {
  isEditing.value = true
  nextTick(() => {
    formRef.value?.clearValidate()
  })
}

// 取消
const handleCancel = () => {
  if (isEditing.value && !isAdd.value) {
    // 如果正在编辑，返回到查看状态
    isEditing.value = false
    // 恢复原始数据
    if (copyForm.value) {
      form.value.name = copyForm.value.name
      form.value.description = copyForm.value.description
      form.value.protocol = copyForm.value.protocol
      nextTick(() => {
        form.value.parameters = JSON.parse(JSON.stringify(copyForm.value!.parameters)) as any
        formRef.value?.clearValidate()
      })
    }
  } else {
    // 如果在查看状态，关闭对话框
    if (formDialogRef.value) {
      formDialogRef.value.dialogVisible = false
    }
    emit('cancel')
  }
}

// 提交
const handleSubmit = () => {
  formRef.value?.validate(async (valid) => {
    if (valid) {
      if (isAdd.value) {
        const res = await createChannel(form.value)
        if (res.success) {
          ElMessage.success('Channel created successfully')
          formDialogRef.value.dialogVisible = false
          isEditing.value = false
          isAdd.value = false
          emit('submit')
        }
      } else {
        if (!form.value.id) return
        // 更新时不携带 enabled 字段
        const payload: any = JSON.parse(JSON.stringify(form.value))
        delete payload.enabled
        const res = await updateChannel(form.value.id, payload)
        if (res.success) {
          ElMessage.success('Channel updated successfully')
          formDialogRef.value.dialogVisible = false
          isEditing.value = false
          emit('submit')
        }
      }
    }
  })
}
const open = async (id: number | undefined) => {
  form.value.id = id
  if (id) {
    // 打开详情：查看模式，明确重置新增标记
    isAdd.value = false
    isEditing.value = false
    const res = await getChannelDetail(id)
    if (res.success) {
      form.value = res.data
      formDialogRef.value.dialogVisible = true
      isEditing.value = false
      copyForm.value = {
        description: res.data.description,
        protocol: res.data.protocol,
        name: res.data.name,
        parameters: res.data.parameters,
      }
    }
  } else {
    form.value = {
      name: '',
      description: '',
      protocol: 'modbus_tcp',
      enabled: false,
      parameters: {},
    } as any
    isAdd.value = true
    isEditing.value = true
    formDialogRef.value.dialogVisible = true
  }

  setTimeout(() => {
    if (formDialogRef.value && formRef.value) {
      formDialogRef.value.dialogVisible = true
      formRef.value.clearValidate()
    }
  }, 100)
}
// 关闭
const handleClose = () => {
  isEditing.value = false
  emit('cancel')
}
defineExpose({
  open,
})
</script>

<style scoped lang="scss">
.voltage-class .channel-detail-dialog {
  max-height: 6rem;
  overflow-y: auto;
  .channel-detail__section {
    margin-bottom: 0.3rem;
    padding-bottom: 0.2rem;
    border-bottom: 0.01rem solid rgba(255, 255, 255, 0.1);

    &:last-child {
      border-bottom: none;
    }

    .channel-detail__section-title {
      font-size: 0.16rem;
      font-weight: 600;
      color: #fff;
      margin: 0 0 0.15rem 0;
    }

    .channel-detail__parameters {
      display: block;
    }
  }

  .el-form-item {
    margin-bottom: 0.2rem;
  }

  .channel-detail__text {
    color: #fff;
    font-size: 0.14rem;
    line-height: 1.5;
  }
}
</style>
