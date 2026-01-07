<template>
  <FormDialog
    ref="dialogRef"
    :title="isEdit ? 'Edit RuleChain' : 'Add RuleChain'"
    width="6rem"
    @close="handleClose"
  >
    <template #dialog-body>
      <div class="rule-edit-dialog">
        <el-form :model="form" label-width="1.2rem" :rules="rules" ref="formRef">
          <el-form-item label="Name:" prop="name">
            <el-input v-model="form.name" placeholder="name" />
          </el-form-item>
          <el-form-item label="Description:" prop="description">
            <el-input
              v-model="form.description"
              type="textarea"
              :rows="3"
              placeholder="description"
            />
          </el-form-item>
        </el-form>
      </div>
    </template>
    <template #dialog-footer>
      <div class="dialog-footer">
        <el-button @click="handleCancel">Cancel</el-button>
        <el-button type="primary" :loading="submitting" @click="handleSubmit">Submit</el-button>
      </div>
    </template>
  </FormDialog>
</template>

<script setup lang="ts">
import { ElMessage } from 'element-plus'
import FormDialog from '@/components/dialog/FormDialog.vue'
import type { Rule } from '@/types/ruleConfiguration'
import { createRule, updateRule } from '@/api/rulesManagement'

const dialogRef = ref()
const isEdit = ref(false)
const submitting = ref(false)
const formRef = ref()

const form = reactive<Rule>({
  name: '',
  description: '',
  id: '',
  enabled: true,
})

const rules = {
  name: [
    { required: true, message: 'Please input name', trigger: 'blur' },
    {
      validator: (_: any, val: string, cb: (err?: Error) => void) => {
        if (typeof val !== 'string' || !val.trim()) return cb(new Error('Name is required'))
        cb()
      },
      trigger: 'blur',
    },
  ],
}

const emit = defineEmits<{ (e: 'submitted'): void }>()

function open(row?: Rule) {
  if (row) {
    isEdit.value = true
    Object.assign(form, row)
  } else {
    isEdit.value = false
    Object.assign(form, { id: '', name: '', description: '', enabled: true })
  }
  dialogRef.value.dialogVisible = true
  nextTick(() => {
    formRef.value?.clearValidate?.()
  })
}

function handleCancel() {
  dialogRef.value.dialogVisible = false
}

function handleClose() {
  // 关闭后的清理工作（如果需要）
}

function validate(): Promise<boolean> {
  return new Promise((resolve) => {
    formRef.value?.validate((ok: boolean) => resolve(ok))
  })
}

async function handleSubmit() {
  const ok = await validate()
  if (!ok) return
  submitting.value = true
  try {
    if (isEdit.value) {
      const { id, name, description } = form
      const res = await updateRule({ name, description: description as string, id: id as string })
      if (res.success) {
        ElMessage.success('Updated successfully')
        dialogRef.value.dialogVisible = false
        emit('submitted')
      }
    } else {
      // 新增：调用 /ruleApi/api/rules 创建规则
      const payload = { name: form.name, description: form.description }
      const res = await createRule(payload)
      if (res.success) {
        ElMessage.success('Created successfully')
        Object.assign(form, { id: '', name: '', description: '', enabled: true })
        dialogRef.value.dialogVisible = false
        emit('submitted')
      }
    }
  } finally {
    submitting.value = false
  }
}

defineExpose({ open })
</script>

<style scoped lang="scss">
.rule-edit-dialog {
  .dialog-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.1rem;
  }
  :deep(.el-textarea),
  :deep(.el-input) {
    width: 3.4rem !important;
  }
}
</style>
