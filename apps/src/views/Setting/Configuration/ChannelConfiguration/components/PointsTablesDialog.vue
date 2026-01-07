<template>
  <FormDialog
    ref="dialogRef"
    :title="viewMode === 'points' ? 'Points Table' : 'Mappings Table'"
    width="14rem"
    :appendToBody="false"
    :before-close="handleDialogBeforeClose"
    @close="handleClose"
  >
    <template #dialog-body>
      <div class="voltage-class rule-management__expand-content">
        <!-- 导出按钮 - 所有表格最上方 -->
        <!-- <div v-if="!isEditing && !isPublish" class="top-actions">
          <el-button type="primary" @click="handleExport">Export</el-button>
        </div> -->

        <div class="rule-management__config-section">
          <div class="config-section__header">
            <!-- 视图模式切换器 - 移到tabs上方 -->
            <div v-if="!isEditing" class="config-section__controls">
              <div class="view-mode-switch">
                <span class="switch-label">View Mode:</span>
                <el-radio-group v-model="viewModeSwitch" class="el-radio-group--button">
                  <el-radio :label="false">Points</el-radio>
                  <el-radio :label="true">Mappings</el-radio>
                </el-radio-group>
              </div>
            </div>
            <div class="config-section__tabs-wrapper">
              <LoadingBg :loading="globalStore.loading">
                <el-tabs
                  v-model="activeTab"
                  type="card"
                  :before-leave="handleBeforeLeave"
                  @tab-change="handleTabChange"
                  class="config-section__tabs"
                >
                  <el-tab-pane
                    label="telemetry"
                    name="telemetry"
                    v-if="channelProtocol !== 'di_do'"
                  >
                    <template v-if="viewMode === 'points'">
                      <PointTablePoints
                        ref="telemetryTableRef"
                        pointType="T"
                        :points="pointsData.telemetry"
                        :original-points="originalPointsData.telemetry"
                        :view-mode="viewMode"
                        :edit-filters="editFilters"
                        :is-editing="isEditing"
                        :publish-mode="isPublish && activeTab === 'telemetry'"
                        :channelProtocol="channelProtocol"
                        @toggle-publish="togglePublishMode"
                        @enter-edit-mode="handleEdit"
                        @publish-change="
                          (dirty: boolean) => {
                            publishDirty = dirty
                          }
                        "
                      />
                    </template>
                    <template v-else>
                      <PointTableMappings
                        ref="telemetryTableRef"
                        pointType="T"
                        :points="pointsData.telemetry"
                        :original-points="originalPointsData.telemetry"
                        :view-mode="viewMode"
                        :edit-filters="editFilters"
                        :is-editing="isEditing"
                        :channelProtocol="channelProtocol"
                      />
                    </template>
                  </el-tab-pane>
                  <el-tab-pane label="signal" name="signal">
                    <template v-if="viewMode === 'points'">
                      <PointTablePoints
                        ref="signalTableRef"
                        pointType="S"
                        :points="pointsData.signal"
                        :original-points="originalPointsData.signal"
                        :view-mode="viewMode"
                        :edit-filters="editFilters"
                        :is-editing="isEditing"
                        :publish-mode="isPublish && activeTab === 'signal'"
                        :channelProtocol="channelProtocol"
                        @toggle-publish="togglePublishMode"
                        @enter-edit-mode="handleEdit"
                        @publish-change="
                          (dirty: boolean) => {
                            publishDirty = dirty
                          }
                        "
                      />
                    </template>
                    <template v-else>
                      <PointTableMappings
                        ref="signalTableRef"
                        pointType="S"
                        :points="pointsData.signal"
                        :original-points="originalPointsData.signal"
                        :view-mode="viewMode"
                        :edit-filters="editFilters"
                        :is-editing="isEditing"
                        :channelProtocol="channelProtocol"
                      />
                    </template>
                  </el-tab-pane>
                  <el-tab-pane label="control" name="control">
                    <template v-if="viewMode === 'points'">
                      <PointTablePoints
                        ref="controlTableRef"
                        pointType="C"
                        :points="pointsData.control"
                        :original-points="originalPointsData.control"
                        :view-mode="viewMode"
                        :edit-filters="editFilters"
                        :is-editing="isEditing"
                        :publish-mode="isPublish && activeTab === 'control'"
                        :channelProtocol="channelProtocol"
                        @toggle-publish="togglePublishMode"
                        @enter-edit-mode="handleEdit"
                        @publish-change="
                          (dirty: boolean) => {
                            publishDirty = dirty
                          }
                        "
                      />
                    </template>
                    <template v-else>
                      <PointTableMappings
                        ref="controlTableRef"
                        pointType="C"
                        :points="pointsData.control"
                        :original-points="originalPointsData.control"
                        :view-mode="viewMode"
                        :edit-filters="editFilters"
                        :is-editing="isEditing"
                        :channelProtocol="channelProtocol"
                      />
                    </template>
                  </el-tab-pane>
                  <el-tab-pane
                    label="adjustment"
                    name="adjustment"
                    v-if="channelProtocol !== 'di_do'"
                  >
                    <template v-if="viewMode === 'points'">
                      <PointTablePoints
                        ref="adjustmentTableRef"
                        pointType="A"
                        :points="pointsData.adjustment"
                        :original-points="originalPointsData.adjustment"
                        :view-mode="viewMode"
                        :edit-filters="editFilters"
                        :is-editing="isEditing"
                        :publish-mode="isPublish && activeTab === 'adjustment'"
                        :channelProtocol="channelProtocol"
                        @toggle-publish="togglePublishMode"
                        @enter-edit-mode="handleEdit"
                        @publish-change="
                          (dirty: boolean) => {
                            publishDirty = dirty
                          }
                        "
                      />
                    </template>
                    <template v-else>
                      <PointTableMappings
                        ref="adjustmentTableRef"
                        pointType="A"
                        :points="pointsData.adjustment"
                        :original-points="originalPointsData.adjustment"
                        :view-mode="viewMode"
                        :edit-filters="editFilters"
                        :is-editing="isEditing"
                        :channelProtocol="channelProtocol"
                      />
                    </template>
                  </el-tab-pane>
                </el-tabs>

                <!-- Status 筛选器 - 使用定位放在 tab 右侧，在 tab 下方线段的上方 -->
                <div v-if="isEditing" class="config-section__status-filter">
                  <el-checkbox-group
                    v-model="statusFilterValue"
                    @change="handleStatusFilterChange"
                    class="status-checkbox-group"
                  >
                    <el-checkbox
                      v-for="option in statusFilterOptions"
                      :key="option.value"
                      :label="option.value"
                    >
                      {{ option.label }}
                    </el-checkbox>
                  </el-checkbox-group>
                </div>
              </LoadingBg>
            </div>
          </div>
        </div>
      </div>
    </template>

    <template #dialog-footer>
      <div class="dialog-footer">
        <el-button v-if="!isPublish" @click="handleClose">
          {{ isEditing ? 'Cancel Edit' : 'Cancel' }}
        </el-button>
        <el-button v-if="!isEditing && !isPublish" type="primary" @click="handleEdit">
          Edit
        </el-button>
        <!-- <el-button v-if="isEditing && !isPublish" type="primary" @click="handleSubmit">
          Submit
        </el-button> -->

        <IconButton
          v-if="isEditing && !isPublish"
          type="primary"
          :icon="submitIcon"
          text="Submit"
          @click="handleSubmit"
        />
        <!-- <el-button v-if="!isEditing && isPublish" type="primary" @click="handleSubmitPublish">
          Submit Publish
        </el-button> -->
        <IconButton
          v-if="!isEditing && isPublish"
          type="primary"
          :icon="submitIcon"
          text="Submit Publish"
          @click="handleSubmitPublish"
        />
      </div>
    </template>
  </FormDialog>
</template>

<script setup lang="ts">
import { nextTick } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import FormDialog from '@/components/dialog/FormDialog.vue'
import PointTablePoints from './PointTablePoints.vue'
import PointTableMappings from './PointTableMappings.vue'
import LoadingBg from '@/components/common/LoadingBg.vue'
import tableSubmitIcon from '@/assets/icons/btn-submit.svg'
// @ts-ignore - SVG导入类型问题
const submitIcon: string = tableSubmitIcon
import type { PointInfoResponse } from '@/types/channelConfiguration'
import {
  getPointsTables,
  getChannelDetail,
  batchUpdateMappingPoint,
  publishPointValue,
  postPointsBatch,
} from '@/api/channelsManagement'
import type {
  BatchUpdateMappingPointRequest,
  BatchPointsChangeRequest,
} from '@/types/channelConfiguration'
import type { MappingCategoryResponse } from '@/types/channelConfiguration'
import { ChannelIdKey, ChannelNameKey, ChannelProtocolKey, OriginalPointsKey } from '@/utils/key'
import wsManager from '@/utils/websocket'
import type { DataType } from '@/types/websocket'
import { useGlobalStore } from '@/stores/global'
const globalStore = useGlobalStore()
// lodash-es 替换
const deepClone = <T,>(v: T): T => JSON.parse(JSON.stringify(v))
const uniqueArray = <T,>(arr: T[]): T[] => Array.from(new Set(arr))
// 内部状态
const dialogRef = ref()
const isEditing = ref(false)
const activeTab = ref('telemetry')
const channelId = ref(0)
const channelName = ref('')
const channelProtocol = ref<'modbus_tcp' | 'modbus_rtu' | 'virt' | 'can' | 'di_do'>('modbus_tcp')
const viewModeSwitch = ref(false) // false = points, true = mappings
const viewMode = computed(() => (viewModeSwitch.value ? 'mappings' : 'points'))
const editFilters = ref<string[]>([])
const loading = computed(() => globalStore.loading)
// Status 筛选器：使用 checkbox-group 但限制为单选
const statusFilterValue = ref<string[]>([])
// Status 筛选选项：根据 viewMode 显示不同选项
const statusFilterOptions = computed(() => {
  if (viewMode.value === 'points') {
    return [
      { label: 'modified', value: 'modified' },
      { label: 'added', value: 'added' },
      { label: 'deleted', value: 'deleted' },
      { label: 'invalid', value: 'invalid' },
    ]
  } else {
    return [
      { label: 'modified', value: 'modified' },
      { label: 'invalid', value: 'invalid' },
    ]
  }
})
// Status 筛选器变化处理：限制为单选
const handleStatusFilterChange = (values: string[]) => {
  // 限制为单选：如果选择了多个，只保留最后一个
  if (values.length > 1) {
    const lastValue = values[values.length - 1]
    statusFilterValue.value = [lastValue]
    editFilters.value = [lastValue]
  } else {
    editFilters.value = values
  }
}
// 监听 editFilters 变化，同步到 statusFilterValue（用于外部设置时同步）
watch(
  () => editFilters.value,
  (val) => {
    if (Array.isArray(val) && val.length > 0) {
      // 如果 editFilters 有值，同步到 statusFilterValue
      const currentValue = statusFilterValue.value
      if (currentValue.length === 0 || currentValue[0] !== val[0]) {
        statusFilterValue.value = [val[0]]
      }
    } else {
      // 如果 editFilters 为空，清空 statusFilterValue
      statusFilterValue.value = []
    }
  },
  { immediate: true },
)
const showSignalNameFilter = ref(false)
const isPublish = ref(false) // 批量发布模式
const publishDirty = ref(false) // 发布数据是否有修改
const mappingTypeToTabName: Record<string, 'T' | 'S' | 'C' | 'A'> = {
  telemetry: 'T',
  signal: 'S',
  control: 'C',
  adjustment: 'A',
}
// 反向映射：数据类型 -> 表引用
const dataTypeToRef: Record<DataType, any> = {
  T: null as any,
  S: null as any,
  C: null as any,
  A: null as any,
}
// 点位数据存储
const pointsData = ref<PointInfoResponse>({
  telemetry: [],
  signal: [],
  control: [],
  adjustment: [],
})

// 原始点位数据（用于对比变化）
const originalPointsData = ref<PointInfoResponse>({
  telemetry: [],
  signal: [],
  control: [],
  adjustment: [],
})

// Mappings 数据存储
const mappingsData = ref<MappingCategoryResponse>({
  telemetry: [],
  signal: [],
  control: [],
  adjustment: [],
})

// 编辑状态管理方法
const telemetryTableRef = ref()
const signalTableRef = ref()
const controlTableRef = ref()
const adjustmentTableRef = ref()

// 页面订阅ID
const pageId = ref<string>('')

provide(ChannelIdKey, readonly(channelId))
provide(ChannelNameKey, readonly(channelName))
provide(ChannelProtocolKey, readonly(channelProtocol))

// 提供原始数据给子组件用于对比
provide(
  OriginalPointsKey,
  computed(() => {
    const category = activeTab.value as 'telemetry' | 'signal' | 'control' | 'adjustment'
    return originalPointsData.value[category]
  }),
)

const handleEdit = (payload?: { fromImport?: boolean }) => {
  isEditing.value = true
  editFilters.value = []
  statusFilterValue.value = []
  // 用户主动进入编辑时，清除四个表中上一次的导入文件名；
  // 若由子表导入触发（fromImport），则保留当前导入文件名。
  if (!payload?.fromImport) {
    telemetryTableRef.value?.clearImportedFileName?.()
    signalTableRef.value?.clearImportedFileName?.()
    controlTableRef.value?.clearImportedFileName?.()
    adjustmentTableRef.value?.clearImportedFileName?.()
  }
}

// 统一刷新点位数据并设置对比基线
const refreshPointsBaseline = async () => {
  try {
    const res = await getPointsTables(channelId.value)
    if (res.success) {
      pointsData.value = res.data
      originalPointsData.value = deepClone(res.data)
    }
  } catch {
    console.error('Failed to refresh points baseline')
  }
}

const handleSubmit = async () => {
  // 编辑提交前：检查四个Tab是否存在 invalid
  const ensureInvalidHandled = (targetTab: 'telemetry' | 'signal' | 'control' | 'adjustment') => {
    // 勾选 invalid 筛选
    if (!editFilters.value.includes('invalid')) {
      editFilters.value = uniqueArray([...(editFilters.value || []), 'invalid'])
      statusFilterValue.value = ['invalid']
    }
    // 切换到有问题的 Tab
    activeTab.value = targetTab
  }

  if (viewMode.value === 'mappings') {
    const invalidTabs: Array<'telemetry' | 'signal' | 'control' | 'adjustment'> = []
    if (telemetryTableRef.value?.hasInvalid?.()) invalidTabs.push('telemetry')
    if (signalTableRef.value?.hasInvalid?.()) invalidTabs.push('signal')
    if (controlTableRef.value?.hasInvalid?.()) invalidTabs.push('control')
    if (adjustmentTableRef.value?.hasInvalid?.()) invalidTabs.push('adjustment')
    if (invalidTabs.length > 0) {
      ElMessage.warning('Mappings Table has invalid data, please correct and submit again')
      ensureInvalidHandled(invalidTabs[0])
      return
    }

    const param: BatchUpdateMappingPointRequest = {
      mappings: [
        ...(telemetryTableRef.value?.getEditedData?.() || []),
        ...(signalTableRef.value?.getEditedData?.() || []),
        ...(controlTableRef.value?.getEditedData?.() || []),
        ...(adjustmentTableRef.value?.getEditedData?.() || []),
      ],
      mode: 'merge',
      reload_channel: false,
      validate_only: false,
    }

    if (!param.mappings.length) {
      ElMessage.info('No mapping changes to submit')
      return
    }

    const res = await batchUpdateMappingPoint(channelId.value, param)
    if (res.success) {
      ElMessage.success('Point mapping updated successfully')
      isEditing.value = false
      // 刷新 points 数据，作为新的原始对比基线
      await refreshPointsBaseline()
      // 提交完成后清空筛选并显示全部
      clearStatusFilters()
      statusFilterValue.value = []
    }
  } else {
    const invalidTabs: Array<'telemetry' | 'signal' | 'control' | 'adjustment'> = []
    if (telemetryTableRef.value?.hasInvalid?.()) invalidTabs.push('telemetry')
    if (signalTableRef.value?.hasInvalid?.()) invalidTabs.push('signal')
    if (controlTableRef.value?.hasInvalid?.()) invalidTabs.push('control')
    if (adjustmentTableRef.value?.hasInvalid?.()) invalidTabs.push('adjustment')
    if (invalidTabs.length > 0) {
      ElMessage.warning('Points Table has invalid data, please correct and submit again')
      ensureInvalidHandled(invalidTabs[0])
      return
    }

    // 组装批量增删改 payload
    const toArray = (x: any) => (Array.isArray(x) ? x : [])
    const tRows = toArray(telemetryTableRef.value?.getEditedData?.())
    const sRows = toArray(signalTableRef.value?.getEditedData?.())
    const cRows = toArray(controlTableRef.value?.getEditedData?.())
    const aRows = toArray(adjustmentTableRef.value?.getEditedData?.())

    const pickData = (row: any, fields: string[]) => {
      const out: any = {}
      fields.forEach((f) => {
        if (row[f] !== undefined) out[f] = row[f]
      })
      return out
    }
    const BASE_FIELDS = [
      'data_type',
      'description',
      'offset',
      'reverse',
      'scale',
      'signal_name',
      'unit',
    ]
    const buildCreate = (rows: any[], point_type: 'T' | 'S' | 'C' | 'A') =>
      rows
        .filter((r) => r && r.rowStatus === 'added' && r.point_id > 0)
        .map((r) => ({
          point_id: r.point_id,
          point_type,
          data: pickData(r, BASE_FIELDS),
          // 文件导入得到的新增行，强制覆盖
          ...(r?.isImported ? { force: true } : {}),
        }))
    const buildDelete = (rows: any[], point_type: 'T' | 'S' | 'C' | 'A') =>
      rows
        .filter((r) => r && r.rowStatus === 'deleted' && r.point_id > 0)
        .map((r) => ({
          point_id: r.point_id,
          point_type,
        }))
    const buildUpdate = (rows: any[], point_type: 'T' | 'S' | 'C' | 'A') =>
      rows
        .filter((r) => r && r.rowStatus === 'modified' && r.point_id > 0)
        .map((r) => {
          const fields = Array.isArray(r.modifiedFields) ? r.modifiedFields : []
          const data = pickData(
            r,
            fields.filter((f: string) => f !== 'point_id'),
          )
          return { point_id: r.point_id, point_type, data }
        })

    const payload: BatchPointsChangeRequest = {
      create: [
        ...buildCreate(tRows, 'T'),
        ...buildCreate(sRows, 'S'),
        ...buildCreate(cRows, 'C'),
        ...buildCreate(aRows, 'A'),
      ],
      delete: [
        ...buildDelete(tRows, 'T'),
        ...buildDelete(sRows, 'S'),
        ...buildDelete(cRows, 'C'),
        ...buildDelete(aRows, 'A'),
      ],
      update: [
        ...buildUpdate(tRows, 'T'),
        ...buildUpdate(sRows, 'S'),
        ...buildUpdate(cRows, 'C'),
        ...buildUpdate(aRows, 'A'),
      ],
    }

    // 如果三类均为空，则提示无变化
    const totalCount =
      (payload.create?.length || 0) + (payload.delete?.length || 0) + (payload.update?.length || 0)
    if (totalCount === 0) {
      ElMessage.info('No point changes to submit')
      return
    }

    const res = await postPointsBatch(channelId.value, payload)
    if (res.success) {
      ElMessage.success('Points updated successfully')
      isEditing.value = false
      await refreshPointsBaseline()
      // 提交完成后清空筛选并显示全部
      clearStatusFilters()
      statusFilterValue.value = []
    }
  }
}

const handleTabChange = (name: string) => {
  // Tab 切换时重置筛选
  if (showSignalNameFilter.value) {
    showSignalNameFilter.value = false
  }

  // 切换完成后，根据新的表刷新 publishDirty（保持批量发布模式不变）
  if (isPublish.value) {
    if (name === 'control') {
      publishDirty.value = !!controlTableRef.value?.hasPublishChanges?.()
    } else if (name === 'adjustment') {
      publishDirty.value = !!adjustmentTableRef.value?.hasPublishChanges?.()
    } else if (name === 'telemetry') {
      publishDirty.value = !!telemetryTableRef.value?.hasPublishChanges?.()
    } else if (name === 'signal') {
      publishDirty.value = !!signalTableRef.value?.hasPublishChanges?.()
    }
  }
  // 切换 Tab 后滚动到顶
  nextTick(() => {
    const ref = getCurrentTableRef()
    ref?.value?.scrollToTop?.()
  })
}

// Tabs 切换拦截：批量发布中如有修改则提示；确认后切换并退出批量发布，取消则停留
const handleBeforeLeave = async (newName: string, oldName: string) => {
  if (!isPublish.value) return true

  if (publishDirty.value) {
    try {
      await ElMessageBox.confirm(
        'You have unsaved publish values. Do you want to switch tabs?',
        'Confirm Switch',
        {
          confirmButtonText: 'Switch',
          cancelButtonText: 'Stay',
          type: 'warning',
        },
      )
      // 用户确认切换：清空当前 tab 的发布值并退出批量发布
      if (activeTab.value === 'control') {
        controlTableRef.value?.resetPublish?.()
      } else if (activeTab.value === 'adjustment') {
        adjustmentTableRef.value?.resetPublish?.()
      } else if (activeTab.value === 'telemetry') {
        telemetryTableRef.value?.resetPublish?.()
      } else if (activeTab.value === 'signal') {
        signalTableRef.value?.resetPublish?.()
      }
      publishDirty.value = false
      isPublish.value = false
      return true
    } catch {
      // 取消切换，保持原 Tab
      return false
    }
  }

  // 无修改：直接退出批量发布并允许切换
  isPublish.value = false
  return true
}

// Dialog 关闭前拦截：发布模式且有修改时提醒用户提交或放弃，或编辑模式有修改时提醒
const handleDialogBeforeClose = async (done: () => void) => {
  // 如果正在编辑，检查是否有修改
  if (isEditing.value) {
    const hasChanges =
      (viewMode.value === 'points' &&
        (telemetryTableRef.value?.hasChanges?.() ||
          signalTableRef.value?.hasChanges?.() ||
          controlTableRef.value?.hasChanges?.() ||
          adjustmentTableRef.value?.hasChanges?.())) ||
      (viewMode.value === 'mappings' &&
        (telemetryTableRef.value?.hasChanges?.() ||
          signalTableRef.value?.hasChanges?.() ||
          controlTableRef.value?.hasChanges?.() ||
          adjustmentTableRef.value?.hasChanges?.()))
    if (hasChanges) {
      try {
        await ElMessageBox.confirm(
          'You have unsaved changes. Do you want to discard them?',
          'Unsaved Changes',
          {
            confirmButtonText: 'Discard',
            cancelButtonText: 'Cancel',
            type: 'warning',
          },
        )
        isEditing.value = false
        editFilters.value = []
        statusFilterValue.value = []
        telemetryTableRef.value?.clearImportedFileName?.()
        signalTableRef.value?.clearImportedFileName?.()
        controlTableRef.value?.clearImportedFileName?.()
        adjustmentTableRef.value?.clearImportedFileName?.()
        clearAllSignalFilters()
        done()
      } catch {
        // 用户取消，不做任何操作
        return
      }
    } else {
      isEditing.value = false
      editFilters.value = []
      statusFilterValue.value = []
      telemetryTableRef.value?.clearImportedFileName?.()
      signalTableRef.value?.clearImportedFileName?.()
      controlTableRef.value?.clearImportedFileName?.()
      adjustmentTableRef.value?.clearImportedFileName?.()
      clearAllSignalFilters()
      done()
    }
    return
  }

  if (isPublish.value && publishDirty.value) {
    try {
      await ElMessageBox.confirm(
        'You have unsaved publish values. Submit them before closing?',
        'Unsaved Changes',
        {
          confirmButtonText: 'Submit',
          cancelButtonText: 'Discard',
          type: 'warning',
        },
      )
      // 用户选择提交
      await handleSubmitPublish()
      clearAllSignalFilters()
      done()
    } catch {
      // 用户选择放弃
      controlTableRef.value?.resetPublish?.()
      adjustmentTableRef.value?.resetPublish?.()
      telemetryTableRef.value?.resetPublish?.()
      signalTableRef.value?.resetPublish?.()
      publishDirty.value = false
      isPublish.value = false
      clearAllSignalFilters()
      done()
    }
  } else {
    // 无需拦截，正常关闭
    if (isPublish.value && !publishDirty.value) {
      isPublish.value = false
    }
    clearAllSignalFilters()
    done()
  }
}

// 切换发布模式
const togglePublishMode = async () => {
  if (isPublish.value) {
    if (publishDirty.value) {
      try {
        await ElMessageBox.confirm(
          'You have unsaved publish values. Do you want to discard them?',
          'Unsaved Changes',
          {
            confirmButtonText: 'Discard',
            cancelButtonText: 'Cancel',
            type: 'warning',
          },
        )
        // 用户确认放弃
        isPublish.value = false
        publishDirty.value = false
        controlTableRef.value?.resetPublish?.()
        adjustmentTableRef.value?.resetPublish?.()
        telemetryTableRef.value?.resetPublish?.()
        signalTableRef.value?.resetPublish?.()
      } catch {
        // 用户取消，不做任何操作
        return
      }
    } else {
      isPublish.value = false
      publishDirty.value = false
      controlTableRef.value?.resetPublish?.()
      adjustmentTableRef.value?.resetPublish?.()
      telemetryTableRef.value?.resetPublish?.()
      signalTableRef.value?.resetPublish?.()
    }
  } else {
    // 开启发布模式：清空当前 tab 的发布值
    isPublish.value = true
    if (activeTab.value === 'control') {
      controlTableRef.value?.resetPublish?.()
    } else if (activeTab.value === 'adjustment') {
      adjustmentTableRef.value?.resetPublish?.()
    } else if (activeTab.value === 'telemetry') {
      telemetryTableRef.value?.resetPublish?.()
    } else if (activeTab.value === 'signal') {
      signalTableRef.value?.resetPublish?.()
    }
    publishDirty.value = false
  }
}

// 提交发布
const handleSubmitPublish = async () => {
  let commands = []
  const activeRef = getCurrentTableRef()
  const activeInstance = activeRef?.value
  commands = activeInstance?.getPublishCommands?.()

  if (!Array.isArray(commands) || commands.length === 0) return
  const res = await publishPointValue(channelId.value, {
    type: mappingTypeToTabName[activeTab.value],
    points: commands,
  })
  if (res.success) {
    publishDirty.value = false
    isPublish.value = false
    activeInstance?.resetPublish?.()
    ElMessage.success('Batch publish successful')
  }

  // if (activeTab.value === 'control' || activeTab.value === 'signal') {
  //   const res = await postControlBatch(channelId.value, commands)
  //   if (res.success) {
  //     publishDirty.value = false
  //     isPublish.value = false
  //     controlTableRef.value?.resetPublish()
  //     ElMessage.success('Control batch publish successful')
  //   }
  // } else if (activeTab.value === 'adjustment' || activeTab.value === 'telemetry') {
  //   const res = await postAdjustmentBatch(channelId.value, commands)
  //   if (res.success) {
  //     publishDirty.value = false
  //     isPublish.value = false
  //     adjustmentTableRef.value?.resetPublish()
  //     ElMessage.success('Adjustment batch publish successful')
  //   }
  // }
}

// // 导出功能（暂时不做操作）
// const handleExport = () => {
//   ElMessage.info('Export function will be implemented soon')
// }

// // 添加点位方法
// const handleAddPoint = () => {
//   const newId = Date.now()
//   const newPoint: PointInfo = {
//     point_id: newId,
//     signal_name: '',
//     scale: 1,
//     offset: 0,
//     unit: '',
//     data_type: 'float',
//     reverse: false,
//     description: '',
//   }

//   const currentRef = getCurrentTableRef()
//   if (currentRef?.addPoint) {
//     currentRef.addPoint(newPoint)
//     ElMessage.success('Point added successfully')
//   }
// }

// 根据激活 tab 获取对应表格实例
const getCurrentTableRef = () => {
  switch (activeTab.value) {
    case 'telemetry':
      return telemetryTableRef
    case 'signal':
      return signalTableRef
    case 'control':
      return controlTableRef
    case 'adjustment':
      return adjustmentTableRef
    default:
      return null
  }
}

// 对话框控制方法
const open = async (
  id: number,
  name?: string,
  protocol?: 'modbus_tcp' | 'modbus_rtu' | 'virt' | 'can' | 'di_do',
) => {
  // 如已有订阅，先取消
  if (pageId.value) {
    try {
      wsManager.unsubscribe(pageId.value)
    } catch {}
    pageId.value = ''
  }
  isEditing.value = false
  isPublish.value = false
  publishDirty.value = false
  viewModeSwitch.value = false
  editFilters.value = []
  statusFilterValue.value = []
  channelId.value = id
  if (name) channelName.value = name
  if (protocol) channelProtocol.value = protocol
  dialogRef.value.dialogVisible = true
  activeTab.value = protocol === 'di_do' ? 'signal' : 'telemetry'
  // 加载 points 数据
  await refreshPointsBaseline()
  // 打开后将当前表滚动到顶部
  nextTick(() => {
    const ref = getCurrentTableRef()
    ref?.value?.scrollToTop?.()
  })

  // 准备 dataType -> 表引用映射
  dataTypeToRef.T = telemetryTableRef
  dataTypeToRef.S = signalTableRef
  dataTypeToRef.C = controlTableRef
  dataTypeToRef.A = adjustmentTableRef

  // 建立WebSocket连接并订阅当前通道的四类数据
  // try {
  //   // await wsManager.connect()
  // } catch (e) {
  //   // 忽略连接失败（可能未登录），不阻塞UI
  // }
  pageId.value = `points-${id}-${Date.now()}`
  wsManager.subscribe(
    {
      source: 'comsrv',
      channels: [id],
      dataTypes: ['T', 'S', 'C', 'A'],
      interval: 1000,
    },
    {
      onBatchDataUpdate: (payload: any) => {
        if (!payload?.updates?.length) return
        payload.updates.forEach((upd: any) => {
          if (upd.channel_id !== channelId.value) return
          const refMap = dataTypeToRef[upd.data_type as DataType]
          refMap?.value?.applyRealtimeValues?.(upd.values)
        })
      },
    },
  )

  // 加载 mappings 数据
  // const mappingsRes = await getChannelMappings(id)
  // if (mappingsRes.success) {
  //   mappingsData.value = mappingsRes.data
  //   // 合并 mappings 数据到 points 数据中
  //   mergeMappingsToPoints()
  //   // 也合并到原始数据中
  //   const categories: Array<'telemetry' | 'signal' | 'control' | 'adjustment'> = [
  //     'telemetry',
  //     'signal',
  //     'control',
  //     'adjustment',
  //   ]
  //   categories.forEach((category) => {
  //     const points = originalPointsData.value[category]
  //     const mappings = mappingsData.value[category]
  //     points.forEach((point) => {
  //       const mapping = mappings.find((m) => m.point_id === point.point_id)
  //       if (mapping) {
  //         point.protocol_mapping = JSON.parse(JSON.stringify(mapping.protocol_data))
  //       }
  //     })
  //   })
  // }

  if (!name) {
    const detail = await getChannelDetail(id)
    if (detail.success) {
      channelName.value = detail.data.name || ''
    }
  }
}

// 合并 mappings 数据到 points 数据
const mergeMappingsToPoints = () => {
  const categories: Array<'telemetry' | 'signal' | 'control' | 'adjustment'> = [
    'telemetry',
    'signal',
    'control',
    'adjustment',
  ]

  categories.forEach((category) => {
    const points = pointsData.value[category]
    const mappings = mappingsData.value[category]

    // 为每个 point 查找对应的 mapping
    points.forEach((point) => {
      const mapping = mappings.find((m) => m.point_id === point.point_id)
      if (mapping) {
        point.protocol_mapping = mapping.protocol_data as any
      }
    })
  })
}

const close = () => {
  dialogRef.value.dialogVisible = false
  // 关闭时取消页面订阅
  if (pageId.value) {
    try {
      wsManager.unsubscribe(pageId.value)
    } catch {}
    pageId.value = ''
  }
}

// 统一清理：关闭前清理所有表的 Signal Name 筛选
const clearAllSignalFilters = () => {
  telemetryTableRef.value?.clearSignalNameFilter?.()
  signalTableRef.value?.clearSignalNameFilter?.()
  controlTableRef.value?.clearSignalNameFilter?.()
  adjustmentTableRef.value?.clearSignalNameFilter?.()
}

// 统一清空状态筛选并显示全部
const clearStatusFilters = () => {
  editFilters.value = []
  statusFilterValue.value = []
  clearAllSignalFilters()
}

const handleClose = async () => {
  // 如果正在编辑，检查是否有修改
  if (isEditing.value) {
    const hasChanges =
      (viewMode.value === 'points' &&
        (telemetryTableRef.value?.hasChanges?.() ||
          signalTableRef.value?.hasChanges?.() ||
          controlTableRef.value?.hasChanges?.() ||
          adjustmentTableRef.value?.hasChanges?.())) ||
      (viewMode.value === 'mappings' &&
        (telemetryTableRef.value?.hasChanges?.() ||
          signalTableRef.value?.hasChanges?.() ||
          controlTableRef.value?.hasChanges?.() ||
          adjustmentTableRef.value?.hasChanges?.()))
    if (hasChanges) {
      try {
        await ElMessageBox.confirm(
          'You have unsaved changes. Do you want to discard them?',
          'Unsaved Changes',
          {
            confirmButtonText: 'Discard',
            cancelButtonText: 'Cancel',
            type: 'warning',
          },
        )
        isEditing.value = false
        // 退出编辑时，清空编辑筛选（防止仍勾选 invalid 导致只显示错误或无数据）
        editFilters.value = []
        statusFilterValue.value = []
        // 清除所有表格的导入文件名
        telemetryTableRef.value?.clearImportedFileName?.()
        signalTableRef.value?.clearImportedFileName?.()
        controlTableRef.value?.clearImportedFileName?.()
        adjustmentTableRef.value?.clearImportedFileName?.()
      } catch {
        // 用户取消，不做任何操作
        return
      }
    } else {
      isEditing.value = false
      // 退出编辑时，清空编辑筛选（防止仍勾选 invalid 导致只显示错误或无数据）
      editFilters.value = []
      statusFilterValue.value = []
      // 清除所有表格的导入文件名
      telemetryTableRef.value?.clearImportedFileName?.()
      signalTableRef.value?.clearImportedFileName?.()
      controlTableRef.value?.clearImportedFileName?.()
      adjustmentTableRef.value?.clearImportedFileName?.()
    }
    return
  }

  // 如果处于发布模式且有未提交的值，提醒用户
  if (isPublish.value && publishDirty.value) {
    try {
      await ElMessageBox.confirm(
        'You have unsaved publish values. Do you want to submit them before closing?',
        'Unsaved Changes',
        {
          confirmButtonText: 'Submit',
          cancelButtonText: 'Discard',
          type: 'warning',
        },
      )
      // 用户选择提交
      await handleSubmitPublish()
      clearAllSignalFilters()
      close()
    } catch {
      // 用户选择放弃或取消
      controlTableRef.value?.resetPublish?.()
      adjustmentTableRef.value?.resetPublish?.()
      publishDirty.value = false
      isPublish.value = false
      clearAllSignalFilters()
      close()
    }
  } else {
    clearAllSignalFilters()
    close()
  }
}

// 暴露方法
defineExpose({
  open,
  close,
})
</script>

<style scoped lang="scss">
.voltage-class .rule-management__expand-content {
  .top-actions {
    display: flex;
    justify-content: flex-end;
    margin-bottom: 0.15rem;
  }
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.1rem;
}

.voltage-class .rule-management__config-section {
  .config-section__header {
    position: relative;

    .config-section__controls {
      display: flex;
      align-items: center;
      justify-content: flex-end;
      gap: 0.15rem;
      margin-bottom: 0.1rem;

      .view-mode-switch {
        display: flex;
        align-items: center;
        gap: 0.08rem;

        .switch-label {
          font-size: 0.18rem;
          color: #fff;
        }
      }

      .edit-filters {
        :deep(.el-checkbox-group) {
          display: flex;
          gap: 0.08rem;
        }

        :deep(.el-checkbox) {
          color: #fff;

          .el-checkbox__label {
            font-size: 0.12rem;
          }
        }
      }
    }

    .config-section__tabs-wrapper {
      position: relative;
      min-height: 5rem;

      .config-section__tabs {
        width: 100%;

        // 确保 tab header 有相对定位，以便 Status 筛选器可以相对于它定位
        :deep(.el-tabs__header) {
          position: relative;
        }
      }

      .config-section__status-filter {
        position: absolute;
        top: 0;
        right: 0;
        display: flex;
        align-items: center;
        gap: 0.08rem;
        z-index: 10;
        // 与 tab 标签对齐（tab 标签的高度通常是 0.4rem 左右）
        height: 0.4rem;
        line-height: 0.4rem;

        .filter-label {
          font-size: 0.14rem;
          color: #fff;
          white-space: nowrap;
        }

        .status-checkbox-group {
          display: flex;
          gap: 0.12rem;
          flex-wrap: wrap;
          align-items: center;
        }
      }
    }
  }
}

:deep(.el-tabs__content) {
  position: static;
}

/* 通用：操作列中的错误提示图标样式（编辑态下无效行） */
.voltage-class .point-table {
  .point-table__operation-cell {
    .point-table__error-tip {
      color: #f56c6c;
      font-size: 0.18rem;
      margin-right: 0.04rem;
    }
  }
}
</style>
