<template>
  <div class="voltage-class device-routing-table">
    <!-- 操作区：编辑时显示导入，否则显示导出 -->
    <div v-if="props.viewMode === 'routing'" class="table-action-controls">
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
        <el-button type="primary" @click="handleExport">Export</el-button>
      </template>
      <template v-else>
        <span v-if="importedFileName" class="imported-file-name">{{ importedFileName }}</span>
        <el-button type="primary" @mousedown="handleImportClick">Import</el-button>
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
        <div class="vtable__cell vtable__cell--channel-id">Channel</div>
        <div class="vtable__cell vtable__cell--channel-type">Channel Point Type</div>
        <div class="vtable__cell vtable__cell--channel-point">Channel Point</div>
        <div class="vtable__cell vtable__cell--enabled">Enabled</div>
        <div v-if="props.isEditing" class="vtable__cell vtable__cell--operation">
          <span>Operation</span>
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
                <span>{{ item.name }}</span>
              </div>
              <div class="vtable__cell vtable__cell--channel-id">
                <template v-if="props.isEditing && item.isEditing">
                  <div class="inline-edit-container">
                    <el-select
                      v-model="item.routing.channel_id"
                      popper-class="inline-mapping-popper"
                      :fit-input-width="true"
                      placeholder="Select channel"
                      @change="() => onSelectChannel(item)"
                      clearable
                      filterable
                    >
                      <el-option
                        v-for="opt in props.channels"
                        :key="opt.id"
                        :label="opt.name"
                        :value="opt.id"
                      />
                    </el-select>
                  </div>
                </template>
                <template v-else>
                  <span :class="getFieldClass(item, 'routing_channel_id')">{{
                    item.routing?.channel_name ?? ''
                  }}</span>
                </template>
                <div
                  v-if="props.isEditing && getRoutingFieldError(item, 'channel_id')"
                  class="field-error"
                >
                  {{ getRoutingFieldError(item, 'channel_id') }}
                </div>
              </div>
              <div class="vtable__cell vtable__cell--channel-type">
                <template v-if="props.isEditing && item.isEditing">
                  <div class="inline-edit-container">
                    <el-select
                      v-model="item.routing.channel_type"
                      popper-class="inline-mapping-popper"
                      :fit-input-width="true"
                      :disabled="!item.routing?.channel_id"
                      @change="() => onChannelTypeChange(item)"
                      clearable
                      filterable
                    >
                      <el-option
                        v-for="opt in getChannelTypeOptions()"
                        :key="opt.value"
                        :label="opt.label"
                        :value="opt.value"
                      />
                    </el-select>
                  </div>
                </template>
                <template v-else>
                  <span :class="getFieldClass(item, 'routing_channel_type')">{{
                    getChannelTypeLabel(item.routing?.channel_type)
                  }}</span>
                </template>
                <div
                  v-if="props.isEditing && getRoutingFieldError(item, 'channel_type')"
                  class="field-error"
                >
                  {{ getRoutingFieldError(item, 'channel_type') }}
                </div>
              </div>
              <div class="vtable__cell vtable__cell--channel-point">
                <template v-if="props.isEditing && item.isEditing">
                  <div class="inline-edit-container">
                    <el-select
                      v-model="item.routing.channel_point_id"
                      popper-class="inline-mapping-popper"
                      :fit-input-width="true"
                      placeholder="Select point"
                      :disabled="!item.routing?.channel_id || !item.routing?.channel_type"
                      @change="(val: string | number) => onSelectChannelPoint(item, val)"
                      clearable
                      filterable
                    >
                      <el-option
                        v-for="opt in getChannelPointOptions(item)"
                        :key="opt.value"
                        :label="opt.label"
                        :value="opt.value"
                      />
                    </el-select>
                  </div>
                </template>
                <template v-else>
                  <span :class="getFieldClass(item, 'routing_channel_point_id')">{{
                    item.routing?.channel_point_name ?? ''
                  }}</span>
                </template>
                <div
                  v-if="props.isEditing && getRoutingFieldError(item, 'channel_point_id')"
                  class="field-error"
                >
                  {{ getRoutingFieldError(item, 'channel_point_id') }}
                </div>
              </div>
              <div class="vtable__cell vtable__cell--enabled">
                <template v-if="props.isEditing && item.isEditing">
                  <div class="inline-edit-container">
                    <el-select
                      v-model="item.routing.enabled"
                      :teleported="false"
                      popper-class="inline-mapping-popper"
                      :fit-input-width="true"
                      placeholder="Select"
                      @change="() => onRoutingFieldChange(item, 'enabled')"
                      clearable
                      filterable
                    >
                      <el-option label="true" :value="true" />
                      <el-option label="false" :value="false" />
                    </el-select>
                  </div>
                </template>
                <template v-else>
                  <span :class="getFieldClass(item, 'routing_enabled')">{{
                    item.routing?.enabled === true
                      ? 'true'
                      : item.routing?.enabled === false
                        ? 'false'
                        : ''
                  }}</span>
                </template>
                <div
                  v-if="props.isEditing && getRoutingFieldError(item, 'enabled')"
                  class="field-error"
                >
                  {{ getRoutingFieldError(item, 'enabled') }}
                </div>
              </div>

              <template v-if="props.isEditing">
                <div class="vtable__cell vtable__cell--operation">
                  <div class="point-table__operation-cell">
                    <template v-if="item.isEditing">
                      <div class="point-table__confirm-btn" @click="handleConfirmEdit(item)">
                        <el-icon><Check /></el-icon>
                      </div>
                      <div class="point-table__cancel-btn" @click="handleCancelEdit(item)">
                        <el-icon><Close /></el-icon>
                      </div>
                    </template>
                    <template v-else>
                      <div class="point-table__edit-btn" @click="handleStartEdit(item)">
                        <el-icon><Edit /></el-icon>
                      </div>
                    </template>
                  </div>
                </div>
              </template>
            </div>
          </DynamicScrollerItem>
        </template>
      </DynamicScroller>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, computed, inject } from 'vue'
import { pxToResponsive } from '@/utils/responsive'
import { ElMessage } from 'element-plus'
import { Edit, Close, Check, Filter } from '@element-plus/icons-vue'
import type {
  InstanceActionItem,
  InstanceMeasurementItem,
  InstancePropertyItem,
  InstancePointRouting,
} from '@/types/deviceConfiguration'
import { InstanceNameKey } from '@/utils/key'
import { getPointsTables, getChannelsByIds } from '@/api/channelsManagement'
import type { PointInfoResponse, PointType } from '@/types/channelConfiguration'

interface Props {
  category: 'measurement' | 'action' | 'property'
  points: Array<InstanceActionItem | InstanceMeasurementItem | InstancePropertyItem>
  originalPoints?: Array<InstanceActionItem | InstanceMeasurementItem | InstancePropertyItem>
  viewMode: 'points' | 'routing'
  editFilters: string[]
  isEditing: boolean
  channels?: Array<{ id: number; name: string }>
}
const props = withDefaults(defineProps<Props>(), {
  viewMode: 'routing',
  editFilters: () => [],
  channels: () => [],
})
// Status 筛选逻辑已移至父组件，不再需要 emit

const editPoints = ref<any[]>([])
const scrollerRef = ref()
const rowHeight = ref(pxToResponsive(22))
const fileInputRef = ref<HTMLInputElement>()
const importedFileName = ref('')
const injectedInstanceName = inject(InstanceNameKey, ref(''))
const signalNameFilter = ref('')
const showSignalNameFilter = ref(false)

const signalNameOptions = computed(() => {
  const names = (editPoints.value || []).map((p: any) => p.name).filter((n: any) => n)
  return Array.from(new Set(names))
})

// 提前声明，避免 watch(immediate) 时访问未初始化的变量
let rowKeySeed = 1
function createRowKey(): string {
  rowKeySeed += 1
  return `${Date.now()}-${rowKeySeed}-${Math.random().toString(36).slice(2, 8)}`
}

// 通道与点位缓存（通道列表来自父组件）
const channelPointsCache = ref<Record<number, PointInfoResponse>>({})

// 列表筛选：支持 signal name 关键字与"Status（modified/invalid）"
// Status 筛选由父组件通过 editFilters prop 传递
const filteredPoints = computed(() => {
  const list = Array.isArray(editPoints.value) ? editPoints.value : []
  let result = [...list]
  if (signalNameFilter.value) {
    const kw = String(signalNameFilter.value || '').toLowerCase()
    result = result.filter((p) =>
      String(p.name || '')
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

// Status 筛选逻辑已移至父组件 PointsTablesDialog.vue

onMounted(() => {
  const onResize = () => (rowHeight.value = pxToResponsive(22))
  window.addEventListener('resize', onResize as any)
  ;(onResize as any)()
})
onUnmounted(() => {})

watch(
  () => ({ points: props.points, isEditing: props.isEditing }),
  async (val) => {
    if (!Array.isArray(val.points)) return

    // 统一处理：从原始数据创建副本，重置所有状态
    editPoints.value = val.points.map((item: any) => {
      const clone: any = {
        ...item,
        routing: item.routing
          ? { ...item.routing }
          : {
              channel_id: undefined,
              channel_point_id: undefined,
              channel_type: '',
              enabled: undefined,
              channel_name: '',
              channel_point_name: '',
            },
        rowStatus: 'normal',
        isEditing: false,
        isInvalid: false,
        modifiedFields: [],
        fieldErrors: {},
      }
      clone.rowKey = (item as any).rowKey || createRowKey()
      return clone
    })

    // 仅在编辑模式下执行校验
    if (val.isEditing) {
      importedFileName.value = ''
      // 进入编辑模式后，对当前路由情况进行一次总的检查
      editPoints.value.forEach((p: any) => {
        validateRoutingValidity(p)
        refreshRoutingFieldErrorsForRow(p)
      })
    }
  },
  { immediate: true, deep: true },
)

async function ensureChannelPoints(channelId: number) {
  if (!channelId) return
  try {
    // 跳过全局loading，因为这是后台数据加载，不应该影响全局loading状态
    const res = await getPointsTables(channelId, undefined, { skipGlobalLoading: true })
    if (res?.success && res.data) {
      channelPointsCache.value[channelId] = res.data as PointInfoResponse
    } else if (res && (res.telemetry || res.signal || res.control || res.adjustment)) {
      channelPointsCache.value[channelId] = res as unknown as PointInfoResponse
    }
  } catch (e) {
    // 忽略错误
  }
}

function getPointId(item: any): number {
  if (props.category === 'measurement') return Number(item.measurement_id)
  if (props.category === 'action') return Number(item.action_id)
  if (props.category === 'property') return Number(item.property_id)
  return 0
}
function getPointTypeChar(): 'M' | 'A' | 'P' {
  if (props.category === 'measurement') return 'M'
  if (props.category === 'action') return 'A'
  return 'P'
}

function getChannelTypeOptions() {
  if (props.category === 'measurement') {
    return [
      { label: 'Telemetry', value: 'T' },
      { label: 'Signal', value: 'S' },
    ]
  }
  if (props.category === 'action') {
    return [
      { label: 'Control', value: 'C' },
      { label: 'Adjustment', value: 'A' },
    ]
  }
  return [
    { label: 'Telemetry', value: 'T' },
    { label: 'Signal', value: 'S' },
    { label: 'Control', value: 'C' },
    { label: 'Adjustment', value: 'A' },
  ]
}

function getChannelTypeLabel(v?: string) {
  if (!v) return ''
  const map: Record<string, string> = {
    C: 'Control',
    S: 'Signal',
    T: 'Telemetry',
    A: 'Adjustment',
  }
  // 严格大小写：只接受大写缩写，其他都按原始值展示
  return map[String(v)] || String(v)
}

function parsePointTypeStrict(input: unknown): {
  isValidLiteral: boolean
  abbr?: 'T' | 'S' | 'C' | 'A'
} {
  const raw = String(input ?? '').trim()
  if (!raw) return { isValidLiteral: false }
  if (raw === 'T' || raw === 'S' || raw === 'C' || raw === 'A')
    return { isValidLiteral: true, abbr: raw }
  const fullToShort: Record<string, 'T' | 'S' | 'C' | 'A'> = {
    Telemetry: 'T',
    Signal: 'S',
    Control: 'C',
    Adjustment: 'A',
  }
  if (raw in fullToShort) return { isValidLiteral: true, abbr: fullToShort[raw] }
  return { isValidLiteral: false }
}

function getRowClass(item: any) {
  const classes = [`row-status-${item.rowStatus || 'normal'}`]
  // 只在编辑模式下显示错误行样式
  if (props.isEditing && (item as any).isInvalid) classes.push('row-invalid')
  // 只在编辑模式下显示修改行样式
  if (!props.isEditing) {
    // 非编辑模式下，移除修改相关的样式
    return 'row-status-normal'
  }
  return classes.join(' ')
}
function getFieldClass(item: any, fieldName: string) {
  // 只在编辑模式下显示修改字段样式
  if (!props.isEditing) return ''
  const status = item.rowStatus
  if (status === 'modified' && item.modifiedFields?.includes(fieldName)) return 'field-modified'
  return ''
}

function onSelectChannel(item: any) {
  const chId = Number(item.routing?.channel_id || 0)
  const ch = props.channels?.find((c) => Number(c.id) === chId)
  item.routing.channel_name = ch ? ch.name : ''
  // 重置后续选择
  item.routing.channel_type = ''
  item.routing.channel_point_id = undefined
  item.routing.channel_point_name = ''
  onRoutingFieldChange(item, 'channel_id')
  // 加载该通道点位
  if (chId > 0) ensureChannelPoints(chId)
}
function getChannelPointOptions(item: any) {
  const chId = Number(item.routing?.channel_id || 0)
  const tp = String(item.routing?.channel_type || '') as PointType
  const cache = channelPointsCache.value[chId]
  if (!chId || !tp || !cache) return []
  let list: any[] = []
  if (tp === 'T') list = cache.telemetry || []
  else if (tp === 'S') list = cache.signal || []
  else if (tp === 'C') list = cache.control || []
  else if (tp === 'A') list = cache.adjustment || []
  return (list || []).map((p) => ({
    label: p.signal_name || `#${p.point_id}`,
    value: p.point_id,
  }))
}
function onSelectChannelPoint(item: any, val: string | number) {
  const chId = Number(item.routing?.channel_id || 0)
  const tp = String(item.routing?.channel_type || '') as PointType
  const cache = channelPointsCache.value[chId]
  if (!cache) {
    onRoutingFieldChange(item, 'channel_point_id')
    return
  }
  let list: any[] = []
  if (tp === 'T') list = cache.telemetry || []
  else if (tp === 'S') list = cache.signal || []
  else if (tp === 'C') list = cache.control || []
  else if (tp === 'A') list = cache.adjustment || []
  const found = (list || []).find((p) => Number(p.point_id) === Number(val))
  item.routing.channel_point_name = found ? found.signal_name : ''
  onRoutingFieldChange(item, 'channel_point_id')
}
function onChannelTypeChange(item: any) {
  // 切换类型时清空 point 选择
  item.routing.channel_point_id = undefined
  item.routing.channel_point_name = ''
  onRoutingFieldChange(item, 'channel_type')
}

function handleStartEdit(item: any) {
  item.originalData = {
    routing_channel_id: item.routing?.channel_id,
    routing_channel_type: item.routing?.channel_type,
    routing_channel_point_id: item.routing?.channel_point_id,
    routing_enabled: item.routing?.enabled,
    routing_channel_name: item.routing?.channel_name,
    routing_channel_point_name: item.routing?.channel_point_name,
  }
  // 进入编辑时预加载：若已有 channel / type / point，准备好下拉数据
  const chId = Number(item.routing?.channel_id || 0)
  if (chId > 0) {
    const ch = props.channels?.find((c) => Number(c.id) === chId)
    if (ch && !item.routing?.channel_name) item.routing.channel_name = ch.name
    // 预加载点表，供 point 下拉使用
    ensureChannelPoints(chId)
  }
  item.isEditing = true
}
function handleCancelEdit(item: any) {
  if (item.originalData) {
    item.routing = {
      channel_id: item.originalData.routing_channel_id,
      channel_type: item.originalData.routing_channel_type,
      channel_point_id: item.originalData.routing_channel_point_id,
      enabled: item.originalData.routing_enabled,
      channel_name: item.originalData.routing_channel_name,
      channel_point_name: item.originalData.routing_channel_point_name,
    }
    delete item.originalData
  }
  item.isEditing = false
  // 取消编辑后，重置状态和错误
  item.rowStatus = 'normal'
  item.modifiedFields = []
  item.isInvalid = false
  item.fieldErrors = {}
  validateRoutingValidity(item)
}
function handleConfirmEdit(item: any) {
  // 补齐展示名称
  fillRoutingNames(item)
  // 基于父层原始基线做变更判断
  updateRoutingChangeStatus(item)
  validateRoutingValidity(item)
  // 刷新字段级别的错误提示
  refreshRoutingFieldErrorsForRow(item)
  delete item.originalData
  item.isEditing = false
}

function validateRoutingValidity(item: any): boolean {
  const r = item.routing || ({} as InstancePointRouting)
  let valid = true
  let reason = ''
  const isPositiveInt = (n: unknown) => Number.isInteger(n) && (n as number) > 0
  const allowedTypes = getChannelTypeOptions().map((o) => o.value)
  const meta = (item as any).__routingMeta as
    | { channelExists?: boolean; pointExists?: boolean }
    | undefined
  // 允许为空：仅当字段有值时才校验
  if (r.channel_id !== undefined && r.channel_id !== null && r.channel_id !== '') {
    if (!isPositiveInt(r.channel_id)) {
      valid = false
      reason = 'invalid channel_id'
    }
  }
  if (valid && meta?.channelExists === false) {
    valid = false
    reason = 'channel not exists'
  }
  if (valid && r.channel_type !== undefined && r.channel_type !== null && r.channel_type !== '') {
    const parsed = parsePointTypeStrict(r.channel_type)
    if (!parsed.isValidLiteral || !parsed.abbr) {
      valid = false
      reason = 'invalid channel_type'
    } else if (!allowedTypes.includes(parsed.abbr)) {
      valid = false
      reason = 'invalid channel_type'
    }
  }
  if (
    valid &&
    r.channel_point_id !== undefined &&
    r.channel_point_id !== null &&
    r.channel_point_id !== ''
  ) {
    if (!isPositiveInt(r.channel_point_id)) {
      valid = false
      reason = 'invalid channel_point_id'
    }
  }
  if (valid && meta?.pointExists === false) {
    valid = false
    reason = 'point not exists'
  }
  if (valid && r.enabled !== undefined && r.enabled !== null && r.enabled !== '') {
    if (!(r.enabled === true || r.enabled === false)) {
      valid = false
      reason = 'invalid enabled (bool)'
    }
  }
  ;(item as any).isInvalid = !valid
  return valid
}

function getRoutingFieldError(item: any, field: string): string {
  return (item.fieldErrors && item.fieldErrors[field]) || ''
}
function setRoutingFieldError(item: any, field: string, message: string) {
  if (!item.fieldErrors) item.fieldErrors = {}
  if (message) item.fieldErrors[field] = message
  else delete item.fieldErrors[field]
}
function validateRoutingFieldOnly(item: any, field: string): string {
  const r = item.routing || {}
  const meta = (item as any).__routingMeta as
    | { channelExists?: boolean; pointExists?: boolean }
    | undefined
  switch (field) {
    case 'channel_id': {
      if (r.channel_id === undefined || r.channel_id === null || r.channel_id === '') return ''
      const v = Number(r.channel_id)
      if (!Number.isInteger(v) || v <= 0) return 'must be positive integer'
      if (meta?.channelExists === false) return 'Does not exist'
      return ''
    }
    case 'channel_type': {
      if (r.channel_type === undefined || r.channel_type === null || r.channel_type === '')
        return ''
      // 属于 T/S/C/A：必须符合当前 category 允许的类型
      const allowedOptions = getChannelTypeOptions()
      const allowed = allowedOptions.map((o) => o.value)
      const labels = allowedOptions.map((o) => o.label).join(' / ')
      const parsed = parsePointTypeStrict(r.channel_type)
      // 非严格写法（t/telemetry/…）都按错误处理
      if (!parsed.isValidLiteral || !parsed.abbr) return `Must be ${labels}`
      if (!allowed.includes(parsed.abbr)) return `Must be ${labels}`
      return ''
    }
    case 'channel_point_id': {
      if (
        r.channel_point_id === undefined ||
        r.channel_point_id === null ||
        r.channel_point_id === ''
      )
        return ''
      const v = Number(r.channel_point_id)
      if (!Number.isInteger(v) || v <= 0) return 'must be positive integer'
      // 通道不存在：点位也视为不存在
      if (meta?.channelExists === false) return 'Does not exist'
      // 通道存在但点位不存在
      if (meta?.pointExists === false) return 'Does not exist'
      return ''
    }
    case 'enabled': {
      const v = r.enabled
      if (v === undefined || v === null || v === '') return ''
      if (!(v === true || v === false)) return 'must be boolean'
      return ''
    }
    default:
      return ''
  }
}
function refreshRoutingFieldErrorsForRow(item: any) {
  setRoutingFieldError(item, 'channel_id', validateRoutingFieldOnly(item, 'channel_id'))
  setRoutingFieldError(item, 'channel_type', validateRoutingFieldOnly(item, 'channel_type'))
  setRoutingFieldError(item, 'channel_point_id', validateRoutingFieldOnly(item, 'channel_point_id'))
  setRoutingFieldError(item, 'enabled', validateRoutingFieldOnly(item, 'enabled'))
}
function refreshRoutingFieldErrorsForList() {
  if (!Array.isArray(editPoints.value)) return
  editPoints.value.forEach((p: any) => refreshRoutingFieldErrorsForRow(p))
}
function onRoutingFieldChange(item: any, field: string) {
  updateRoutingChangeStatus(item)
  const msg = validateRoutingFieldOnly(item, field)
  setRoutingFieldError(item, field, msg)
}
function updateRoutingChangeStatus(item: any) {
  // 与原始基线（父组件传入的 originalPoints）进行对比，避免来回改动仍标记为修改
  const baseline = getOriginalBaselineByPointId(getPointId(item))
  const prev = {
    routing_channel_id: baseline?.routing?.channel_id,
    routing_channel_type: baseline?.routing?.channel_type,
    routing_channel_point_id: baseline?.routing?.channel_point_id,
    routing_enabled: baseline?.routing?.enabled,
  } as any
  const cur = item.routing || {}
  const changes: string[] = []
  const normInt = (v: any) => (v === '' || v === null || v === undefined ? null : Number(v))
  const normStr = (v: any) => String(v || '')
  const normBool = (v: any) => (v === true ? 'true' : v === false ? 'false' : '')
  if (normInt(cur.channel_id) !== normInt(prev.routing_channel_id))
    changes.push('routing_channel_id')
  if (normStr(cur.channel_type) !== normStr(prev.routing_channel_type))
    changes.push('routing_channel_type')
  if (normInt(cur.channel_point_id) !== normInt(prev.routing_channel_point_id))
    changes.push('routing_channel_point_id')
  if (normBool(cur.enabled) !== normBool(prev.routing_enabled)) changes.push('routing_enabled')

  if (changes.length > 0) {
    item.rowStatus = 'modified'
    item.modifiedFields = changes
  } else {
    item.rowStatus = 'normal'
    item.modifiedFields = []
  }
}

function getOriginalBaselineByPointId(pid: number): any | undefined {
  if (!Array.isArray((props as any).originalPoints)) return undefined
  return (props as any).originalPoints.find((p: any) => {
    if (!p) return false
    if (props.category === 'measurement') return Number(p.measurement_id) === pid
    if (props.category === 'action') return Number(p.action_id) === pid
    if (props.category === 'property') return Number(p.property_id) === pid
    return false
  })
}

function resolveChannelNameById(id?: number) {
  if (!id) return ''
  const ch = props.channels?.find((c) => Number(c.id) === Number(id))
  return ch ? String(ch.name || '') : ''
}
function resolvePointName(channelId?: number, type?: string, pointId?: number) {
  const chId = Number(channelId || 0)
  const tp = String(type || '') as PointType
  const pid = Number(pointId || 0)
  const cache = channelPointsCache.value[chId]
  if (!chId || !tp || !pid || !cache) return ''
  let list: any[] = []
  if (tp === 'T') list = cache.telemetry || []
  else if (tp === 'S') list = cache.signal || []
  else if (tp === 'C') list = cache.control || []
  else if (tp === 'A') list = cache.adjustment || []
  const found = (list || []).find((p) => Number(p.point_id) === pid)
  return found ? String(found.signal_name || '') : ''
}
function fillRoutingNames(item: any) {
  if (!item || !item.routing) return
  const r = item.routing
  // 若缺少名称，按当前选择补齐
  r.channel_name = resolveChannelNameById(r.channel_id) || r.channel_name || ''
  r.channel_point_name =
    resolvePointName(r.channel_id, r.channel_type, r.channel_point_id) || r.channel_point_name || ''
}

function getEditedData() {
  const updates: Array<{
    point_id: number
    point_type: 'M' | 'A'
    routing: InstancePointRouting
  }> = []
  editPoints.value.forEach((item: any) => {
    if (!item.routing) return
    const changed = item.modifiedFields || []
    const consider =
      changed.includes('routing_channel_id') ||
      changed.includes('routing_channel_type') ||
      changed.includes('routing_channel_point_id') ||
      changed.includes('routing_enabled')
    // 仅 measurement/action 输出
    const pt = getPointTypeChar()
    if (consider && (pt === 'M' || pt === 'A')) {
      const pid = getPointId(item)
      if (pid > 0) {
        updates.push({
          point_id: pid,
          point_type: pt,
          routing: {
            channel_id: Number(item.routing.channel_id),
            channel_type: String(item.routing.channel_type) as any,
            channel_point_id: Number(item.routing.channel_point_id),
            enabled: !!item.routing.enabled,
            channel_name: item.routing.channel_name,
            channel_point_name: item.routing.channel_point_name,
          },
        })
      }
    }
  })
  return updates
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
const handleFileChange = (event: Event) => {
  const target = event.target as HTMLInputElement
  const file = target.files?.[0]
  if (!file) return

  const reader = new FileReader()
  reader.onload = async (e) => {
    try {
      // 检查是否已获取到通道数组
      if (!props.channels || props.channels.length === 0) {
        ElMessage.error('Channels not loaded. Please wait for channels to load before importing.')
        target.value = ''
        return
      }

      const content = e.target?.result as string
      const lines = content.split('\n').filter((line) => line.trim())
      if (lines.length === 0) {
        ElMessage.error('CSV file is empty')
        return
      }
      // 解析表头，只要求必须的列存在，其他列忽略
      const header = lines[0].trim()
      const headerColumns = header.split(',').map((col) => col.trim().toLowerCase())

      // 必须的列（标准表头）
      const requiredColumns = [
        'point_id',
        'channel_id',
        'channel_point_type',
        'channel_point_id',
        'enabled',
      ]
      const columnIndices: Record<string, number> = {}

      // 检查必须的列是否存在
      for (const colName of requiredColumns) {
        const index = headerColumns.findIndex((h) => h === colName)
        if (index === -1) {
          ElMessage.error(`Required column "${colName}" not found in CSV header`)
          return
        }
        columnIndices[colName] = index
      }

      const byId: Record<
        number,
        {
          channel_id?: number
          channel_type: string
          channel_point_id?: number
          enabled: boolean
          isInvalid?: boolean
          reason?: string
        }
      > = {}
      const invalidRecords: number[] = []

      for (let i = 1; i < lines.length; i++) {
        const line = lines[i].trim()
        if (!line) continue

        const values = line.split(',').map((v) => v.trim())
        const getValue = (columnKey: string, defaultValue: string = ''): string => {
          const index = columnIndices[columnKey]
          return index !== undefined && index >= 0 && index < values.length
            ? values[index]
            : defaultValue
        }

        const pid = Number(getValue('point_id')) || 0
        let channel_id = getValue('channel_id') ? Number(getValue('channel_id')) : undefined
        const channel_type = String(getValue('channel_point_type') || '')
        const channel_point_id = getValue('channel_point_id')
          ? Number(getValue('channel_point_id'))
          : undefined
        const enabled = String(getValue('enabled')).toLowerCase() === 'true'

        let isInvalid = false
        let reason = ''

        if (!Number.isInteger(pid) || pid <= 0) {
          isInvalid = true
          reason = 'invalid point_id'
        }

        // 验证 channel_id（如果提供了 channel_id，必须是有效的正整数）
        if (
          !isInvalid &&
          channel_id !== undefined &&
          (!Number.isInteger(channel_id) || channel_id <= 0)
        ) {
          isInvalid = true
          reason = 'invalid channel_id'
        }

        byId[pid] = {
          channel_id,
          channel_type,
          channel_point_id,
          enabled,
          isInvalid,
          reason,
        }
        if (isInvalid) invalidRecords.push(i + 1)
      }

      importedFileName.value = file.name

      // 应用导入：整体覆盖，如果导入文件中没有某个点，清除该点的routing信息
      editPoints.value = await Promise.all(
        (editPoints.value as any[]).map(async (item: any) => {
          const pid = getPointId(item)
          const inc = byId[pid]

          if (!inc) {
            // 导入文件中没有该点，清除routing信息
            item.routing = {
              channel_id: undefined,
              channel_type: '',
              channel_point_id: undefined,
              enabled: undefined,
              channel_name: '',
              channel_point_name: '',
            }
            // 与原始基线对比，判断是否真的修改
            const baseline = getOriginalBaselineByPointId(pid)
            const prev = {
              routing_channel_id: baseline?.routing?.channel_id,
              routing_channel_type: baseline?.routing?.channel_type,
              routing_channel_point_id: baseline?.routing?.channel_point_id,
              routing_enabled: baseline?.routing?.enabled,
            } as any
            const cur = item.routing || {}
            const changes: string[] = []
            const normInt = (v: any) =>
              v === '' || v === null || v === undefined ? null : Number(v)
            const normStr = (v: any) => String(v || '')
            const normBool = (v: any) => (v === true ? 'true' : v === false ? 'false' : '')
            if (normInt(cur.channel_id) !== normInt(prev.routing_channel_id))
              changes.push('routing_channel_id')
            if (normStr(cur.channel_type) !== normStr(prev.routing_channel_type))
              changes.push('routing_channel_type')
            if (normInt(cur.channel_point_id) !== normInt(prev.routing_channel_point_id))
              changes.push('routing_channel_point_id')
            if (normBool(cur.enabled) !== normBool(prev.routing_enabled))
              changes.push('routing_enabled')

            if (changes.length > 0) {
              item.rowStatus = 'modified'
              item.modifiedFields = Array.from(
                new Set([...(item.modifiedFields || []), ...changes]),
              )
            } else {
              item.rowStatus = 'normal'
              item.modifiedFields = []
            }
            return item
          }

          // 导入文件中有该点，覆盖routing信息
          let finalChannelPointId = inc.channel_point_id
          let finalChannelPointName = ''

          // 验证通道是否存在（只检查 props.channels，不调用 API）
          const chId = Number(inc.channel_id || 0)
          let channelExists = false
          if (chId > 0) {
            channelExists = props.channels?.some((ch) => Number(ch.id) === chId) ?? false
          }

          // 验证 channel_type
          const channelTypeStr = String(inc.channel_type || '').trim()
          const allowedTypes = getChannelTypeOptions().map((o) => o.value)
          let finalChannelType = inc.channel_type as any
          let channelTypeError = ''

          if (channelTypeStr) {
            const allowedTypesLabels = getChannelTypeOptions()
              .map((o) => o.label)
              .join(' / ')

            const parsed = parsePointTypeStrict(channelTypeStr)
            if (!parsed.isValidLiteral || !parsed.abbr) {
              // 不合法输入（如 t、s、telemetry、signal）
              finalChannelType = channelTypeStr // 保留原值用于展示
              channelTypeError = `Must be ${allowedTypesLabels}`
              item.isInvalid = true
            } else {
              // 输入合法：写入缩写（T/S/C/A）
              finalChannelType = parsed.abbr
              // 但仍需符合当前 category 规则
              if (!allowedTypes.includes(parsed.abbr)) {
                channelTypeError = `Must be ${allowedTypesLabels}`
                item.isInvalid = true
              }
            }
          }

          // 设置 routing 数据
          // 如果通道不存在，channel_id 和 channel_name 都设置为导入文件中的 id
          if (!channelExists && chId > 0) {
            // 通道不存在：点位名称不应该来自任何“名称列”，这里只展示导入的点位ID（方便定位）
            finalChannelPointName =
              finalChannelPointId !== undefined && finalChannelPointId !== null
                ? String(finalChannelPointId)
                : ''
            item.isInvalid = true
            // channel_id 和 channel_name 都设置为导入文件中的 id
            item.routing = {
              channel_id: inc.channel_id,
              channel_type: finalChannelType,
              channel_point_id: finalChannelPointId,
              enabled: inc.enabled,
              channel_name: inc.channel_id ? String(inc.channel_id) : '',
              channel_point_name: finalChannelPointName,
            }
          } else {
            // 通道存在，正常设置
            item.routing = {
              channel_id: inc.channel_id,
              channel_type: finalChannelType,
              channel_point_id: finalChannelPointId,
              enabled: inc.enabled,
              channel_name: '',
              channel_point_name: finalChannelPointName,
            }
          }

          // 记录导入元信息：用于后续 getChannelsByIds 回填点位名称 & 校验“是否存在”
          ;(item as any).__routingMeta = {
            channelExists: channelExists ? true : chId > 0 ? false : undefined,
            // 先不判点位存在性，等 getChannelsByIds 回来再判断；如果通道不存在则点位也不存在
            pointExists: !channelExists && chId > 0 ? false : undefined,
            importedChannelId: inc.channel_id,
            importedChannelPointId: finalChannelPointId,
          }

          if (channelTypeError) {
            setRoutingFieldError(item, 'channel_type', channelTypeError)
            // 通道存在但通道类型不符合规范：点位也应视为不存在（上级条件不成立）
            const pointIdNum = Number(finalChannelPointId || 0)
            if (channelExists && pointIdNum > 0) {
              ;(item as any).__routingMeta.pointExists = false
              item.isInvalid = true
            }
          }

          // 与原始基线对比，判断是否真的修改
          const baseline = getOriginalBaselineByPointId(pid)
          const prev = {
            routing_channel_id: baseline?.routing?.channel_id,
            routing_channel_type: baseline?.routing?.channel_type,
            routing_channel_point_id: baseline?.routing?.channel_point_id,
            routing_enabled: baseline?.routing?.enabled,
          } as any
          const cur = item.routing || {}
          const changes: string[] = []
          const normInt = (v: any) => (v === '' || v === null || v === undefined ? null : Number(v))
          const normStr = (v: any) => String(v || '')
          const normBool = (v: any) => (v === true ? 'true' : v === false ? 'false' : '')
          if (normInt(cur.channel_id) !== normInt(prev.routing_channel_id))
            changes.push('routing_channel_id')
          if (normStr(cur.channel_type) !== normStr(prev.routing_channel_type))
            changes.push('routing_channel_type')
          if (normInt(cur.channel_point_id) !== normInt(prev.routing_channel_point_id))
            changes.push('routing_channel_point_id')
          if (normBool(cur.enabled) !== normBool(prev.routing_enabled))
            changes.push('routing_enabled')

          if (changes.length > 0) {
            item.rowStatus = 'modified'
            item.modifiedFields = Array.from(new Set([...(item.modifiedFields || []), ...changes]))
          } else {
            item.rowStatus = 'normal'
            item.modifiedFields = []
          }
          return item
        }),
      )

      // 执行校验，自动设置字段级错误和行级 invalid 状态
      editPoints.value.forEach((p: any) => validateRoutingValidity(p))

      // 统计需要获取通道信息的通道ID（只要在 props.channels 列表中存在，就去请求，避免被 isInvalid 误过滤）
      const channelIdsToFetch = new Set<number>()
      editPoints.value.forEach((item: any) => {
        const chId = Number(item.routing?.channel_id || 0)
        if (chId > 0) {
          const channelExists = props.channels?.some((ch) => Number(ch.id) === chId) ?? false
          if (channelExists) {
            channelIdsToFetch.add(chId)
          }
        }
      })

      // 批量获取通道信息并填充通道名称
      if (channelIdsToFetch.size > 0) {
        try {
          const idsArray = Array.from(channelIdsToFetch)
          const res = await getChannelsByIds(idsArray, { skipGlobalLoading: true })
          const channelsList = Array.isArray(res?.data?.list) ? res.data.list : []

          // 创建通道信息映射
          const channelMap = new Map<number, any>()
          channelsList.forEach((ch: any) => {
            const chId = Number(ch?.id)
            if (Number.isFinite(chId) && chId > 0) {
              channelMap.set(chId, ch)
            }
          })

          // 填充通道名称 + 分配点位名称（点位名称只允许来自 getChannelsByIds 返回的 channel.points）
          editPoints.value.forEach((item: any) => {
            const chId = Number(item.routing?.channel_id || 0)
            const meta = (item as any).__routingMeta as
              | { channelExists?: boolean; pointExists?: boolean }
              | undefined

            if (chId <= 0) return

            // 通道是否存在：以 props.channels 为准（用户要求：不在列表里就不存在）
            const channelExistsInList =
              props.channels?.some((ch) => Number(ch.id) === chId) ?? false
            if (meta) meta.channelExists = channelExistsInList ? true : false

            // 通道名称：优先接口返回，其次 props.channels 名称，最后回退到 id 字符串
            const channelFromApi = channelMap.get(chId)
            if (channelFromApi) {
              item.routing.channel_name = channelFromApi.name || item.routing.channel_name || ''
            } else {
              const channelFromList = props.channels?.find((ch) => Number(ch.id) === chId)
              item.routing.channel_name =
                channelFromList?.name || item.routing.channel_name || String(chId)
            }

            // 点位名称分配：仅当 channel_type 为 T/S/C/A 且存在 points 才可分配
            const rawType = String(item.routing?.channel_type || '').trim()
            const pointId = Number(item.routing?.channel_point_id || 0)
            const points = channelFromApi?.points
            const hasPoints = points && typeof points === 'object'

            // 默认：如果没有从接口解析到名称，则展示点位ID（方便排查）
            if (pointId > 0 && !item.routing.channel_point_name) {
              item.routing.channel_point_name = String(pointId)
            }

            // 通道存在但通道类型不符合当前规则：点位视为不存在（上级条件不成立）
            const allowedOptions = getChannelTypeOptions()
            const allowed = allowedOptions.map((o) => o.value)
            const parsedType = parsePointTypeStrict(rawType)
            const channelTypeValid = !!parsedType.abbr && allowed.includes(parsedType.abbr)
            if (pointId > 0 && !channelTypeValid) {
              if (meta) {
                meta.pointExists = false
              } else {
                ;(item as any).__routingMeta = {
                  ...(item as any).__routingMeta,
                  pointExists: false,
                }
              }
              // 点位不存在时只展示ID
              item.routing.channel_point_name = String(pointId)
              item.isInvalid = true
              return
            }

            // 没有接口 points 数据时，不对“点位是否存在”做结论，避免误报
            if (!hasPoints || pointId <= 0) {
              if (meta && meta.channelExists === true) meta.pointExists = undefined
              return
            }

            // 只有 TSCA 才能从 points 里取
            if (!parsedType.abbr) return

            let list: any[] = []
            if (parsedType.abbr === 'T') list = points.telemetry || []
            else if (parsedType.abbr === 'S') list = points.signal || []
            else if (parsedType.abbr === 'C') list = points.control || []
            else if (parsedType.abbr === 'A') list = points.adjustment || []

            const found = (list || []).find((p: any) => Number(p.point_id) === pointId)
            if (found) {
              item.routing.channel_point_name = found.signal_name || String(pointId)
              if (meta) meta.pointExists = true
            } else {
              // 通道存在但点位不存在
              item.routing.channel_point_name = String(pointId)
              if (meta) meta.pointExists = false
              item.isInvalid = true
            }
          })
        } catch (error) {
          console.error('Failed to fetch channel information:', error)
        }
      }

      // 刷新字段级错误
      refreshRoutingFieldErrorsForList()

      // 只显示简单的成功/信息提示，不显示详细错误（错误已在每行下方显示）
      if (invalidRecords.length > 0) {
        ElMessage.warning(
          `CSV file has ${invalidRecords.length} invalid row(s). Please check the format.`,
        )
      }

      const invalidCount = editPoints.value.filter((p: any) => (p as any).isInvalid).length
      if (invalidCount === 0) {
        ElMessage.success('Imported routing successfully')
      } else {
        ElMessage.warning(
          `Imported routing, but ${invalidCount} point(s) have errors. Please check the rows marked in red.`,
        )
      }
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
  const header =
    'point_id,point_name,channel_id,channel_name,channel_point_type,channel_point_id,channel_point_name,enabled'
  const rows = allPoints.map((item: any) => {
    const r = item.routing || {}
    // 导出格式：包含ID和名称
    return [
      getPointId(item),
      String(item.name || ''),
      r.channel_id ?? '',
      String(r.channel_name || ''),
      String(r.channel_type || ''),
      r.channel_point_id ?? '',
      String(r.channel_point_name || ''),
      r.enabled === true ? 'true' : r.enabled === false ? 'false' : '',
    ].join(',')
  })
  const csvContent = [header, ...rows].join('\n')
  const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' })
  const link = document.createElement('a')
  const url = URL.createObjectURL(blob)
  const safeName =
    String(injectedInstanceName?.value || '')
      .trim()
      .replace(/[^\w-]+/g, '_') || 'device'
  const filename = `${safeName}_${props.category}_routing_${Date.now()}.csv`
  link.href = url
  link.download = filename
  link.style.display = 'none'
  document.body.appendChild(link)
  link.click()
  document.body.removeChild(link)
  URL.revokeObjectURL(url)
  ElMessage.success(`Exported to ${filename}`)
}

// 检查是否有修改
const hasChanges = () => {
  if (!Array.isArray(editPoints.value)) return false
  return editPoints.value.some((p: any) => p && p.rowStatus === 'modified')
}

defineExpose({
  getEditedData,
  clearImportedFileName,
  clearSignalNameFilter: () => {
    signalNameFilter.value = ''
    showSignalNameFilter.value = false
  },
  hasInvalid: () => {
    return Array.isArray(editPoints.value)
      ? editPoints.value.some((p: any) => p && (p as any).isInvalid === true)
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
  .imported-file-name {
    color: #ff6900;
    font-size: 0.14rem;
    padding: 0 0.1rem;
  }
}

.voltage-class .device-routing-table {
  color: #fff;

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
  .inline-edit-container {
    width: 100%;
    position: relative;

    :deep(.el-input),
    :deep(.el-input-number),
    :deep(.el-select) {
      width: 100%;
    }
  }
  .inline-mapping-popper {
    z-index: 9999 !important;
  }

  .vtable__cell--point-id {
    width: 1.3rem;
  }
  .vtable__cell--name {
    width: 2.61rem; // 20%
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
  .vtable__cell--channel-id {
    width: 1.96rem;
  } // 15%
  .vtable__cell--channel-type {
    width: 1.96rem;
  } // 15%
  .vtable__cell--channel-point {
    width: 2.61rem;
  } // 20%
  .vtable__cell--enabled {
    width: 1.3rem;
  } // 10%
  .vtable__cell--operation {
    width: 1.3rem;
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
        gap: 0.15rem;
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

  .row-status-normal {
    background-color: transparent;
  }
  .row-status-modified {
    border-left: 0.03rem solid #409eff;
  }
  .row-invalid {
    background-color: rgba(245, 108, 108, 0.1);
    border-left: 0.03rem solid #f56c6c;
  }
  .field-modified {
    color: #409eff !important;
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
