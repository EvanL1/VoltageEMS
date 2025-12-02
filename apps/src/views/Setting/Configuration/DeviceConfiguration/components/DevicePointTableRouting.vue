<template>
  <div class="voltage-class device-routing-table">
    <!-- 操作区：编辑时显示导入，否则显示导出 -->
    <div v-if="props.viewMode === 'routing'" class="table-action-controls">
      <template v-if="!props.isEditing">
        <el-button type="primary" @click="handleExport">Export</el-button>
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
          <span>Name</span>
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
              :teleported="false"
              placeholder="Search Name"
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
        <div class="vtable__cell vtable__cell--channel-id">Channel</div>
        <div class="vtable__cell vtable__cell--channel-type">Channel Type</div>
        <div class="vtable__cell vtable__cell--channel-point">Point</div>
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
                      :teleported="false"
                      popper-class="inline-mapping-popper"
                      :fit-input-width="true"
                      placeholder="Select channel"
                      @change="() => onSelectChannel(item)"
                    >
                      <el-option
                        v-for="opt in props.channels || []"
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
                      :teleported="false"
                      popper-class="inline-mapping-popper"
                      :fit-input-width="true"
                      :disabled="!item.routing?.channel_id"
                      @change="() => onChannelTypeChange(item)"
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
                      :teleported="false"
                      popper-class="inline-mapping-popper"
                      :fit-input-width="true"
                      placeholder="Select point"
                      :disabled="!item.routing?.channel_id || !item.routing?.channel_type"
                      @change="(val: string | number) => onSelectChannelPoint(item, val)"
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
                      <div class="point-table__cancel-btn" @click="handleCancelEdit(item)">
                        <el-icon><Close /></el-icon>
                      </div>
                      <div class="point-table__confirm-btn" @click="handleConfirmEdit(item)">
                        <el-icon><Check /></el-icon>
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
import { getPointsTables } from '@/api/channelsManagement'
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
  const onResize = () => (rowHeight.value = pxToResponsive(22))
  window.addEventListener('resize', onResize as any)
  ;(onResize as any)()
})
onUnmounted(() => {})

watch(
  () => ({ points: props.points, isEditing: props.isEditing }),
  (val) => {
    if (Array.isArray(val.points)) {
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
          rowStatus: item.rowStatus || 'normal',
        }
        clone.rowKey = (item as any).rowKey || createRowKey()
        return clone
      })
      // 首次初始化校验
      editPoints.value.forEach((p: any) => validateRoutingValidity(p))
      refreshRoutingFieldErrorsForList()
    }
  },
  { immediate: true, deep: true },
)

async function ensureChannelPoints(channelId: number) {
  if (!channelId || channelPointsCache.value[channelId]) return
  try {
    const res = await getPointsTables(channelId)
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
  const map: Record<string, string> = {
    C: 'Control',
    S: 'Signal',
    T: 'Telemetry',
    A: 'Adjustment',
  }
  const key = String(v || '').toUpperCase()
  return map[key] || ''
}

function getRowClass(item: any) {
  const classes = [`row-status-${item.rowStatus || 'normal'}`]
  if (props.isEditing && (item as any).isInvalid) classes.push('row-invalid')
  return classes.join(' ')
}
function getFieldClass(item: any, fieldName: string) {
  const status = item.rowStatus
  if (status === 'added') return 'field-added'
  if (status === 'modified' && item.modifiedFields?.includes(fieldName)) return 'field-modified'
  if (status === 'deleted') return 'field-deleted'
  return ''
}

function onSelectChannel(item: any) {
  const chId = Number(item.routing?.channel_id || 0)
  const ch = (props.channels || []).find((c) => Number(c.id) === chId)
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
    const ch = (props.channels || []).find((c) => Number(c.id) === chId)
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
  validateRoutingValidity(item)
}
function handleConfirmEdit(item: any) {
  // 补齐展示名称
  fillRoutingNames(item)
  // 基于父层原始基线做变更判断
  updateRoutingChangeStatus(item)
  validateRoutingValidity(item)
  delete item.originalData
  item.isEditing = false
}

function validateRoutingValidity(item: any): boolean {
  const r = item.routing || ({} as InstancePointRouting)
  let valid = true
  let reason = ''
  const isPositiveInt = (n: unknown) => Number.isInteger(n) && (n as number) > 0
  const allowedTypes = getChannelTypeOptions().map((o) => o.value)
  if (!isPositiveInt(r.channel_id)) {
    valid = false
    reason = 'invalid channel_id'
  } else if (!allowedTypes.includes(String(r.channel_type))) {
    valid = false
    reason = 'invalid channel_type'
  } else if (!isPositiveInt(r.channel_point_id)) {
    valid = false
    reason = 'invalid channel_point_id'
  } else if (!(r.enabled === true || r.enabled === false)) {
    valid = false
    reason = 'invalid enabled (bool)'
  }
  ;(item as any).isInvalid = !valid
  if (!valid) item.description = `Error: ${reason}`
  else if (item.description && String(item.description).startsWith('Error:')) item.description = ''
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
  switch (field) {
    case 'channel_id': {
      const v = Number(r.channel_id)
      if (!Number.isInteger(v) || v <= 0) return 'must be positive integer'
      return ''
    }
    case 'channel_type': {
      const allowed = getChannelTypeOptions().map((o) => o.value)
      const v = String(r.channel_type || '')
      if (!allowed.includes(v)) return 'not allowed'
      return ''
    }
    case 'channel_point_id': {
      const v = Number(r.channel_point_id)
      if (!Number.isInteger(v) || v <= 0) return 'must be positive integer'
      return ''
    }
    case 'enabled': {
      const v = r.enabled
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
  const ch = (props.channels || []).find((c) => Number(c.id) === Number(id))
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
      // 期望表头
      const header = lines[0].trim()
      const expectedHeader = 'point_id,channel_id,channel_type,channel_point_id,enabled'
      if (header !== expectedHeader) {
        ElMessage.error(`Invalid CSV header. Expected: ${expectedHeader}, Got: ${header}`)
        return
      }
      const byId: Record<
        number,
        {
          channel_id: number
          channel_type: string
          channel_point_id: number
          enabled: boolean
          isInvalid?: boolean
          reason?: string
        }
      > = {}
      const invalidRecords: number[] = []

      for (let i = 1; i < lines.length; i++) {
        const line = lines[i].trim()
        if (!line) continue
        const [pidStr, chStr, typeStr, cpStr, enStr] = line.split(',').map((v) => v.trim())
        const pid = Number(pidStr)
        const channel_id = Number(chStr)
        const channel_type = String(typeStr || '')
        const channel_point_id = Number(cpStr)
        const enabled = ['1', 'true', 'TRUE'].includes(String(enStr))
        let isInvalid = false
        let reason = ''
        const allowed = ['T', 'S', 'C', 'A']
        if (!Number.isInteger(pid) || pid <= 0) {
          isInvalid = true
          reason = 'invalid point_id'
        }
        if (!isInvalid && (!Number.isInteger(channel_id) || channel_id <= 0)) {
          isInvalid = true
          reason = 'invalid channel_id'
        }
        if (!isInvalid && !allowed.includes(channel_type)) {
          isInvalid = true
          reason = 'invalid channel_type'
        }
        if (!isInvalid && (!Number.isInteger(channel_point_id) || channel_point_id <= 0)) {
          isInvalid = true
          reason = 'invalid channel_point_id'
        }
        byId[pid] = { channel_id, channel_type, channel_point_id, enabled, isInvalid, reason }
        if (isInvalid) invalidRecords.push(i + 1)
      }

      importedFileName.value = file.name
      // 应用导入
      editPoints.value = (editPoints.value as any[]).map((item: any) => {
        const pid = getPointId(item)
        const inc = byId[pid]
        if (!inc) return item
        item.routing = {
          channel_id: inc.channel_id,
          channel_type: inc.channel_type as any,
          channel_point_id: inc.channel_point_id,
          enabled: inc.enabled,
        }
        if (inc.isInvalid) {
          item.isInvalid = true
          item.description = `Error: ${inc.reason}`
        } else {
          item.isInvalid = false
          if (item.description && item.description.startsWith('Error:')) item.description = ''
        }
        // 标记变更
        item.rowStatus = item.rowStatus === 'added' ? 'added' : 'modified'
        item.modifiedFields = Array.from(
          new Set([
            ...(item.modifiedFields || []),
            'routing_channel_id',
            'routing_channel_type',
            'routing_channel_point_id',
            'routing_enabled',
          ]),
        )
        return item
      })
      // 刷新字段级错误
      refreshRoutingFieldErrorsForList()
      if (invalidRecords.length > 0) {
        ElMessage.warning(
          `Imported with invalid rows: ${invalidRecords.slice(0, 10).join(', ')}${
            invalidRecords.length > 10 ? '...' : ''
          }`,
        )
      } else {
        ElMessage.success('Imported routing successfully')
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
  if (!editPoints.value || editPoints.value.length === 0) {
    ElMessage.warning('No data to export')
    return
  }
  const header = 'point_id,channel_id,channel_type,channel_point_id,enabled'
  const rows = editPoints.value.map((item: any) => {
    const r = item.routing || {}
    return [
      getPointId(item),
      r.channel_id ?? '',
      r.channel_type ?? '',
      r.channel_point_id ?? '',
      r.enabled === true ? 1 : 0,
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

defineExpose({
  getEditedData,
  clearImportedFileName,
  clearSignalNameFilter: () => {
    signalNameFilter.value = ''
    showSignalNameFilter.value = false
  },
  hasInvalid: () => {
    return Array.isArray(editPoints.value)
      ? editPoints.value.some(
          (p: any) => p && p.rowStatus !== 'deleted' && (p as any).isInvalid === true,
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
  .imported-file-name {
    color: #67c23a;
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
    width: 10%;
  }
  .vtable__cell--name {
    width: 20%;
    position: relative;
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
      min-width: 2.5rem;
    }
  }
  .vtable__cell--channel-id {
    width: 15%;
  }
  .vtable__cell--channel-type {
    width: 15%;
  }
  .vtable__cell--channel-point {
    width: 20%;
  }
  .vtable__cell--enabled {
    width: 10%;
  }
  .vtable__cell--operation {
    width: 10%;
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
