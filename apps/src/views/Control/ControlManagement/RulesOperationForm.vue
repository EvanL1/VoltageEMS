<template>
  <FormDialog width="13.46rem" ref="dialogRef" :title="dialogTitle">
    <template #dialog-body>
      <el-form
        ref="formRef"
        :model="form"
        :rules="rules"
        label-width="0.98rem"
        class="rules-form"
        label-position="right"
        inline
      >
        <el-form-item label="Rule Name:" prop="rule_name">
          <el-input v-model="form.rule_name" placeholder="Enter rule name" />
        </el-form-item>

        <!-- 拆分Monitor Data为三个el-form-item，分别校�?-->
        <div class="monitor-data-group" ref="monitorDataGroupRef">
          <el-form-item label="Monitor Data:" prop="service_type" style="margin-right: 0">
            <el-select
              v-model="form.service_type"
              placeholder="Select Service Type"
              popper-class="rules-dialog-popper"
              :append-to="monitorDataGroupRef"
            >
              <el-option
                v-for="item in service_types"
                :key="item.value"
                :label="item.label"
                :value="item.value"
              />
            </el-select>
          </el-form-item>
          <el-form-item prop="point_id" style="margin-right: 0">
            <el-select
              v-model="form.point_id"
              placeholder="Select Point"
              popper-class="rules-dialog-popper"
              :append-to="monitorDataGroupRef"
            >
              <el-option
                v-for="item in points"
                :key="item.value"
                :label="item.label"
                :value="item.value"
              />
            </el-select>
          </el-form-item>
          <el-form-item prop="data_type" style="margin-right: 0">
            <el-select
              v-model="form.data_type"
              placeholder="Select Data Type"
              popper-class="rules-dialog-popper"
              :append-to="monitorDataGroupRef"
            >
              <el-option
                v-for="item in data_types"
                :key="item.value"
                :label="item.label"
                :value="item.value"
              />
            </el-select>
          </el-form-item>
        </div>

        <el-form-item label="Alarm Level:" prop="warning_level">
          <div class="alarm-level-group" ref="alarmLevelGroupRef">
            <el-select
              v-model="form.warning_level"
              placeholder="Select level"
              popper-class="rules-dialog-popper"
              :append-to="alarmLevelGroupRef"
            >
              <el-option
                v-for="item in alarmLevelOptions"
                :key="item.value"
                :label="item.label"
                :value="item.value"
              />
            </el-select>
          </div>
        </el-form-item>

        <!-- 拆分Condition为两个el-form-item，分别校�?-->
        <div class="condition-group" ref="conditionGroupRef">
          <el-form-item prop="operator" label="Condition:" style="margin-right: 0">
            <el-select
              v-model="form.operator"
              placeholder="Operator"
              popper-class="rules-dialog-popper"
              :append-to="conditionGroupRef"
            >
              <el-option label=">" value=">" />
              <el-option label=">=" value=">=" />
              <el-option label="<" value="<" />
              <el-option label="<=" value="<=" />
              <el-option label="=" value="=" />
            </el-select>
          </el-form-item>
          <el-form-item prop="value" style="margin-right: 0">
            <el-input-number
              v-model="form.value"
              :min="0"
              :max="999999"
              :controls="false"
              placeholder="Value"
              align="left"
              style="width: 4.96rem"
            />
          </el-form-item>
        </div>

        <el-form-item label="Switch:" prop="enabled" style="display: flex; margin-right: 0">
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
import { getRuleDetail, createRule, updateRule } from '@/api/alarm'
import type { Operator, RuleFormModel, DialogExpose } from '@/types/controlManagement'

const formRef = ref<FormInstance>()
const dialogRef = ref<DialogExpose>()
const monitorDataGroupRef = ref<HTMLElement>()
const alarmLevelGroupRef = ref<HTMLElement>()
const conditionGroupRef = ref<HTMLElement>()

const getDefaultForm = (): RuleFormModel => ({
  rule_name: '',
  service_type: null,
  point_id: null,
  data_type: null,
  warning_level: null,
  operator: null,
  value: null,
  enabled: true,
})

const form = ref<RuleFormModel>(getDefaultForm())

const service_types = [{ label: 'comsrv', value: 'comsrv' }]
const points = [
  { label: '1', value: 1 },
  { label: '2', value: 2 },
  { label: '3', value: 3 },
  { label: '4', value: 4 },
  { label: '5', value: 5 },
  { label: '6', value: 6 },
  { label: '7', value: 7 },
  { label: '8', value: 8 },
]
const data_types = [
  { label: 'T', value: 'T' },
  { label: 'A', value: 'A' },
]

const alarmLevelOptions = [
  { label: 'L1', value: 1 },
  { label: 'L2', value: 2 },
  { label: 'L3', value: 3 },
]

const rules = {
  rule_name: [{ required: true, message: 'Please input rule name', trigger: 'blur' }],
  service_type: [{ required: true, message: 'Required', trigger: 'change' }],
  point_id: [{ required: true, message: 'Required', trigger: 'change' }],
  data_type: [{ required: true, message: 'Required', trigger: 'change' }],
  warning_level: [{ required: true, message: 'Please select alarm level', trigger: 'change' }],
  operator: [{ required: true, message: 'Please select operator', trigger: 'change' }],
  value: [{ required: true, message: 'Please input value', trigger: 'blur' }],
}

const mode = ref<'create' | 'edit'>('create')
const dialogTitle = computed(() => (mode.value === 'edit' ? 'Edit Rule' : 'New Rule'))

const rules_id = ref<string>()
async function open(rulesId?: string, openMode: 'create' | 'edit' = 'create') {
  try {
    mode.value = openMode
    form.value = getDefaultForm()
    rules_id.value = rulesId || ''
    if (rulesId) {
      const res = await getRuleDetail(rules_id.value)
      if (res.success) {
        form.value.rule_name = res.data.list[0].rule_name
        form.value.service_type = res.data.list[0].service_type
        form.value.point_id = res.data.list[0].point_id
        form.value.data_type = res.data.list[0].data_type
        form.value.warning_level = res.data.list[0].warning_level
        form.value.operator = res.data.list[0].operator
        form.value.value = res.data.list[0].value
        form.value.enabled = res.data.list[0].enabled
      }
    }
    nextTick(() => {
      setTimeout(() => {
        formRef.value?.clearValidate()
      }, 100)
    })
    dialogRef.value && (dialogRef.value.dialogVisible = true)
  } catch (error) {
    console.error(error)
  }
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

async function onSubmit() {
  formRef.value?.validate(async (valid) => {
    if (!valid) return
    if (mode.value === 'create') {
      const res = await createRule({ ...form.value, channel_id: 1001 })
      if (res.success) {
        emit('submit', form.value)
        close()
      } else {
        throw new Error(res.message)
      }
    } else {
      if (!rules_id.value) {
        throw new Error('rules_id is required')
      }
      const res = await updateRule(rules_id.value, { ...form.value, channel_id: 1001 })
      if (res.success) {
        emit('submit', form.value)
        close()
      }
    }
  })
}

defineExpose({ open, close })
</script>

<style scoped lang="scss">
.rules-form {
  display: flex;
  flex-wrap: wrap;

  .monitor-data-group,
  .alarm-level-group,
  .condition-group {
    position: relative;
    display: flex;
    gap: 0.16rem;
  }

  :deep(.el-switch) {
    height: 0.32rem !important;
  }

  :deep(.el-input__inner) {
    width: 2.4rem;
  }
}

// 使用自定义类名的下拉框样
:deep(.rules-dialog-popper) {
  top: 0.44rem !important;
}
</style>
