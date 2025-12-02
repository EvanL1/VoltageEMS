<template>
  <FormDialog ref="dialogRef" title="Value Publish" width="5rem" @close="handleClose">
    <template #dialog-body>
      <div class="voltage-class value-publish">
        <el-form label-width="1rem" inline ref="formRef" :model="form" :rules="rules">
          <el-form-item label="Value" style="margin-right: 0" prop="value">
            <div
              class="value-publish__boolean"
              v-if="form.category === 'C' || form.category === 'S'"
            >
              <el-select
                v-model="form.value"
                placeholder="Select"
                :teleported="false"
                append-to=".value-publish__boolean"
              >
                <el-option label="1" :value="1" />
                <el-option label="0" :value="0" />
              </el-select>
            </div>
            <template v-else-if="form.category === 'A' || form.category === 'T'">
              <el-input-number align="left" v-model="form.value" :controls="false" />
            </template>
          </el-form-item>
        </el-form>
      </div>
    </template>
    <template #dialog-footer>
      <div class="dialog-footer">
        <el-button @click="close">Cancel</el-button>
        <el-button type="primary" @click="submit">Submit</el-button>
      </div>
    </template>
  </FormDialog>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { ElMessage } from 'element-plus'
import FormDialog from '@/components/dialog/FormDialog.vue'
import { publishPointValue } from '@/api/channelsManagement'
import { ChannelIdKey, ChannelNameKey } from '@/utils/key'
interface OpenPayload {
  pointId: number
  dataType: string
  category: 'C' | 'A'
}
const channelName = inject(ChannelNameKey)
const channelId = inject(ChannelIdKey)
const dialogRef = ref()
const formRef = ref()
const form = ref<{
  pointId: number
  dataType: string
  value: boolean | number | string | undefined
  category: 'C' | 'A' | 'T' | 'S' | ''
}>({
  pointId: 0,
  dataType: '',
  value: undefined,
  category: '',
})

const rules = {
  value: [{ required: true, message: 'Please enter value', trigger: 'blur' }],
}

const open = (payload: OpenPayload) => {
  form.value.pointId = payload.pointId
  form.value.dataType = payload.dataType
  form.value.category = payload.category
  // 清理输入值与错误提示
  form.value.value = undefined
  dialogRef.value.dialogVisible = true
  setTimeout(() => {
    formRef.value?.clearValidate()
  }, 100)
}

const close = () => {
  dialogRef.value.dialogVisible = false
}

const handleClose = () => {
  close()
}

const submit = async () => {
  formRef.value.validate(async (valid: boolean) => {
    if (!valid) return
    let outValue: any = form.value.value
    // 如果是'A'或'T'类型，需要将输入转为浮点数，且如果整数要强制变成带小数点的数（如1变为1.0）
    // if (form.value.category === 'A' || form.value.category === 'T') {
    //   let n = Number(outValue)
    //   if (Number.isInteger(n)) {
    //     // 转成带小数点的数据
    //     outValue = n.toFixed(1) // 1 => "1.0"
    //   } else if (!isNaN(n)) {
    //     outValue = n
    //   }
    // }
    const res = await publishPointValue(channelId!.value, {
      type: form.value.category as 'C' | 'A' | 'T' | 'S',
      id: form.value.pointId.toString(),
      value: outValue,
    })
    if (res.success) {
      ElMessage.success('Published successfully')
      close()
    }
  })
}

defineExpose({
  open,
  close,
})
</script>

<style scoped lang="scss">
.voltage-class .value-publish {
  // padding: 0.1rem 0.06rem;
}
.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.1rem;
}
</style>
