<template>
  <div
    class="voltage-class device-point-table"
    :class="{
      'no-operation': !props.isEditing && !props.publishMode && props.category === 'property',
    }"
  >
    <div v-if="props.viewMode === 'points'" class="table-action-controls">
      <div style="flex: 1; display: flex; align-items: center; gap: 0.08rem">
        <span class="filter-label">Point Name:</span>
        <el-select
          v-model="signalNameFilter"
          filterable
          allow-create
          clearable
          popper-class="signal-name-popper"
          placeholder="Search Point Name"
          style="width: 2.8rem"
          :fit-input-width="true"
        >
          <el-option v-for="name in signalNameOptions" :key="name" :label="name" :value="name" />
        </el-select>
      </div>
      <template v-if="!props.isEditing">
        <!-- <el-button
          v-if="props.category === 'action'"
          :type="props.publishMode ? 'warning' : 'primary'"
          @click="handleTogglePublish"
        >
          {{ props.publishMode ? 'Cancel Execute' : 'Batch Execute' }}
        </el-button> -->
        <el-button v-if="!props.publishMode" type="primary" @click="handleExport">Export</el-button>
      </template>
      <template v-else>
        <span v-if="importedFileName" class="imported-file-name">{{ importedFileName }}</span>
        <el-button type="primary" @click="handleImportClick">Import</el-button>
      </template>
    </div>

    <input
      ref="fileInputRef"
      type="file"
      accept=".csv"
      style="display: none"
      @change="handleFileChange"
    />

    <div class="vtable" style="height: 5rem">
      <div class="vtable__header">
        <div class="vtable__cell vtable__cell--point-id">Point ID</div>
        <div class="vtable__cell vtable__cell--name">
          <span>Point Name</span>
        </div>
        <div class="vtable__cell vtable__cell--value">Value</div>
        <div class="vtable__cell vtable__cell--unit">Unit</div>
        <div class="vtable__cell vtable__cell--desc">Description</div>
        <div
          v-if="
            (!props.publishMode &&
              (props.category === 'action' || props.category === 'measurement') &&
              !props.isEditing) ||
            (props.isEditing && !props.publishMode)
          "
          class="vtable__cell vtable__cell--operation"
        >
          <template v-if="props.isEditing">
            <el-icon
              v-if="props.isEditing"
              style="cursor: pointer; color: #67c23a"
              @click="handleAddNewPoint"
            >
              <Plus />
            </el-icon>
          </template>
          <template v-else>
            <span>Operation</span>
          </template>
        </div>
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
                <span>{{ getPointId(item) }}</span>
              </div>
              <div class="vtable__cell vtable__cell--name">
                <template v-if="props.isEditing && item.isEditing">
                  <div class="inline-edit-container">
                    <el-input
                      v-model="item.name"
                      placeholder="Enter name"
                      @input="() => onFieldInput(item, 'name')"
                      style="width: 100% !important"
                    />
                  </div>
                </template>
                <template v-else>
                  <span :class="getFieldClass(item, 'name')">{{ item.name }}</span>
                </template>
                <div v-if="props.isEditing && getFieldError(item, 'name')" class="field-error">
                  {{ getFieldError(item, 'name') }}
                </div>
              </div>
              <div class="vtable__cell vtable__cell--value">
                <span class="value-field">{{ item.value ?? '-' }}</span>
              </div>
              <div class="vtable__cell vtable__cell--unit">
                <template v-if="props.isEditing && item.isEditing">
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
                  <span :class="getFieldClass(item, 'unit')">{{ item.unit || '' }}</span>
                </template>
                <div v-if="props.isEditing && getFieldError(item, 'unit')" class="field-error">
                  {{ getFieldError(item, 'unit') }}
                </div>
              </div>
              <div class="vtable__cell vtable__cell--desc">
                <template v-if="props.isEditing && item.isEditing">
                  <div class="inline-edit-container">
                    <el-input
                      v-model="item.description"
                      placeholder="Enter description"
                      @input="() => onFieldInput(item, 'description')"
                      style="width: 100% !important"
                    />
                  </div>
                </template>
                <template v-else>
                  <span :class="getFieldClass(item, 'description')">{{
                    item.description || ''
                  }}</span>
                </template>
                <div
                  v-if="props.isEditing && getFieldError(item, 'description')"
                  class="field-error"
                >
                  {{ getFieldError(item, 'description') }}
                </div>
              </div>
              <div
                v-if="
                  !props.publishMode &&
                  (props.isEditing ||
                    props.category === 'action' ||
                    props.category === 'measurement')
                "
                class="vtable__cell vtable__cell--operation"
              >
                <template v-if="props.isEditing">
                  <div class="point-table__operation-cell">
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
                  </div>
                </template>
                <template v-else>
                  <div
                    class="point-table__operation-cell"
                    v-if="props.category === 'action' || props.category === 'measurement'"
                  >
                    <div class="point-table__publish-btn" @click="handlePublish(item)">
                      <el-icon><Position /></el-icon>
                      <span>Execute</span>
                    </div>
                  </div>
                </template>
              </div>
              <!-- <div v-else class="vtable__cell vtable__cell--publish-value">
                <template v-if="props.category === 'action'">
                  <el-select
                    v-model="publishValues[getPointId(item)]"
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
              </div> -->
            </div>
          </DynamicScrollerItem>
        </template>
      </DynamicScroller>
    </div>
    <ExecuteDialog ref="executeDialogRef" />
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, computed, nextTick, inject } from 'vue'
import { pxToResponsive } from '@/utils/responsive'
import { ElMessage } from 'element-plus'
import {
  Position,
  Edit,
  Delete,
  RefreshLeft,
  Close,
  Check,
  Plus,
  Filter,
} from '@element-plus/icons-vue'
import ExecuteDialog from './ExecuteDialog.vue'
import type {
  InstanceActionItem,
  InstanceMeasurementItem,
  InstancePropertyItem,
} from '@/types/deviceConfiguration'
import { InstanceNameKey } from '@/utils/key'

interface Props {
  category: 'measurement' | 'action' | 'property'
  points: Array<InstanceActionItem | InstanceMeasurementItem | InstancePropertyItem>
  originalPoints?: Array<InstanceActionItem | InstanceMeasurementItem | InstancePropertyItem>
  viewMode: 'points' | 'routing'
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
}>()

const editPoints = ref<any[]>([])
const scrollerRef = ref()
const rowHeight = ref(pxToResponsive(36))
const publishValues = ref<Record<number | string, any>>({})
const fileInputRef = ref<HTMLInputElement>()
const importedFileName = ref('')
const injectedInstanceName = inject(InstanceNameKey, ref(''))
const executeDialogRef = ref<{
  open: (point_id: string, category: 'action' | 'measurement') => void
} | null>(null)

// Name 关键字筛选
const signalNameFilter = ref('')
const showSignalNameFilter = ref(false)
const signalNameOptions = computed(() => {
  const names = (editPoints.value || []).map((p: any) => String(p.name || '')).filter((n) => n)
  return Array.from(new Set(names))
})

const originalPointsList = computed<any[]>(() => {
  return Array.isArray(props.originalPoints) ? (props.originalPoints as any[]) : []
})

const filteredPoints = computed(() => {
  const list = Array.isArray(editPoints.value) ? editPoints.value : []
  let result = [...list]
  if (signalNameFilter.value) {
    const kw = String(signalNameFilter.value || '').toLowerCase()
    result = result.filter((p: any) =>
      String(p.name || '')
        .toLowerCase()
        .includes(kw),
    )
  }
  if ((props.editFilters || []).length > 0) {
    result = result.filter((p: any) => {
      const status = p.rowStatus || 'normal'
      if (props.editFilters.includes('invalid')) {
        return (p as any).isInvalid === true
      }
      return props.editFilters.includes(status)
    })
  }
  return result
})

// 先声明 rowKeySeed 与 createRowKey，避免 watch(immediate) 期间访问未初始化变量
let rowKeySeed = 1
function createRowKey(): string {
  rowKeySeed += 1
  return `${Date.now()}-${rowKeySeed}-${Math.random().toString(36).slice(2, 8)}`
}

onMounted(() => {
  const onResize = () => (rowHeight.value = pxToResponsive(36))
  window.addEventListener('resize', onResize as any)
  ;(onResize as any)()
})
onUnmounted(() => {})

watch(
  () => ({ points: props.points, isEditing: props.isEditing }),
  (val) => {
    if (Array.isArray(val.points)) {
      editPoints.value = val.points.map((item: any) => {
        const clone = { ...item }
        clone.rowKey = (item as any).rowKey || createRowKey()
        clone.rowStatus = clone.rowStatus || 'normal'
        return clone
      })
      editPoints.value.forEach((p: any) => validateRowValidity(p))
      refreshFieldErrorsForList()
    }
  },
  { immediate: true, deep: true },
)

function getPointId(item: any): number {
  if (props.category === 'measurement') return Number(item.measurement_id)
  if (props.category === 'action') return Number(item.action_id)
  if (props.category === 'property') return Number(item.property_id)
  return 0
}

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
    case 'name': {
      const v = String(item.name || '')
      if (!v || !/^\S.+$/.test(v)) return 'required'
      return ''
    }
    case 'unit': {
      const u = String(item.unit ?? '')
      if (!(u === '' || /^\S+$/.test(u))) return 'unit must be empty or without spaces'
      return ''
    }
    case 'description':
      return ''
    default:
      return ''
  }
}
function onFieldInput(item: any, field: string) {
  const msg = validateFieldOnly(item, field)
  setFieldError(item, field, msg)
}
function refreshFieldErrorsForRow(item: any) {
  setFieldError(item, 'name', validateFieldOnly(item, 'name'))
  setFieldError(item, 'unit', validateFieldOnly(item, 'unit'))
  setFieldError(item, 'description', validateFieldOnly(item, 'description'))
}
function refreshFieldErrorsForList() {
  if (!Array.isArray(editPoints.value)) return
  editPoints.value.forEach((p: any) => refreshFieldErrorsForRow(p))
}
function getFieldClass(item: any, fieldName: string) {
  const status = item.rowStatus
  if (status === 'added') return 'field-added'
  if (status === 'modified' && item.modifiedFields?.includes(fieldName)) return 'field-modified'
  if (status === 'deleted') return 'field-deleted'
  return ''
}
function validateRowValidity(item: any): boolean {
  if ((item as any).rowStatus === 'deleted') {
    ;(item as any).isInvalid = false
    return true
  }
  let valid = true
  let reason = ''
  const nameOk = String(item.name || '').trim().length > 0
  const unitOk = (() => {
    const u = String(item.unit ?? '')
    return u === '' || /^\S+$/.test(u)
  })()
  if (!nameOk) {
    valid = false
    reason = 'invalid name'
  } else if (!unitOk) {
    valid = false
    reason = 'invalid unit'
  }
  ;(item as any).isInvalid = !valid
  if (!valid) item.description = reason
  return valid
}
function recomputeAllValidity() {
  if (!Array.isArray(editPoints.value)) return
  editPoints.value.forEach((p: any) => validateRowValidity(p))
}

function getRowClass(item: any) {
  const baseClass = (item as any).isImported
    ? 'row-status-added'
    : `row-status-${item.rowStatus || 'normal'}`
  const classes: string[] = [baseClass]
  if (props.isEditing && (item as any).isInvalid) classes.push('row-invalid')
  return classes.join(' ')
}

function getOriginalById(id: number): any | undefined {
  return (originalPointsList.value || []).find((p: any) => getPointId(p) === id)
}
function handleStartInlineEdit(item: any) {
  item.originalData = { name: item.name, unit: item.unit, description: item.description }
  item.isEditing = true
}
function handleCancelInlineEdit(item: any) {
  if (item.isNewUnconfirmed) {
    const idx = editPoints.value.findIndex((p) => getPointId(p) === getPointId(item))
    if (idx !== -1) editPoints.value.splice(idx, 1)
  } else if (item.originalData) {
    item.name = item.originalData.name
    item.unit = item.originalData.unit
    item.description = item.originalData.description
    delete item.originalData
  }
  item.isEditing = false
  recomputeAllValidity()
}
function handleConfirmInlineEdit(item: any) {
  const id = getPointId(item)
  const original = getOriginalById(id)
  const prev = item.originalData || {}
  const changes: string[] = []
  if (original) {
    if (item.name !== original.name && item.name !== prev.name) changes.push('name')
    if (item.unit !== original.unit && item.unit !== prev.unit) changes.push('unit')
    if (item.description !== original.description && item.description !== prev.description)
      changes.push('description')
  } else {
    if (item.name !== prev.name) changes.push('name')
    if (item.unit !== prev.unit) changes.push('unit')
    if (item.description !== prev.description) changes.push('description')
  }
  if (item.isNewUnconfirmed) {
    item.isNewUnconfirmed = false
    item.rowStatus = 'added'
  } else if (changes.length > 0) {
    item.rowStatus = 'modified'
  } else {
    item.rowStatus = 'normal'
  }
  item.modifiedFields = changes
  validateRowValidity(item)
  item.isEditing = false
  delete item.originalData
}
function deletePoint(item: any) {
  const idx = editPoints.value.findIndex((p) => getPointId(p) === getPointId(item))
  if (idx === -1) return
  const target: any = editPoints.value[idx]
  if (target.isNewUnconfirmed || target.rowStatus === 'added') {
    editPoints.value.splice(idx, 1)
    return
  }
  target.rowStatus = 'deleted'
  ;(target as any).isInvalid = false
  recomputeAllValidity()
}
function restorePoint(item: any) {
  const idx = editPoints.value.findIndex((p) => getPointId(p) === getPointId(item))
  if (idx !== -1) {
    editPoints.value[idx].rowStatus = 'normal'
    delete editPoints.value[idx].modifiedFields
    validateRowValidity(editPoints.value[idx])
  }
}
function handleAddNewPoint() {
  const nextId = getNextPointId()
  const newItem: any = {
    name: '',
    unit: '',
    description: '',
    value: '-',
    isEditing: true,
    isNewUnconfirmed: true,
    rowStatus: 'normal',
  }
  if (props.category === 'measurement') newItem.measurement_id = nextId
  else if (props.category === 'action') newItem.action_id = nextId
  else if (props.category === 'property') newItem.property_id = nextId
  ;(newItem as any).rowKey = createRowKey()
  editPoints.value.unshift(newItem)
  refreshFieldErrorsForRow(newItem)
}
function getNextPointId(): number {
  const ids = (editPoints.value || [])
    .map((p) => getPointId(p))
    .filter((id) => Number.isInteger(id) && id > 0)
  if (!ids.length) return 1
  return Math.max(...ids) + 1
}
function notifyPublishChange() {
  emit('publish-change', hasPublishChanges())
}
function hasPublishChanges(): boolean {
  const cmds = getPublishCommands()
  return cmds.length > 0
}
function resetPublish() {
  publishValues.value = {}
  notifyPublishChange()
}
function getPublishCommands(): Array<{ id: string; value: number }> {
  if (props.category !== 'action') return []
  const commands: Array<{ id: string; value: number }> = []
  Object.entries(publishValues.value).forEach(([key, val]) => {
    if (val !== '' && val !== null && val !== undefined) {
      const num = Number(val)
      commands.push({ id: key, value: num })
    }
  })
  return commands
}

function applyRealtimeValues(values: Record<string | number, number>) {
  if (!values || !Array.isArray(editPoints.value)) return
  const valueMap = new Map<number, number>()
  Object.entries(values).forEach(([k, v]) => {
    const id = Number(k)
    if (Number.isFinite(id)) valueMap.set(id, Number(v))
  })
  if (valueMap.size === 0) return
  editPoints.value.forEach((p: any) => {
    const id = getPointId(p)
    const newVal = valueMap.get(id)
    if (newVal !== undefined) {
      p.value = newVal
    }
  })
}

function handlePublish(item: any) {
  const id = getPointId(item)
  executeDialogRef.value?.open(String(id), props.category as 'action' | 'measurement')
}

const handleTogglePublish = () => {
  emit('toggle-publish')
}

const handleImportClick = () => {
  fileInputRef.value?.click()
}
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
      // 导入格式：point_id,point_name,value,unit,description
      const header = lines[0].trim()
      const expectedHeader = 'point_id,point_name,value,unit,description'
      if (header !== expectedHeader) {
        ElMessage.error(`Invalid CSV header. Expected: ${expectedHeader}, Got: ${header}`)
        return
      }
      const byId: Record<
        number,
        {
          name?: string
          value?: number
          unit?: string
          description?: string
        }
      > = {}
      for (let i = 1; i < lines.length; i++) {
        const line = lines[i].trim()
        if (!line) continue
        const [idStr, nameStr, valueStr, unitStr, descStr] = line.split(',').map((v) => v.trim())
        const pid = Number(idStr)
        if (!Number.isInteger(pid) || pid <= 0) continue
        const v = valueStr ? Number(valueStr) : undefined
        byId[pid] = {
          name: nameStr || undefined,
          value: v !== undefined && Number.isFinite(v) ? v : undefined,
          unit: unitStr || undefined,
          description: descStr || undefined,
        }
      }
      importedFileName.value = file.name
      nextTick(() => {
        // 将导入数据应用到所有数据（不仅仅是当前可见的）
        editPoints.value = (editPoints.value || []).map((item: any) => {
          const id = getPointId(item)
          const imported = byId[id]
          if (imported) {
            if (imported.name !== undefined) item.name = imported.name
            if (imported.value !== undefined) item.value = imported.value
            if (imported.unit !== undefined) item.unit = imported.unit
            if (imported.description !== undefined) item.description = imported.description
          }
          return item
        })
      })
      ElMessage.success('Imported data applied')
    } catch (error) {
      console.error('Error parsing CSV:', error)
      ElMessage.error('Failed to parse CSV file')
    } finally {
      target.value = ''
    }
  }
  reader.onerror = () => {
    ElMessage.error('Failed to read CSV file')
    target.value = ''
  }
  reader.readAsText(file)
}
const clearImportedFileName = () => {
  importedFileName.value = ''
}
const handleExport = () => {
  // 导出所有数据，不受筛选影响
  const allPoints = Array.isArray(props.points) ? props.points : editPoints.value || []
  if (!allPoints || allPoints.length === 0) {
    ElMessage.warning('No data to export')
    return
  }
  const header = 'point_id,point_name,value,unit,description'
  const rows = allPoints.map((item: any) => {
    const id = getPointId(item)
    const name = String(item.name || '')
    const value = item.value ?? ''
    const unit = String(item.unit || '')
    const description = String(item.description || '')
    return [id, name, value, unit, description].join(',')
  })
  const csvContent = [header, ...rows].join('\n')
  const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' })
  const link = document.createElement('a')
  const url = URL.createObjectURL(blob)
  const safeName =
    String(injectedInstanceName?.value || '')
      .trim()
      .replace(/[^\w-]+/g, '_') || 'device'
  const filename = `${safeName}_${props.category}_points_${Date.now()}.csv`
  link.href = url
  link.download = filename
  link.style.display = 'none'
  document.body.appendChild(link)
  link.click()
  document.body.removeChild(link)
  URL.revokeObjectURL(url)
  ElMessage.success(`Exported to ${filename}`)
}

defineExpose({
  getPublishCommands,
  resetPublish,
  hasPublishChanges,
  applyRealtimeValues,
  clearImportedFileName,
  getEditedData: () => editPoints.value,
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

.voltage-class .device-point-table {
  color: #fff;

  .vtable__body {
    position: relative;
    z-index: 1;
    :deep(.vue-recycle-scroller__item-wrapper) {
      overflow: visible !important;
    }
    :deep(.vue-recycle-scroller__item-view) {
      overflow: visible !important;
      z-index: 100;
      @for $i from 1 through 40 {
        &:nth-child(#{$i}) {
          z-index: #{100 - $i};
        }
      }
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
  .vtable__header {
    position: relative;
    padding-left: 0.03rem;
    padding-right: 0.08rem;
  }
  .row-status-float {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 0.03rem;
    background: transparent;
    pointer-events: none;
  }
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
  .vtable__cell--point-id {
    width: 1.3rem;
  }
  .vtable__cell--name {
    width: 3.26rem;
    position: relative;
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
      min-width: 2.5rem;
    }
  }
  // 让筛选下拉与输入框左侧对齐
  :deep(.signal-name-popper) {
    left: 0 !important;
    transform: none !important;
    min-width: 100% !important;
  }
  .vtable__cell--value {
    width: 1.56rem;
  }
  .vtable__cell--unit {
    width: 1.3rem;
  }
  .vtable__cell--desc {
    width: 4.31rem;
  } // 33% of 13.07 -> 4.31
  .vtable__cell--operation {
    width: 1.69rem;

    .point-table__operation-cell {
      display: flex;
      gap: 0.15rem;
      align-items: center;
      justify-content: center;

      .point-table__edit-btn,
      .point-table__delete-btn,
      .point-table__restore-btn {
        cursor: pointer;
        display: flex;
        align-items: center;
        gap: 0.04rem;
        font-size: 0.16rem;
        transition: color 0.3s;
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
    }
  }
  &.no-operation {
    .vtable__cell--desc {
      width: 43%;
    }
  }

  .point-table__operation-cell {
    display: flex;
    gap: 0.15rem;
    align-items: center;
    justify-content: center;
  }
  .point-table__publish-btn {
    color: #fff;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 0.04rem;
    font-size: 0.16rem;
    transition: color 0.3s;
    &:hover {
      color: #ff6900;
    }
    span {
      font-size: 0.12rem;
    }
  }

  .vtable__row {
    min-height: 0.36rem;
    position: relative;
    z-index: 1;
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
  .value-field {
    color: #fff !important;
  }
  .vtable__cell--publish-value {
    width: 14%;
    position: relative;
    z-index: 10;
    &:has(.el-select.is-focus),
    &:has(.el-select:hover) {
      z-index: 100;
    }
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
  .field-error {
    position: absolute;
    bottom: 0.02rem;
    left: 0.12rem;
    width: 100%;
    color: #ff4d4f;
    font-size: 0.12rem;
    line-height: 1;
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
}
</style>
