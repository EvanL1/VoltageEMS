<template>
  <div class="voltage-class point-table">
    <!-- 按钮控制区域 -->
    <div v-if="props.viewMode === 'points'" class="table-action-controls">
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
        <el-button type="primary" @click="handleImportClick">Import</el-button>
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
        <div class="vtable__cell vtable__cell--point-id" style="margin-left: 0.03rem">Point ID</div>
        <div
          class="vtable__cell vtable__cell--signal-name"
          :class="props.pointType === 'C' || props.pointType === 'S' ? 'CorS' : ''"
        >
          <span>Signal Name</span>
          <el-icon
            class="filter-icon"
            @click="showSignalNameFilter = !showSignalNameFilter"
            style="margin-left: 0.05rem; cursor: pointer"
          >
            <Filter />
          </el-icon>
          <div v-if="showSignalNameFilter" class="signal-name-filter" @click.stop>
            <el-select
              v-model="signalNameFilter"
              filterable
              allow-create
              clearable
              placeholder="Keyword Search"
              :teleported="false"
              style="width: 100%"
              :fit-input-width="true"
            >
              <el-option
                v-for="name in signalNameOptions"
                :key="name"
                :label="name"
                :value="name"
              />
            </el-select>
          </div>
        </div>
        <div
          class="vtable__cell vtable__cell--value"
          :class="props.pointType === 'C' || props.pointType === 'S' ? 'CorS' : ''"
        >
          Value
        </div>
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
                  <span :class="getFieldClass(item, 'point_id')">{{
                    item.point_id > 0 ? item.point_id : '-'
                  }}</span>
                </template>
                <div v-if="props.isEditing && getFieldError(item, 'point_id')" class="field-error">
                  {{ getFieldError(item, 'point_id') }}
                </div>
              </div>
              <div
                class="vtable__cell vtable__cell--signal-name"
                :class="props.pointType === 'C' || props.pointType === 'S' ? 'CorS' : ''"
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
                  v-if="props.isEditing && getFieldError(item, 'signal_name')"
                  class="field-error"
                >
                  {{ getFieldError(item, 'signal_name') }}
                </div>
              </div>
              <div
                class="vtable__cell vtable__cell--value"
                :class="props.pointType === 'C' || props.pointType === 'S' ? 'CorS' : ''"
              >
                <span class="value-field">{{ item.value ?? '-' }}</span>
              </div>
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
                  <div v-if="props.isEditing && getFieldError(item, 'scale')" class="field-error">
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
                  <div v-if="props.isEditing && getFieldError(item, 'offset')" class="field-error">
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
                  <div v-if="props.isEditing && getFieldError(item, 'unit')" class="field-error">
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
                      :teleported="false"
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
                    v-model="publishValues[item.point_id]"
                    placeholder="Select"
                    :teleported="false"
                    popper-class="inline-publish-popper"
                    :fit-input-width="true"
                    @change="notifyPublishChange"
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
import { uniq, isArray, isInteger, toNumber, toLower, countBy, throttle } from 'lodash-es'

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
}
const props = withDefaults(defineProps<Props>(), {
  viewMode: 'points',
  editFilters: () => [],
  publishMode: false,
})
const emit = defineEmits<{
  'publish-change': [dirty: boolean]
  'toggle-publish': []
  'enter-edit-mode': [payload?: { fromImport?: boolean }]
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
const signalNameFilter = ref('')
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
  return uniq(names)
})

// 列表筛选：支持 signal name 关键字与编辑态的标签筛选（modified/added/deleted/invalid）
const filteredPoints = computed(() => {
  const list = isArray(editPoints.value) ? editPoints.value : []
  let result = [...list]
  if (signalNameFilter.value) {
    const kw = toLower(String(signalNameFilter.value || ''))
    result = result.filter((p) => toLower(String(p.signal_name || '')).includes(kw))
  }
  if ((props.editFilters || []).length > 0) {
    result = result.filter((p) => {
      const status = p.rowStatus || 'normal'
      if (props.editFilters.includes('invalid')) {
        return (p as any).isInvalid === true
      }
      return props.editFilters.includes(status)
    })
  }
  return result
})

onMounted(() => {
  const onResize = throttle(() => (rowHeight.value = pxToResponsive(36)), 500)
  window.addEventListener('resize', onResize as any)
  ;(onResize as any)()
})
onUnmounted(() => {})

watch(
  () => ({ points: props.points, isEditing: props.isEditing }),
  (val) => {
    if (!val.isEditing && isArray(val.points)) {
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
      if (isArray(editPoints.value)) {
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
    if (editing && isArray(editPoints.value)) {
      editPoints.value.forEach((p) => validateRowValidity(p))
      refreshFieldErrorsForList()
    } else if (!editing) {
      // 退出编辑：恢复为原始对比基线（originalPointsList）
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
  const originalIndex = editPoints.value.findIndex((p) => p.point_id === item.point_id)
  if (originalIndex === -1) return
  const target: any = editPoints.value[originalIndex]
  const isNewUnconfirmed = !!target.isNewUnconfirmed
  const isAdded = target.rowStatus === 'added' || !!target.isImported
  // 新增记录（含导入/未确认新增）删除时直接移除，不能恢复
  if (isNewUnconfirmed || isAdded) {
    editPoints.value.splice(originalIndex, 1)
    if (pendingNewRow.value && pendingNewRow.value.point_id === item.point_id) {
      pendingNewRow.value = null
    }
    return
  }
  // 非新增记录：标记为删除，可恢复
  editPoints.value[originalIndex].rowStatus = 'deleted'
  ;(editPoints.value[originalIndex] as any).isInvalid = false
  // ElMessage.success('Point marked as deleted')
  recomputeAllValidity()
}
const restorePoint = (item: PointInfo) => {
  const originalIndex = editPoints.value.findIndex((p) => p.point_id === item.point_id)
  if (originalIndex !== -1) {
    editPoints.value[originalIndex].rowStatus = 'normal'
    delete editPoints.value[originalIndex].modifiedFields
    // 恢复后立即按当前内容重新校验（如原本有问题则仍判为无效）
    validateRowValidity(editPoints.value[originalIndex])
    applyDuplicatePointIdInvalid()
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
}
const handleCancelInlineEdit = (item: PointInfo) => {
  if (item.isNewUnconfirmed) {
    const idx = editPoints.value.findIndex((p) => p.point_id === item.point_id)
    if (idx !== -1) editPoints.value.splice(idx, 1)
    pendingNewRow.value = null
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
    item.isEditing = false
    delete item.originalData
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
  editPoints.value.unshift(newPoint)
  // 初始化该行的字段级错误
  refreshFieldErrorsForRow(newPoint as any)
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
// - 公共：point_id 为正整数；signal_name 非空且无空格；reverse 为布尔
// - T/A 专属：scale/offset 必须为数；unit 允许空字符串但不允许仅空白
// - S/C：无需校验 scale/offset/unit
function validateRowValidity(point: PointInfo): boolean {
  // 删除状态不做内容校验，视为有效
  if ((point as any).rowStatus === 'deleted') {
    ;(point as any).isInvalid = false
    return true
  }
  // 公共校验：point_id 为正整数，signal_name 无空格且非空，reverse 为布尔
  const isPositiveInt = (n: unknown) => Number.isInteger(n) && (n as number) > 0
  const hasNoWhitespace = (s: unknown) => typeof s === 'string' && s.length > 0 && /^\S+$/.test(s)
  const isBool = (v: unknown) => typeof v === 'boolean'

  let valid = true
  let reason = ''

  if (!isPositiveInt(point.point_id)) {
    valid = false
    reason = 'invalid point_id'
  } else if (!hasNoWhitespace(point.signal_name)) {
    valid = false
    reason = 'invalid signal_name (no spaces)'
  } else if (!isBool(point.reverse)) {
    valid = false
    reason = 'invalid reverse (must be true/false)'
  }

  // 仅 Telemetry/Adjustment 校验 scale/offset/unit
  if (valid && (props.pointType === 'T' || props.pointType === 'A')) {
    const isNum = (v: unknown) => typeof v === 'number' && Number.isFinite(v)
    // unit 允许为空串，但不允许仅包含空白
    const isUnitValid = (u: unknown) => typeof u === 'string' && (u === '' || /^\S+$/.test(u))

    if (!isNum(point.scale)) {
      valid = false
      reason = 'invalid scale (must be number)'
    } else if (!isNum(point.offset)) {
      valid = false
      reason = 'invalid offset (must be number)'
    } else if (!isUnitValid(point.unit)) {
      valid = false
      reason = 'invalid unit (no spaces)'
    }
  }

  // Signal/Control 无需校验 scale/offset/unit

  if (!valid) {
    ;(point as any).isInvalid = true
    point.description = reason
  } else {
    ;(point as any).isInvalid = false
    if (point.description && point.description.startsWith('Error:')) {
      point.description = ''
    }
  }
  return valid
}

// 检查并标记重复的 point_id（正整数）
function applyDuplicatePointIdInvalid() {
  const ids = (editPoints.value || [])
    .map((p: PointInfo) => toNumber((p as any).point_id))
    .filter((id) => isInteger(id) && id > 0)
  const counts = countBy(ids)
  ;(editPoints.value || []).forEach((p: any) => {
    const id = toNumber(p.point_id)
    const dup = isInteger(id) && id > 0 && (counts[id] || 0) > 1
    if (dup) {
      p.isInvalid = true
      p.description = 'Error: duplicate point_id'
    } else {
      // 如果之前是重复导致的错误且现在不重复，恢复为基础校验状态
      if (p.description === 'Error: duplicate point_id') {
        // 基础校验会在外部重新跑，这里不处理
        p.isInvalid = false
        p.description = ''
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
      if (!Number.isInteger(n) || n <= 0) return 'must be a positive integer'
      return ''
    }
    case 'signal_name': {
      const v = String(item.signal_name || '')
      if (!v || !/^\S+$/.test(v)) return 'required and cannot contain spaces'
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
      const u = String(item.unit ?? '')
      if (!(u === '' || /^\S+$/.test(u))) return 'unit must be empty or without spaces'
      return ''
    }
    default:
      return ''
  }
}
function onFieldInput(item: any, field: string) {
  const msg = validateFieldOnly(item, field)
  setFieldError(item, field, msg)
}
function refreshFieldErrorsForRow(item: any) {
  // 始终校验的字段
  setFieldError(item, 'point_id', validateFieldOnly(item, 'point_id'))
  setFieldError(item, 'signal_name', validateFieldOnly(item, 'signal_name'))
  // 仅在 T/A 下校验数值/单位字段
  if (isTA.value) {
    setFieldError(item, 'scale', validateFieldOnly(item, 'scale'))
    setFieldError(item, 'offset', validateFieldOnly(item, 'offset'))
    setFieldError(item, 'unit', validateFieldOnly(item, 'unit'))
  } else {
    // 非 T/A 清理这些字段的错误
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

const handleImportClick = () => {
  fileInputRef.value?.click()
}

// CSV 导入（Points）：
// - 期望表头：point_id,signal_name,scale,offset,unit,reverse
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

      // 验证表头
      const header = lines[0].trim()
      const expectedHeader = 'point_id,signal_name,scale,offset,unit,reverse'
      if (header !== expectedHeader) {
        ElMessage.error(`Invalid CSV header. Expected: ${expectedHeader}, Got: ${header}`)
        return
      }

      // 解析数据行
      const importedPoints: PointInfo[] = []
      const invalidRecords: number[] = []

      for (let i = 1; i < lines.length; i++) {
        const line = lines[i].trim()
        if (!line) continue

        const values = line.split(',').map((v) => v.trim())
        let hasError = false
        let errorReason = ''

        // 检查字段数量
        if (values.length !== 6) {
          hasError = true
          errorReason = 'incorrect number of fields'
        }

        const [pointIdStr, signalName, scaleStr, offsetStr, unit, reverseStr] = values

        // 验证必填字段
        if (!hasError && (!pointIdStr || !signalName)) {
          hasError = true
          errorReason = 'point_id and signal_name are required'
        }

        const pointId = Number(pointIdStr)
        if (!hasError && (!Number.isInteger(pointId) || pointId <= 0)) {
          hasError = true
          errorReason = 'invalid point_id'
        }

        // 解析可选字段
        const scale = scaleStr ? Number(scaleStr) : 1
        const offset = offsetStr ? Number(offsetStr) : 0

        // 解析 reverse（支持 true/false/0/1）
        let reverse = false
        if (!hasError && reverseStr) {
          const lowerReverse = reverseStr.toLowerCase()
          if (lowerReverse === 'true' || lowerReverse === '1') {
            reverse = true
          } else if (lowerReverse === 'false' || lowerReverse === '0') {
            reverse = false
          } else {
            hasError = true
            errorReason = 'invalid reverse value (must be true/false/0/1)'
          }
        }

        // 创建记录，包括错误的记录
        const point: PointInfo = {
          point_id: hasError ? -i : pointId,
          signal_name: signalName || '',
          scale,
          offset,
          unit: unit || '',
          data_type: 'float',
          reverse,
          description: hasError ? `Error: ${errorReason}` : '',
          rowStatus: 'added',
          isEditing: false,
        }

        // 如果有错误，标记为invalid
        if (hasError) {
          ;(point as any).isInvalid = true
          invalidRecords.push(i + 1)
        }

        // 标记为文件导入的记录
        ;(point as any).isImported = true
        ;(point as any).rowKey = createRowKey()
        ;(point as any).originalPointId = undefined

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

      // 直接更新本地数据，立即显示
      nextTick(() => {
        editPoints.value = importedPoints
        // 导入完成后进行一次重复ID与基础校验
        recomputeAllValidity()
        // 导入完成后为字段级错误做一次初始化校验
        refreshFieldErrorsForList()
      })
      if (invalidRecords.length > 0) {
        ElMessage.warning(
          `Imported ${importedPoints.length} records, ${invalidRecords.length} with errors (lines: ${invalidRecords.slice(0, 5).join(', ')}${invalidRecords.length > 5 ? '...' : ''})`,
        )
      } else {
        ElMessage.success(`Successfully imported ${importedPoints.length} points`)
      }
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

  // 生成CSV内容
  const header = 'point_id,signal_name,scale,offset,unit,reverse'
  const rows = editPoints.value.map((point) => {
    return [
      point.point_id,
      point.signal_name,
      point.scale,
      point.offset,
      point.unit,
      point.reverse ? 'true' : 'false',
    ].join(',')
  })

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
    :deep(.vue-recycle-scroller__item-wrapper:has()) {
    }
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
    width: 10%;
  }
  .vtable__cell--signal-name {
    width: 25%;
    position: relative;
    &.CorS {
      width: 35%;
    }
    .filter-icon {
      margin-left: 0.05rem;
      font-size: 0.14rem;
    }

    .signal-name-filter {
      position: absolute;
      top: 100%;
      left: 0;
      z-index: 100;
      background: #1e2f52;
      padding: 0.1rem;
      border-radius: 0.04rem;
      //   box-shadow: 0 0.02rem 0.1rem rgba(0, 0, 0, 0.3);
      min-width: 2.5rem;
    }
  }
  .vtable__cell--value {
    width: 12%;
    &.CorS {
      width: 22%;
    }
  }
  .vtable__cell--scale {
    width: 10%;
  }
  .vtable__cell--offset {
    width: 10%;
  }
  .vtable__cell--unit {
    width: 10%;
  }
  .vtable__cell--reverse {
    width: 10%;
    &.CorS {
      width: 20%;
    }
  }
  .vtable__cell--operation {
    width: 13%;

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
  .row-status-modified {
    border-left: 0.03rem solid #409eff;
  }
  .row-status-added {
    border-left: 0.03rem solid #67c23a;
  }
  .row-status-deleted {
    border-left: 0.03rem solid #f56c6c;
    opacity: 0.6;
  }
  .row-invalid {
    background-color: rgba(245, 108, 108, 0.1);
    border-left: 0.03rem solid #f56c6c;
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
    width: 14%;
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
