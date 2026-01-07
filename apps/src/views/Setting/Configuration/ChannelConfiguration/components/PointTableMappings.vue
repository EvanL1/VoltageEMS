<template>
  <div class="voltage-class point-table">
    <!-- 按钮控制区域 -->
    <div v-if="props.viewMode === 'mappings'" class="table-action-controls">
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
      <!-- 非编辑模式：显示Export -->
      <template v-if="!props.isEditing">
        <el-button type="primary" @click="handleExport">Export</el-button>
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
      <!-- Mappings 表头 -->
      <div class="vtable__header">
        <div class="vtable__cell vtable__cell--point-id">Point ID</div>
        <div
          class="vtable__cell vtable__cell--signal-name"
          :class="props.isEditing ? '' : 'notEdit'"
        >
          <span>Point Name</span>
        </div>
        <div v-if="props.channelProtocol !== 'di_do'" class="vtable__cell vtable__cell--slave-id">
          Slave ID
        </div>
        <div
          v-if="props.channelProtocol !== 'di_do'"
          class="vtable__cell vtable__cell--function-code"
        >
          Function Code
        </div>
        <div
          v-if="props.channelProtocol !== 'di_do'"
          class="vtable__cell vtable__cell--register-address"
        >
          Register Address
        </div>
        <div v-if="props.channelProtocol !== 'di_do'" class="vtable__cell vtable__cell--data-type">
          Data Type
        </div>
        <div v-if="props.channelProtocol !== 'di_do'" class="vtable__cell vtable__cell--byte-order">
          Byte Order
        </div>
        <div
          v-if="props.channelProtocol !== 'di_do'"
          class="vtable__cell vtable__cell--bit-position"
        >
          Bit Position
        </div>
        <div
          v-if="props.channelProtocol === 'di_do'"
          class="vtable__cell vtable__cell--gpio-number"
        >
          GPIO Number
        </div>
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
        key-field="point_id"
        :buffer="4"
        :prerender="8"
      >
        <template #default="{ item, index }">
          <DynamicScrollerItem :item="item" :index="index" :active="true">
            <div class="vtable__row" :class="getRowClass(item)">
              <div class="row-status-float"></div>
              <div class="vtable__cell vtable__cell--point-id">
                <span>{{ item.point_id }}</span>
              </div>
              <div
                class="vtable__cell vtable__cell--signal-name"
                :class="props.isEditing ? '' : 'notEdit'"
              >
                <span>{{ item.signal_name }}</span>
              </div>
              <div
                v-if="props.channelProtocol === 'di_do'"
                class="vtable__cell vtable__cell--gpio-number"
              >
                <template v-if="props.isEditing && item.isEditing">
                  <div class="inline-edit-container">
                    <el-input-number
                      v-model="(item.protocol_mapping as any).gpio_number"
                      :min="1"
                      :step="1"
                      :controls="false"
                      align="left"
                      @change="() => onMappingFieldChange(item, 'gpio_number')"
                      style="width: 100% !important"
                    />
                  </div>
                </template>
                <template v-else>
                  <span :class="getFieldClass(item, 'mapping_gpio_number')">{{
                    (item.protocol_mapping as any)?.gpio_number ?? ''
                  }}</span>
                </template>
                <div
                  v-if="props.isEditing && getMappingFieldError(item, 'gpio_number')"
                  class="field-error"
                >
                  {{ getMappingFieldError(item, 'gpio_number') }}
                </div>
              </div>
              <div
                v-if="props.channelProtocol !== 'di_do'"
                class="vtable__cell vtable__cell--slave-id"
              >
                <template v-if="props.isEditing && item.isEditing">
                  <div class="inline-edit-container">
                    <el-input-number
                      v-model="item.protocol_mapping.slave_id"
                      :min="0"
                      :max="999"
                      :controls="false"
                      align="left"
                      @change="() => onMappingFieldChange(item, 'slave_id')"
                      style="width: 100% !important"
                    />
                  </div>
                </template>
                <template v-else>
                  <span :class="getFieldClass(item, 'mapping_slave_id')">{{
                    item.protocol_mapping?.slave_id
                  }}</span>
                </template>
                <div
                  v-if="props.isEditing && getMappingFieldError(item, 'slave_id')"
                  class="field-error"
                >
                  {{ getMappingFieldError(item, 'slave_id') }}
                </div>
              </div>
              <div
                v-if="props.channelProtocol !== 'di_do'"
                class="vtable__cell vtable__cell--function-code"
              >
                <template v-if="props.isEditing && item.isEditing">
                  <div class="inline-edit-container">
                    <el-select
                      v-model="item.protocol_mapping.function_code"
                      popper-class="inline-mapping-popper"
                      :fit-input-width="true"
                      filterable
                      @change="() => onFunctionCodeChange(item)"
                      clearable
                    >
                      <el-option
                        v-for="option in FC_BY_POINT[props.pointType]"
                        :key="option"
                        :label="option"
                        :value="option"
                      />
                    </el-select>
                  </div>
                </template>
                <template v-else>
                  <span :class="getFieldClass(item, 'mapping_function_code')">{{
                    getMappingFunctionCodeLabel(item.protocol_mapping?.function_code)
                  }}</span>
                </template>
                <div
                  v-if="props.isEditing && getMappingFieldError(item, 'function_code')"
                  class="field-error"
                >
                  {{ getMappingFieldError(item, 'function_code') }}
                </div>
              </div>
              <div
                v-if="props.channelProtocol !== 'di_do'"
                class="vtable__cell vtable__cell--register-address"
              >
                <template v-if="props.isEditing && item.isEditing">
                  <div class="inline-edit-container">
                    <el-input
                      :model-value="getMappingRegisterAddressStr(item)"
                      placeholder="0-65535"
                      style="width: 100% !important"
                      @input="(val: any) => onMappingRegisterAddressInput(item, String(val))"
                    />
                  </div>
                </template>
                <template v-else>
                  <span :class="getFieldClass(item, 'mapping_register_address')">{{
                    item.protocol_mapping?.register_address ?? ''
                  }}</span>
                </template>
                <div
                  v-if="props.isEditing && getMappingFieldError(item, 'register_address')"
                  class="field-error"
                >
                  {{ getMappingFieldError(item, 'register_address') }}
                </div>
              </div>
              <div
                v-if="props.channelProtocol !== 'di_do'"
                class="vtable__cell vtable__cell--data-type"
              >
                <template v-if="props.isEditing && item.isEditing">
                  <div class="inline-edit-container">
                    <el-select
                      v-model="item.protocol_mapping.data_type"
                      popper-class="inline-mapping-popper"
                      :fit-input-width="true"
                      filterable
                      @change="onMappingDataTypeChange(item)"
                      clearable
                    >
                      <el-option
                        v-for="option in getMappingDataTypeOptions()"
                        :key="option.value"
                        :label="option.label"
                        :value="option.value"
                      />
                    </el-select>
                  </div>
                </template>
                <template v-else>
                  <span :class="getFieldClass(item, 'mapping_data_type')">{{
                    item.protocol_mapping?.data_type
                  }}</span>
                </template>
                <div
                  v-if="props.isEditing && getMappingFieldError(item, 'data_type')"
                  class="field-error"
                >
                  {{ getMappingFieldError(item, 'data_type') }}
                </div>
              </div>
              <div
                v-if="props.channelProtocol !== 'di_do'"
                class="vtable__cell vtable__cell--byte-order"
              >
                <template v-if="props.isEditing && item.isEditing">
                  <div class="inline-edit-container">
                    <el-select
                      v-model="item.protocol_mapping.byte_order"
                      popper-class="inline-mapping-popper"
                      :fit-input-width="true"
                      filterable
                      @change="() => onMappingFieldChange(item, 'byte_order')"
                      clearable
                    >
                      <el-option
                        v-for="option in getMappingByteOrderOptions(item)"
                        :key="option.value"
                        :label="option.label"
                        :value="option.value"
                      />
                    </el-select>
                  </div>
                </template>
                <template v-else>
                  <span :class="getFieldClass(item, 'mapping_byte_order')">{{
                    item.protocol_mapping?.byte_order
                  }}</span>
                </template>
                <div
                  v-if="props.isEditing && getMappingFieldError(item, 'byte_order')"
                  class="field-error"
                >
                  {{ getMappingFieldError(item, 'byte_order') }}
                </div>
              </div>
              <div
                v-if="props.channelProtocol !== 'di_do'"
                class="vtable__cell vtable__cell--bit-position"
              >
                <template v-if="props.isEditing && item.isEditing">
                  <div class="inline-edit-container">
                    <el-input-number
                      v-model="item.protocol_mapping.bit_position"
                      :min="0"
                      :max="15"
                      :controls="false"
                      align="left"
                      :disabled="!canEditMappingBitPosition(item)"
                      @change="() => onMappingFieldChange(item, 'bit_position')"
                      style="width: 100% !important"
                    />
                  </div>
                </template>
                <template v-else>
                  <span :class="getFieldClass(item, 'mapping_bit_position')">{{
                    item.protocol_mapping?.bit_position
                  }}</span>
                </template>
                <div
                  v-if="props.isEditing && getMappingFieldError(item, 'bit_position')"
                  class="field-error"
                >
                  {{ getMappingFieldError(item, 'bit_position') }}
                </div>
              </div>

              <template v-if="props.isEditing">
                <div class="vtable__cell vtable__cell--operation">
                  <div class="point-table__operation-cell">
                    <template v-if="item.isEditing">
                      <div class="point-table__cancel-btn" @click="handleCancelMappingEdit(item)">
                        <el-icon><Close /></el-icon>
                      </div>
                      <div class="point-table__confirm-btn" @click="handleConfirmMappingEdit(item)">
                        <el-icon><Check /></el-icon>
                      </div>
                    </template>
                    <template v-else>
                      <div class="point-table__edit-btn" @click="handleStartMappingEdit(item)">
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
import { ref, watch, onMounted, onUnmounted, computed, inject, nextTick } from 'vue'
import { pxToResponsive } from '@/utils/responsive'
import { ElMessage } from 'element-plus'
import { OriginalPointsKey, ChannelNameKey } from '@/utils/key'
import { Edit, Filter, Close, Check, WarningFilled } from '@element-plus/icons-vue'
import type { PointInfo, UpdateMappingPoint } from '@/types/channelConfiguration'
import {
  DATA_TYPE_OPTIONS,
  BYTE_ORDER_OPTIONS,
  BYTE_ORDER_64_OPTIONS,
  FUNCTION_CODE_OPTIONS,
} from '@/types/channelConfiguration'
// lodash-es 替换
const toNumber = (v: any) => Number(v as any)
const toLower = (v: any) => String(v ?? '').toLowerCase()
const toUpper = (v: any) => String(v ?? '').toUpperCase()

// Props 说明
// - pointType: 当前表所属的大类（T/S/C/A），决定映射可选范围与校验
// - points: 父组件传入的当前 Tab 可编辑点位集合（用于渲染）
// - originalPoints: 父组件传入的“接口原始点位数据”（作为变化对比基线）
// - viewMode/editFilters/isEditing: 控制渲染模式、筛选标签与编辑态
interface Props {
  pointType: 'T' | 'S' | 'C' | 'A'
  points: PointInfo[]
  originalPoints?: PointInfo[]
  viewMode: 'points' | 'mappings'
  editFilters: string[]
  isEditing: boolean
  channelProtocol: 'modbus_tcp' | 'modbus_rtu' | 'virt' | 'can' | 'di_do'
}
const props = withDefaults(defineProps<Props>(), {
  viewMode: 'mappings',
  editFilters: () => [],
})

const emit = defineEmits<{
  'enter-edit-mode': []
  'change-edit-filter': [value: string]
}>()

// 从 provide/inject 获取回退用的原始点位数据与通道名称
const originalPoints = inject(OriginalPointsKey, ref<PointInfo[]>([]))
const injectedChannelName = inject(ChannelNameKey, ref(''))
// 原始点位对比基线：优先使用父组件透传 originalPoints；否则回退到注入值
const originalPointsList = computed<PointInfo[]>(() => {
  if (Array.isArray(props.originalPoints) && props.originalPoints.length >= 0) {
    return props.originalPoints as PointInfo[]
  }
  return (originalPoints?.value || []) as PointInfo[]
})

const editPoints = ref<PointInfo[]>([])
const signalNameFilterRaw = ref('')
const signalNameFilter = ref('')
// Status 筛选逻辑已移至父组件 PointsTablesDialog.vue
const scrollerRef = ref()
const rowHeight = ref(pxToResponsive(22))
const fileInputRef = ref<HTMLInputElement>()
const importedFileName = ref('')

const signalNameOptions = computed(() => {
  const names = (editPoints.value || []).map((p) => p.signal_name).filter((n) => n)
  return Array.from(new Set(names))
})

// 放在前面，避免“before initialization”错误（用于校验与选项生成）
const DATA_TYPE_BY_POINT: Record<string, string[]> = {
  T: ['int16', 'uint16', 'int32', 'float32', 'uint32', 'int64', 'uint64', 'float64'],
  S: ['int16', 'uint16', 'int32', 'float32', 'uint32', 'int64', 'uint64', 'float64', 'bool'],
  C: ['int16', 'uint16', 'int32', 'float32', 'uint32', 'int64', 'uint64', 'float64', 'bool'],
  A: ['int16', 'uint16', 'int32', 'float32', 'uint32', 'int64', 'uint64', 'float64'],
}
const FC_BY_POINT: Record<string, number[]> = {
  T: [3, 4],
  S: [1, 2, 3, 4],
  C: [5, 15, 6, 16],
  A: [6, 16],
}

// 列表筛选：支持 signal name 关键字与“Status（modified/invalid）”
// Status 筛选由父组件通过 editFilters prop 传递
const filteredPoints = computed(() => {
  const list = Array.isArray(editPoints.value) ? editPoints.value : []
  let result = [...list]
  if (signalNameFilter.value) {
    const kw = String(signalNameFilter.value || '').toLowerCase()
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
  const onResize = () => (rowHeight.value = pxToResponsive(22))
  window.addEventListener('resize', onResize as any)
  ;(onResize as any)()
})
onUnmounted(() => {})

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
// 深拷贝 point（包含 protocol_mapping），避免与原始基线共享引用导致二次编辑无法识别变更
function clonePointForEdit(item: PointInfo): PointInfo {
  const cp: any = {
    ...item,
    rowStatus: item.rowStatus || 'normal',
  }
  if (item.protocol_mapping && typeof item.protocol_mapping === 'object') {
    cp.protocol_mapping = JSON.parse(JSON.stringify(item.protocol_mapping))
  }
  // 新会话基线下不带上一次会话的 modifiedFields / isEditing / fieldErrors
  delete cp.modifiedFields
  delete cp.isEditing
  delete cp.fieldErrors
  delete cp.originalData
  return cp as PointInfo
}
watch(
  () => ({ points: props.points, isEditing: props.isEditing }),
  (val) => {
    if (!val.isEditing && Array.isArray(val.points)) {
      editPoints.value = val.points.map((item: PointInfo) => clonePointForEdit(item))
      // 首次载入或刷新后执行一次有效性检测
      if (Array.isArray(editPoints.value)) {
        editPoints.value.forEach((p) => validateMappingValidity(p))
        refreshMappingFieldErrorsForList()
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
      editPoints.value.forEach((p) => validateMappingValidity(p))
      refreshMappingFieldErrorsForList()
    } else if (!editing) {
      // 退出编辑：恢复为最初基线（originalPointsList）的 protocol_mapping
      const baseline = (originalPointsList.value as PointInfo[]).map((item: PointInfo) =>
        clonePointForEdit(item),
      )
      editPoints.value = baseline
      refreshMappingFieldErrorsForList()
    }
  },
)

function normalizeType(v: string | undefined): string {
  return toLower(v || '')
}
// 根据当前数据类型 + pointType 生成可选的功能码
const getMappingFunctionCodeOptions = (item: PointInfo) => {
  const dt = normalizeType(item.protocol_mapping?.data_type || '')
  const allowed = FC_BY_POINT[props.pointType] || []
  let validCodes: number[] = []
  if (dt === 'bool' || dt === 'boolean')
    validCodes = [1, 2, 5, 15].filter((c) => allowed.includes(c))
  else validCodes = [3, 4, 6, 16].filter((c) => allowed.includes(c))
  return validCodes.map((code) => ({ label: String(code), value: code }))
}
const getMappingFunctionCodeLabel = (fc: number | undefined) => (fc == null ? '' : String(fc))
const getMappingDataTypeOptions = () => {
  const allow = DATA_TYPE_BY_POINT[props.pointType] || []
  return DATA_TYPE_OPTIONS.filter((opt) => allow.includes(String(opt.value)))
}
// 不同字长/布尔类型的字节序可选项
function getMappingByteOrderOptions(item: PointInfo) {
  const t = normalizeType(item.protocol_mapping?.data_type || '')
  if (t === 'bool' || t === 'boolean') return [{ label: 'AB', value: 'AB' }]
  if (t.includes('16'))
    return [
      { label: 'AB', value: 'AB' },
      { label: 'BA', value: 'BA' },
    ]
  if (t.includes('32'))
    return [{ label: 'AB', value: 'AB' }, { label: 'BA', value: 'BA' }, ...BYTE_ORDER_OPTIONS]
  if (t.includes('64'))
    return [
      { label: 'AB', value: 'AB' },
      { label: 'BA', value: 'BA' },
      ...BYTE_ORDER_OPTIONS,
      ...BYTE_ORDER_64_OPTIONS,
    ]
  return BYTE_ORDER_OPTIONS
}
// 字段级即时错误（仅字段本身，不改变整行 isInvalid）
function getMappingFieldError(item: any, field: string): string {
  return (item.fieldErrors && item.fieldErrors[field]) || ''
}
function setMappingFieldError(item: any, field: string, message: string) {
  if (!item.fieldErrors) item.fieldErrors = {}
  if (message) item.fieldErrors[field] = message
  else delete item.fieldErrors[field]
}
function validateMappingFieldOnly(item: any, field: string): string {
  if (props.channelProtocol === 'di_do' && field !== 'gpio_number') return ''
  const m = item.protocol_mapping || {}

  // 检查部分字段有值但部分字段没有的情况（除了 bit_position）
  const hasSlaveId = !(m.slave_id === undefined || m.slave_id === null || m.slave_id === '')
  const hasFunctionCode = !(
    m.function_code === undefined ||
    m.function_code === null ||
    m.function_code === ''
  )
  const hasRegisterAddress = !(m.register_address === undefined || m.register_address === null)
  const hasDataType = !(m.data_type === undefined || m.data_type === null || m.data_type === '')
  const hasByteOrder = !(m.byte_order === undefined || m.byte_order === null || m.byte_order === '')

  const filledFieldsCount = [
    hasSlaveId,
    hasFunctionCode,
    hasRegisterAddress,
    hasDataType,
    hasByteOrder,
  ].filter(Boolean).length

  // 如果部分字段有值但部分字段没有，则显示 require 错误
  const isPartialFill = filledFieldsCount > 0 && filledFieldsCount < 5

  switch (field) {
    case 'gpio_number': {
      // 允许为空，仅在有值时校验
      if (m.gpio_number === undefined || m.gpio_number === null || m.gpio_number === '') return ''
      const gpio = Number(m.gpio_number)
      // 检查是否为 NaN（无法解析为数字）或无效的正整数
      if (isNaN(gpio) || !Number.isInteger(gpio) || gpio <= 0) return 'must positive integer'
      return ''
    }
    case 'slave_id': {
      if (isPartialFill && !hasSlaveId) return 'required'
      if (m.slave_id === undefined || m.slave_id === null || m.slave_id === '') return ''
      const sid = Number(m.slave_id)
      if (!Number.isInteger(sid) || sid < 1 || sid > 247) return 'must be 1-247'
      return ''
    }
    case 'function_code': {
      if (isPartialFill && !hasFunctionCode) return 'required'
      if (m.function_code === undefined || m.function_code === null || m.function_code === '')
        return ''
      const allowedFC = FC_BY_POINT[props.pointType] || []
      const fc = Number(m.function_code)
      if (!allowedFC.includes(fc)) return 'not allowed'
      return ''
    }
    case 'register_address': {
      if (isPartialFill && !hasRegisterAddress) return 'required'
      if (m.register_address == null || m.register_address === '') return ''
      const ra = Number(m.register_address)
      if (!Number.isInteger(ra) || ra < 0 || ra > 65535) return ' must be 0-65535'
      return ''
    }
    case 'data_type': {
      if (isPartialFill && !hasDataType) return 'required'
      if (m.data_type === undefined || m.data_type === null || m.data_type === '') return ''
      const dt = normalizeType(m.data_type || '')
      const allowDT = DATA_TYPE_BY_POINT[props.pointType] || []
      if (!allowDT.includes(dt)) return 'not allowed '
      return ''
    }
    case 'byte_order': {
      if (isPartialFill && !hasByteOrder) return 'required'
      if (m.byte_order === undefined || m.byte_order === null || m.byte_order === '') return ''
      // 当 Data Type 为 bool 时，不对 Byte Order 做限制
      {
        const dt = normalizeType(m.data_type || '')
        if (dt === 'bool' || dt === 'boolean') return ''
      }
      const allowed = getMappingByteOrderOptions(item).map((o: any) =>
        String(o.value).toUpperCase(),
      )
      const cur = String(m.byte_order || '').toUpperCase()
      if (!allowed.includes(cur)) return 'not allowed'
      return ''
    }
    case 'bit_position': {
      const rawBp = m.bit_position
      if (rawBp === undefined || rawBp === null || rawBp === '') return ''
      const n = Number(rawBp)
      if (!Number.isInteger(n) || n < 0 || n > 15) return 'must be 0-15 when provided'
      return ''
    }
    default:
      return ''
  }
}
function refreshMappingFieldErrorsForRow(item: any) {
  if (props.channelProtocol === 'di_do') {
    setMappingFieldError(item, 'gpio_number', validateMappingFieldOnly(item, 'gpio_number'))
    return
  }
  setMappingFieldError(item, 'slave_id', validateMappingFieldOnly(item, 'slave_id'))
  setMappingFieldError(item, 'function_code', validateMappingFieldOnly(item, 'function_code'))
  setMappingFieldError(item, 'register_address', validateMappingFieldOnly(item, 'register_address'))
  setMappingFieldError(item, 'data_type', validateMappingFieldOnly(item, 'data_type'))
  setMappingFieldError(item, 'byte_order', validateMappingFieldOnly(item, 'byte_order'))
  setMappingFieldError(item, 'bit_position', validateMappingFieldOnly(item, 'bit_position'))
}
function refreshMappingFieldErrorsForList() {
  if (!Array.isArray(editPoints.value)) return
  editPoints.value.forEach((p: any) => refreshMappingFieldErrorsForRow(p))
}
function onMappingFieldChange(item: any, field: string) {
  updateMappingChangeStatus(item)
  const msg = validateMappingFieldOnly(item, field)
  setMappingFieldError(item, field, msg)
}
function onFunctionCodeChange(item: any) {
  resetBitPositionIfNeeded(item)
  updateMappingChangeStatus(item)
  const msg = validateMappingFieldOnly(item, 'function_code')
  setMappingFieldError(item, 'function_code', msg)
}
function canEditMappingBitPosition(item: PointInfo) {
  if (!item.protocol_mapping) return false
  const fc = Number(item.protocol_mapping.function_code)
  const dt = normalizeType(item.protocol_mapping.data_type)
  const isBoolFC = (fc === 3 || fc === 4) && (dt === 'bool' || dt === 'boolean')
  const is16 = dt.includes('16')
  return isBoolFC || is16
}
// 当不满足位编辑能力时，重置 bit_position
// 注意：如果原来的值是 0，应该保留，而不是清空
const resetBitPositionIfNeeded = (item: PointInfo) => {
  if (!item.protocol_mapping) return
  // 不可编辑时，只有在原来值不是 0 的情况下才清空
  // 如果原来是 0，保留这个值
  if (!canEditMappingBitPosition(item)) {
    const currentBp = item.protocol_mapping.bit_position
    // 如果当前值不是 0（包括 undefined、null、非 0 数字），才清空
    if (currentBp !== 0 && currentBp !== undefined && currentBp !== null) {
      ;(item.protocol_mapping as any).bit_position = undefined
    }
    // 如果当前值是 0，保持不变
  }
  updateMappingChangeStatus(item)
}
const getMappingRegisterAddressStr = (item: PointInfo) => {
  const v = item.protocol_mapping?.register_address
  return v == null ? '' : String(v)
}
// 寄存器地址输入限制：仅数字、最大 65535，并触发变更检测
const onMappingRegisterAddressInput = (item: PointInfo, str: string) => {
  if (!item.protocol_mapping) return
  const trimmed = (str || '').trim()
  // 允许完全清空（设置为 undefined）
  if (trimmed === '') {
    item.protocol_mapping.register_address = undefined
  } else {
    const digits = trimmed.replace(/\D+/g, '').slice(0, 5)
    if (digits === '') {
      item.protocol_mapping.register_address = undefined
    } else {
      const num = Number(digits)
      item.protocol_mapping.register_address = Math.min(65535, Math.max(0, num))
    }
  }
  updateMappingChangeStatus(item)
  const msg = validateMappingFieldOnly(item, 'register_address')
  setMappingFieldError(item, 'register_address', msg)
}

// 校验当前映射行有效性；返回 true 表示有效
function validateMappingValidity(point: PointInfo): boolean {
  // 删除状态不做内容校验
  if ((point as any).rowStatus === 'deleted') {
    ;(point as any).isInvalid = false
    return true
  }
  const m = point.protocol_mapping as any
  if (props.channelProtocol === 'di_do') {
    const gpio = m?.gpio_number
    const hasValue = !(gpio === undefined || gpio === null || gpio === '')
    // 允许为空；仅当有值时校验数值与范围
    // 检查是否为 NaN（无法解析为数字）或无效的正整数
    if (hasValue) {
      const gpioNum = Number(gpio)
      if (isNaN(gpioNum) || !Number.isInteger(gpioNum) || gpioNum <= 0) {
        ;(point as any).isInvalid = true
        return false
      }
    }
    ;(point as any).isInvalid = false
    return true
  }
  // 无映射视为有效
  if (!m) {
    ;(point as any).isInvalid = false
    return true
  }

  // 检查部分字段有值但部分字段没有的情况（除了 bit_position）
  // 如果一行都没有数据，那这是不会被判错的
  // 但是假如只有部分数据有部分数据没有（除了 bit_position），那么这一行要报错
  const hasSlaveId = !(m.slave_id === undefined || m.slave_id === null || m.slave_id === '')
  const hasFunctionCode = !(
    m.function_code === undefined ||
    m.function_code === null ||
    m.function_code === ''
  )
  const hasRegisterAddress = !(m.register_address === undefined || m.register_address === null)
  const hasDataType = !(m.data_type === undefined || m.data_type === null || m.data_type === '')
  const hasByteOrder = !(m.byte_order === undefined || m.byte_order === null || m.byte_order === '')

  // 统计必填字段（除了 bit_position）中有值的数量
  const filledFieldsCount = [
    hasSlaveId,
    hasFunctionCode,
    hasRegisterAddress,
    hasDataType,
    hasByteOrder,
  ].filter(Boolean).length

  // 如果部分字段有值但部分字段没有，则报错
  if (filledFieldsCount > 0 && filledFieldsCount < 5) {
    ;(point as any).isInvalid = true
    // 设置各个字段的 require 错误
    if (!hasSlaveId) setMappingFieldError(point as any, 'slave_id', 'required')
    if (!hasFunctionCode) setMappingFieldError(point as any, 'function_code', 'required')
    if (!hasRegisterAddress) setMappingFieldError(point as any, 'register_address', 'required')
    if (!hasDataType) setMappingFieldError(point as any, 'data_type', 'required')
    if (!hasByteOrder) setMappingFieldError(point as any, 'byte_order', 'required')
    return false
  }

  // 允许为空：仅对已填写字段执行原有校验
  // 数据类型允许性
  if (!(m.data_type === undefined || m.data_type === null || m.data_type === '')) {
    const dt = normalizeType(m.data_type || '')
    const allowDT = DATA_TYPE_BY_POINT[props.pointType] || []
    if (!allowDT.includes(dt)) {
      ;(point as any).isInvalid = true
      return false
    }
  }
  // 功能码匹配
  if (!(m.function_code === undefined || m.function_code === null || m.function_code === '')) {
    const allowedFC = FC_BY_POINT[props.pointType] || []
    if (!allowedFC.includes(Number(m.function_code))) {
      ;(point as any).isInvalid = true
      return false
    }
  }
  // 从可选项校验字节序（Data Type 为 bool 时跳过）
  if (!(m.byte_order === undefined || m.byte_order === null || m.byte_order === '')) {
    const dtForBO = normalizeType(m.data_type || '')
    if (!(dtForBO === 'bool' || dtForBO === 'boolean')) {
      const allowedOrders = getMappingByteOrderOptions(point).map((o: any) =>
        String(o.value).toUpperCase(),
      )
      const order = String(m.byte_order || '').toUpperCase()
      if (!allowedOrders.includes(order)) {
        ;(point as any).isInvalid = true
        return false
      }
    }
  }
  // slave_id 范围有效（可为空）
  if (!(m.slave_id === undefined || m.slave_id === null || m.slave_id === '')) {
    const sid = Number(m.slave_id)
    if (!Number.isInteger(sid) || sid < 1 || sid > 247) {
      ;(point as any).isInvalid = true
      return false
    }
  }
  // register_address 范围（存在时）
  if (m.register_address != null) {
    const ra = Number(m.register_address)
    if (!Number.isInteger(ra) || ra < 0 || ra > 65535) {
      ;(point as any).isInvalid = true
      return false
    }
  }
  // bit_position 规则：可为空；若填写则必须为 0-15 的整数
  const rawBp = m.bit_position
  if (!(rawBp === undefined || rawBp === null || rawBp === '')) {
    const n = Number(rawBp)
    if (!Number.isInteger(n) || n < 0 || n > 15) {
      ;(point as any).isInvalid = true
      return false
    }
  }
  ;(point as any).isInvalid = false
  return true
}

// 进入单行编辑：保存原值快照以支持取消与差异可视化
const handleStartMappingEdit = (item: PointInfo) => {
  if (!item.protocol_mapping) {
    if (props.channelProtocol === 'di_do') item.protocol_mapping = {} as any
    else {
      // ElMessage.warning('No mapping data available for this point')
      item.protocol_mapping = {
        slave_id: undefined,
        function_code: undefined,
        register_address: undefined,
        data_type: undefined,
        byte_order: undefined,
        bit_position: undefined,
      }
      // return
    }
  }
  if (props.channelProtocol === 'di_do') {
    item.originalData = {
      mapping_gpio_number: (item.protocol_mapping as any).gpio_number,
    }
  } else {
    resetBitPositionIfNeeded(item)
    item.originalData = {
      mapping_slave_id: item.protocol_mapping?.slave_id,
      mapping_function_code: item.protocol_mapping?.function_code,
      mapping_register_address: item.protocol_mapping?.register_address,
      mapping_data_type: item.protocol_mapping?.data_type,
      mapping_byte_order: item.protocol_mapping?.byte_order,
      mapping_bit_position: item.protocol_mapping?.bit_position,
    }
  }
  item.isEditing = true
}
const handleCancelMappingEdit = (item: PointInfo) => {
  if (item.originalData && item.protocol_mapping) {
    if (props.channelProtocol === 'di_do') {
      ;(item.protocol_mapping as any).gpio_number = item.originalData.mapping_gpio_number as number
    } else {
      item.protocol_mapping.slave_id = item.originalData.mapping_slave_id as number
      item.protocol_mapping.function_code = item.originalData.mapping_function_code as number
      item.protocol_mapping.register_address = item.originalData.mapping_register_address as number
      item.protocol_mapping.data_type = item.originalData.mapping_data_type as string
      item.protocol_mapping.byte_order = item.originalData.mapping_byte_order as string
      item.protocol_mapping.bit_position = item.originalData.mapping_bit_position as number
    }
    delete item.originalData
  }
  item.isEditing = false
  // 取消编辑后清除当前筛选条件
  signalNameFilter.value = ''
}
// 确认单行编辑：与“接口原始数据”对比生成 modifiedFields/rowStatus
const handleConfirmMappingEdit = (item: PointInfo) => {
  if (!item.protocol_mapping) return
  const original = (originalPointsList.value as PointInfo[]).find(
    (p) => p.point_id === item.point_id,
  )
  if (props.channelProtocol === 'di_do') {
    const changes: string[] = []
    const normGpio = (v: any) => (v === '' || v === null || v === undefined ? null : Number(v))
    const curGpio = normGpio((item.protocol_mapping as any).gpio_number)
    const origGpio = normGpio((original?.protocol_mapping as any)?.gpio_number)
    if (curGpio !== origGpio) changes.push('mapping_gpio_number')
    const isNew = item.rowStatus === 'added'
    if (!isNew && changes.length > 0) {
      item.rowStatus = 'modified'
      item.modifiedFields = [...(item.modifiedFields || []), ...changes]
    } else if (!isNew && changes.length === 0) {
      item.rowStatus = 'normal'
      item.modifiedFields = []
    } else if (isNew) {
      item.modifiedFields = changes
    }
    validateMappingValidity(item)
    // 刷新字段级别的错误提示
    refreshMappingFieldErrorsForRow(item)
    delete item.originalData
    item.isEditing = false
    return
  }
  const changes: string[] = []
  if (original && original.protocol_mapping) {
    const o = original.protocol_mapping
    const c = item.protocol_mapping

    // 检查原始 mapping 是否所有字段都为空（除了 bit_position）
    const origHasAnyValue =
      !(o.slave_id === undefined || o.slave_id === null) ||
      !(o.function_code === undefined || o.function_code === null) ||
      !(o.register_address === undefined || o.register_address === null) ||
      !(o.data_type === undefined || o.data_type === null || o.data_type === '') ||
      !(o.byte_order === undefined || o.byte_order === null || o.byte_order === '')

    // 如果原始 mapping 所有字段都为空，则任何修改都标记为修改
    if (!origHasAnyValue) {
      const cur = item.protocol_mapping
      if (cur.slave_id !== undefined && cur.slave_id !== null) changes.push('mapping_slave_id')
      if (cur.function_code !== undefined && cur.function_code !== null)
        changes.push('mapping_function_code')
      if (cur.register_address !== undefined && cur.register_address !== null)
        changes.push('mapping_register_address')
      if (cur.data_type !== undefined && cur.data_type !== null && cur.data_type !== '')
        changes.push('mapping_data_type')
      if (cur.byte_order !== undefined && cur.byte_order !== null && cur.byte_order !== '')
        changes.push('mapping_byte_order')
      if (cur.bit_position !== undefined && cur.bit_position !== null)
        changes.push('mapping_bit_position')
    } else {
      // 原始 mapping 有值，正常对比
      const normInt = (v: any) => (v === '' || v === null || v === undefined ? null : Number(v))
      const normRA = (v: any) =>
        v === '' || v === null || v === undefined
          ? null
          : Number.isFinite(Number(v))
            ? Number(v)
            : null
      const normDT = (v: any) => normalizeType(v || '')
      const normBO = (v: any) => String(v || '').toUpperCase()
      const normBP = (v: any) => (v === '' || v === null || v === undefined ? null : Number(v))

      if (normInt(c.slave_id) !== normInt(o.slave_id)) changes.push('mapping_slave_id')
      if (normInt(c.function_code) !== normInt(o.function_code))
        changes.push('mapping_function_code')
      if (normRA(c.register_address) !== normRA(o.register_address))
        changes.push('mapping_register_address')
      if (normDT(c.data_type) !== normDT(o.data_type)) changes.push('mapping_data_type')
      if (normBO(c.byte_order) !== normBO(o.byte_order)) changes.push('mapping_byte_order')
      if (normBP(c.bit_position) !== normBP(o.bit_position)) changes.push('mapping_bit_position')
    }
  } else if (item.protocol_mapping) {
    // 原始数据无 protocol_mapping，则认为当前行均为修改
    const cur = item.protocol_mapping
    if (cur.slave_id !== undefined && cur.slave_id !== null) changes.push('mapping_slave_id')
    if (cur.function_code !== undefined && cur.function_code !== null)
      changes.push('mapping_function_code')
    if (cur.register_address !== undefined && cur.register_address !== null)
      changes.push('mapping_register_address')
    if (cur.data_type !== undefined && cur.data_type !== null && cur.data_type !== '')
      changes.push('mapping_data_type')
    if (cur.byte_order !== undefined && cur.byte_order !== null && cur.byte_order !== '')
      changes.push('mapping_byte_order')
    if (cur.bit_position !== undefined && cur.bit_position !== null)
      changes.push('mapping_bit_position')
  }
  const isNew = item.rowStatus === 'added'
  if (!isNew && changes.length > 0) {
    item.rowStatus = 'modified'
    item.modifiedFields = [...(item.modifiedFields || []), ...changes]
  } else if (!isNew && changes.length === 0) {
    item.rowStatus = 'normal'
    item.modifiedFields = []
  }
  // 行编辑确认后立即进行有效性校验，便于及时提示错误
  validateMappingValidity(item)
  // 刷新字段级别的错误提示
  refreshMappingFieldErrorsForRow(item)
  delete item.originalData
  item.isEditing = false
}

// 行样式：根据 rowStatus 渲染边框；无效行叠加错误高亮（参考 Points）
const getRowClass = (item: PointInfo) => {
  const classes = [`row-status-${item.rowStatus || 'normal'}`]
  if (props.isEditing && (item as any).isInvalid) classes.push('row-invalid')
  return classes.join(' ')
}
const getFieldClass = (item: PointInfo, fieldName: string) => {
  const status = item.rowStatus
  if (status === 'added') return 'field-added'
  if (status === 'modified' && item.modifiedFields?.includes(fieldName)) return 'field-modified'
  if (status === 'deleted') return 'field-deleted'
  return ''
}

function onMappingDataTypeChange(item: PointInfo) {
  resetBitPositionIfNeeded(item)
  adjustByteOrderForNewType(item)
  updateMappingChangeStatus(item)
  // 同步更新与数据类型相关的字段错误
  setMappingFieldError(item as any, 'data_type', validateMappingFieldOnly(item, 'data_type'))
  setMappingFieldError(item as any, 'byte_order', validateMappingFieldOnly(item, 'byte_order'))
}
function adjustByteOrderForNewType(item: PointInfo) {
  if (!item.protocol_mapping) return
  const t = normalizeType(item.protocol_mapping.data_type || '')
  const allowed = getMappingByteOrderOptions(item).map((o: any) => toUpper(String(o.value)))
  const current = toUpper(String(item.protocol_mapping.byte_order || ''))
  if (allowed.includes(current)) return
  const is16 = t.includes('16') || t === 'bool' || t === 'boolean'
  const is32 = t.includes('32')
  const map64to32: Record<string, string> = {
    ABCDEFGH: 'ABCD',
    HGFEDCBA: 'DCBA',
    BADCFEHG: 'BADC',
    GHEFCDAB: 'CDAB',
  }
  const map32to16: Record<string, string> = { ABCD: 'AB', DCBA: 'BA', BADC: 'BA', CDAB: 'AB' }
  let downgraded = current
  if (current.length === 8 && (is32 || is16)) downgraded = map64to32[current] || 'ABCD'
  if (is16) {
    const base32 = downgraded.length === 4 ? downgraded : current.length === 4 ? current : 'ABCD'
    downgraded = map32to16[base32] || 'AB'
  }
  const finalOrder = allowed.includes(downgraded) ? downgraded : is16 ? 'AB' : is32 ? 'ABCD' : 'AB'
  item.protocol_mapping.byte_order = finalOrder
}

// 依据接口原始数据 protocol_mapping 实时计算是否修改
function updateMappingChangeStatus(item: PointInfo) {
  const original = (originalPointsList.value as PointInfo[]).find(
    (p) => p.point_id === item.point_id,
  )
  const origMap = original?.protocol_mapping
  const cur = item.protocol_mapping
  const changes: string[] = []
  if (props.channelProtocol === 'di_do') {
    const normGpio = (v: any) => (v === '' || v === null || v === undefined ? null : Number(v))
    const curGpio = normGpio((cur as any)?.gpio_number)
    const origGpio = normGpio((origMap as any)?.gpio_number)
    if (curGpio !== origGpio) changes.push('mapping_gpio_number')
    if (item.rowStatus === 'added') {
      item.modifiedFields = changes
      return
    }
    if (changes.length > 0) {
      item.rowStatus = 'modified'
      item.modifiedFields = changes
    } else {
      item.rowStatus = 'normal'
      item.modifiedFields = []
    }
    return
  }

  if (cur) {
    if (origMap) {
      // 检查原始 mapping 是否所有字段都为空（除了 bit_position）
      const origHasAnyValue =
        !(origMap.slave_id === undefined || origMap.slave_id === null) ||
        !(origMap.function_code === undefined || origMap.function_code === null) ||
        !(origMap.register_address === undefined || origMap.register_address === null) ||
        !(
          origMap.data_type === undefined ||
          origMap.data_type === null ||
          origMap.data_type === ''
        ) ||
        !(
          origMap.byte_order === undefined ||
          origMap.byte_order === null ||
          origMap.byte_order === ''
        )

      // 如果原始 mapping 所有字段都为空，则任何修改都标记为修改
      if (!origHasAnyValue) {
        if (cur.slave_id !== undefined && cur.slave_id !== null) changes.push('mapping_slave_id')
        if (cur.function_code !== undefined && cur.function_code !== null)
          changes.push('mapping_function_code')
        if (cur.register_address !== undefined && cur.register_address !== null)
          changes.push('mapping_register_address')
        if (cur.data_type !== undefined && cur.data_type !== null && cur.data_type !== '')
          changes.push('mapping_data_type')
        if (cur.byte_order !== undefined && cur.byte_order !== null && cur.byte_order !== '')
          changes.push('mapping_byte_order')
        if (cur.bit_position !== undefined && cur.bit_position !== null)
          changes.push('mapping_bit_position')
      } else {
        // 原始 mapping 有值，正常对比
        const normInt = (v: any) => (v === '' || v === null || v === undefined ? null : Number(v))
        const normRA = (v: any) =>
          v === '' || v === null || v === undefined
            ? null
            : Number.isFinite(Number(v))
              ? Number(v)
              : null
        const normDT = (v: any) => normalizeType(v || '')
        const normBO = (v: any) => String(v || '').toUpperCase()
        const normBP = (v: any) => (v === '' || v === null || v === undefined ? null : Number(v))

        if (normInt(cur.slave_id) !== normInt(origMap.slave_id)) changes.push('mapping_slave_id')
        if (normInt(cur.function_code) !== normInt(origMap.function_code))
          changes.push('mapping_function_code')
        if (normRA(cur.register_address) !== normRA(origMap.register_address))
          changes.push('mapping_register_address')
        if (normDT(cur.data_type) !== normDT(origMap.data_type)) changes.push('mapping_data_type')
        if (normBO(cur.byte_order) !== normBO(origMap.byte_order))
          changes.push('mapping_byte_order')
        if (normBP(cur.bit_position) !== normBP(origMap.bit_position))
          changes.push('mapping_bit_position')
      }
    } else {
      // 无原始映射时，任意当前映射都视为修改
      if (cur.slave_id !== undefined && cur.slave_id !== null) changes.push('mapping_slave_id')
      if (cur.function_code !== undefined && cur.function_code !== null)
        changes.push('mapping_function_code')
      if (cur.register_address !== undefined && cur.register_address !== null)
        changes.push('mapping_register_address')
      if (cur.data_type !== undefined && cur.data_type !== null && cur.data_type !== '')
        changes.push('mapping_data_type')
      if (cur.byte_order !== undefined && cur.byte_order !== null && cur.byte_order !== '')
        changes.push('mapping_byte_order')
      if (cur.bit_position !== undefined && cur.bit_position !== null)
        changes.push('mapping_bit_position')
    }
  } else if (origMap) {
    // 原来有映射，现在被清空（保护性处理）
    changes.push(
      'mapping_slave_id',
      'mapping_function_code',
      'mapping_register_address',
      'mapping_data_type',
      'mapping_byte_order',
      'mapping_bit_position',
    )
  }

  if (item.rowStatus === 'added') {
    item.modifiedFields = changes
    return
  }

  if (changes.length > 0) {
    item.rowStatus = 'modified'
    item.modifiedFields = changes
  } else {
    item.rowStatus = 'normal'
    item.modifiedFields = []
  }
}

const getEditedData = () => {
  const updates: UpdateMappingPoint[] = []
  editPoints.value.forEach((item) => {
    if (!item.protocol_mapping || !item.modifiedFields || item.modifiedFields.length === 0) return
    const changed = item.modifiedFields.filter((f) => f.startsWith('mapping_'))
    if (changed.length === 0) return
    const data: Record<string, any> = {}
    // 归一化：将“清空/未填”的值转成 null，确保请求中携带清空操作
    const normIntOrNull = (v: any) => (v === '' || v === null || v === undefined ? null : Number(v))
    const normStrOrNull = (v: any) => (v === '' || v === null || v === undefined ? null : String(v))
    if (props.channelProtocol === 'di_do') {
      if (changed.includes('mapping_gpio_number')) {
        const v = (item.protocol_mapping as any).gpio_number
        data.gpio_number = normIntOrNull(v)
      }
    } else {
      if (changed.includes('mapping_slave_id'))
        data.slave_id = normIntOrNull(item.protocol_mapping.slave_id)
      if (changed.includes('mapping_function_code'))
        data.function_code = normIntOrNull(item.protocol_mapping.function_code)
      if (changed.includes('mapping_register_address'))
        data.register_address = normIntOrNull(item.protocol_mapping.register_address)
      if (changed.includes('mapping_data_type'))
        data.data_type = normStrOrNull(item.protocol_mapping.data_type)
      if (changed.includes('mapping_byte_order'))
        data.byte_order = normStrOrNull(item.protocol_mapping.byte_order)
      if (changed.includes('mapping_bit_position'))
        data.bit_position = normIntOrNull(item.protocol_mapping.bit_position)
    }
    if (Object.keys(data).length > 0 && item.point_id > 0) {
      updates.push({
        four_remote: props.pointType,
        point_id: item.point_id,
        protocol_data: data as any,
      })
    }
  })
  return updates
}

// CSV 导入功能
// 期望表头：
// - di_do：至少包含 point_id、gpio_number（其他列可有可无，顺序不限）
// - modbus_tcp/modbus_rtu：必须包含 point_id、slave_id、function_code、register_address、data_type、byte_order、bit_position；
//   point_name 可有可无（其他列也可有可无，顺序不限）
// - 校验字段取值范围并标注 isInvalid，用于“invalid”筛选
// - 仅更新当前 Tab 存在的 point_id 的映射
// - 应用后与“接口原始数据”对比，计算 modified 状态
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
  reader.onload = (e) => {
    try {
      const content = e.target?.result as string
      const lines = content.split('\n').filter((line) => line.trim())
      if (lines.length === 0) {
        ElMessage.error('CSV file is empty')
        return
      }

      // 根据通道类型验证表头
      const header = lines[0].trim()
      const headerColumns = header.split(',').map((col) => col.trim().toLowerCase())
      let expectedColumnCount: number
      // di_do：只要求必需列存在（point_id、gpio_number），其余列可有可无
      let diDoPointIdIndex = 0
      let diDoGpioIndex = 0
      let diDoRequiredMaxIndex = 0
      // modbus：必需列
      const modbusRequiredColumns = [
        'point_id',
        // point_name 可选
        'slave_id',
        'function_code',
        'register_address',
        'data_type',
        'byte_order',
        'bit_position',
      ]
      const modbusColumnIndices: Record<string, number> = {}
      let modbusRequiredMaxIndex = 0

      if (props.channelProtocol === 'di_do') {
        // di_do 类型：只强制要求 point_id、gpio_number；其他列（如 point_name）可有可无
        diDoPointIdIndex = headerColumns.findIndex((h) => h === 'point_id')
        diDoGpioIndex = headerColumns.findIndex((h) => h === 'gpio_number')
        if (diDoPointIdIndex === -1 || diDoGpioIndex === -1) {
          ElMessage.error('Invalid CSV header. Required columns: point_id,gpio_number')
          return
        }
        diDoRequiredMaxIndex = Math.max(diDoPointIdIndex, diDoGpioIndex)
        // di_do 行列数不固定，不再强制 expectedColumnCount
        expectedColumnCount = 0
      } else {
        // modbus_tcp/modbus_rtu 类型：只要求必需列存在；point_name 可带可不带
        for (const colName of modbusRequiredColumns) {
          const idx = headerColumns.findIndex((h) => h === colName)
          if (idx === -1) {
            ElMessage.error(`Required column "${colName}" not found in CSV header`)
            return
          }
          modbusColumnIndices[colName] = idx
        }
        modbusRequiredMaxIndex = Math.max(...Object.values(modbusColumnIndices))
        // 行列数不固定，不再强制 expectedColumnCount
        expectedColumnCount = 0
      }

      // 构建按 point_id 的导入映射
      const byId: Record<
        number,
        {
          slave_id?: number
          function_code?: number
          register_address?: number
          data_type?: string
          byte_order?: string
          bit_position?: number
          gpio_number?: number | string
          isInvalid?: boolean
        }
      > = {}
      const invalidRecords: number[] = []
      const skippedNoPoint: number[] = []

      for (let i = 1; i < lines.length; i++) {
        const line = lines[i].trim()
        if (!line) continue

        const values = line.split(',').map((v) => v.trim())
        if (props.channelProtocol === 'di_do') {
          // 只要能覆盖必需列的索引即可（允许额外列；允许省略非必需列）
          if (values.length <= diDoRequiredMaxIndex) {
            invalidRecords.push(i + 1)
            continue
          }
        } else {
          // modbus：只要能覆盖必需列的索引即可（允许额外列；允许 point_name 缺失）
          if (values.length <= modbusRequiredMaxIndex) {
            invalidRecords.push(i + 1)
            continue
          }
        }

        const pointIdStr =
          props.channelProtocol === 'di_do'
            ? values[diDoPointIdIndex]
            : values[modbusColumnIndices.point_id]
        const pointId = Number(pointIdStr)
        if (!Number.isInteger(pointId) || pointId <= 0) {
          invalidRecords.push(i + 1)
          continue
        }

        // 是否存在于当前tab
        const exist = (editPoints.value as PointInfo[]).some((p) => p.point_id === pointId)
        if (!exist) {
          skippedNoPoint.push(pointId)
          continue
        }

        // 根据通道类型解析字段
        let slave_id: number
        let function_code: number
        let register_address: number
        let data_type: string
        let byte_order: string
        let bit_position: number
        let gpio_number: number | string | undefined

        if (props.channelProtocol === 'di_do') {
          // di_do 类型：point_id + gpio_number（其他列可有可无）
          const gpioStr = values[diDoGpioIndex] ?? ''
          // 无论是否有效，都要尝试解析并保存值（即使无效也要显示）
          if (gpioStr && gpioStr.trim()) {
            const gpioNum = Number(gpioStr.trim())
            // 保存解析后的数值，即使无效也要保存（用于显示）
            // 如果无法解析为数字，保存为 NaN，否则保存解析后的数字
            gpio_number = isNaN(gpioNum) ? gpioStr : gpioNum
          } else {
            // 空字符串保持为 undefined
            gpio_number = undefined
          }
          // point_name 在 CSV 中存在，但不需要用于更新（points 数据中已有）
          // di_do 类型不需要其他字段
          slave_id = 0
          function_code = 0
          register_address = 0
          data_type = ''
          byte_order = ''
          bit_position = 0
        } else {
          // modbus_tcp/modbus_rtu 类型：按表头索引取值，point_name 可忽略
          const slaveIdStr = values[modbusColumnIndices.slave_id] ?? ''
          const fcStr = values[modbusColumnIndices.function_code] ?? ''
          const regStr = values[modbusColumnIndices.register_address] ?? ''
          const dtStr = values[modbusColumnIndices.data_type] ?? ''
          const orderStr = values[modbusColumnIndices.byte_order] ?? ''
          const bitStr = values[modbusColumnIndices.bit_position] ?? ''
          slave_id = Number(slaveIdStr)
          function_code = Number(fcStr)
          register_address = Number(regStr)
          data_type = normalizeType(dtStr)
          byte_order = String(orderStr || '').toUpperCase()
          bit_position = Number(bitStr)
        }

        // 校验规则
        let isInvalid = false

        if (props.channelProtocol === 'di_do') {
          // di_do 类型：只校验 gpio_number
          // 如果 gpio_number 为空（undefined/null），不报错（允许为空）
          // 只有当 gpio_number 有值但无效时才报错（但值仍会保存用于显示）
          if (gpio_number !== undefined && gpio_number !== null) {
            // 检查是否为 NaN（无法解析为数字）或无效的正整数
            if (
              isNaN(gpio_number as number) ||
              !Number.isInteger(gpio_number) ||
              Number(gpio_number) <= 0
            ) {
              isInvalid = true
            }
          }
        } else {
          // modbus_tcp/modbus_rtu 类型：校验所有字段
          // 1) slave_id 1-247
          if (!Number.isInteger(slave_id) || slave_id < 1 || slave_id > 247) {
            isInvalid = true
          }

          // 2) data_type 合规
          if (!isInvalid) {
            const allowedDT = DATA_TYPE_BY_POINT[props.pointType] || []
            if (!allowedDT.includes(data_type)) {
              isInvalid = true
            }
          }

          // 3) function_code 合规（结合 pointType 与 data_type）
          if (!isInvalid) {
            const allowedByPoint = FC_BY_POINT[props.pointType] || []
            const base =
              data_type === 'bool' || data_type === 'boolean' ? [1, 2, 5, 15] : [3, 4, 6, 16]
            const allowedFC = base.filter((c) => allowedByPoint.includes(c))
            if (!allowedFC.includes(function_code)) {
              isInvalid = true
            }
          }

          // 4) register_address 0-65535
          if (
            !isInvalid &&
            (!Number.isInteger(register_address) ||
              register_address < 0 ||
              register_address > 65535)
          ) {
            isInvalid = true
          }

          // 5) byte_order 合规
          if (!isInvalid) {
            // 构造临时 item 以复用字节序校验
            const temp: any = { protocol_mapping: { data_type } }
            const allowedOrders = getMappingByteOrderOptions(temp as PointInfo).map((o: any) =>
              String(o.value).toUpperCase(),
            )
            if (!allowedOrders.includes(byte_order)) {
              isInvalid = true
            }
          }

          // 6) bit_position 合规
          if (!isInvalid) {
            const temp: any = { protocol_mapping: { data_type, function_code } }
            const canEdit = canEditMappingBitPosition(temp as PointInfo)
            if (canEdit) {
              if (!Number.isInteger(bit_position) || bit_position < 0 || bit_position > 15) {
                isInvalid = true
              }
            } else {
              if (bit_position !== 0) {
                isInvalid = true
              }
            }
          }
        }

        byId[pointId] = {
          slave_id,
          function_code,
          register_address,
          data_type,
          byte_order,
          bit_position,
          gpio_number: props.channelProtocol === 'di_do' ? gpio_number : undefined,
          isInvalid,
        }
      }

      // 存储文件名并进入编辑模式
      importedFileName.value = file.name
      emit('enter-edit-mode')

      // 应用到当前tab的点位：整体覆盖，如果导入文件中没有某个点，清除该点的mapping信息
      let updated = 0
      nextTick(() => {
        editPoints.value = (editPoints.value as PointInfo[]).map((item) => {
          const incoming = byId[item.point_id]
          const orig = (originalPointsList.value as PointInfo[]).find(
            (p) => p.point_id === item.point_id,
          )
          const origMap = orig?.protocol_mapping || {}

          if (!incoming) {
            // 导入文件中没有该点，清除mapping信息
            item.protocol_mapping =
              props.channelProtocol === 'di_do'
                ? ({} as any)
                : {
                    slave_id: undefined,
                    function_code: undefined,
                    register_address: undefined,
                    data_type: undefined,
                    byte_order: undefined,
                    bit_position: undefined,
                  }

            // 计算修改字段（相对接口原始数据）
            const changes: string[] = []
            const cur = item.protocol_mapping
            if (cur || origMap) {
              const normInt = (v: any) =>
                v === '' || v === null || v === undefined ? null : Number(v)
              const normStr = (v: any) => String(v || '').toUpperCase()
              const normBP = (v: any) =>
                v === '' || v === null || v === undefined ? null : Number(v)

              if (props.channelProtocol === 'di_do') {
                const curGpio = normInt((cur as any)?.gpio_number)
                const origGpio = normInt((origMap as any)?.gpio_number)
                if (curGpio !== origGpio) changes.push('mapping_gpio_number')
              } else {
                if (normInt(cur?.slave_id) !== normInt(origMap?.slave_id))
                  changes.push('mapping_slave_id')
                if (normInt(cur?.function_code) !== normInt(origMap?.function_code))
                  changes.push('mapping_function_code')
                if (normInt(cur?.register_address) !== normInt(origMap?.register_address))
                  changes.push('mapping_register_address')
                if (normStr(cur?.data_type || '') !== normStr(origMap?.data_type || ''))
                  changes.push('mapping_data_type')
                if (normStr(cur?.byte_order || '') !== normStr(origMap?.byte_order || ''))
                  changes.push('mapping_byte_order')
                if (normBP(cur?.bit_position) !== normBP(origMap?.bit_position))
                  changes.push('mapping_bit_position')
              }
            }
            if (changes.length > 0) {
              item.rowStatus = item.rowStatus === 'added' ? 'added' : 'modified'
              item.modifiedFields = Array.from(
                new Set([...(item.modifiedFields || []), ...changes]),
              )
              updated++
            }
            return item
          }

          // 导入文件中有该点，覆盖 mapping
          if (props.channelProtocol === 'di_do') {
            // 即使 gpio_number 无效，也要保存值用于显示
            item.protocol_mapping = {
              gpio_number: incoming.gpio_number,
            } as any
            // 如果导入数据标记为无效，设置错误状态
            if (incoming.isInvalid) {
              ;(item as any).isInvalid = true
            }
          } else {
            item.protocol_mapping = {
              slave_id: incoming.slave_id,
              function_code: incoming.function_code,
              register_address: incoming.register_address,
              data_type: incoming.data_type,
              byte_order: incoming.byte_order,
              bit_position: incoming.bit_position,
            }
          }

          // 计算修改字段（相对接口原始数据）
          const changes: string[] = []
          const cur = item.protocol_mapping
          if (cur) {
            if (props.channelProtocol === 'di_do') {
              const normGpio = (v: any) =>
                v === '' || v === null || v === undefined ? null : Number(v)
              const curGpio = normGpio((cur as any).gpio_number)
              const origGpio = normGpio((origMap as any)?.gpio_number)
              if (curGpio !== origGpio) changes.push('mapping_gpio_number')
            } else {
              const normInt = (v: any) =>
                v === '' || v === null || v === undefined ? null : Number(v)
              const normStr = (v: any) => String(v || '').toUpperCase()
              const normBP = (v: any) =>
                v === '' || v === null || v === undefined ? null : Number(v)
              if (normInt(cur.slave_id) !== normInt(origMap.slave_id))
                changes.push('mapping_slave_id')
              if (normInt(cur.function_code) !== normInt(origMap.function_code))
                changes.push('mapping_function_code')
              if (normInt(cur.register_address) !== normInt(origMap.register_address))
                changes.push('mapping_register_address')
              if (normStr(cur.data_type || '') !== normStr(origMap.data_type || ''))
                changes.push('mapping_data_type')
              if (normStr(cur.byte_order || '') !== normStr(origMap.byte_order || ''))
                changes.push('mapping_byte_order')
              if (normBP(cur.bit_position) !== normBP(origMap.bit_position))
                changes.push('mapping_bit_position')
            }
          }
          if (changes.length > 0) {
            item.rowStatus = item.rowStatus === 'added' ? 'added' : 'modified'
            item.modifiedFields = Array.from(new Set([...(item.modifiedFields || []), ...changes]))
            updated++
          }
          return item
        })

        // 导入后执行校验，自动设置字段级错误和行级 invalid 状态
        editPoints.value.forEach((p) => validateMappingValidity(p))
        refreshMappingFieldErrorsForList()

        // 只显示简单的成功/信息提示，不显示详细错误（错误已在每行下方显示）
        if (invalidRecords.length > 0) {
          ElMessage.warning(
            `CSV file has ${invalidRecords.length} invalid row(s). Please check the format.`,
          )
        }

        if (updated === 0) {
          ElMessage.info('No mappings updated (no matching point_id or no changes)')
        } else {
          const invalidCount = editPoints.value.filter((p: any) => (p as any).isInvalid).length
          if (invalidCount === 0) {
            ElMessage.success(`Successfully applied mappings to ${updated} point(s)`)
          } else {
            ElMessage.warning(
              `Applied mappings to ${updated} point(s), but ${invalidCount} point(s) have errors. Please check the rows marked in red.`,
            )
          }
        }

        if (skippedNoPoint.length > 0) {
          ElMessage.info(
            `Skipped ${skippedNoPoint.length} non-existing point_id(s): ${Array.from(
              new Set(skippedNoPoint),
            )
              .slice(0, 10)
              .join(', ')}${skippedNoPoint.length > 10 ? '...' : ''}`,
          )
        }
      })
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

// 导出功能（导出当前tab的映射）
// 文件名：{channelName}_{tab}_mapping_{timestamp}.csv
const handleExport = () => {
  if (!editPoints.value || editPoints.value.length === 0) {
    ElMessage.warning('No data to export')
    return
  }

  // 根据通道类型生成CSV内容
  let header: string
  let rows: string[]

  if (props.channelProtocol === 'di_do') {
    // di_do 类型：point_id,point_name,gpio_number
    header = 'point_id,point_name,gpio_number'
    rows = editPoints.value.map((point) => {
      const m = point.protocol_mapping || ({} as any)
      return [point.point_id, point.signal_name ?? '', m.gpio_number ?? ''].join(',')
    })
  } else {
    // modbus_tcp/modbus_rtu 类型
    header =
      'point_id,point_name,slave_id,function_code,register_address,data_type,byte_order,bit_position'
    rows = editPoints.value.map((point) => {
      const m = point.protocol_mapping || ({} as any)
      return [
        point.point_id,
        point.signal_name ?? '',
        m.slave_id ?? '',
        m.function_code ?? '',
        m.register_address ?? '',
        m.data_type ?? '',
        m.byte_order ?? '',
        m.bit_position ?? '',
      ].join(',')
    })
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
  const filename = `${channelName}_${tabNames[props.pointType]}_mapping_${timestamp}.csv`

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

defineExpose({
  getEditedData,
  clearImportedFileName,
  clearSignalNameFilter: () => {
    signalNameFilter.value = ''
  },
  hasInvalid: () => {
    return Array.isArray(editPoints.value)
      ? editPoints.value.some(
          (p: any) =>
            (p && p.rowStatus !== 'deleted' && (p as any).isInvalid === true) ||
            (p && p.rowStatus !== 'deleted' && !validateMappingValidity(p as PointInfo)),
        )
      : false
  },
  scrollToTop: () => {
    nextTick(() => {
      const sc = scrollerRef.value
      if (sc && sc.$el) sc.scrollToItem(0)
    })
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

.voltage-class .point-table {
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

  // 仅表头预留左右占位，行内不加 padding，左侧状态使用浮动条
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
    width: 10%;
  }
  .vtable__cell--signal-name {
    width: 15%;
    position: relative;
    &.notEdit {
      width: 28%;
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
  // 让筛选下拉与输入框左侧对齐
  :deep(.signal-name-popper) {
    left: 0 !important;
    transform: none !important;
    min-width: 100% !important;
  }
  .vtable__cell--value {
    width: 1.56rem; // 12% of 13.07
  }
  .vtable__cell--scale {
    width: 1.3rem; // 10%
  }
  .vtable__cell--offset {
    width: 1.3rem; // 10%
  }
  .vtable__cell--unit {
    width: 1.3rem; // 10%
  }
  .vtable__cell--reverse {
    width: 1.3rem; // 10%
  }
  .vtable__cell--operation {
    width: 1.69rem; // 13%

    .point-table__operation-cell {
      display: flex;
      gap: 0.15rem;
      align-items: center;
      justify-content: center;

      .point-table__error-tip {
        color: #f56c6c;
        font-size: 0.18rem;
        margin-right: 0.04rem;
      }

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

    :deep(.el-input),
    :deep(.el-input-number),
    :deep(.el-select) {
      width: 100%;
    }

    :deep(.el-input__inner) {
      padding: 0.02rem 0.08rem;
      height: 0.28rem;
      line-height: 0.28rem;
    }
  }

  .inline-reverse-popper {
    z-index: 9999 !important;
  }
  .inline-mapping-popper {
    z-index: 9999 !important;
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
  .vtable__cell--gpio-number {
    width: 61%;
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
  .field-added {
    color: #67c23a !important;
  }
  .field-deleted {
    color: #f56c6c !important;
  }

  .value-field {
    color: #fff !important;
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
