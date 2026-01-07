<template>
  <div class="voltage-class action-change-value-form">
    <el-form ref="formRef" label-width="0.6rem" :model="cardData">
      <!-- 基础设置 -->
      <div class="section basic-section">
        <div class="section__header">
          <span class="section__title">Basic Settings</span>
        </div>
        <div class="section__body" style="flex-direction: row">
          <el-form-item label="label:" prop="label" style="margin-bottom: 0">
            <el-input v-model="cardData.label" />
          </el-form-item>
          <el-form-item
            label="description:"
            prop="description"
            style="margin-bottom: 0; width: 100% !important"
            label-width="1.2rem"
          >
            <el-input v-model="cardData.description" style="width: 100% !important" />
          </el-form-item>
        </div>
      </div>

      <div class="main-section">
        <!-- 变量设置（完全对齐 FunctionSwitchForm） -->
        <div class="section variable-section">
          <div class="section__header">
            <span class="section__title">Variable Settings</span>
            <el-button class="section__add-btn" type="primary" @click="addVariable">
              <el-icon><Plus /></el-icon>
            </el-button>
          </div>
          <div class="section__body variable">
            <div
              v-for="(v, idx) in cardData.config.variables"
              :key="`var-${idx}-${v.name}`"
              class="variable-row"
            >
              <el-form-item :label="v.name + ':'" class="variable-row__item">
                <div class="variable-row__mode">
                  <el-radio-group v-model="v.type" @change="onVariableTypeChange(v, idx)">
                    <el-radio label="single">single</el-radio>
                    <el-radio label="combined">combined</el-radio>
                  </el-radio-group>
                </div>

                <!-- single 模式：先 instance -> pointType -> point -->
                <div v-if="v.type !== 'combined'" class="variable-row__controls">
                  <el-select
                    v-model="v.instance_id"
                    placeholder="instance"
                    class="flex-item variable-row__select"
                    filterable
                    @change="() => onVarInstanceChange(idx)"
                  >
                    <el-option
                      v-for="opt in instanceOptions"
                      :key="opt.value"
                      :label="opt.label"
                      :value="opt.value"
                    />
                  </el-select>
                  <el-select
                    v-model="v.pointType"
                    placeholder="point type"
                    class="flex-item variable-row__select"
                    :disabled="!v.instance_id"
                    @change="() => onVarPointTypeChange(idx)"
                  >
                    <el-option label="measurement" value="measurement" />
                    <el-option label="property" value="property" />
                    <el-option label="action" value="action" />
                  </el-select>
                  <el-select
                    v-model="v.point_id"
                    placeholder="point"
                    class="flex-item variable-row__select"
                    filterable
                    :disabled="!v.instance_id || !v.pointType"
                    @change="() => onVarPointChange(idx)"
                  >
                    <el-option
                      v-for="opt in getVarPointOptions(idx)"
                      :key="opt.value"
                      :label="opt.label"
                      :value="opt.value"
                    />
                  </el-select>
                </div>

                <!-- combined：公式编辑器 -->
                <div v-else class="variable-row__combined">
                  <div class="combined-row">
                    <el-select
                      v-model="v.formula[0]"
                      class="variable-row__select"
                      placeholder="variable or number"
                      filterable
                      allow-create
                      default-first-option
                      reserve-keyword
                      @change="(val: any) => onCombinedOperandChange(v, 0, val)"
                    >
                      <el-option
                        v-for="name in getVariableNameOptionsExcluding(idx)"
                        :key="name"
                        :label="name"
                        :value="name"
                      />
                    </el-select>
                    <el-select v-model="v.formula[1]" class="variable-row__select" placeholder="op">
                      <el-option
                        v-for="op in arithmeticOperatorOptions"
                        :key="op"
                        :label="op"
                        :value="op"
                      />
                    </el-select>
                    <el-select
                      v-model="v.formula[2]"
                      class="variable-row__select"
                      placeholder="variable or number"
                      filterable
                      allow-create
                      default-first-option
                      reserve-keyword
                      @change="(val: any) => onCombinedOperandChange(v, 2, val)"
                    >
                      <el-option
                        v-for="name in getVariableNameOptionsExcluding(idx)"
                        :key="name"
                        :label="name"
                        :value="name"
                      />
                    </el-select>
                  </div>

                  <div
                    v-for="pairIdx in getCombinedExtraPairCount(v)"
                    :key="`pair-${idx}-${pairIdx}`"
                    class="combined-row"
                  >
                    <el-select
                      v-model="v.formula[3 + (pairIdx - 1) * 2]"
                      class="variable-row__select"
                      placeholder="op"
                    >
                      <el-option
                        v-for="op in arithmeticOperatorOptions"
                        :key="op"
                        :label="op"
                        :value="op"
                      />
                    </el-select>
                    <el-select
                      v-model="v.formula[3 + (pairIdx - 1) * 2 + 1]"
                      class="variable-row__select"
                      placeholder="variable or number"
                      filterable
                      allow-create
                      default-first-option
                      reserve-keyword
                      @change="
                        (val: any) =>
                          onCombinedExtraOperandChange(v, 3 + (pairIdx - 1) * 2 + 1, val)
                      "
                    >
                      <el-option
                        v-for="name in getVariableNameOptionsExcluding(idx)"
                        :key="name"
                        :label="name"
                        :value="name"
                      />
                    </el-select>
                    <el-button
                      class="combined-row__delete"
                      type="warning"
                      link
                      @click="removeCombinedRow(v, pairIdx - 1)"
                    >
                      <el-icon style="color: red"><CircleClose /></el-icon>
                    </el-button>
                  </div>

                  <div class="variable-row__combined-add">
                    <el-button type="primary" link @click="addCombinedRow(v)">
                      <el-icon style="color: green"><CirclePlus /></el-icon>
                    </el-button>
                  </div>
                </div>
              </el-form-item>
              <el-button
                class="variable-row__delete"
                style="width: 0.32rem !important"
                type="warning"
                @click="removeVariable(idx)"
              >
                <el-icon><Delete /></el-icon>
              </el-button>
            </div>
          </div>
        </div>

        <!-- 规则设置（简化：Variables = value） -->
        <div class="section rule-section">
          <div class="section__header">
            <span class="section__title">Rule Settings</span>
            <el-button class="section__add-btn" type="primary" @click="addRuleRow">
              <el-icon><Plus /></el-icon>
            </el-button>
          </div>
          <div class="section__body rule simple">
            <div
              v-for="(r, rIdx) in cardData.config.rule"
              :key="`rule-${rIdx}`"
              class="rule-row-simple"
            >
              <el-select
                v-model="r.Variables"
                class="rule-row__select"
                placeholder="Variables"
                style="margin-left: 0.2rem"
                filterable
              >
                <el-option v-for="vn in variableNameOptions" :key="vn" :label="vn" :value="vn" />
              </el-select>
              <span class="rule-row__eq">=</span>
              <el-select
                v-model="r.value"
                class="rule-row__select"
                placeholder="value"
                filterable
                allow-create
                default-first-option
                reserve-keyword
                @change="(val: any) => onRuleSimpleValueChange(r, val)"
              >
                <el-option v-for="vn in variableNameOptions" :key="vn" :label="vn" :value="vn" />
              </el-select>
              <el-button
                class="rule-row__delete"
                type="warning"
                style="width: 0.32rem !important"
                @click="removeRuleRow(rIdx)"
              >
                <el-icon><Delete /></el-icon>
              </el-button>
            </div>
          </div>
        </div>
      </div>
    </el-form>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { Plus, Delete, CircleClose, CirclePlus } from '@element-plus/icons-vue'
import { getInstancePoints, getAllInstances, getInstancesByIds } from '@/api/devicesManagement'

const formRef = ref()
const props = defineProps<{ cardData: any }>()
const cardData = ref<any>({})
watch(
  () => props.cardData,
  (v) => {
    if (!v) return
    cardData.value = v
    cardData.value.config = cardData.value.config || {}
    if (!Array.isArray(cardData.value.config.variables)) cardData.value.config.variables = []
    if (!Array.isArray(cardData.value.config.rule)) cardData.value.config.rule = []
  },
  { immediate: true },
)
// 实例/点位（与 FunctionSwitchForm 一致）
const instanceOptions = ref<Array<{ label: string; value: number | string; d: any }>>([])
const varPointOptions = ref<
  Record<number, Array<{ label: string; value: number | string; unit?: string; raw?: any }>>
>({})
function getVarPointOptions(idx: number) {
  return varPointOptions.value[idx] || []
}
// 缓存每个变量索引对应实例的点位数据，便于切换类型时直接复用
const instancePointsCache = ref<Record<number, any>>({})
function getActionOptionsFromCard(): Array<{ label: string; value: number | string }> {
  const cfg = cardData.value?.config || {}
  const raw =
    (Array.isArray(cfg.actions) && cfg.actions) ||
    (Array.isArray(cfg.action) && cfg.action) ||
    (Array.isArray(cardData.value?.actions) && cardData.value.actions) ||
    []
  return (raw as any[])
    .map((it: any) => {
      if (it == null) return { label: '', value: '' }
      if (typeof it === 'string' || typeof it === 'number') {
        return { label: String(it), value: it }
      }
      const label = String(it.label ?? it.name ?? it.title ?? it.value ?? it.id ?? '')
      const value = it.value ?? it.id ?? it.key ?? label
      return { label, value }
    })
    .filter((x) => x.label !== '')
}
function buildPointOptionsFromData(
  data: any,
  pointType: 'property' | 'measurement' | 'action',
): Array<{ label: string; value: number | string; unit?: string; raw?: any }> {
  if (pointType === 'action') {
    const raw = (
      Array.isArray((data as any)?.actions)
        ? (data as any).actions
        : Object.values((data as any)?.actions || {})
    ) as any[]
    return raw
      .filter((a: any) => a && (a.action_id != null || a.id != null) && a.name != null)
      .map((a: any) => ({
        label: String(a.name || ''),
        value:
          typeof a.action_id !== 'undefined'
            ? Number.isFinite(Number(a.action_id))
              ? Number(a.action_id)
              : String(a.action_id)
            : Number.isFinite(Number(a.id))
              ? Number(a.id)
              : String(a.id),
        unit: a.unit || '',
        raw: a,
      }))
  }
  if (pointType === 'property') {
    const props = Object.values((data as any)?.properties || {}) as any[]
    return props
      .filter((p: any) => p && p.property_id && p.name != null)
      .map((p: any) => ({
        label: String(p.name || ''),
        value: Number(p.property_id),
        unit: p.unit || '',
        raw: p,
      }))
  }
  const measurements = Object.values(
    (data as any)?.elements || (data as any)?.measurements || {},
  ) as any[]
  return measurements
    .filter((m: any) => m && m.measurement_id && m.name != null)
    .map((m: any) => ({
      label: String(m.name || ''),
      value: Number(m.measurement_id),
      unit: m.unit || '',
      raw: m,
    }))
}
async function fetchInstances() {
  try {
    const res = await getAllInstances()
    const list = Array.isArray(res?.data?.list) ? res.data.list : []
    instanceOptions.value = list
      .map((it: any) => ({
        label: String(it?.name || ''),
        value: Number(it?.id),
        d: it,
      }))
      .filter((opt: any) => !!opt.label && (Number.isFinite(opt.value) || String(opt.value)))
  } catch {
    instanceOptions.value = []
  }
}
function resolveInstanceId(raw: number | string): number {
  if (Number.isFinite(raw as any)) {
    const n = Number(raw)
    return n > 0 ? n : 0
  }
  const name = String(raw || '')
  const found = instanceOptions.value.find((o) => o.label === name || String(o.value) === name)
  const id = Number(found?.d?.id ?? found?.value)
  return Number.isFinite(id) && id > 0 ? id : 0
}
async function onVarInstanceChange(idx: number) {
  const v = cardData.value.config.variables[idx]
  // 重选实例时清空 point 及相关信息
  v.point_id = undefined
  v.point_name = ''
  v.unit = ''
  // 清空当前第三个下拉并获取该实例的点位
  varPointOptions.value[idx] = []
  const instanceId = v.instance_id
  // 保存实例名称
  if (Number.isFinite(instanceId) && instanceId > 0) {
    const instanceOpt = instanceOptions.value.find((opt) => opt.value === instanceId)
    v.instance_name = instanceOpt?.label || ''
  } else {
    v.instance_id = undefined
    v.instance_name = ''
  }
  if (!Number.isFinite(instanceId) || instanceId <= 0) {
    delete instancePointsCache.value[idx]
    return
  }
  try {
    const res = await getInstancePoints(instanceId)
    const data = res?.data || {}
    instancePointsCache.value[idx] = data
    // 若已存在 pointType（property/measurement/action），立即据此填充第三个下拉
    if (v.pointType) {
      varPointOptions.value[idx] = buildPointOptionsFromData(data, v.pointType as any)
    }
  } catch {
    delete instancePointsCache.value[idx]
    varPointOptions.value[idx] = []
  }
}
async function onVarPointTypeChange(idx: number) {
  const v = cardData.value.config.variables[idx]
  v.point_id = undefined
  v.point_name = ''
  v.unit = ''
  varPointOptions.value[idx] = []
  const instanceId = v.instance_id
  if (!v.pointType) return
  // 优先使用缓存
  const cached = instancePointsCache.value[idx]
  if (cached) {
    varPointOptions.value[idx] = buildPointOptionsFromData(cached, v.pointType as any)
    return
  }
  // 兜底：若无缓存但有实例，则请求一次
  if (!Number.isFinite(instanceId) || instanceId <= 0) return
  try {
    const res = await getInstancePoints(instanceId)
    const data = res?.data || {}
    instancePointsCache.value[idx] = data
    varPointOptions.value[idx] = buildPointOptionsFromData(data, v.pointType as any)
  } catch {
    delete instancePointsCache.value[idx]
    varPointOptions.value[idx] = []
  }
}
// 点位选择变化时，保存 point_name 和 unit
function onVarPointChange(idx: number) {
  const v = cardData.value.config.variables[idx]
  const pointId = v.point_id
  if (pointId == null || pointId === '') {
    v.point_id = undefined
    v.point_name = ''
    v.unit = ''
    return
  }
  // 从选项中找到对应的点位信息
  const options = varPointOptions.value[idx] || []
  const pointOpt = options.find((opt) => opt.value === pointId)
  if (pointOpt) {
    v.point_name = pointOpt.label || ''
    v.unit = pointOpt.unit || ''
  } else {
    // 如果找不到，尝试从缓存中查找
    const cached = instancePointsCache.value[idx]
    if (cached && v.pointType) {
      const allPoints = buildPointOptionsFromData(cached, v.pointType as any)
      const found = allPoints.find((opt) => opt.value === pointId)
      if (found) {
        v.point_name = found.label || ''
        v.unit = found.unit || ''
      } else {
        v.point_name = String(pointId)
        v.unit = ''
      }
    } else {
      v.point_name = String(pointId)
      v.unit = ''
    }
  }
}

function getCombinedExtraPairCount(v: any) {
  if (!Array.isArray(v.formula) || v.formula.length <= 3) return 0
  return Math.floor((v.formula.length - 3) / 2)
}
function addCombinedRow(v: any) {
  if (!Array.isArray(v.formula)) v.formula = []
  v.formula.push('')
  v.formula.push('')
}
function removeCombinedRow(v: any, pairIdx: number) {
  if (!Array.isArray(v.formula)) return
  v.formula.splice(3 + pairIdx * 2, 2)
}
function onCombinedOperandChange(v: any, index: number, val: any) {
  const s = String(val ?? '').trim()
  const n = Number(s)
  const isVar = variableNameOptions.value.includes(s)
  if (!Array.isArray(v.formula)) v.formula = ['', '', '']
  v.formula[index] = Number.isFinite(n) ? n : isVar ? s : ''
}
function onCombinedExtraOperandChange(v: any, index: number, val: any) {
  onCombinedOperandChange(v, index, val)
}

function onVariableTypeChange(v: any, idx: number) {
  if (v.type === 'combined') {
    if (!Array.isArray(v.formula) || v.formula.length < 3) v.formula = ['', '', '']
    v.instance_id = undefined
    v.instance_name = ''
    v.pointType = ''
    v.point_id = undefined
    v.point_name = ''
  } else {
    v.formula = []
  }
}
function addVariable() {
  const nextIdx = getNextVariableIndex()
  cardData.value.config.variables.push({
    name: `X${nextIdx}`,
    type: 'single',
    instance_id: undefined,
    instance_name: '',
    pointType: '',
    point_id: undefined,
    point_name: '',
    unit: '', // 单位字段
  })
}
function removeVariable(idx: number) {
  const vars = cardData.value.config.variables
  if (!Array.isArray(vars)) return
  vars.splice(idx, 1)
}
function getNextVariableIndex(): number {
  const list = cardData.value?.config?.variables
  const names: string[] = (Array.isArray(list) ? list : []).map((v: any) => v?.name).filter(Boolean)
  let max = 0
  for (const n of names) {
    const m = /^X(\d+)$/.exec(n)
    if (m) max = Math.max(max, Number(m[1]))
  }
  return max + 1
}
// 规则：简化为 Variables = value
const arithmeticOperatorOptions = ref<string[]>(['+', '-', '*', '/'])
const variableNameOptions = computed(() => {
  const list = cardData.value?.config?.variables
  const arr = Array.isArray(list) ? list : []
  return arr.map((v: any) => v?.name).filter(Boolean)
})
function addRuleRow() {
  if (!Array.isArray(cardData.value.config.rule)) cardData.value.config.rule = []
  cardData.value.config.rule.push({ Variables: '', value: '' })
}
function removeRuleRow(idx: number) {
  cardData.value.config.rule.splice(idx, 1)
}
function onRuleSimpleValueChange(r: any, val: any) {
  const s = String(val ?? '').trim()
  const n = Number(s)
  r.value = Number.isFinite(n) ? n : s
}

function normalizeVariables() {
  const vars = Array.isArray(cardData.value?.config?.variables)
    ? cardData.value.config.variables
    : []
  for (let idx = 0; idx < vars.length; idx++) {
    const v = vars[idx]
    if (!v) continue
    if (v.type === 'combined') {
      if (!Array.isArray(v.formula) || v.formula.length < 3) v.formula = ['', '', '']
      v.instance_id = undefined
      v.instance_name = ''
      v.pointType = ''
      v.point_id = undefined
      v.point_name = ''
      v.unit = v.unit || '' // 保留单位字段
    } else {
      if (v.pointType == null) v.pointType = ''
      if (v.unit == null) v.unit = '' // 确保单位字段存在
      if (!Array.isArray(v.formula)) v.formula = []
      // 确保 instance_id 和 instance_name 存在
      if (v.instance_id != null && Number.isFinite(v.instance_id) && v.instance_id > 0) {
        const instanceOpt = instanceOptions.value.find((opt) => opt.value === v.instance_id)
        if (instanceOpt) {
          v.instance_name = instanceOpt.label
        }
      }
      // 确保 point_name 和 unit 存在
      if (v.point_id != null && (!v.point_name || !v.unit)) {
        onVarPointChange(idx)
      }
    }
  }
  // 规则：将 value 可数值化的转 number
  const rules = cardData.value?.config?.rule
  if (Array.isArray(rules)) {
    for (const r of rules) {
      const n = Number(String(r?.value))
      if (String(r?.value ?? '').trim() !== '' && Number.isFinite(n)) r.value = n
    }
  }
}

function getVariableNameOptionsExcluding(selfIdx: number) {
  const list = Array.isArray(cardData.value?.config?.variables)
    ? cardData.value.config.variables
    : []
  return list
    .map((it: any, i: number) => (i === selfIdx ? '' : it?.name))
    .filter((x: string) => !!x)
}

function validateForm(): Promise<{ valid: boolean; data: any }> {
  return new Promise((resolve) => {
    const form = formRef.value as any
    if (form && typeof form.validate === 'function') {
      form.validate((valid: boolean) => {
        resolve({ valid: !!valid, data: cardData.value })
      })
    } else {
      resolve({ valid: true, data: cardData.value })
    }
  })
}

defineExpose({ validateForm })

onMounted(() => {
  fetchInstances().then(async () => {
    normalizeVariables()
    // 回显：批量获取实例信息以减少请求次数
    const vars = Array.isArray(cardData.value?.config?.variables)
      ? cardData.value.config.variables
      : []

    // 收集所有需要批量获取的 instance_id
    const instanceIdsToFetch = new Set<number>()
    const varIndexMap = new Map<number, number[]>() // instanceId -> [varIndexes]

    for (let idx = 0; idx < vars.length; idx++) {
      const v = vars[idx]
      if (!v || v.type === 'combined' || !v.instance_id || !v.pointType) continue
      const instanceId = Number(v.instance_id)
      if (!Number.isFinite(instanceId) || instanceId <= 0) continue
      instanceIdsToFetch.add(instanceId)
      if (!varIndexMap.has(instanceId)) {
        varIndexMap.set(instanceId, [])
      }
      varIndexMap.get(instanceId)!.push(idx)
    }

    // 批量获取实例信息
    if (instanceIdsToFetch.size > 0) {
      try {
        const idsArray = Array.from(instanceIdsToFetch)
        const res = await getInstancesByIds(idsArray)
        const instancesList = Array.isArray(res?.data?.list) ? res.data.list : []

        // 将返回的数据缓存到 instancePointsCache 中
        for (const instance of instancesList) {
          const instanceId = Number(instance?.instance_id)
          if (!Number.isFinite(instanceId) || instanceId <= 0) continue

          const pointsData = instance?.points || {}
          const varIndexes = varIndexMap.get(instanceId) || []

          for (const idx of varIndexes) {
            const v = vars[idx]
            if (!v || v.type === 'combined' || !v.instance_id || !v.pointType) continue

            // 缓存点位数据
            instancePointsCache.value[idx] = pointsData

            // 构建点位选项
            varPointOptions.value[idx] = buildPointOptionsFromData(pointsData, v.pointType as any)

            // 回显时同步保存 instance_name
            if (!v.instance_name) {
              v.instance_name = instance?.instance_name || ''
            }

            // 回显时同步保存 point_name 和 unit
            if (v.point_id && (!v.point_name || !v.unit)) {
              onVarPointChange(idx)
            }
          }
        }
      } catch {
        // 如果批量请求失败，回退到逐个请求
        for (let idx = 0; idx < vars.length; idx++) {
          const v = vars[idx]
          if (!v || v.type === 'combined' || !v.instance_id || !v.pointType) continue
          const instanceId = v.instance_id
          if (!Number.isFinite(instanceId) || instanceId <= 0) continue
          try {
            const res = await getInstancePoints(instanceId)
            const data = res?.data || {}
            instancePointsCache.value[idx] = data
            varPointOptions.value[idx] = buildPointOptionsFromData(data, v.pointType as any)
          } catch {
            delete instancePointsCache.value[idx]
            varPointOptions.value[idx] = []
          }
        }
      }
    }
  })
})
watch(
  () => cardData.value?.config?.variables,
  () => {
    normalizeVariables()
  },
  { deep: true },
)
</script>

<style lang="scss" scoped>
.voltage-class {
  .action-change-value-form {
    .main-section {
      display: flex;
      gap: 0.1rem;
    }
    .section {
      margin-bottom: 0.24rem;

      .section__header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        margin-bottom: 0.12rem;

        .section__title {
          font-weight: 700;
          color: #fff;
          font-size: 0.18rem;
        }

        .section__add-btn {
          width: 0.32rem !important;
        }
      }

      .section__body {
        display: flex;
        flex-direction: column;
        gap: 0.12rem;
        &.variable,
        &.rule {
          height: 4rem;
          overflow-y: auto;
        }
      }
    }
    .variable-section {
      padding-right: 0.1rem;
      width: 50%;
      border-right: 0.01rem solid rgba(255, 255, 255, 0.1);
    }
    .rule-section {
      width: calc(50% - 0.1rem);
    }
    .variable-row__controls,
    .combined-row {
      display: flex;
      align-items: center;
      gap: 0.08rem;
    }
    .variable-row__select {
      width: 1.64rem;
    }
    :deep(.el-select .el-select__selected-item) {
      max-width: 100%;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .variable-row__delete {
      padding: 0 0.04rem;
    }
    .rule-row {
      display: flex;
      flex-direction: column;
      gap: 0.12rem;
      padding-bottom: 0.2rem;
      border-bottom: 0.01rem solid rgba(255, 255, 255, 0.1);
      &:last-child {
        padding-bottom: 0;
        border-bottom: none;
      }
    }
    .rule-row__head,
    .variable-row {
      display: flex;
      justify-content: space-between;
    }
    .rule-row__name,
    .variable-row__item {
      margin-bottom: 0;
      align-items: flex-start;
    }
    .rule-row__type {
      width: 2.4rem;
      .rule-row__delete {
        padding: 0 0.04rem;
      }
    }
    .rule-row__body {
      display: flex;
      align-items: center;
      gap: 0.08rem;
      justify-content: center;
    }
    .rule-row__select {
      min-width: 0;
    }
    .rule-row__default,
    .variable-row__combined {
      display: flex;
      flex-direction: column;
      gap: 0.08rem;
    }
    .default-cond-row {
      display: flex;
      align-items: center;
      gap: 0.08rem;
    }
    .default-cond-row__relation {
      width: 1rem !important;
    }
    .default-cond-row__delete {
      padding: 0 0.04rem;
    }
    .rule-row-simple {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 0.08rem;
      .rule-row__eq {
        color: #fff;
        opacity: 0.9;
      }
    }
  }
}
</style>
