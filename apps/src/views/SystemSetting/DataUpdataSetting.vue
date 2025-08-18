<template>
  <div class="voltage-class time-setting">
    <ModuleCard title="Data Upload" :isShowFooter="true">
      <el-form
        ref="formRef"
        :model="formData"
        class="data-updata-form"
        label-width="1.35rem"
        label-position="left"
      >
        <el-form-item label="Upload Enable" prop="Enable">
          <el-switch v-model="formData.Enable" />
        </el-form-item>
        <el-form-item label="Upload Interval" prop="Interval">
          <div class="data-updata-form-item" ref="intervalRef">
            <el-select v-model="formData.Interval" :append-to="intervalRef">
              <el-option label="5MIN" :value="5" />
              <el-option label="10MIN" :value="10" />
              <el-option label="30MIN" :value="30" />
              <el-option label="60MIN" :value="60" />
            </el-select>
          </div>
        </el-form-item>

        <el-form-item label="Upload address" prop="address">
          <div class="data-updata-form-item" ref="addressRef">
            <el-select v-model="formData.address" :append-to="addressRef">
              <el-option label="5MIN" :value="5" />
              <el-option label="10MIN" :value="10" />
              <el-option label="30MIN" :value="30" />
              <el-option label="60MIN" :value="60" />
            </el-select>
          </div>
        </el-form-item>
      </el-form>
    </ModuleCard>
  </div>
</template>

<script setup lang="ts">
import type { FormInstance } from 'element-plus'

const formRef = ref<FormInstance>()
const intervalRef = ref<HTMLElement | null>(null)
const addressRef = ref<HTMLElement | null>(null)
const formData = ref({
  Enable: false,
  Interval: 5,
  address: '192.168.1.100',
})
</script>

<style scoped lang="scss">
.time-setting {
  width: 100%;
  height: 100%;

  .data-updata-form {
    width: 100%;
    height: 100%;
    padding: 20px 0;
    .data-updata-form-item {
      width: 100%;
      height: 32px;
      display: flex;
      align-items: center;
      justify-content: center;
    }
    :deep(.el-select) {
      width: 100% !important;
    }
    :deep(.el-select__popper.el-popper) {
      top: 44px !important;
    }
  }
}
</style>
