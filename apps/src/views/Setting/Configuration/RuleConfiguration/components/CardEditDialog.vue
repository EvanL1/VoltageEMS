<template>
  <FormDialog
    ref="formDialogRef"
    :title="`Edit ${card?.label}`"
    :width="widthList[card?.type as keyof typeof widthList] || '10rem'"
    @close="handleClose"
  >
    <template #dialog-body>
      <div v-if="card" class="voltage-class card-edit-dialog">
        <!-- 使用动态组件承载具体类型的表单组件 -->
        <component :is="currentFormComponent" ref="childFormRef" :cardData="localCard" />
      </div>
    </template>
    <template #dialog-footer>
      <el-button @click="handleClose">Cancel</el-button>
      <el-button type="primary" @click="handleSave">Submit</el-button>
    </template>
  </FormDialog>
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import FunctionSwitchForm from './customCardForm/FunctionSwitchForm.vue'
import ActionChangeValue from './customCardForm/ActionChangeValue.vue'
import type { RuleCard } from '@/types/ruleConfiguration'

interface Props {
  visible: boolean
  card: RuleCard | null
}
const widthList = {
  'function-switch': '14rem',
  'action-changeValue': '14rem',
}
interface Emits {
  (e: 'update:visible', value: boolean): void
  (e: 'save', card: RuleCard): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

const localCard = ref<RuleCard | null>(null)
const formDialogRef = ref<any>(null)
const childFormRef = ref<any>(null)

const componentMap: Record<string, any> = {
  'function-switch': FunctionSwitchForm,
  'action-changeValue': ActionChangeValue,
}

const currentFormComponent = computed(() => {
  const type = localCard.value?.type || ''
  return componentMap[type] || null
})

// 监听外部card变化，同步到本地
watch(
  () => props.card,
  (newCard) => {
    if (newCard) {
      localCard.value = deepClone(newCard)
    }
  },
  { immediate: true },
)

// 同步外部可见性到 FormDialog 内部的 dialogVisible
watch(
  () => props.visible,
  (visible) => {
    if (formDialogRef.value) {
      formDialogRef.value.dialogVisible = visible
    }
  },
  { immediate: true },
)

const handleClose = () => {
  emit('update:visible', false)
}

const handleSave = async () => {
  if (!localCard.value) return
  const result = await (childFormRef.value?.validateForm?.() ??
    Promise.resolve({ valid: true, data: localCard.value }))
  if (!result?.valid) return
  emit('save', result.data)
}

function deepClone<T>(obj: T): T {
  try {
    // @ts-ignore
    if (typeof structuredClone === 'function') return structuredClone(obj)
  } catch {}
  return JSON.parse(JSON.stringify(obj))
}
</script>

<style lang="scss" scoped>
.voltage-class {
  .card-edit-dialog {
    .el-form {
      .el-form-item {
        margin-bottom: 0.2rem;

        .el-form-item__label {
          font-weight: 600;
          color: #2c3e50;
        }
      }
    }
  }
}
</style>
