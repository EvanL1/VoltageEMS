<template>
  <div class="voltage-class data-update-setting">
    <ModuleCard title="Data Upload">
      <el-form
        ref="formRef"
        :model="formData"
        :rules="rules"
        class="data-update-form"
        label-width="1.35rem"
        label-position="right"
      >
        <!-- Enable 开关，仅展示，不改动原有逻辑 -->

        <!-- 仅展示 Host、Port 两项（保持原始校验/数据结构不变） -->
        <div class="config-collapse">
          <div class="collapse-content">
            <el-form-item label="Is Connected:">
              <div class="connection-status">
                <el-switch
                  v-model="connected"
                  @change="handleEnableChange"
                  :disabled="isConnecting"
                  :loading="isConnecting"
                />
                <!-- <el-icon v-if="isConnecting" class="loading-icon">
                  <Loading />
                </el-icon> -->
              </div>
            </el-form-item>
            <el-form-item label="Host:" prop="host">
              <el-input v-model="formData.host" placeholder="Enter host address" />
            </el-form-item>
            <el-form-item label="Port:" prop="port">
              <el-input-number
                v-model="formData.port"
                :min="1"
                :max="65535"
                :controls="false"
                align="left"
                placeholder="Enter port number"
                style="width: 100% !important"
              />
            </el-form-item>
          </div>
        </div>
      </el-form>
      <template #footer>
        <div class="card__content-footer">
          <!-- <IconButton
            type="primary"
            :icon="detailIcon"
            text="Detail"
            custom-class="card__content-footer-button"
            @click="openDetail"
          /> -->
          <el-button type="primary" @click="openDetail">Detail</el-button>
          <!-- <IconButton
            type="primary"
            :icon="submitIcon"
            text="Submit"
            custom-class="card__content-footer-button"
            @click="handleSubmit"
            :loading="submitLoading"
          /> -->
          <el-button type="primary" @click="handleSubmit" :loading="submitLoading"
            >Submit</el-button
          >
        </div>
      </template>
    </ModuleCard>
    <!-- 详情弹窗 -->
    <DataUploadDialog ref="detailDialogRef" @update="handleUpdate" />
  </div>
</template>

<script setup lang="ts">
import type { FormInstance, FormRules } from 'element-plus'
import submitIcon from '@/assets/icons/btn-submit.svg'
import DataUploadDialog from './components/DataUploadDialog.vue'
import { Loading } from '@element-plus/icons-vue'
import detailIcon from '@/assets/icons/button-detail.svg'

import {
  getMqttConfig,
  updateMqttConfig,
  disconnectMqtt,
  reconnectMqtt,
  getMqttStatus,
} from '@/api/System'
const formRef = ref<FormInstance>()

// 保留简化后的展示，不再使用折叠面板

export interface FormData {
  port: number
  host: string
  username: string
  password: string
  client_id: string

  ssl: {
    enabled: boolean
    ca_cert: {
      path: string
      full_path: string
    }
    client_cert: {
      path: string
      full_path: string
    }
    client_key: {
      path: string
      full_path: string
    }
  }
  reconnect: {
    enabled: boolean
    delay: number
    max_attempts: number
  }
  status: {
    will_message_enabled: boolean
    auto_online_message: boolean
  }
}
const connected = ref(false)
const isConnecting = ref(false)
const formData = ref<FormData>({
  port: 1,
  host: '127.0.0.1',
  username: '',
  password: '',
  client_id: '',
  ssl: {
    enabled: false,
    ca_cert: {
      path: '',
      full_path: '',
    },
    client_cert: {
      path: '',
      full_path: '',
    },
    client_key: {
      path: '',
      full_path: '',
    },
  },
  reconnect: {
    enabled: false,
    delay: 0,
    max_attempts: 0,
  },
  status: {
    will_message_enabled: true,
    auto_online_message: true,
  },
})
const submitLoading = ref(false)
// 表单验证规则
const rules = ref<FormRules<FormData>>({
  client_id: [
    { required: true, message: 'Please enter client ID', trigger: 'blur' },
    { min: 1, max: 50, message: 'Client ID length should be 1-50 characters', trigger: 'blur' },
  ],
  host: [
    { required: true, message: 'Please enter host address', trigger: 'blur' },
    // {
    //   pattern: /^((25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$|^[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/,
    //   message: 'Please enter a valid host address (IP or domain)',
    //   trigger: 'blur'
    // }
  ],
  port: [
    { required: true, message: 'Please enter port number', trigger: 'blur' },
    {
      type: 'number',
      min: 1,
      max: 65535,
      message: 'Port must be between 1 and 65535',
      trigger: 'blur',
    },
  ],
  'ssl.enabled': [
    { required: true, message: 'Please select SSL enable status', trigger: 'change' },
  ],
  'ssl.ca_cert.path': [
    {
      validator: (rule, value, callback) => {
        if (formData.value.ssl.enabled && (!value || value.trim() === '')) {
          callback(new Error('CA Certificate is required when SSL is enabled'))
        } else {
          callback()
        }
      },
      trigger: 'blur',
    },
  ],
  'ssl.client_cert.path': [
    {
      validator: (rule, value, callback) => {
        if (formData.value.ssl.enabled && (!value || value.trim() === '')) {
          callback(new Error('Client Certificate is required when SSL is enabled'))
        } else {
          callback()
        }
      },
      trigger: 'blur',
    },
  ],
  'ssl.client_key.path': [
    {
      validator: (rule, value, callback) => {
        if (formData.value.ssl.enabled && (!value || value.trim() === '')) {
          callback(new Error('Client Key is required when SSL is enabled'))
        } else {
          callback()
        }
      },
      trigger: 'blur',
    },
  ],
  'reconnect.enabled': [
    { required: true, message: 'Please select reconnect enable status', trigger: 'change' },
  ],
  'reconnect.delay': [
    {
      validator: (rule, value, callback) => {
        if (formData.value.reconnect.enabled && (!value || value < 1 || value > 360000)) {
          callback(
            new Error('Delay must be between 1 and 360000 seconds when reconnect is enabled'),
          )
        } else {
          callback()
        }
      },
      trigger: 'blur',
    },
  ],
  'reconnect.max_attempts': [
    {
      validator: (rule, value, callback) => {
        if (formData.value.reconnect.enabled && (!value || value < 1 || value > 10000)) {
          callback(new Error('Max attempts must be between 1 and 10000 when reconnect is enabled'))
        } else {
          callback()
        }
      },
      trigger: 'blur',
    },
  ],
})

// 表单提交方法
const handleSubmit = async () => {
  if (!formRef.value) return

  try {
    submitLoading.value = true
    await formRef.value.validate(async (valid: boolean) => {
      if (!valid) return
      const params = Object.assign({}, toRaw(formData.value), {
        ssl: {
          ca_cert: formData.value.ssl.ca_cert.path,
          client_cert: formData.value.ssl.client_cert.path,
          client_key: formData.value.ssl.client_key.path,
        },
      })
      const res = await updateMqttConfig({ mqtt_connection: { broker: params } })
      if (res.status == 'success') {
        ElMessage.success(res.message || 'Update success')
        checkConnected()
      } else {
        ElMessage.error(res.message || 'Update failed')
      }
    })
  } finally {
    submitLoading.value = false
  }
}
const getMqttConfigData = async () => {
  const response = await getMqttConfig()
  formData.value = response.data.broker
}
const getMqttStatusData = async () => {
  const response = await getMqttStatus()
  connected.value = response.connected
}
onMounted(() => {
  getMqttConfigData()
  getMqttStatusData()
})

const handleEnableChange = async (value: boolean) => {
  try {
    connected.value = !value
    isConnecting.value = true
    if (value) {
      const response = await reconnectMqtt()
      if (response.status == 'failed') {
        connected.value = false
        ElMessage.error(response.message)
      } else {
        connected.value = true
        ElMessage.success(response.message)
      }
    } else {
      const response = await disconnectMqtt()
      if (response.status == 'failed') {
        connected.value = true
        ElMessage.error(response.message)
      } else {
        connected.value = false
        ElMessage.success(response.message)
      }
    }
  } finally {
    isConnecting.value = false
  }
}

// 打开详情弹窗（仅UI交互，不改变原有业务逻辑）
const detailDialogRef = ref()
const openDetail = () => {
  detailDialogRef.value?.open(formData.value)
}
const checkConnected = async () => {
  try {
    if (connected.value) {
      isConnecting.value = true
      const response = await reconnectMqtt()
      if (response.status == 'failed') {
        connected.value = false
        ElMessage.error(response.message)
      } else {
        ElMessage.success(response.message)
      }
    }
  } finally {
    isConnecting.value = false
  }
}
const handleUpdate = async () => {
  getMqttConfigData()
  checkConnected()
}
</script>

<style scoped lang="scss">
.data-update-setting {
  width: 100%;
  height: 100%;

  .data-update-form {
    width: 100%;
    height: 100%;
    // margin: 0.2rem 0;
    overflow-y: auto;

    // 折叠面板样式
    .config-collapse {
      border: none;
      background: transparent;

      & > div {
        margin-bottom: 0.2rem;

        &:last-child {
          margin-bottom: 0;
        }
      }

      // :deep(.el-collapse-item) {
      //   margin-bottom: 0.2rem;
      //   border: 1px solid rgba(255, 255, 255, 0.1);
      //   border-radius: 0.08rem;
      //   overflow: hidden;
      //   background: rgba(44, 66, 106, 0.1);

      //   .el-collapse-item__header {
      //     background: linear-gradient(90deg, rgba(44, 66, 106, 0.8) 0%, rgba(44, 66, 106, 0.4) 100%);
      //     color: #ffffff;
      //     font-size: 0.16rem;
      //     font-weight: 600;
      //     padding: 0.12rem 0.2rem;
      //     border-bottom: 1px solid rgba(255, 255, 255, 0.1);
      //     height: auto;
      //     line-height: 1.5;

      //     .el-collapse-item__arrow {
      //       color: #ffffff;
      //       font-size: 0.14rem;
      //       margin-right: 0.1rem;
      //     }

      //     &:hover {
      //       background: linear-gradient(90deg, rgba(44, 66, 106, 0.9) 0%, rgba(44, 66, 106, 0.5) 100%);
      //     }
      //   }

      //   .el-collapse-item__wrap {
      //     border: none;
      //     background: transparent;

      //     .el-collapse-item__content {
      //       padding: 0;
      //       background: rgba(44, 66, 106, 0.1);
      //     }
      //   }
      // }
    }

    // 折叠面板标题样式
    .collapse-title {
      display: flex;
      align-items: center;
      justify-content: space-between;
      width: 100%;
      padding-right: 0.1rem;

      .collapse-title__text {
        flex: 1;
        font-size: 0.16rem;
        font-weight: 600;
        color: #ffffff;
      }
    }

    // 折叠面板内容样式
    .collapse-content {
      padding: 0.2rem;

      .el-form-item {
        margin-bottom: 0.2rem;

        &:last-child {
          margin-bottom: 0;
        }
      }
    }

    // 输入框样式优化
    :deep(.el-input) {
      width: 100% !important;
    }
  }

  // 连接状态样式
  .connection-status {
    display: flex;
    align-items: center;
    gap: 0.1rem;

    .loading-icon {
      font-size: 0.16rem;
      color: #ff6900;
      animation: rotate 1s linear infinite;
    }
  }

  @keyframes rotate {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }
  .card__content-footer {
    display: flex;
    padding: 0.3rem 0 0.1rem 0;
    width: 100%;
    justify-content: space-between;
    align-items: center;
    // gap: 0.1rem;
  }
}
</style>
