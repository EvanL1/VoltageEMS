<template>
  <FormDialog
    ref="formDialogRef"
    :title="'Data Upload Detail'"
    width="8.94rem"
    @close="handleClose"
  >
    <template #dialog-body>
      <el-form
        ref="FormRef"
        :model="formData"
        :rules="rules"
        label-width="1.35rem"
        label-position="right"
        class="data-upload-dialog"
        inline
      >
        <!-- Basic Configuration -->
        <div class="config-title">basic configuration</div>
        <div class="config-collapse">
          <el-form-item label="Client ID:" prop="client_id">
            <el-input v-model="formData.client_id" placeholder="Enter client ID" />
          </el-form-item>
          <el-form-item label="Port:" prop="port">
            <el-input-number
              v-model="formData.port"
              :min="1"
              :max="65535"
              :controls="false"
              align="left"
              placeholder="Enter port number"
            />
          </el-form-item>
          <el-form-item label="Host:" prop="host" style="width: 8.02rem !important">
            <el-input
              v-model="formData.host"
              placeholder="Enter host address"
              style="width: 100% !important"
            />
          </el-form-item>

          <el-form-item label="Username:" prop="username">
            <el-input v-model="formData.username" placeholder="Enter username" />
          </el-form-item>
          <el-form-item label="Password:" prop="password">
            <el-input
              v-model="formData.password"
              type="password"
              show-password
              placeholder="Enter password"
            />
          </el-form-item>
        </div>

        <!-- SSL Configuration -->
        <div class="config-title">SSL Configuration</div>
        <div class="config-collapse">
          <el-form-item label="Enable:" prop="ssl.enabled" style="width: 100% !important">
            <el-switch v-model="formData.ssl.enabled" />
          </el-form-item>
          <template v-if="formData.ssl.enabled">
            <!-- CA Certificate -->
            <el-form-item
              label="CA Certificate:"
              prop="ssl.ca_cert.path"
              style="width: 8.02rem !important"
            >
              <el-input
                v-model="formData.ssl.ca_cert.path"
                placeholder="Enter CA certificate"
                readonly
                style="width: 100% !important"
              >
                <template #suffix>
                  <el-upload
                    :http-request="caProcessor.httpRequest"
                    :show-file-list="false"
                    :multiple="false"
                    :before-upload="caProcessor.beforeUpload"
                    :disabled="caUploading"
                    :on-success="caProcessor.onSuccess"
                    :on-error="caProcessor.onError"
                    :on-progress="caProcessor.onProgress"
                    accept=".pem"
                  >
                    <el-button type="primary" :loading="caUploading" :disabled="submitLoading"
                      >Upload</el-button
                    >
                  </el-upload>
                </template>
              </el-input>
              <div class="upload-hint" v-if="caUploading">
                <span class="upload-hint__name">{{ caFileName }}</span>
                <span class="upload-hint__progress">{{ caProgress }}%</span>
              </div>
            </el-form-item>

            <!-- Client Certificate -->
            <el-form-item
              label="Client Certificate:"
              prop="ssl.client_cert.path"
              style="width: 8.02rem !important"
            >
              <el-input
                v-model="formData.ssl.client_cert.path"
                placeholder="Enter client certificate"
                readonly
                style="width: 100% !important"
              >
                <template #suffix>
                  <el-upload
                    :http-request="clientCertProcessor.httpRequest"
                    :show-file-list="false"
                    :multiple="false"
                    :before-upload="clientCertProcessor.beforeUpload"
                    :disabled="clientCertUploading"
                    :on-success="clientCertProcessor.onSuccess"
                    :on-error="clientCertProcessor.onError"
                    :on-progress="clientCertProcessor.onProgress"
                    accept=".crt"
                  >
                    <el-button
                      type="primary"
                      :loading="clientCertUploading"
                      :disabled="submitLoading"
                      >Upload</el-button
                    >
                  </el-upload>
                </template>
              </el-input>
              <div class="upload-hint" v-if="clientCertUploading">
                <span class="upload-hint__name">{{ clientCertFileName }}</span>
                <span class="upload-hint__progress">{{ clientCertProgress }}%</span>
              </div>
            </el-form-item>

            <!-- Client Key -->
            <el-form-item
              label="Client Key:"
              prop="ssl.client_key.path"
              style="width: 8.02rem !important"
            >
              <el-input
                v-model="formData.ssl.client_key.path"
                placeholder="Enter client key"
                readonly
                style="width: 100% !important"
              >
                <template #suffix>
                  <el-upload
                    :http-request="clientKeyProcessor.httpRequest"
                    :show-file-list="false"
                    :multiple="false"
                    :before-upload="clientKeyProcessor.beforeUpload"
                    :disabled="clientKeyUploading"
                    :on-success="clientKeyProcessor.onSuccess"
                    :on-error="clientKeyProcessor.onError"
                    :on-progress="clientKeyProcessor.onProgress"
                    accept=".key"
                  >
                    <el-button
                      type="primary"
                      :loading="clientKeyUploading"
                      :disabled="submitLoading"
                      >Upload</el-button
                    >
                  </el-upload>
                </template>
              </el-input>
              <div class="upload-hint" v-if="clientKeyUploading">
                <span class="upload-hint__name">{{ clientKeyFileName }}</span>
                <span class="upload-hint__progress">{{ clientKeyProgress }}%</span>
              </div>
            </el-form-item>
          </template>
        </div>

        <!-- Reconnect Configuration -->
        <div class="config-title">Reconnect Configuration</div>
        <div class="config-collapse">
          <el-form-item label="Enable:" prop="reconnect.enabled" style="width: 100% !important">
            <el-switch v-model="formData.reconnect.enabled" />
          </el-form-item>
          <template v-if="formData.reconnect.enabled">
            <el-form-item label="Delay (seconds):" prop="reconnect.delay">
              <el-input-number
                v-model="formData.reconnect.delay"
                :min="1"
                :max="3600"
                :controls="false"
                align="left"
                placeholder="Enter delay in seconds"
              />
            </el-form-item>
            <el-form-item label="Max Attempts:" prop="reconnect.max_attempts">
              <el-input-number
                v-model="formData.reconnect.max_attempts"
                :min="1"
                :max="100"
                :controls="false"
                align="left"
                placeholder="Enter max attempts"
              />
            </el-form-item>
          </template>
        </div>
      </el-form>
    </template>

    <template #dialog-footer>
      <div class="dialog-footer">
        <el-button @click="close">Cancel</el-button>
        <el-button type="primary" @click="submitDialog" :loading="submitLoading">Submit</el-button>
      </div>
    </template>
  </FormDialog>
</template>

<script setup lang="ts">
import type { FormInstance, FormRules } from 'element-plus'
import { Request } from '@/utils/request'
import { updateMqttConfig } from '@/api/System'
import type { FormData } from '../DataUpdataSetting.vue'
import type { UploadRequestOptions, UploadRawFile } from 'element-plus'

const formData = ref<FormData>({
  port: 0,
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
const formDialogRef = ref()

const FormRef = ref<FormInstance>()

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

// upload states
const caFileName = ref('')
const caProgress = ref(0)
const clientCertFileName = ref('')
const clientCertProgress = ref(0)
const clientKeyFileName = ref('')
const clientKeyProgress = ref(0)
// uploading states
const caUploading = ref(false)
const clientCertUploading = ref(false)
const clientKeyUploading = ref(false)

const emit = defineEmits(['update'])
const open = (data: FormData) => {
  formData.value = data
  formDialogRef.value.dialogVisible = true
}
const close = () => {
  formDialogRef.value.dialogVisible = false
}
const handleClose = () => {
  // keep form data
}

type CertType = 'ca_cert' | 'client_cert' | 'client_key'

const beforeUpload = (file: UploadRawFile | File, certType: CertType) => {
  const name = (file as File).name || ''
  const ext = name.split('.').pop()?.toLowerCase()
  const allowMap: Record<CertType, string[]> = {
    ca_cert: ['pem'],
    client_cert: ['crt'],
    client_key: ['key'],
  }
  const allowed = allowMap[certType]
  if (!ext || !allowed.includes(ext)) {
    switch (certType) {
      case 'ca_cert':
        ElMessage.error('Only .pem files are supported')
        break
      case 'client_cert':
        ElMessage.error('Only .crt files are supported')
        break
      case 'client_key':
        ElMessage.error('Only .key files are supported')
        break
    }
    return false
  }
  return true
}

// 定义上传响应类型
interface UploadResponse {
  status: string
  message: string
  cert_type: string
  filename: string
  path: string
  config_updated: boolean
}

// 上传进度处理
const handleUploadProgress = (event: { percent: number }, certType: CertType) => {
  const percent = Math.round(event.percent || 0)
  if (certType === 'ca_cert') {
    caProgress.value = percent
  } else if (certType === 'client_cert') {
    clientCertProgress.value = percent
  } else if (certType === 'client_key') {
    clientKeyProgress.value = percent
  }
}

// 上传成功处理
const handleUploadSuccess = (response: UploadResponse, certType: CertType) => {
  // 根据新的返回数据结构处理
  const { status, message, path, filename } = response

  if (status === 'success') {
    // 更新表单数据
    if (certType === 'ca_cert') {
      formData.value.ssl.ca_cert.path = path
      formData.value.ssl.ca_cert.full_path = path
    } else if (certType === 'client_cert') {
      formData.value.ssl.client_cert.path = path
      formData.value.ssl.client_cert.full_path = path
    } else if (certType === 'client_key') {
      formData.value.ssl.client_key.path = path
      formData.value.ssl.client_key.full_path = path
    }

    // 显示成功消息
    ElMessage.success(message || 'Upload success')

    // 延迟隐藏进度提示
    setTimeout(() => {
      if (certType === 'ca_cert') {
        caFileName.value = ''
        caProgress.value = 0
        caUploading.value = false
      } else if (certType === 'client_cert') {
        clientCertFileName.value = ''
        clientCertProgress.value = 0
        clientCertUploading.value = false
      } else if (certType === 'client_key') {
        clientKeyFileName.value = ''
        clientKeyProgress.value = 0
        clientKeyUploading.value = false
      }
    }, 1000)
  }
}

// 上传失败处理
const handleUploadError = (error: Error, certType: CertType) => {
  const errorMessage = error?.message || 'Upload failed'
  const certTypeName =
    certType === 'ca_cert'
      ? 'CA Certificate'
      : certType === 'client_cert'
        ? 'Client Certificate'
        : 'Client Key'

  ElMessage.error(`${certTypeName} Upload failed: ${errorMessage}`)

  // 重置上传状态
  if (certType === 'ca_cert') {
    caUploading.value = false
    caFileName.value = ''
    caProgress.value = 0
  } else if (certType === 'client_cert') {
    clientCertUploading.value = false
    clientCertFileName.value = ''
    clientCertProgress.value = 0
  } else if (certType === 'client_key') {
    clientKeyUploading.value = false
    clientKeyFileName.value = ''
    clientKeyProgress.value = 0
  }
}

const requestUpload = async (options: UploadRequestOptions, certType: CertType) => {
  const file = options.file as File

  // 初始化显示状态
  if (certType === 'ca_cert') {
    caFileName.value = file.name
    caProgress.value = 0
    caUploading.value = true
  } else if (certType === 'client_cert') {
    clientCertFileName.value = file.name
    clientCertProgress.value = 0
    clientCertUploading.value = true
  } else if (certType === 'client_key') {
    clientKeyFileName.value = file.name
    clientKeyProgress.value = 0
    clientKeyUploading.value = true
  }

  // 执行上传请求
  return Request.upload<any>(
    '/netApi/certificate/upload',
    file,
    { cert_type: certType },
    {
      onUploadProgress: (evt: any) => {
        const total = evt.total || 0
        const loaded = evt.loaded || 0
        const percent = total > 0 ? Math.round((loaded / total) * 100) : 0
        handleUploadProgress({ percent }, certType)
        options.onProgress && options.onProgress({ percent } as any)
      },
    },
  )
}

// 通用上传处理器创建函数
const createUploadProcessor = (certType: CertType) => {
  return {
    httpRequest: (options: UploadRequestOptions) => requestUpload(options, certType),
    beforeUpload: (file: UploadRawFile) => beforeUpload(file, certType),
    onSuccess: (response: UploadResponse) => handleUploadSuccess(response, certType),
    onError: (error: Error) => handleUploadError(error, certType),
    onProgress: (event: { percent: number }) => handleUploadProgress(event, certType),
  }
}

// 为每个证书类型创建所有处理器
const caProcessor = createUploadProcessor('ca_cert')
const clientCertProcessor = createUploadProcessor('client_cert')
const clientKeyProcessor = createUploadProcessor('client_key')

const submitDialog = async () => {
  FormRef.value?.validate(async (valid: boolean) => {
    if (!valid) return
    try {
      submitLoading.value = true
      const params = Object.assign({}, toRaw(formData.value), {
        ssl: {
          ca_cert: formData.value.ssl.ca_cert.path,
          client_cert: formData.value.ssl.client_cert.path,
          client_key: formData.value.ssl.client_key.path,
        },
      })
      const res = await updateMqttConfig({ mqtt_connection: { broker: params } })
      if (res.status == 'success') {
        ElMessage.success('Update success')
        close()
        emit('update')
      } else {
        ElMessage.error(res.message || 'Update failed')
      }
    } finally {
      submitLoading.value = false
    }
  })
}

defineExpose({ open })
</script>

<style scoped lang="scss">
.voltage-class {
  .data-upload-dialog {
    max-height: 6rem;
    overflow-y: auto;
  }
  .config-title {
    font-size: 0.16rem;
    color: #fff;
    margin-bottom: 0.16rem;
    font-weight: 700;
    padding-bottom: 0.1rem;
    border-bottom: 0.01rem solid rgba(255, 255, 255, 0.1);
  }
  .config-collapse {
    border: none;
    display: flex;
    flex-wrap: wrap;
    gap: 0.2rem;
    // flex-direction: column;
    // :deep(.el-input) {
    //   width: 100% !important;
    // }
    :deep(.el-form-item) {
      width: 3.91rem !important;
      position: relative;
      margin-right: 0 !important;
      margin-bottom: 0.2rem !important;
    }
  }

  .upload-hint {
    position: absolute;
    top: 0.27rem;
    left: 0;
    display: flex;
    align-items: center;
    gap: 0.08rem;
    font-size: 0.12rem;
    color: #fff;
    // margin-top: 0.06rem;

    .upload-hint__name {
      // max-width: 2.6rem;
      // overflow: hidden;
      // text-overflow: ellipsis;
      // white-space: nowrap;
    }
    .upload-hint__progress {
      color: #ff6900;
    }
  }

  .dialog-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.1rem;
  }
}
</style>
