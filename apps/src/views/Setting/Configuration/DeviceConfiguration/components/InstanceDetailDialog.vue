<template>
  <FormDialog title="Device Instance Detail" width="8rem" ref="dialogRef" @close="handleClose">
    <template #dialog-body>
      <div class="voltage-class instance-detail">
        <!-- 基础信息 -->
        <div class="instance-detail__basic-info">
          <h3 class="instance-detail__section-title">Basic Information</h3>
          <el-form
            ref="formRef"
            :model="form"
            :rules="rules"
            label-width="1.2rem"
            class="instance-detail__form"
          >
            <el-form-item v-if="!isCreateMode" label="Instance ID:" prop="instance_id">
              <span class="instance-detail__readonly-text">{{ form.instance_id }}</span>
            </el-form-item>
            <el-form-item label="Instance Name:" prop="instance_name">
              <el-input
                v-if="isEditing"
                v-model="form.instance_name"
                placeholder="Please enter instance name"
              />
              <span v-else class="instance-detail__readonly-text">{{ form.instance_name }}</span>
            </el-form-item>

            <el-form-item label="Product Name:" prop="product_name">
              <!-- 新增时可选，修改时只读 -->
              <el-select
                v-if="isCreateMode"
                v-model="form.product_name"
                placeholder="Please select product"
                filterable
              >
                <el-option
                  v-for="product in props.productOptions"
                  :key="product.value"
                  :label="product.label"
                  :value="product.value"
                />
              </el-select>
              <span v-else class="instance-detail__readonly-text">{{ form.product_name }}</span>
            </el-form-item>

            <el-form-item label="Properties:">
              <div class="instance-detail__properties">
                <div class="instance-detail__properties-grid">
                  <!-- 编辑态：基于数组 editProperties 渲染，可编辑 key 与 value -->
                  <template v-if="isEditing">
                    <div
                      v-for="(prop, index) in editProperties"
                      :key="`property-edit-${index}`"
                      class="instance-detail__property-item"
                    >
                      <div class="instance-detail__property-content">
                        <div class="instance-detail__property-key-container">
                          <el-input
                            v-model="prop.key"
                            placeholder="Key"
                            class="instance-detail__property-key-input"
                          >
                          </el-input>
                        </div>
                        <span class="instance-detail__property-separator">:</span>
                        <div class="instance-detail__property-value-container">
                          <el-input
                            v-model="prop.value"
                            placeholder="Value"
                            class="instance-detail__property-value-input"
                          >
                            <template #suffix>
                              <el-button
                                type="warning"
                                @click="removePropertyByIndex(index)"
                                circle
                              >
                                <el-icon>
                                  <Delete />
                                </el-icon>
                              </el-button>
                            </template>
                          </el-input>
                        </div>
                      </div>
                    </div>
                  </template>
                  <!-- 只读态：基于对象 entries 渲染显示 key 与 value -->
                  <template v-else>
                    <div
                      v-for="(value, key) in form.properties"
                      :key="`property-${key}`"
                      class="instance-detail__property-item"
                    >
                      <div class="instance-detail__property-content">
                        <div class="instance-detail__property-key-container">
                          <span class="instance-detail__property-key">{{ key }}</span>
                        </div>
                        <span class="instance-detail__property-separator">:</span>
                        <div class="instance-detail__property-value-container">
                          <span class="instance-detail__property-value">{{ value }}</span>
                        </div>
                      </div>
                    </div>
                  </template>
                </div>
              </div>
              <el-button
                v-if="isEditing"
                type="primary"
                size="small"
                @click="addProperty('', '')"
                style="margin-left: 0.1rem"
                circle
              >
                <el-icon>
                  <Plus />
                </el-icon>
              </el-button>
            </el-form-item>
          </el-form>
        </div>
      </div>
    </template>

    <template #dialog-footer>
      <el-button type="warning" @click="handleCancel">{{
        isCreateMode ? 'Cancel' : isEditing ? 'Cancel Edit' : 'Cancel'
      }}</el-button>
      <el-button v-if="!isEditing" type="primary" @click="handleEdit"> Edit </el-button>
      <el-button v-else type="primary" @click="handleSubmit"> Submit </el-button>
    </template>
  </FormDialog>
</template>

<script setup lang="ts">
import type { FormInstance } from 'element-plus'
import { ElMessage } from 'element-plus'
import { Delete, Plus } from '@element-plus/icons-vue'
import { getInstanceDetail } from '@/api/devicesManagement'
import type { DeviceInstanceDetail, AddDeviceInstanceDetail } from '@/types/deviceConfiguration'
import { createInstance, updateInstance } from '@/api/devicesManagement'
const props = defineProps<{
  productOptions: { label: string; value: string }[]
}>()
const formRef = ref<FormInstance>()
const dialogRef = ref<any>()

// 响应式数据
const isEditing = ref(false)
const currentInstanceId = ref<number | null>(null)
const isCreateMode = computed(() => isEditing.value && form.value.instance_id === null)

// 表单数据
const form = ref<DeviceInstanceDetail>({
  instance_id: 0,
  instance_name: '',
  product_name: '',
  properties: {},
})

// 编辑时的属性数据副本（数组形式，便于编辑 key 与 value）
const editProperties = ref<Array<{ key: string; value: string | number }>>([])

// 原始表单快照，用于取消编辑时恢复
const originalFormSnapshot = ref<DeviceInstanceDetail | null>(null)

// 表单验证规则
const rules = {
  instance_name: [
    { required: true, message: 'Please enter instance name', trigger: 'blur' },
    { min: 2, max: 32, message: 'Instance name length is 2-32 characters', trigger: 'blur' },
  ],
  // 产品不可编辑，校验保持必填但由初始数据提供
  product_name: [{ required: true, message: 'Please select product', trigger: 'change' }],
}

// 添加属性
const addProperty = (key: string, value: string | number) => {
  if (isEditing.value) {
    editProperties.value.push({ key, value })
  } else {
    form.value.properties[key] = value as string | number
  }
}

// 删除属性
const removeProperty = (key: string) => {
  if (isEditing.value) {
    const idx = editProperties.value.findIndex((p) => p.key === key)
    if (idx > -1) editProperties.value.splice(idx, 1)
  } else {
    delete form.value.properties[key]
  }
}

const removePropertyByIndex = (index: number) => {
  if (!isEditing.value) return
  editProperties.value.splice(index, 1)
}

// 打开对话框
const open = async (instanceIdOrNull: number | null) => {
  // 新建：传入空字符串时，清空表单并进入编辑状态
  try {
  if (!instanceIdOrNull) {
    isEditing.value = true
    form.value = {
      instance_id: null,
      instance_name: '',
      product_name: '',
      properties: {},
    } as any
      editProperties.value = []
      originalFormSnapshot.value = JSON.parse(JSON.stringify(form.value))
    } else {
      isEditing.value = false
      const detailData = await getInstanceDetail(instanceIdOrNull)

      // 填充表单数据
      form.value = {
        instance_id: detailData.data.instance.instance_id,
        instance_name: detailData.data.instance.instance_name,
        product_name: detailData.data.instance.product_name,
        properties: detailData.data.instance.properties,
      } as any

      // 同步原始快照
      originalFormSnapshot.value = JSON.parse(JSON.stringify(form.value))
    }

    nextTick(() => {
      if (dialogRef.value) {
        dialogRef.value.dialogVisible = true
        formRef.value?.clearValidate()
      }
    })
  } catch (error) {
    console.log(error)
  }
}

// 关闭对话框
const close = () => {
  if (dialogRef.value) {
    dialogRef.value.dialogVisible = false
  }
}

// 取消
const handleCancel = () => {
  // 重置编辑状态
  if (isEditing.value && !isCreateMode.value) {
    isEditing.value = false
    // 恢复原始表单
    if (originalFormSnapshot.value) {
      form.value = JSON.parse(JSON.stringify(originalFormSnapshot.value))
    }
    editProperties.value = []
  } else {
    close()
  }
}

// 编辑
const handleEdit = () => {
  // 创建编辑数据副本
  editProperties.value = Object.entries(form.value.properties).map(([k, v]) => ({
    key: k,
    value: v as string | number,
  }))
  // 生成原始快照（若尚未生成）
  if (!originalFormSnapshot.value) {
    originalFormSnapshot.value = JSON.parse(JSON.stringify(form.value))
  }
  isEditing.value = true
}

// 提交
const handleSubmit = () => {
  formRef.value?.validate(async (valid) => {
    if (valid) {
      // 将编辑数据同步到表单数据（数组转对象，忽略空 key）
      const newProps: Record<string, string | number> = {}
      for (const item of editProperties.value) {
        if (item.key !== '') newProps[item.key] = item.value
      }
      form.value.properties = newProps
      if (isCreateMode.value) {
        const data: AddDeviceInstanceDetail = {
          instance_name: form.value.instance_name,
          product_name: form.value.product_name,
          properties: form.value.properties,
        }
        const res = await createInstance(data)
        if (res.success) {
          ElMessage.success('Device instance created successfully')
          isEditing.value = false
          emit('submit')
          close()
        }
      } else {
        const res = await updateInstance(form.value)
        if (res.success) {
          ElMessage.success('Device instance updated successfully')
          isEditing.value = false
          emit('submit')
          close()
        }
      }

      // ElMessage.success('Device instance updated successfully')
      // // 更新原始快照
      // originalFormSnapshot.value = JSON.parse(JSON.stringify(form.value))
      // close()
    }
  })
}

// 关闭事件
const handleClose = () => {
  // 重置编辑状态
  if (isEditing.value) {
    isEditing.value = false
    editProperties.value = []
  }
  close()
}

// 定义事件
const emit = defineEmits<{
  (e: 'submit'): void
}>()

// 暴露方法
defineExpose({ open, close })
</script>

<style scoped lang="scss">
.voltage-class .instance-detail {
  //   max-height: 8rem;
  overflow-y: auto;

  .instance-detail__section-title {
    font-size: 0.16rem;
    font-weight: 600;
    color: #fff;
    margin-bottom: 0.1rem;
    // border-bottom: 0.01rem solid rgba(255, 255, 255, 0.1);
  }

  .instance-detail__form {
    .el-form-item {
      // margin-bottom: 0.15rem;
      // margin-right: 0.2rem;
      align-items: flex-start !important;
    }
  }

  .instance-detail__readonly-text {
    color: #fff;
    font-size: 0.14rem;
  }
  :deep(.el-form-item__content) {
    align-items: flex-start !important;
  }
  .instance-detail__properties {
    max-height: 3rem;
    overflow-y: auto;
    overflow-x: visible;
    // border: 0.01rem solid #dcdfe6;
    // border-radius: 0.04rem;
    // padding: 0.1rem;
    background-color: transparent;
    position: relative;
    padding: 0.05rem 0;

    .instance-detail__property-item {
      display: flex;
      flex-direction: row;
      align-items: center;
      gap: 0.05rem;
      // padding: 0.04rem;
      background-color: transparent;
      // border-radius: 0.03rem;
      border-top: 0.01rem solid #e4e7ed;
      min-height: 0.32rem;
      position: relative;
      padding: 0.05rem 0;

      &:last-child {
        // padding-bottom: 0.05rem;
        border-bottom: 0.01rem solid #e4e7ed;
      }
    }

    .instance-detail__property-content {
      display: flex;
      align-items: center;
      gap: 0.05rem;
      flex: 1;
    }

    .instance-detail__property-key-container {
      // flex: 0 0 1.2rem; // 固定宽度，较小
    }

    .instance-detail__property-value-container {
      // flex: 1; // 占据剩余空间
    }

    .instance-detail__property-key-input {
      width: 100%;
    }

    .instance-detail__property-value-input {
      width: 100%;
    }

    .instance-detail__property-key,
    .instance-detail__property-value {
      font-size: 0.14rem;
      color: #fff;
      word-break: break-all;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      display: block;
      min-width: 2.4rem;
    }

    .instance-detail__property-key {
      text-align: right;
    }

    .instance-detail__property-separator {
      color: #fff;
      font-weight: bold;
      margin: 0 0.02rem;
      flex-shrink: 0;
      font-size: 0.16rem;
    }
  }
}
</style>
