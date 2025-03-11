<template>
  <FormDialog width="13.36rem" ref="dialogRef" :title="dialogTitle">
    <template #dialog-body>
      <el-form
        ref="formRef"
        :model="form"
        :rules="rules"
        label-width="0.98rem"
        class="voltage-class rules-form"
        label-position="right"
        inline
      >
        <el-form-item label="Rule Name" prop="ruleName">
          <el-input v-model="form.ruleName" placeholder="Enter rule name" />
        </el-form-item>
        <el-form-item label="Monitor Data" required style="margin-right: 0">
          <div class="monitor-data-group">
            <el-select v-model="form.monitorData.part1" placeholder="Select A">
              <el-option
                v-for="item in monitorAOptions"
                :key="item.value"
                :label="item.label"
                :value="item.value"
              />
            </el-select>
            <el-select v-model="form.monitorData.part2" placeholder="Select B">
              <el-option
                v-for="item in monitorBOptions"
                :key="item.value"
                :label="item.label"
                :value="item.value"
              />
            </el-select>
            <el-select v-model="form.monitorData.part3" placeholder="Select C">
              <el-option
                v-for="item in monitorCOptions"
                :key="item.value"
                :label="item.label"
                :value="item.value"
              />
            </el-select>
          </div>
        </el-form-item>

        <el-form-item label="Alarm Level" prop="alarmLevel">
          <el-select v-model="form.alarmLevel" placeholder="Select level">
            <el-option
              v-for="item in alarmLevelOptions"
              :key="item.value"
              :label="item.label"
              :value="item.value"
            />
          </el-select>
        </el-form-item>

        <el-form-item label="Condition" required style="margin-right: 0">
          <div class="condition-group">
            <el-select v-model="form.condition.operator" placeholder="Operator">
              <el-option label=">" value="gt" />
              <el-option label=">=" value="gte" />
              <el-option label="<" value="lt" />
              <el-option label="<=" value="lte" />
              <el-option label="=" value="eq" />
            </el-select>
            <el-input-number
              v-model="form.condition.threshold"
              :min="0"
              :max="999999"
              :controls="false"
              placeholder="Value"
              style="width: 4.96rem"
            />
          </div>
        </el-form-item>

        <el-form-item label="Notification" prop="notification">
          <el-checkbox-group v-model="form.notification">
            <el-checkbox label="HomePage" />
            <el-checkbox label="Email" />
            <el-checkbox label="SMS" />
            <el-checkbox label="Cloud" />
          </el-checkbox-group>
        </el-form-item>

        <el-form-item label="Switch" prop="enabled" style="display: flex">
          <el-switch v-model="form.enabled" />
        </el-form-item>
      </el-form>
    </template>

    <template #dialog-footer>
      <el-button type="warning" @click="onCancel" style="margin-right: 0.2rem">Cancel</el-button>
      <el-button type="primary" @click="onSubmit">Submit</el-button>
    </template>
  </FormDialog>
</template>

<script setup lang="ts">
import type { FormInstance } from 'element-plus'
type Operator = 'gt' | 'gte' | 'lt' | 'lte' | 'eq'

interface RuleFormModel {
  ruleName: string
  monitorData: {
    part1: string | number | null
    part2: string | number | null
    part3: string | number | null
  }
  alarmLevel: string | number | null
  condition: {
    operator: Operator | null
    threshold: number | null
  }
  notification: string[]
  enabled: boolean
}

interface DialogExpose {
  // 使用公开实例上的解包后类型，直接作为 boolean 处理
  dialogVisible: boolean
}

const formRef = ref<FormInstance>()
const dialogRef = ref<DialogExpose>()

const getDefaultForm = (): RuleFormModel => ({
  ruleName: '',
  monitorData: { part1: null, part2: null, part3: null },
  alarmLevel: null,
  condition: { operator: null, threshold: null },
  notification: [],
  enabled: true,
})

const form = ref<RuleFormModel>(getDefaultForm())

const monitorAOptions = [
  { label: 'Plant A', value: 'plantA' },
  { label: 'Plant B', value: 'plantB' },
]
const monitorBOptions = [
  { label: 'Device 01', value: 'dev01' },
  { label: 'Device 02', value: 'dev02' },
]
const monitorCOptions = [
  { label: 'Voltage', value: 'voltage' },
  { label: 'Current', value: 'current' },
]

const alarmLevelOptions = [
  { label: 'Info', value: 'info' },
  { label: 'Warning', value: 'warning' },
  { label: 'Critical', value: 'critical' },
]

const rules = {
  ruleName: [{ required: true, message: 'Please input rule name', trigger: 'blur' }],
  'monitorData.part1': [{ required: true, message: 'Required', trigger: 'change' }],
  'monitorData.part2': [{ required: true, message: 'Required', trigger: 'change' }],
  'monitorData.part3': [{ required: true, message: 'Required', trigger: 'change' }],
  alarmLevel: [{ required: true, message: 'Please select alarm level', trigger: 'change' }],
  'condition.operator': [{ required: true, message: 'Please select operator', trigger: 'change' }],
  'condition.threshold': [{ required: true, message: 'Please input value', trigger: 'blur' }],
}

const mode = ref<'create' | 'edit'>('create')
const dialogTitle = computed(() => (mode.value === 'edit' ? 'Edit Rule' : 'New Rule'))

function deepClone<T>(obj: T): T {
  return JSON.parse(JSON.stringify(obj))
}

function open(payload?: Partial<RuleFormModel>, openMode: 'create' | 'edit' = 'create') {
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
  (e: 'submit', value: RuleFormModel): void
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
  :deep(.el-switch) {
    height: 32px !important;
  }
  :deep(.el-input__inner) {
    width: 240px;
  }
}
</style>
