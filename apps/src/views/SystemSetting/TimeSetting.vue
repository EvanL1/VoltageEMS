<template>
  <div class="voltage-class time-setting">
    <ModuleCard title="Time Setting" :isShowFooter="true">
      <el-form
        ref="formRef"
        :model="formData"
        class="time-setting-form"
        label-width="1.35rem"
        label-position="left"
      >
        <el-form-item label="Time Zone" prop="timeZone">
          <div class="time-setting-form-item" ref="timeSettingFormItemRef">
            <el-select
              v-model="formData.timeZone"
              placeholder="Please select"
              :append-to="timeSettingFormItemRef"
            >
              <el-option label="GMT+8" value="GMT+8" />
              <el-option label="UTC-0" value="UTC-0" />
            </el-select>
          </div>
        </el-form-item>
        <el-form-item label="Synchronization" prop="Synchronization">
          <el-radio-group v-model="formData.synchronization">
            <el-radio :label="true">auto</el-radio>
            <el-radio :label="false">Manual</el-radio>
          </el-radio-group>
        </el-form-item>
      </el-form>
    </ModuleCard>
  </div>
</template>

<script setup lang="ts">
import type { FormInstance } from 'element-plus'

const formRef = ref<FormInstance>()
const formData = ref({
  timeZone: 'GMT+8',
  synchronization: false,
})
const timeSettingFormItemRef = ref<HTMLElement | null>(null)
</script>

<style scoped lang="scss">
.time-setting {
  width: 100%;
  height: 100%;

  .time-setting-form {
    width: 100%;
    height: 100%;
    padding: 20px 0;
    .time-setting-form-item {
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
