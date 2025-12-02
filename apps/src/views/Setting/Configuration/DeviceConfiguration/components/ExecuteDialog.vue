<template>
  <FormDialog ref="formDialogRef" title="Execute" width="4.8rem" @close="close">
    <template #dialog-body>
      <div class="voltage-class execute-dialog">
        <el-form label-width="0.9rem" ref="formRef" :model="form" :rules="rules">
          <el-form-item label="Value" required>
            <el-input-number v-model="form.value" :controls="false" align="left" />
          </el-form-item>
        </el-form>
      </div>
    </template>
    <template #dialog-footer>
      <el-button @click="close">Cancel</el-button>
      <el-button type="primary" @click="submit">Submit</el-button>
    </template>
  </FormDialog>
</template>

<script setup lang="ts">
import { ref, inject } from 'vue'
import { ElMessage } from 'element-plus'
import FormDialog from '@/components/dialog/FormDialog.vue'
import { executeAction, executeMeasurement } from '@/api/devicesManagement'
import { InstanceIdKey } from '@/utils/key'
const formDialogRef = ref<{ dialogVisible: boolean } | null>(null)
const formRef = ref()
const form = ref<{
  value: number | undefined
  point_id: string
  category: 'action' | 'measurement'
}>({
  value: undefined,
  point_id: '',
  category: 'action',
})
const instanceId = inject(InstanceIdKey)
const rules = {
  value: [{ required: true, message: 'Please enter value', trigger: 'blur' }],
}
function open(point_id: string, category: 'action' | 'measurement' = 'action') {
  form.value.value = undefined
  form.value.point_id = point_id
  form.value.category = category
  if (formDialogRef.value) formDialogRef.value.dialogVisible = true
}

function close() {
  if (formDialogRef.value) formDialogRef.value.dialogVisible = false
}

function submit() {
  formRef.value.validate(async (valid: boolean) => {
    if (valid) {
      if (instanceId?.value) {
        const payload = {
          value: form.value.value as number,
          point_id: form.value.point_id,
        }
        const res =
          form.value.category === 'measurement'
            ? await executeMeasurement(Number(instanceId?.value) as number, payload)
            : await executeAction(Number(instanceId?.value) as number, payload)
        if (res.success) {
          ElMessage.success('Execute success!')
          close()
        }
      }
    }
  })
}

defineExpose({ open, close })
</script>

<style scoped lang="scss">
.voltage-class {
  .execute-dialog {
    width: 100%;
  }
}
</style>
