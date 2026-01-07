<template>
  <div class="voltage-class point-table">
    <!-- 按钮控制区域 -->
    <div v-if="props.viewMode === 'points'" class="table-action-controls">
      <div
        class="table-action-controls__filters"
        style="flex: 1; display: flex; gap: 0.08rem; align-items: center"
      >
        <span class="filter-label">Point Name:</span>
        <el-select
          v-model="signalNameFilterRaw"
          filterable
          allow-create
          clearable
          placeholder="Search Point Name"
          :teleported="false"
          popper-class="signal-name-popper"
          style="width: 2.8rem"
          :fit-input-width="true"
        >
          <el-option v-for="name in signalNameOptions" :key="name" :label="name" :value="name" />
        </el-select>
      </div>
      <!-- 非编辑模式：显示Batch Publish和Export -->
      <template v-if="!props.isEditing">
        <el-button :type="props.publishMode ? 'warning' : 'primary'" @click="handleTogglePublish">
          {{ props.publishMode ? 'Cancel Publish' : 'Batch Publish' }}
        </el-button>
        <el-button v-if="!props.publishMode" type="primary" @click="handleExport">
          Export
        </el-button>
      </template>

      <!-- 编辑模式：显示文件名和Import -->
      <template v-else>
        <span v-if="importedFileName" class="imported-file-name">{{ importedFileName }}</span>
        <el-button type="primary" @mousedown="handleImportClick">Import</el-button>
      </template>
    </div>

    <!-- 隐藏的文件输入 -->
    <input
      ref="fileInputRef"
      type="file"
      accept=".csv"
      style="display: none"
      @change="handleFileChange"
    />

    <div class="vtable" style="height: 5rem">
      <!-- Points 表头 -->
      <div class="vtable__header">
        <div class="vtable__cell vtable__cell--point-id">Point ID</div>
        <div
          class="vtable__cell vtable__cell--signal-name"
          :class="{
            CorS: props.pointType === 'C' || props.pointType === 'S',
            isEditing: props.isEditing,
          }"
        >
          <span>Point Name</span>
        </div>
        <template v-if="!props.isEditing">
          <div
            class="vtable__cell vtable__cell--value"
            :class="props.pointType === 'C' || props.pointType === 'S' ? 'CorS' : ''"
          >
            Value
          </div>
        </template>
        <template v-if="isTA">
          <div class="vtable__cell vtable__cell--scale">Scale</div>
          <div class="vtable__cell vtable__cell--offset">Offset</div>
          <div class="vtable__cell vtable__cell--unit">Unit</div>
        </template>
        <div
          class="vtable__cell vtable__cell--reverse"
          :class="props.pointType === 'C' || props.pointType === 'S' ? 'CorS' : ''"
        >
          Reverse
        </div>
        <div v-if="!props.publishMode" class="vtable__cell vtable__cell--operation">
          <el-icon
            v-if="props.isEditing"
            style="cursor: pointer; color: #67c23a"
            @click="handleAddNewPoint"
          >
            <Plus />
          </el-icon>
          <span v-else>Operation</span>
        </div>
        <div v-else class="vtable__cell vtable__cell--publish-value">Publish Value</div>
      </div>

      <div v-if="filteredPoints.length === 0" class="vtable__empty">no Data</div>
      <DynamicScroller
        v-else
        ref="scrollerRef"
        class="vtable__body"
        :items="filteredPoints"
        :min-item-size="rowHeight"
        key-field="rowKey"
        :buffer="4"
        :prerender="8"
      >
        <template #default="{ item, index }">
          <DynamicScrollerItem :item="item" :index="index" :active="true">
            <div class="vtable__row" :class="getRowClass(item)">
              <div class="row-status-float"></div>
              <div class="vtable__cell vtable__cell--point-id">
                <template
                  v-if="
                    item.isEditing &&
                    (item.rowStatus === 'added' || item.isNewUnconfirmed || item.isImported)
                  "
                >
                  <div class="inline-edit-container">
                    <el-input-number
                      v-model="item.point_id"
                      :min="1"
                      @change="() => onFieldInput(item, 'point_id')"
                      :precision="0"
                      :controls="false"
                      align="left"
                      style="width: 100% !important"
                    />
                  </div>
                </template>
                <template v-else>
                  <span :class="getFieldClass(item, 'point_id')">{{ item.point_id }}</span>
                </template>
                <div
                  v-if="
                    props.isEditing &&
                    getFieldError(item, 'point_id') &&
                    item.rowStatus !== 'deleted'
                  "
                  class="field-error"
                >
                  {{ getFieldError(item, 'point_id') }}
                </div>
              </div>
              <div
                class="vtable__cell vtable__cell--signal-name"
                :class="{
                  CorS: props.pointType === 'C' || props.pointType === 'S',
                  isEditing: props.isEditing,
                }"
              >
                <template v-if="item.isEditing">
                  <div class="inline-edit-container">
                    <el-input
                      v-model="item.signal_name"
                      placeholder="Enter signal name"
                      @input="() => onFieldInput(item, 'signal_name')"
                      style="width: 100% !important"
                    />
                  </div>
                </template>
                <template v-else>
                  <span :class="getFieldClass(item, 'signal_name')">{{ item.signal_name }}</span>
                </template>
                <div
                  v-if="
                    props.isEditing &&
                    getFieldError(item, 'signal_name') &&
                    item.rowStatus !== 'deleted'
                  "
                  class="field-error"
                >
                  {{ getFieldError(item, 'signal_name') }}
                </div>
              </div>
              <template v-if="!props.isEditing">
                <div
                  class="vtable__cell vtable__cell--value"
                  :class="props.pointType === 'C' || props.pointType === 'S' ? 'CorS' : ''"
                >
                  <span class="value-field">{{ item.value ?? '-' }}</span>
                </div>
              </template>
              <template v-if="props.pointType == 'T' || props.pointType == 'A'">
                <div class="vtable__cell vtable__cell--scale">
                  <template v-if="item.isEditing">
                    <div class="inline-edit-container">
                      <el-input-number
                        v-model="item.scale"
                        :min="0"
                        @change="() => onFieldInput(item, 'scale')"
                        :controls="false"
                        align="left"
                        style="width: 100% !important"
                      />
                    </div>
                  </template>
                  <template v-else>
                    <span :class="getFieldClass(item, 'scale')">{{ item.scale }}</span>
                  </template>
                  <div
                    v-if="
                      props.isEditing &&
                      getFieldError(item, 'scale') &&
                      item.rowStatus !== 'deleted'
                    "
                    class="field-error"
                  >
                    {{ getFieldError(item, 'scale') }}
                  </div>
                </div>
                <div class="vtable__cell vtable__cell--offset">
                  <template v-if="item.isEditing">
                    <div class="inline-edit-container">
                      <el-input-number
                        v-model="item.offset"
                        @change="() => onFieldInput(item, 'offset')"
                        :controls="false"
                        align="left"
                        style="width: 100% !important"
                      />
                    </div>
                  </template>
                  <template v-else>
                    <span :class="getFieldClass(item, 'offset')">{{ item.offset }}</span>
                  </template>
                  <div
                    v-if="
                      props.isEditing &&
                      getFieldError(item, 'offset') &&
                      item.rowStatus !== 'deleted'
                    "
                    class="field-error"
                  >
                    {{ getFieldError(item, 'offset') }}
                  </div>
                </div>
                <div class="vtable__cell vtable__cell--unit">
                  <template v-if="item.isEditing">
                    <div class="inline-edit-container">
                      <el-input
                        v-model="item.unit"
                        placeholder="Enter unit"
                        @input="() => onFieldInput(item, 'unit')"
                        style="width: 100% !important"
                      />
                    </div>
                  </template>
                  <template v-else>
                    <span :class="getFieldClass(item, 'unit')">{{ item.unit }}</span>
                  </template>
                  <div
                    v-if="
                      props.isEditing && getFieldError(item, 'unit') && item.rowStatus !== 'deleted'
                    "
                    class="field-error"
                  >
                    {{ getFieldError(item, 'unit') }}
                  </div>
                </div>
              </template>
              <div
                class="vtable__cell vtable__cell--reverse"
                :class="props.pointType === 'C' || props.pointType === 'S' ? 'CorS' : ''"
              >
                <template v-if="item.isEditing">
                  <div class="inline-edit-container inline-edit-reverse-container">
                    <el-select
                      v-model="item.reverse"
                      popper-class="inline-reverse-popper"
                      :fit-input-width="true"
                      filterable
                    >
                      <el-option label="true" :value="true" />
                      <el-option label="false" :value="false" />
                    </el-select>
                  </div>
                </template>
                <template v-else>
                  <span :class="getFieldClass(item, 'reverse')">{{ item.reverse }}</span>
                </template>
              </div>

              <div v-if="!props.publishMode" class="vtable__cell vtable__cell--operation">
                <div class="point-table__operation-cell">
                  <template v-if="props.isEditing">
                    <template v-if="item.isEditing">
                      <div class="point-table__confirm-btn" @click="handleConfirmInlineEdit(item)">
                        <el-icon><Check /></el-icon>
                      </div>
                      <div class="point-table__cancel-btn" @click="handleCancelInlineEdit(item)">
                        <el-icon><Close /></el-icon>
                      </div>
                    </template>
                    <template v-else-if="item.rowStatus === 'deleted'">
                      <div class="point-table__restore-btn" @click="restorePoint(item)">
                        <el-icon><RefreshLeft /></el-icon>
                      </div>
                    </template>
                    <template v-else>
                      <div class="point-table__edit-btn" @click="handleStartInlineEdit(item)">
                        <el-icon><Edit /></el-icon>
                      </div>
                      <div class="point-table__delete-btn" @click="deletePoint(item)">
                        <el-icon><Delete /></el-icon>
                      </div>
                    </template>
                  </template>
                  <template v-else>
                    <div class="point-table__publish-btn" @click="handlePublish(item)">
                      <el-icon><Position /></el-icon>
                      <span>Publish</span>
                    </div>
                  </template>
                </div>
              </div>
              <div v-else class="vtable__cell vtable__cell--publish-value">
                <template v-if="props.pointType === 'C' || props.pointType === 'S'">
                  <el-select
                    filterable
                    v-model="publishValues[item.point_id]"
                    placeholder="Select"
                    popper-class="inline-publish-popper"
                    :fit-input-width="true"
                    @change="notifyPublishChange"
                    clearable
                  >
                    <el-option label="1" :value="1" />
                    <el-option label="0" :value="0" />
                  </el-select>
                </template>
                <template v-else-if="props.pointType === 'A' || props.pointType === 'T'">
                  <el-input-number
                    v-model="publishValues[item.point_id]"
                    :controls="false"
                    align="left"
                    @change="notifyPublishChange"
                  />
                </template>
              </div>
            </div>
          </DynamicScrollerItem>
        </template>
      </DynamicScroller>
    </div>

    <ValuePublishDialog ref="valuePublishDialogRef" />
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, computed, nextTick, inject } from 'vue'
import { pxToResponsive } from '@/utils/responsive'
import { ElMessage } from 'element-plus'
import { OriginalPointsKey, ChannelNameKey } from '@/utils/key'
import {
  Delete,
  Edit,
  Filter,
  Plus,
  Position,
  RefreshLeft,
  Close,
  Check,
  WarningFilled,
} from '@element-plus/icons-vue'
import type { PointInfo } from '@/types/channelConfiguration'
import ValuePublishDialog from './ValuePublishDialog.vue'
// lodash-es 替换
const toLower = (v: any) => String(v ?? '').toLowerCase()

// Props 说明
// - pointType: 当前表所属的大类（T/S/C/A），影响列与校验
// - points: 父组件传入的当前 Tab 点位集合（用于渲染）
// - originalPoints: 父组件传入的“接口原始点位数据”（仅用于非导入行的对比）
// - viewMode/editFilters/isEditing/publishMode: 控制渲染模式、筛选标签、编辑态与发布态
interface Props {
  pointType: 'T' | 'S' | 'C' | 'A'
  points: PointInfo[]
  originalPoints?: PointInfo[]
  viewMode: 'points' | 'mappings'
  editFilters: string[]
  isEditing: boolean
  publishMode?: boolean
  channelProtocol?: 'modbus_tcp' | 'modbus_rtu' | 'virt' | 'can' | 'di_do'
}
const props = withDefaults(defineProps<Props>(), {
  viewMode: 'points',
  editFilters: () => [],
  publishMode: false,
  channelProtocol: 'modbus_tcp',
})
const emit = defineEmits<{
  'publish-change': [dirty: boolean]
  'toggle-publish': []
  'enter-edit-mode': [payload?: { fromImport?: boolean }]
  'change-edit-filter': [value: string]
}>()

const injectedOriginalPoints = inject(OriginalPointsKey, ref<PointInfo[]>([]))
const injectedChannelName = inject(ChannelNameKey, ref(''))
// 原始点位对比基线：优先使用父组件透传 originalPoints；否则回退到注入值
const originalPointsList = computed<PointInfo[]>(() => {
  if (Array.isArray(props.originalPoints) && props.originalPoints.length >= 0) {
    return props.originalPoints as PointInfo[]
  }
  return (injectedOriginalPoints?.value || []) as PointInfo[]
})

const editPoints = ref<PointInfo[]>([])
const signalNameFilterRaw = ref('')
const signalNameFilter = ref('')
// Status 筛选逻辑已移至父组件 PointsTablesDialog.vue
const showSignalNameFilter = ref(false)
const scrollerRef = ref()
const pendingNewRow = ref<PointInfo | null>(null)
const valuePublishDialogRef = ref()
const publishValues = ref<Record<number, any>>({})
const fileInputRef = ref<HTMLInputElement>()
const importedFileName = ref('')

// 行唯一键：避免 DynamicScroller 在 point_id 重复时发生复用混乱
let rowKeySeed = 1
function createRowKey(): string {
  rowKeySeed += 1
  return `${Date.now()}-${rowKeySeed}-${Math.random().toString(36).slice(2, 8)}`
}

const isTA = computed(() => props.pointType === 'T' || props.pointType === 'A')
// const isCA = computed(() => props.pointType === 'C' || props.pointType === 'A')
const rowHeight = ref(pxToResponsive(36))
const signalNameOptions = computed(() => {
  const names = (editPoints.value || []).map((p) => p.signal_name).filter((n) => n)
  return Array.from(new Set(names))
})

// 列表筛选：支持 signal name 关键字与"Status（modified/added/deleted/invalid）"
// Status 筛选由父组件通过 editFilters prop 传递
// 导入后，页面只显示导入的点位信息（不显示被删除的点位，除非用户主动筛选 deleted）
const filteredPoints = computed(() => {
  const list = Array.isArray(editPoints.value) ? editPoints.value : []
  let result = [...list]

  // 如果当前有导入的文件名，且没有主动筛选 deleted 状态，则过滤掉 deleted 状态的点位
  // 这样页面只显示导入的点位信息
  if (
    importedFileName.value &&
    (!props.editFilters || props.editFilters.length === 0 || props.editFilters[0] !== 'deleted')
  ) {
    result = result.filter((p) => (p as any).rowStatus !== 'deleted')
  }

  if (signalNameFilter.value) {
    const kw = toLower(String(signalNameFilter.value || ''))
    result = result.filter((p) =>
      String(p.signal_name || '')
        .toLowerCase()
        .includes(kw),
    )
  }
  // 使用父组件传递的 editFilters 进行筛选
  if (props.editFilters && props.editFilters.length > 0) {
    const filterValue = props.editFilters[0]
    if (filterValue === 'invalid') {
      result = result.filter((p) => (p as any).isInvalid === true)
    } else {
      result = result.filter((p) => (p as any).rowStatus === filterValue)
    }
  }
  return result
})

onMounted(() => {
  const onResize = () => (rowHeight.value = pxToResponsive(36))
  window.addEventListener('resize', onResize as any)
  ;(onResize as any)()
})

// 简易防抖：筛选 300ms
let _snfTimer: any = null
watch(
  () => signalNameFilterRaw.value,
  (v) => {
    if (_snfTimer) clearTimeout(_snfTimer)
    _snfTimer = setTimeout(() => {
      signalNameFilter.value = String(v || '')
    }, 300)
  },
  { immediate: true },
)
onUnmounted(() => {})

watch(
  () => ({ points: props.points, isEditing: props.isEditing }),
  (val) => {
    if (!val.isEditing && Array.isArray(val.points)) {
      // 非编辑状态：清理未确认的新增行引用
      pendingNewRow.value = null
      editPoints.value = val.points.map((item: PointInfo) => {
        const clone: any = {
          ...item,
          rowStatus: item.rowStatus || 'normal',
        }
        // 为每一行分配稳定且唯一的 rowKey（会覆盖缺失的情况）
        clone.rowKey = (item as any).rowKey || createRowKey()
        // 记录原始 point_id，用于确认时判断是否还原或标记为新增
        if (clone.originalPointId === undefined) clone.originalPointId = item.point_id
        return clone
      })
      // 首次载入或刷新后执行一次有效性检测
      if (Array.isArray(editPoints.value)) {
        editPoints.value.forEach((p) => validateRowValidity(p))
        refreshFieldErrorsForList()
      }
    }
  },
  { immediate: true, deep: true },
)

// 进入编辑状态时，再次执行有效性检测，确保无效项高亮
watch(
  () => props.isEditing,
  (editing) => {
    if (editing && Array.isArray(editPoints.value)) {
      // 进入编辑状态：清理未确认的新增行，重置 pendingNewRow
      const unconfirmedIndex = editPoints.value.findIndex((p: any) => p.isNewUnconfirmed)
      if (unconfirmedIndex !== -1) {
        editPoints.value.splice(unconfirmedIndex, 1)
      }
      pendingNewRow.value = null
      // 执行有效性检测
      editPoints.value.forEach((p) => validateRowValidity(p))
      refreshFieldErrorsForList()
    } else if (!editing) {
      // 退出编辑：恢复为原始对比基线（originalPointsList），清理所有未确认的新增行
      const baseline = (originalPointsList.value as PointInfo[]).map((item: PointInfo) => {
        const clone: any = {
          ...item,
          rowStatus: item.rowStatus || 'normal',
        }
        clone.rowKey = (item as any).rowKey || createRowKey()
        if (clone.originalPointId === undefined) clone.originalPointId = item.point_id
        return clone
      })
      editPoints.value = baseline
      // 清理未确认的新增行引用
      pendingNewRow.value = null
      refreshFieldErrorsForList()
    }
  },
)

// 退出批量发布模式时清空发布值（仅当前 Tab）
watch(
  () => props.publishMode,
  (val, oldVal) => {
    if (oldVal && !val) {
      clearCurrentTabPublish()
    }
  },
)

const deletePoint = (item: PointInfo) => {
  // 使用 rowKey 来查找行，因为 rowKey 是唯一的，避免重复 point_id 时误删
  const itemRowKey = (item as any).rowKey
  const originalIndex = editPoints.value.findIndex((p: any) => (p as any).rowKey === itemRowKey)
  if (originalIndex === -1) return
  const target: any = editPoints.value[originalIndex]
  const isNewUnconfirmed = !!target.isNewUnconfirmed
  const isAdded = target.rowStatus === 'added' || !!target.isImported
  // 新增记录（含导入/未确认新增）删除时直接移除，不能恢复
  if (isNewUnconfirmed || isAdded) {
    editPoints.value.splice(originalIndex, 1)
    if (pendingNewRow.value && (pendingNewRow.value as any).rowKey === itemRowKey) {
      pendingNewRow.value = null
    }
    recomputeAllValidity()
    return
  }
  // 非新增记录：标记为删除，可恢复
  editPoints.value[originalIndex].rowStatus = 'deleted'
  ;(editPoints.value[originalIndex] as any).isInvalid = false
  // ElMessage.success('Point marked as deleted')
  recomputeAllValidity()
}
const restorePoint = (item: PointInfo) => {
  // 使用 rowKey 来查找行，因为 rowKey 是唯一的，避免重复 point_id 时误操作
  const itemRowKey = (item as any).rowKey
  const originalIndex = editPoints.value.findIndex((p: any) => (p as any).rowKey === itemRowKey)
  if (originalIndex !== -1) {
    const restoredItem = editPoints.value[originalIndex]
    restoredItem.rowStatus = 'normal'
    delete restoredItem.modifiedFields
    // 恢复后立即按当前内容重新校验（如原本有问题则仍判为无效）
    validateRowValidity(restoredItem)
    // 恢复后重新检查所有行的重复情况（包括新增行）
    applyDuplicatePointIdInvalid()
    // 刷新字段级别的错误提示
    refreshFieldErrorsForRow(restoredItem)
    // ElMessage.success('Point restored')
  }
}
const handleStartInlineEdit = (item: PointInfo) => {
  item.originalData = {
    point_id: item.point_id,
    signal_name: item.signal_name,
    scale: item.scale,
    offset: item.offset,
    unit: item.unit,
    reverse: item.reverse,
  }
  item.isEditing = true
  ;(item as any).hideErrorsOnce = true
}
const handleCancelInlineEdit = (item: PointInfo) => {
  if (item.isNewUnconfirmed) {
    // 使用 rowKey 来查找行，避免重复 point_id 时误删
    const itemRowKey = (item as any).rowKey
    const idx = editPoints.value.findIndex((p: any) => (p as any).rowKey === itemRowKey)
    if (idx !== -1) editPoints.value.splice(idx, 1)
    if (pendingNewRow.value && (pendingNewRow.value as any).rowKey === itemRowKey) {
      pendingNewRow.value = null
    }
    recomputeAllValidity()
  } else {
    if (item.originalData) {
      Object.assign(item, item.originalData)
      delete item.originalData
    }
    item.isEditing = false
    recomputeAllValidity()
  }
  // 取消编辑后清除当前筛选条件
  signalNameFilter.value = ''
  showSignalNameFilter.value = false
}
// 确认行编辑：
// - 新增（含导入）保持 rowStatus=added，仅记录 modifiedFields 用于字段高亮
// - 非新增与“接口原始数据”对比，设置 rowStatus=modified/normal
// - 对 scale/offset 进行缺省归一化并执行有效性校验（isInvalid）
const handleConfirmInlineEdit = (item: PointInfo) => {
  if (item.isNewUnconfirmed) {
    item.isNewUnconfirmed = false
    item.rowStatus = 'added'
    item.isEditing = false
    delete item.originalData
    pendingNewRow.value = null
    // ElMessage.success('Point added successfully')
    // 归一化数值：为空或非法时赋默认
    // item.scale = typeof item.scale === 'number' && Number.isFinite(item.scale) ? item.scale : 1
    // item.offset = typeof item.offset === 'number' && Number.isFinite(item.offset) ? item.offset : 0
    // 新增或导入确认后，执行有效性校验
    validateRowValidity(item)
    applyDuplicatePointIdInvalid()
    // 刷新字段级别的错误提示
    refreshFieldErrorsForRow(item)
    ;(item as any).hideErrorsOnce = false
  } else {
    const isNew = item.rowStatus === 'added'
    const changes: string[] = []

    // 对于新增的记录（导入的数据），不需要比较original
    if (isNew) {
      // 导入的数据，只检查是否有修改（基于originalData）
      if (item.originalData) {
        if (item.signal_name !== item.originalData.signal_name) changes.push('signal_name')
        if (item.scale !== item.originalData.scale) changes.push('scale')
        if (item.offset !== item.originalData.offset) changes.push('offset')
        if (item.unit !== item.originalData.unit) changes.push('unit')
        if (item.reverse !== item.originalData.reverse) changes.push('reverse')
      }
      // 新增记录保持added状态
      if (changes.length > 0) {
        item.modifiedFields = changes
      }
    } else {
      // 已存在的记录：
      // A) 如果 point_id 修改后在原始接口中不存在，则直接标记为新增
      // B) 否则按“与接口原始值不同”且“相对本次编辑快照也变化”判定 modified
      const origId = (item as any).originalPointId
      if (origId !== undefined && item.point_id !== origId) {
        const existsInOriginal = (originalPointsList.value as PointInfo[]).some(
          (p) => p.point_id === item.point_id,
        )
        if (!existsInOriginal) {
          item.rowStatus = 'added'
          item.modifiedFields = []
          // 归一化后直接进入校验流程（在函数尾部统一处理）
        } else {
          // 继续按照“原始ID对应的原始记录”进行其它字段差异判断
          const original = (originalPointsList.value as PointInfo[]).find(
            (p) => p.point_id === origId,
          )
          const prev = item.originalData || {}
          if (original) {
            if (
              item.signal_name !== original.signal_name &&
              item.signal_name !== (prev as any).signal_name
            )
              changes.push('signal_name')
            if (item.scale !== original.scale && item.scale !== (prev as any).scale)
              changes.push('scale')
            if (item.offset !== original.offset && item.offset !== (prev as any).offset)
              changes.push('offset')
            if (item.unit !== original.unit && item.unit !== (prev as any).unit)
              changes.push('unit')
            if (item.reverse !== original.reverse && item.reverse !== (prev as any).reverse)
              changes.push('reverse')
          }
          if (changes.length > 0) {
            item.rowStatus = 'modified'
            item.modifiedFields = changes
          } else {
            item.rowStatus = 'normal'
            item.modifiedFields = []
          }
        }
      } else {
        // point_id 未变更（或缺少原始ID标记），按常规比较
        // 1) point_id 的“原始记录”用 originalData.point_id（若有）匹配
        // 2) 其它字段仅在“与接口原始值不同”且“本次会话相对 originalData 发生了改变”时，才标为 modified
        const original = (originalPointsList.value as PointInfo[]).find(
          (p) => p.point_id === (item.originalData?.point_id || item.point_id),
        )
        const prev = item.originalData || {}
        if (original) {
          if (
            item.signal_name !== original.signal_name &&
            item.signal_name !== (prev as any).signal_name
          )
            changes.push('signal_name')
          if (item.scale !== original.scale && item.scale !== (prev as any).scale)
            changes.push('scale')
          if (item.offset !== original.offset && item.offset !== (prev as any).offset)
            changes.push('offset')
          if (item.unit !== original.unit && item.unit !== (prev as any).unit) changes.push('unit')
          if (item.reverse !== original.reverse && item.reverse !== (prev as any).reverse)
            changes.push('reverse')
        }
        if (changes.length > 0) {
          item.rowStatus = 'modified'
          item.modifiedFields = changes
        } else {
          item.rowStatus = 'normal'
          item.modifiedFields = []
        }
      }
    }
    // 归一化数值：为空或非法时赋默认
    // item.scale = typeof item.scale === 'number' && Number.isFinite(item.scale) ? item.scale : 1
    // item.offset = typeof item.offset === 'number' && Number.isFinite(item.offset) ? item.offset : 0
    // 编辑确认后，执行有效性校验
    validateRowValidity(item)
    applyDuplicatePointIdInvalid()
    // 刷新字段级别的错误提示
    refreshFieldErrorsForRow(item)
    item.isEditing = false
    delete item.originalData
    ;(item as any).hideErrorsOnce = false
    // ElMessage.success('Point updated successfully')
  }
}

const getNextPointId = () => {
  const allIds = editPoints.value.map((p) => p.point_id).filter((id) => id > 0)
  return allIds.length > 0 ? Math.max(...allIds) + 1 : 1
}
const handleAddNewPoint = () => {
  if (pendingNewRow.value) {
    scrollToTop()
    ElMessage.warning('Please confirm or cancel the pending new point first')
    return
  }
  const newId = getNextPointId()
  const newPoint: PointInfo = {
    point_id: newId,
    signal_name: '',
    scale: 1,
    offset: 0,
    unit: '',
    data_type: 'float',
    reverse: false,
    description: '',
    isEditing: true,
    isNewUnconfirmed: true,
    rowStatus: 'normal',
  }
  ;(newPoint as any).rowKey = createRowKey()
  ;(newPoint as any).originalPointId = undefined
  ;(newPoint as any).hideErrorsOnce = true
  editPoints.value.unshift(newPoint)
  pendingNewRow.value = newPoint
  scrollToTop()
}
const scrollToTop = () => {
  nextTick(() => {
    const scroller = scrollerRef.value
    if (scroller && scroller.$el) scroller.scrollToItem(0)
  })
}

// 行样式：导入记录永久以“新增”样式展示；否则依据 rowStatus
const getRowClass = (item: PointInfo) => {
  const baseClass = (item as any).isImported
    ? 'row-status-added'
    : `row-status-${item.rowStatus || 'normal'}`
  const classes = [baseClass]
  if (props.isEditing && (item as any).isInvalid) {
    classes.push('row-invalid')
  }
  return classes.join(' ')
}
const getFieldClass = (item: PointInfo, fieldName: string) => {
  const status = item.rowStatus
  if (status === 'added') return 'field-added'
  if (status === 'modified' && item.modifiedFields?.includes(fieldName)) return 'field-modified'
  if (status === 'deleted') return 'field-deleted'
  return ''
}

// 校验当前行是否有效；返回 true 表示有效，false 表示无效
// 规则：
// - 公共：point_id 为正整数；signal_name 非空；reverse 为布尔
// - modbus 协议：scale/offset 必须为数字；unit 无限制
// - di_do 协议：无需校验 scale/offset/unit
function validateRowValidity(point: PointInfo): boolean {
  // 删除状态不做内容校验，视为有效
  if ((point as any).rowStatus === 'deleted') {
    ;(point as any).isInvalid = false
    return true
  }
  // 公共校验：point_id 为正整数，signal_name 非空，reverse 为布尔
  const isPositiveInt = (n: unknown) => Number.isInteger(n) && (n as number) > 0
  const isNonEmptyString = (s: unknown) => typeof s === 'string' && s.length > 0
  const isBool = (v: unknown) => typeof v === 'boolean'

  let valid = true

  if (!isPositiveInt(point.point_id)) {
    valid = false
  } else if (!isNonEmptyString(point.signal_name)) {
    valid = false
  } else if (!isBool(point.reverse)) {
    valid = false
  }

  // modbus 协议校验 scale/offset（unit 无限制）
  if (valid && props.channelProtocol !== 'di_do') {
    const isNum = (v: unknown) => typeof v === 'number' && Number.isFinite(v)

    if (!isNum(point.scale)) {
      valid = false
    } else if (!isNum(point.offset)) {
      valid = false
    }
    // unit 字段无任何限制，不进行校验
  }

  // di_do 协议无需校验 scale/offset/unit

  ;(point as any).isInvalid = !valid
  return valid
}

// 检查并标记重复的 point_id（正整数）
// 注意：已删除的行（rowStatus === 'deleted'）不参与重复检查
function applyDuplicatePointIdInvalid() {
  // 只统计未删除的行的 point_id
  const ids = (editPoints.value || [])
    .filter((p: any) => (p as any).rowStatus !== 'deleted')
    .map((p: PointInfo) => Number((p as any).point_id))
    .filter((id) => Number.isInteger(id) && id > 0)
  const counts: Record<number, number> = {}
  ids.forEach((id) => {
    counts[id] = (counts[id] || 0) + 1
  })
  // 只对未删除的行进行重复检查
  ;(editPoints.value || []).forEach((p: any) => {
    // 已删除的行不参与重复检查，也不清除重复错误（保持删除状态）
    if ((p as any).rowStatus === 'deleted') {
      return
    }
    const id = Number(p.point_id)
    const dup = Number.isInteger(id) && id > 0 && (counts[id] || 0) > 1
    if (dup) {
      p.isInvalid = true
      // 设置字段级错误
      setFieldError(p, 'point_id', 'duplicate point_id')
    } else {
      // 如果之前是重复错误，清除字段级重复错误
      if (p.fieldErrors && p.fieldErrors.point_id === 'duplicate point_id') {
        delete p.fieldErrors.point_id
        // 重新校验基础有效性，以确定 isInvalid 的正确状态（可能还有其他错误）
        validateRowValidity(p as PointInfo)
        // 刷新字段级别的错误提示，确保 point_id 字段的错误被正确清除或更新
        refreshFieldErrorsForRow(p)
      }
    }
  })
}

function recomputeAllValidity() {
  if (!Array.isArray(editPoints.value)) return
  editPoints.value.forEach((p) => validateRowValidity(p))
  applyDuplicatePointIdInvalid()
}

function handlePointIdChange(item: PointInfo) {
  // 基础校验 + 全表重复校验
  validateRowValidity(item)
  applyDuplicatePointIdInvalid()
}

function notifyPublishChange() {
  emit('publish-change', hasPublishChanges())
}
// 字段级即时校验（仅字段本身，不改变整行校验逻辑）
function getFieldError(item: any, field: string): string {
  if (item && (item as any).hideErrorsOnce) return ''
  return (item.fieldErrors && item.fieldErrors[field]) || ''
}
function setFieldError(item: any, field: string, message: string) {
  if (!item.fieldErrors) item.fieldErrors = {}
  if (message) item.fieldErrors[field] = message
  else delete item.fieldErrors[field]
}
function validateFieldOnly(item: any, field: string): string {
  switch (field) {
    case 'point_id': {
      const n = Number(item.point_id)
      if (!Number.isInteger(n) || n <= 0) return 'must positive integer'
      // 检查是否与其他未删除的行重复
      if (Number.isInteger(n) && n > 0) {
        const duplicateCount = (editPoints.value || []).filter((p: any) => {
          // 排除当前行和已删除的行
          return p !== item && (p as any).rowStatus !== 'deleted' && Number(p.point_id) === n
        }).length
        if (duplicateCount > 0) {
          return 'duplicate point_id'
        }
      }
      return ''
    }
    case 'signal_name': {
      const v = String(item.signal_name || '')
      if (!v) return 'required'
      return ''
    }
    case 'reverse': {
      const v = item.reverse
      if (!(v === true || v === false)) return 'required (true/false)'
      return ''
    }
    case 'scale': {
      const v = item.scale
      if (typeof v !== 'number' || !Number.isFinite(v)) return 'must be a number'
      return ''
    }
    case 'offset': {
      const v = item.offset
      if (typeof v !== 'number' || !Number.isFinite(v)) return 'must be a number'
      return ''
    }
    case 'unit': {
      // unit 字段无任何限制，不进行校验
      return ''
    }
    default:
      return ''
  }
}
function onFieldInput(item: any, field: string) {
  if (item && (item as any).hideErrorsOnce) (item as any).hideErrorsOnce = false
  const msg = validateFieldOnly(item, field)
  setFieldError(item, field, msg)
  // 当 point_id 变化时，需要重新检查所有行的重复情况
  if (field === 'point_id') {
    applyDuplicatePointIdInvalid()
  }
}
function refreshFieldErrorsForRow(item: any) {
  // 始终校验的字段
  setFieldError(item, 'point_id', validateFieldOnly(item, 'point_id'))
  setFieldError(item, 'signal_name', validateFieldOnly(item, 'signal_name'))
  setFieldError(item, 'reverse', validateFieldOnly(item, 'reverse'))
  // 根据协议类型校验数值字段
  // modbus_tcp/modbus_rtu 协议：需要 scale/offset（unit 无限制）
  // di_do 协议：不需要 scale/offset/unit
  if (props.channelProtocol !== 'di_do') {
    // modbus 协议：校验 scale/offset
    setFieldError(item, 'scale', validateFieldOnly(item, 'scale'))
    setFieldError(item, 'offset', validateFieldOnly(item, 'offset'))
    // unit 字段无任何限制，不进行校验，清理可能存在的错误
    setFieldError(item, 'unit', '')
  } else {
    // di_do 协议：清理这些字段的错误
    setFieldError(item, 'scale', '')
    setFieldError(item, 'offset', '')
    setFieldError(item, 'unit', '')
  }
}
function refreshFieldErrorsForList() {
  if (!Array.isArray(editPoints.value)) return
  editPoints.value.forEach((p: any) => refreshFieldErrorsForRow(p))
}
function getPublishCommands(): Array<{ id: string; value: number }> {
  const commands: Array<{ id: string; value: number }> = []
  Object.entries(publishValues.value).forEach(([key, val]) => {
    if (val !== '' && val !== null && val !== undefined) {
      let num = Number(val)
      // // 对 Adjustment 保证为浮点语义（允许小数）
      // if (props.pointType === 'A' && Number.isInteger(num)) {
      //   num = Number(`${num}.0`)
      // }
      commands.push({ id: key, value: num })
    }
  })
  console.log(commands, 'commands')
  return commands
}
function hasPublishChanges(): boolean {
  return getPublishCommands().length > 0
}
function resetPublish() {
  publishValues.value = {}
  notifyPublishChange()
}

function clearCurrentTabPublish() {
  if (Array.isArray(editPoints.value)) {
    editPoints.value.forEach((p) => {
      publishValues.value[p.point_id] = null
    })
  }
  notifyPublishChange()
}

// 实时值更新：根据 point_id 批量写入 value
function applyRealtimeValues(values: Record<string | number, number>) {
  if (!values || !Array.isArray(editPoints.value)) return
  const valueMap = new Map<number, number>()
  Object.entries(values).forEach(([k, v]) => {
    const id = Number(k)
    if (Number.isFinite(id)) valueMap.set(id, Number(v))
  })
  if (valueMap.size === 0) return
  editPoints.value.forEach((p: any) => {
    const newVal = valueMap.get(p.point_id)
    if (newVal !== undefined) {
      p.value = newVal
    }
  })
}

const handlePublish = (pointRow: PointInfo) => {
  valuePublishDialogRef.value.open({
    pointId: pointRow.point_id,
    dataType: pointRow.data_type,
    category: props.pointType as 'C' | 'A' | 'T' | 'S',
    signalName: pointRow.signal_name,
  })
}

const addPoint = (point: PointInfo) => {
  editPoints.value.push({ ...point, rowStatus: 'added' })
}
const getEditedData = () => editPoints.value

const handleTogglePublish = () => {
  emit('toggle-publish')
}

const handleImportClick = (event: MouseEvent) => {
  event.preventDefault()
  event.stopPropagation()
  // 在 Mac 浏览器中，使用 mousedown 事件可以确保文件选择对话框正常打开
  // 使用 setTimeout 确保在事件处理完成后调用
  setTimeout(() => {
    if (fileInputRef.value) {
      fileInputRef.value.click()
    }
  }, 0)
}

// CSV 导入（Points）：
// - 期望表头：
//   - tab=signal/control：point_id,point_name,reverse（scale/offset/unit 不导入，使用默认值）
//   - tab=telemetry/adjustment：point_id,point_name,scale,offset,unit,reverse（modbus）
// - 所有导入记录标记为 isImported，并作为“新增”渲染（rowStatus=added）
// - 无效数据不丢弃：标记 isInvalid 与描述，便于用户在界面修正
// - 导入后立即进入编辑态并替换当前 Tab 的编辑数据
const handleFileChange = (event: Event) => {
  const target = event.target as HTMLInputElement
  const file = target.files?.[0]
  if (!file) return

  const reader = new FileReader()
  reader.onload = (e) => {
    try {
      const content = e.target?.result as string
      const lines = content.split('\n').filter((line) => line.trim())
      if (lines.length === 0) {
        ElMessage.error('CSV file is empty')
        return
      }

      // 解析表头，找到所需列的索引位置（允许有其他列）
      const header = lines[0].trim()
      const headerColumns = header.split(',').map((col) => col.trim().toLowerCase())

      // 导入必需列只看 Tab（pointType），不看通道协议（不区分大小写）
      // signal_name 已统一展示为 point_name；导入只接受 point_name
      let requiredColumns: Record<string, string>

      const isSignalOrControl = props.pointType === 'S' || props.pointType === 'C'
      if (isSignalOrControl) {
        // signal / control：硬性表头只要 point_id, point_name, reverse
        requiredColumns = {
          point_id: 'point_id',
          signal_name: 'point_name',
          reverse: 'reverse',
        }
      } else {
        // telemetry/adjustment：必需列：point_id, point_name, scale, offset, unit, reverse
        requiredColumns = {
          point_id: 'point_id',
          signal_name: 'point_name',
          scale: 'scale',
          offset: 'offset',
          unit: 'unit',
          reverse: 'reverse',
        }
      }

      // 查找各列的索引位置
      const columnIndices: Record<string, number> = {}

      // 查找必需列（所有必需列都必须存在）
      for (const [key, colName] of Object.entries(requiredColumns)) {
        const index = headerColumns.findIndex((h) => h === colName)
        if (index === -1) {
          ElMessage.error(`Required column "${colName}" not found in CSV header`)
          return
        }
        columnIndices[key] = index
      }

      // 解析数据行
      const importedPoints: PointInfo[] = []

      for (let i = 1; i < lines.length; i++) {
        const line = lines[i].trim()
        if (!line) continue

        const values = line.split(',').map((v) => v.trim())

        // 根据列索引提取字段值
        const getValue = (columnKey: string, defaultValue: string = ''): string => {
          const index = columnIndices[columnKey]
          if (index !== undefined && index >= 0 && index < values.length) {
            return values[index] || defaultValue
          }
          return defaultValue
        }

        const pointIdStr = getValue('point_id')
        const signalName = getValue('signal_name')
        const reverseStr = getValue('reverse')

        // 根据 Tab 解析字段
        let scaleStr: string
        let offsetStr: string
        let unit: string

        if (isSignalOrControl) {
          // signal/control：不导入 scale/offset/unit，使用默认值
          scaleStr = ''
          offsetStr = ''
          unit = ''
        } else {
          // telemetry/adjustment：必需列，从列索引中获取
          scaleStr = getValue('scale')
          offsetStr = getValue('offset')
          unit = getValue('unit')
        }

        // 解析数值字段（直接解析，不做验证，由后续验证方法判断）
        const pointId = Number(pointIdStr) || 0
        // signal/control：默认 scale=1 offset=0 unit=''
        const scale = isSignalOrControl ? 1 : scaleStr ? Number(scaleStr) : 1
        const offset = isSignalOrControl ? 0 : offsetStr ? Number(offsetStr) : 0

        // 解析 reverse（支持 true/false/0/1，默认 false）
        let reverse = false
        if (reverseStr) {
          const lowerReverse = reverseStr.toLowerCase()
          if (lowerReverse === 'true' || lowerReverse === '1') {
            reverse = true
          } else if (lowerReverse === 'false' || lowerReverse === '0') {
            reverse = false
          }
          // 如果值无效，保持默认值 false，由后续验证方法判断
        }

        // 创建记录，正常导入所有数据
        const point: PointInfo = {
          point_id: pointId,
          signal_name: signalName || '',
          scale,
          offset,
          unit: unit || '',
          data_type: 'float',
          reverse,
          description: '',
          rowStatus: 'added',
          isEditing: false,
        }

        // 标记为文件导入的记录
        ;(point as any).isImported = true
        ;(point as any).rowKey = createRowKey()
        ;(point as any).originalPointId = undefined
        // 导入后立即显示错误，不隐藏
        ;(point as any).hideErrorsOnce = false

        importedPoints.push(point)
      }

      if (importedPoints.length === 0) {
        ElMessage.warning('No data to import')
        return
      }

      // 存储文件名
      importedFileName.value = file.name

      // 通知父组件进入编辑模式（标记来自导入，避免被父组件清空文件名）
      emit('enter-edit-mode', { fromImport: true })

      // 计算原始点位数据和导入点位数据的 point_id 差集
      // 将原始中有但导入中没有的点位标记为删除
      const originalPointIds = new Set(
        (originalPointsList.value || []).map((p) => Number(p.point_id)).filter((id) => id > 0),
      )
      const importedPointIds = new Set(
        importedPoints
          .map((p) => Number(p.point_id))
          .filter((id) => Number.isInteger(id) && id > 0),
      )

      // 计算差集：原始中有但导入中没有的点位
      const deletedPointIds = new Set<number>()
      originalPointIds.forEach((id) => {
        if (!importedPointIds.has(id)) {
          deletedPointIds.add(id)
        }
      })

      // 创建被删除的点位记录
      const deletedPoints: PointInfo[] = []
      if (deletedPointIds.size > 0) {
        const originalPointsMap = new Map(
          (originalPointsList.value || []).map((p) => [Number(p.point_id), p]),
        )
        deletedPointIds.forEach((pointId) => {
          const originalPoint = originalPointsMap.get(pointId)
          if (originalPoint) {
            const deletedPoint: PointInfo = {
              ...originalPoint,
              rowStatus: 'deleted',
              isEditing: false,
            }
            ;(deletedPoint as any).rowKey = createRowKey()
            ;(deletedPoint as any).originalPointId = originalPoint.point_id
            deletedPoints.push(deletedPoint)
          }
        })
      }

      // 直接更新本地数据：导入的点位 + 被删除的点位
      // 页面显示时会过滤掉 deleted 状态的点位，只显示导入的点位
      nextTick(() => {
        editPoints.value = [...importedPoints, ...deletedPoints]
        // 导入完成后先进行字段级错误校验（会在单元格中显示错误）
        refreshFieldErrorsForList()
        // 然后进行重复ID与基础校验（会标记 isInvalid）
        recomputeAllValidity()
        // 再次刷新字段错误，确保重复ID的错误也能显示在单元格中
        refreshFieldErrorsForList()
      })
      ElMessage.success(`Successfully imported ${importedPoints.length} points`)
    } catch (error) {
      console.error('Error parsing CSV:', error)
      ElMessage.error('Failed to parse CSV file')
    } finally {
      // 清空文件输入，允许重复选择同一文件
      target.value = ''
    }
  }

  reader.onerror = () => {
    ElMessage.error('Failed to read CSV file')
    target.value = ''
  }

  reader.readAsText(file)
}

// 清除导入的文件名
const clearImportedFileName = () => {
  importedFileName.value = ''
}

// 导出功能（Points）
// 文件名：{channelName}_{tab}_{timestamp}.csv
const handleExport = () => {
  if (!editPoints.value || editPoints.value.length === 0) {
    ElMessage.warning('No data to export')
    return
  }

  // 根据通道类型生成CSV内容
  let header: string
  let rows: string[]
  const isSignalOrControl = props.pointType === 'S' || props.pointType === 'C'

  if (props.channelProtocol === 'di_do') {
    // di_do 类型：point_id,point_name,value,reverse
    header = 'point_id,point_name,value,reverse'
    rows = editPoints.value.map((point) => {
      return [
        point.point_id,
        point.signal_name,
        point.value ?? '',
        point.reverse ? 'true' : 'false',
      ].join(',')
    })
  } else {
    if (isSignalOrControl) {
      // signal/control：导出不包含 scale/offset/unit
      header = 'point_id,point_name,value,reverse'
      rows = editPoints.value.map((point) => {
        return [
          point.point_id,
          point.signal_name,
          point.value ?? '',
          point.reverse ? 'true' : 'false',
        ].join(',')
      })
    } else {
      // telemetry/adjustment（modbus）：point_id,point_name,value,scale,offset,unit,reverse
      header = 'point_id,point_name,value,scale,offset,unit,reverse'
      rows = editPoints.value.map((point) => {
        return [
          point.point_id,
          point.signal_name,
          point.value ?? '',
          point.scale,
          point.offset,
          point.unit,
          point.reverse ? 'true' : 'false',
        ].join(',')
      })
    }
  }

  const csvContent = [header, ...rows].join('\n')

  // 创建Blob并下载
  const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' })
  const link = document.createElement('a')
  const url = URL.createObjectURL(blob)

  // 生成文件名：tab名称_时间戳.csv
  const tabNames: Record<string, string> = {
    T: 'telemetry',
    S: 'signal',
    C: 'control',
    A: 'adjustment',
  }
  const timestamp = new Date()
    .toISOString()
    .replace(/[-:]/g, '')
    .replace(/\..+/, '')
    .replace('T', '')
  const channelName =
    String(injectedChannelName?.value || '')
      .trim()
      .replace(/[^\w-]+/g, '_') || 'channel'
  const filename = `${channelName}_${tabNames[props.pointType]}_${timestamp}.csv`

  link.href = url
  link.download = filename
  link.style.display = 'none'
  document.body.appendChild(link)
  link.click()
  document.body.removeChild(link)
  URL.revokeObjectURL(url)

  ElMessage.success(`Exported to ${filename}`)
}

// 检查是否有修改、添加或删除
const hasChanges = () => {
  if (!Array.isArray(editPoints.value)) return false
  return editPoints.value.some(
    (p: any) =>
      p && (p.rowStatus === 'modified' || p.rowStatus === 'added' || p.rowStatus === 'deleted'),
  )
}

// 对外暴露：供父组件进行提交/发布/清理调用
defineExpose({
  getEditedData,
  addPoint,
  getPublishCommands,
  resetPublish,
  hasPublishChanges,
  clearImportedFileName,
  applyRealtimeValues,
  clearSignalNameFilter: () => {
    signalNameFilter.value = ''
    showSignalNameFilter.value = false
  },
  scrollToTop,
  hasInvalid: () => {
    return Array.isArray(editPoints.value)
      ? editPoints.value.some(
          (p: any) =>
            p &&
            p.rowStatus !== 'deleted' &&
            ((p as any).isInvalid === true ||
              !validateRowValidity(p as PointInfo)) /* 保守校验一次 */,
        )
      : false
  },
  hasChanges,
})
</script>

<style scoped lang="scss">
.table-action-controls {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 0.1rem;
  padding-bottom: 0.1rem;
  width: 100%;

  .imported-file-name {
    color: #ff6900;
    font-size: 0.14rem;
    padding: 0 0.1rem;
  }
}

.voltage-class .point-table {
  color: #fff;

  .vtable__body {
    position: relative;
    z-index: 1;
  }

  // 为表头与数据行统一预留左侧状态与右侧滑块空间，确保列对齐
  .vtable__header {
    position: relative;
    padding-left: 0.03rem; // 左侧状态占位
    padding-right: 0.08rem; // 右侧滑块占位
  }

  // 浮动的左侧状态条（不参与布局）
  .row-status-float {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 0.03rem;
    background: transparent;
    pointer-events: none;
  }
  // 根据行状态着色
  .vtable__row.row-status-added .row-status-float {
    background: #67c23a;
  }
  .vtable__row.row-status-modified .row-status-float {
    background: #409eff;
  }
  .vtable__row.row-status-deleted .row-status-float {
    background: #f56c6c;
  }
  .vtable__row.row-invalid .row-status-float {
    background: #f56c6c;
  }

  .vtable__cell {
    color: #fff;
    padding: 0.14rem 0.12rem;
    position: relative;
    &:has(.field-error) {
      display: flex;
      flex-direction: column;
      align-items: flex-start;
    }
  }

  .vtable__cell--point-id {
    width: 1.3rem; // 10% of 13.07rem
  }
  .vtable__cell--signal-name {
    width: 3.26rem; // 25% of 13.07rem
    position: relative;
    &.isEditing {
      width: 4.83rem; // 37% of 13.07rem
    }
    &.CorS {
      width: 4.57rem; // 35% of 13.07rem
      &.isEditing {
        width: 6.14rem; // 47% of 13.07rem
      }
    }

    .filter-icon {
      margin-left: 0.05rem;
      font-size: 0.14rem;
    }

    .signal-name-filter {
      position: absolute;
      top: 100%;
      left: 0;
      right: 0;
      z-index: 100;
      background: #1e2f52;
      padding: 0.1rem;
      border-radius: 0.04rem;
      //   box-shadow: 0 0.02rem 0.1rem rgba(0, 0, 0, 0.3);
      min-width: 2.5rem;
    }
  }
  // 顶部工具栏中的筛选下拉与输入框左侧对齐
  :deep(.signal-name-popper) {
    left: 0 !important;
    transform: none !important;
    min-width: 100% !important;
  }
  .vtable__cell--value {
    width: 1.56rem; // 12% of 13.07rem
    &.CorS {
      width: 2.87rem; // 22% of 13.07rem
    }
  }
  .vtable__cell--scale {
    width: 1.3rem; // 10% of 13.07rem
  }
  .vtable__cell--offset {
    width: 1.3rem; // 10% of 13.07rem
  }
  .vtable__cell--unit {
    width: 1.3rem; // 10% of 13.07rem
  }
  .vtable__cell--reverse {
    width: 1.3rem; // 10% of 13.07rem
    &.CorS {
      width: 2.61rem; // 20% of 13.07rem
    }
  }
  .vtable__cell--operation {
    width: 1.69rem; // 13% of 13.07rem

    .point-table__operation-cell {
      display: flex;
      gap: 0.15rem;
      align-items: center;
      justify-content: center;

      .point-table__edit-btn,
      .point-table__delete-btn,
      .point-table__setting-btn,
      .point-table__publish-btn,
      .point-table__restore-btn {
        cursor: pointer;
        display: flex;
        align-items: center;
        gap: 0.04rem;
        font-size: 0.16rem;
        transition: color 0.3s;

        span {
          font-size: 0.12rem;
        }
      }

      .point-table__edit-btn {
        color: #409eff;
        &:hover {
          color: #66b1ff;
        }
      }
      .point-table__delete-btn {
        color: #f56c6c;
        &:hover {
          color: #f78989;
        }
      }
      .point-table__restore-btn {
        color: #67c23a;
        &:hover {
          color: #85ce61;
        }
      }

      .point-table__cancel-btn {
        cursor: pointer;
        display: flex;
        align-items: center;
        font-size: 0.18rem;
        color: #f56c6c;
        transition: color 0.3s;
        &:hover {
          color: #f78989;
        }
      }
      .point-table__confirm-btn {
        cursor: pointer;
        display: flex;
        align-items: center;
        font-size: 0.18rem;
        color: #67c23a;
        transition: color 0.3s;
        &:hover {
          color: #85ce61;
        }
      }

      .point-table__setting-btn,
      .point-table__publish-btn {
        color: #fff;
        &:hover {
          color: #ff6900;
        }
      }
    }
  }

  .inline-edit-container {
    width: 100%;
    position: relative;
    z-index: 10;

    :deep(.el-input),
    :deep(.el-input-number),
    :deep(.el-select) {
      width: 100% !important;
    }

    :deep(.el-input__inner) {
      padding: 0.02rem 0.08rem;
      height: 0.28rem;
      line-height: 0.28rem;
    }

    // 编辑状态时提升层级
    &:has(.el-select.is-focus),
    &:has(.el-select:hover) {
      z-index: 100;
    }
  }

  .inline-reverse-popper,
  .inline-mapping-popper,
  .inline-publish-popper {
    z-index: 9999 !important;
    position: absolute !important;
  }

  .vtable__cell--slave-id {
    width: 9%;
  }
  .vtable__cell--function-code {
    width: 11%;
  }
  .vtable__cell--register-address {
    width: 12%;
  }
  .vtable__cell--data-type {
    width: 9%;
  }
  .vtable__cell--byte-order {
    width: 10%;
  }
  .vtable__cell--bit-position {
    width: 9%;
  }

  .vtable__row {
    min-height: 0.36rem;
    position: relative;
    z-index: 1;
  }

  .row-status-normal {
    background-color: transparent;
  }
  .row-status-deleted {
    opacity: 0.6;
  }
  .row-invalid {
    background-color: rgba(245, 108, 108, 0.1);
  }

  .field-modified {
    color: #409eff !important;
  }
  .field-added {
    color: #67c23a !important;
  }
  .field-deleted {
    color: #f56c6c !important;
  }

  .value-field {
    color: #fff !important;
  }

  .vtable__cell--publish-value {
    width: 1.82rem; // 14% of 13.07rem
    position: relative;
    z-index: 10;
    :deep(.el-input-number) {
      width: 100% !important;
    }

    // 聚焦时提升层级
    &:has(.el-select.is-focus),
    &:has(.el-select:hover) {
      z-index: 100;
    }
  }
  .vtable__empty {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #fff;
    opacity: 0.7;
    padding: 0.4rem 0;
  }
  .field-error {
    position: absolute;
    bottom: 0.02rem;
    left: 0.12rem;
    width: 100%;
    color: #ff4d4f;
    font-size: 0.12rem;
    line-height: 1;
  }
}
</style>
