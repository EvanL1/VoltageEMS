<template>
  <FormDialog width="9.16rem" ref="dialogRef" :title="dialogTitle">
    <template #dialog-body>
      <el-form
        ref="formRef"
        :model="form"
        :rules="rules"
        label-width="1.24rem"
        class="user-form"
        label-position="right"
        inline
      >
        <el-form-item label="Username" prop="userName">
          <el-input v-model="form.userName" placeholder="Enter username" />
        </el-form-item>

        <el-form-item label="Role" prop="role">
          <el-select v-model="form.role" placeholder="Select role">
            <el-option
              v-for="item in roleOptions"
              :key="item.value"
              :label="item.label"
              :value="item.value"
            />
          </el-select>
        </el-form-item>

        <el-form-item label="Email" prop="email">
          <el-input v-model="form.email" placeholder="Enter email" />
        </el-form-item>

        <el-form-item label="Phone" prop="phone">
          <el-input v-model="form.phone" placeholder="Enter phone number" />
        </el-form-item>

        <!-- Status 独占一行，去除 inline 并设置宽度100% -->
        <el-form-item label="Status" prop="status" class="user-form__status-row">
          <el-select v-model="form.status" placeholder="Select status" style="width: 100%">
            <el-option
              v-for="item in statusOptions"
              :key="item.value"
              :label="item.label"
              :value="item.value"
            />
          </el-select>
        </el-form-item>

        <el-form-item label="Password" prop="password" v-if="mode === 'create'">
          <el-input
            v-model="form.password"
            type="password"
            placeholder="Enter password"
            show-password
          />
        </el-form-item>

        <el-form-item label="Confirm Password" prop="confirmPassword" v-if="mode === 'create'">
          <el-input
            v-model="form.confirmPassword"
            type="password"
            placeholder="Confirm password"
            show-password
          />
        </el-form-item>
      </el-form>
    </template>

    <template #dialog-footer>
      <el-button type="warning" @click="onCancel">Cancel</el-button>
      <el-button type="primary" @click="onSubmit">Submit</el-button>
    </template>
  </FormDialog>
</template>

<script setup lang="ts">
import type { FormInstance } from 'element-plus'

interface UserFormModel {
  userName: string
  role: string | null
  email: string
  phone: string
  status: string | null
  password: string
  confirmPassword: string
}

interface DialogExpose {
  dialogVisible: boolean
}

const formRef = ref<FormInstance>()
const dialogRef = ref<DialogExpose>()

const getDefaultForm = (): UserFormModel => ({
  userName: '',
  role: null,
  email: '',
  phone: '',
  status: null,
  password: '',
  confirmPassword: '',
})

const form = ref<UserFormModel>(getDefaultForm())

const roleOptions = [
  { label: 'Admin', value: 'Admin' },
  { label: 'User', value: 'User' },
  { label: 'Moderator', value: 'Moderator' },
]

const statusOptions = [
  { label: 'Active', value: 'active' },
  { label: 'Inactive', value: 'inactive' },
  { label: 'Banned', value: 'banned' },
]

// 密码确认验证函数
const validateConfirmPassword = (rule: any, value: string, callback: any) => {
  if (value === '') {
    callback(new Error('Please confirm your password'))
  } else if (value !== form.value.password) {
    callback(new Error('Passwords do not match'))
  } else {
    callback()
  }
}

const rules = computed(() => {
  const baseRules = {
    userName: [
      { required: true, message: 'Please input username', trigger: 'blur' },
      { min: 3, max: 20, message: 'Length should be 3 to 20 characters', trigger: 'blur' },
    ],
    role: [{ required: true, message: 'Please select role', trigger: 'change' }],
    email: [{ type: 'email', message: 'Please input correct email address', trigger: 'blur' }],
    phone: [
      {
        pattern: /^[\+]?[1-9][\d]{0,15}$/,
        message: 'Please input correct phone number',
        trigger: 'blur',
      },
    ],
    status: [{ required: true, message: 'Please select status', trigger: 'change' }],
  }

  // 只在创建模式下添加密码验证规则
  if (mode.value === 'create') {
    return {
      ...baseRules,
      password: [
        { required: true, message: 'Please input password', trigger: 'blur' },
        { min: 6, message: 'Password must be at least 6 characters', trigger: 'blur' },
      ],
      confirmPassword: [{ required: true, validator: validateConfirmPassword, trigger: 'blur' }],
    }
  }

  return baseRules
})

const mode = ref<'create' | 'edit'>('create')
const dialogTitle = computed(() => (mode.value === 'edit' ? 'Edit User' : 'Add User'))

function deepClone<T>(obj: T): T {
  return JSON.parse(JSON.stringify(obj))
}

function open(payload?: Partial<UserFormModel>, openMode: 'create' | 'edit' = 'create') {
  mode.value = openMode
  form.value = getDefaultForm()
  if (payload) Object.assign(form.value, deepClone(payload))
  nextTick(() => {
    formRef.value?.clearValidate()
  })
  dialogRef.value && (dialogRef.value.dialogVisible = true)
}

function close() {
  dialogRef.value && (dialogRef.value.dialogVisible = false)
}

const emit = defineEmits<{
  (e: 'submit', value: UserFormModel): void
  (e: 'cancel'): void
}>()

function onCancel() {
  close()
  emit('cancel')
}

function onSubmit() {
  formRef.value?.validate((valid) => {
    if (!valid) return
    emit('submit', deepClone(form.value))
    close()
  })
}

defineExpose({ open, close })
</script>

<style scoped lang="scss">
.voltage-class {
  .monitor-data-group {
    display: flex;
    gap: 16px;
  }

  .condition-group {
    display: flex;
    gap: 16px;
  }
  :deep(.el-input__inner) {
    width: 240px;
  }
  // 让Status输入框占据整行
  .user-form__status-row {
    width: 100%;
    display: block;
    :deep(.el-form-item__content) {
      width: 660px;
    }
  }
}
</style>
