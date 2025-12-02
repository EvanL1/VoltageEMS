<template>
  <div class="voltage-class function-switch-form">
    <el-form ref="formRef" label-width="0.6rem" :model="cardData">
      <!-- 第一部分：基础设置 -->
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
        <!-- 第二部分：变量设置 -->
        <div class="section variable-section">
          <div class="section__header">
            <span class="section__title">Variable Settings</span>
            <el-button class="section__add-btn" type="primary" link @click="addVariable">
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
                    v-model="v.instance"
                    placeholder="instance"
                    class="flex-item variable-row__select"
                    filterable
                    allow-create
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
                    :disabled="!v.instance"
                    @change="() => onVarPointTypeChange(idx)"
                  >
                    <el-option label="measurement" value="measurement" />
                    <el-option label="property" value="property" />
                  </el-select>
                  <el-select
                    v-model="v.point"
                    placeholder="point"
                    class="flex-item variable-row__select"
                    filterable
                    :disabled="!v.instance || !v.pointType"
                  >
                    <el-option
                      v-for="opt in getVarPointOptions(idx)"
                      :key="opt.value"
                      :label="opt.label"
                      :value="opt.value"
                    />
                  </el-select>
                </div>

                <!-- combined 模式：公式编辑器 -->
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

                  <!-- 追加的 (op, operand) 对 -->
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

        <!-- 第三部分：规则配置 -->
        <div class="section rule-section">
          <div class="section__header">
            <span class="section__title">Rule Settings</span>
            <el-button class="section__add-btn" type="primary" link @click="addRule">
              <el-icon><Plus /></el-icon>
            </el-button>
          </div>
          <div class="section__body rule">
            <div
              v-for="(r, rIdx) in cardData.config.rule"
              :key="`rule-${rIdx}-${r.name}`"
              class="rule-row"
            >
              <!-- 顶层规则项头部：名称 + 类型选择 + 删除 -->
              <div class="rule-row__head">
                <el-form-item :label="r.name + ':'" class="rule-row__name">
                  <!-- <el-radio-group
                  v-model="r.type"
                  class="rule-row__type"
                  @change="onRuleTypeChange(r)"
                >
                  <el-radio v-for="t in ruleTypeOptions" :key="t" :label="t">{{ t }}</el-radio>
                </el-radio-group> -->
                  <!-- alone 类型：三个选择框 -->
                  <div v-if="r.type === 'alone'" class="rule-row__body">
                    <el-select
                      v-model="r.rule.variables"
                      class="rule-row__select"
                      placeholder="variable"
                      filterable
                    >
                      <el-option v-for="v in variableNameOptions" :key="v" :label="v" :value="v" />
                    </el-select>
                    <el-select
                      v-model="r.rule.operator"
                      class="rule-row__select"
                      placeholder="operator"
                    >
                      <el-option v-for="op in operatorOptions" :key="op" :label="op" :value="op" />
                    </el-select>
                    <el-select
                      v-model="r.rule.value"
                      class="rule-row__select"
                      placeholder="value"
                      filterable
                      allow-create
                      default-first-option
                      reserve-keyword
                      @change="(val: any) => onRuleValueChange(r, val)"
                    >
                      <el-option v-for="v in variableNameOptions" :key="v" :label="v" :value="v" />
                    </el-select>
                  </div>

                  <!-- default 类型：多行，每行三选择框；第二行开始前置 relation 选择器，可删除 -->
                  <div v-else-if="r.type === 'default'" class="rule-row__default">
                    <div v-for="(sub, i) in r.rule" :key="`sub-${i}`">
                      <template v-if="sub.type === 'variable'">
                        <div class="default-cond-row">
                          <!-- 从第二个条件开始在行首显示 relation 选择器（绑定前一项） -->
                          <el-select
                            v-if="i > 0 && r.rule[i - 1] && r.rule[i - 1].type === 'relation'"
                            v-model="r.rule[i - 1].value"
                            class="default-cond-row__relation"
                            placeholder="relation"
                          >
                            <el-option
                              v-for="rel in relationOptions"
                              :key="rel"
                              :label="rel"
                              :value="rel"
                            />
                          </el-select>

                          <el-select
                            :style="i == 0 ? 'margin-left: 1.08rem;' : ''"
                            v-model="sub.variables"
                            class="rule-row__select"
                            placeholder="variable"
                            filterable
                          >
                            <el-option
                              v-for="vn in variableNameOptions"
                              :key="vn"
                              :label="vn"
                              :value="vn"
                            />
                          </el-select>
                          <el-select
                            v-model="sub.operator"
                            class="rule-row__select"
                            placeholder="operator"
                          >
                            <el-option
                              v-for="op in operatorOptions"
                              :key="op"
                              :label="op"
                              :value="op"
                            />
                          </el-select>
                          <el-select
                            v-model="sub.value"
                            class="rule-row__select"
                            placeholder="value"
                            filterable
                            allow-create
                            default-first-option
                            reserve-keyword
                            @change="(val: any) => onRuleSubValueChange(sub, val)"
                          >
                            <el-option
                              v-for="v in variableNameOptions"
                              :key="v"
                              :label="v"
                              :value="v"
                            />
                          </el-select>

                          <!-- 从第二个条件开始提供删除按钮（需同时移除其前置 relation） -->
                          <el-button
                            v-if="i > 0"
                            class="default-cond-row__delete"
                            type="warning"
                            link
                            @click="removedefaultCondition(r, i)"
                          >
                            <el-icon style="color: red"><CircleClose /></el-icon>
                          </el-button>
                        </div>
                      </template>
                    </div>
                    <div class="rule-row__default-add">
                      <el-button type="primary" link @click="adddefaultCondition(r)">
                        <el-icon style="color: green"><CirclePlus /></el-icon>
                      </el-button>
                    </div>
                  </div>

                  <!-- custom 类型：自定义输入框 -->
                  <div v-else class="rule-row__body">
                    <el-input
                      v-model="r.rule"
                      type="textarea"
                      :rows="2"
                      placeholder="自定义规则表达式"
                    />
                  </div>
                </el-form-item>
                <el-button
                  class="rule-row__delete"
                  style="width: 0.32rem !important"
                  type="warning"
                  @click="removeRule(rIdx)"
                >
                  <el-icon><Delete /></el-icon>
                </el-button>
              </div>
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
import { getInstancePoints, getAllInstances } from '@/api/devicesManagement'

const formRef = ref()

// props: 从父级接收并直接绑定（引用相同对象，便于双向同步）
const props = defineProps<{ cardData: any }>()
const cardData = ref<any>({})
watch(
  () => props.cardData,
  (v) => {
    if (!v) return
    // 引用同一对象，内部修改将同步到父级
    cardData.value = v
  },
  { immediate: true },
)

// 选项（协议固定，实例/点位从接口获取）
const instanceOptions = ref<Array<{ label: string; value: number | string; d: any }>>([])
const varPointOptions = ref<Record<number, Array<{ label: string; value: number }>>>({})
// 缓存每个变量索引对应实例的点位数据，避免切换类型时重复请求
const instancePointsCache = ref<Record<number, any>>({})
function getVarPointOptions(idx: number) {
  return varPointOptions.value[idx] || []
}
function buildPointOptionsFromData(
  data: any,
  pointType: 'property' | 'measurement',
): Array<{ label: string; value: number }> {
  if (pointType === 'property') {
    const props = Object.values((data as any)?.properties || {}) as any[]
    return props
      .filter((p: any) => p && p.property_id && p.name != null)
      .map((p: any) => ({ label: String(p.name || ''), value: Number(p.property_id) }))
  }
  const measurements = Object.values(
    (data as any)?.elements || (data as any)?.measurements || {},
  ) as any[]
  return measurements
    .filter((m: any) => m && m.measurement_id && m.name != null)
    .map((m: any) => ({ label: String(m.name || ''), value: Number(m.measurement_id) }))
}
async function fetchInstances() {
  try {
    const res = await getAllInstances()
    const list = Array.isArray(res?.data?.list) ? res.data.list : []
    instanceOptions.value = list
      .map((it: any) => ({
        label: String(it?.instance_name || ''),
        value: Number(it?.instance_id),
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
  const id = Number((found as any)?.d?.instance_id ?? (found as any)?.value)
  return Number.isFinite(id) && id > 0 ? id : 0
}
function normalizeVariables() {
  const vars = Array.isArray(cardData.value?.config?.variables)
    ? cardData.value.config.variables
    : []
  for (const v of vars) {
    if (!v) continue
    if (v.type === 'combined') {
      if (!Array.isArray(v.formula) || v.formula.length < 3) v.formula = ['', '', '']
      v.instance = ''
      v.pointType = ''
      v.point = ''
    } else {
      if (v.instance == null) v.instance = ''
      if (v.pointType == null) v.pointType = ''
      if (v.point == null) v.point = ''
      if (!Array.isArray(v.formula)) v.formula = []
    }
  }
}
async function preloadVarOptions() {
  const vars = Array.isArray(cardData.value?.config?.variables)
    ? cardData.value.config.variables
    : []
  for (let idx = 0; idx < vars.length; idx++) {
    const v = vars[idx]
    // 仅当已选 instance + pointType 时加载并缓存，便于回显 label（不清空已选 point）
    if (!v || v.type === 'combined' || !v.instance || !v.pointType) continue
    const instanceId = resolveInstanceId(v.instance)
    try {
      if (!Number.isFinite(instanceId) || instanceId <= 0) continue
      const res = await getInstancePoints(instanceId)
      const data = res?.data || {}
      instancePointsCache.value[idx] = data
      varPointOptions.value[idx] = buildPointOptionsFromData(
        data,
        v.pointType === 'property' ? 'property' : 'measurement',
      )
    } catch {
      delete instancePointsCache.value[idx]
      varPointOptions.value[idx] = []
    }
  }
}
async function onVarInstanceChange(idx: number) {
  const v = cardData.value.config.variables[idx]
  // 用户切换实例：清空已选点位
  v.point = ''
  // 清空第三下拉
  varPointOptions.value[idx] = []
  const instanceId = resolveInstanceId(v.instance)
  if (!Number.isFinite(instanceId) || instanceId <= 0) return
  // 不预选 pointType，等待用户选择 pointType 后再加载具体列表
  try {
    const res = await getInstancePoints(instanceId)
    const data = res?.data || {}
    instancePointsCache.value[idx] = data
    if (v.pointType === 'property' || v.pointType === 'measurement') {
      varPointOptions.value[idx] = buildPointOptionsFromData(
        data,
        v.pointType === 'property' ? 'property' : 'measurement',
      )
    }
  } catch {
    delete instancePointsCache.value[idx]
    varPointOptions.value[idx] = []
  }
}
async function onVarPointTypeChange(idx: number) {
  const v = cardData.value.config.variables[idx]
  // 切换类型：不发请求，优先使用缓存
  v.point = ''
  varPointOptions.value[idx] = []
  const instanceId = resolveInstanceId(v.instance)
  if (!v.pointType) return
  const cached = instancePointsCache.value[idx]
  if (cached) {
    varPointOptions.value[idx] = buildPointOptionsFromData(
      cached,
      v.pointType === 'property' ? 'property' : 'measurement',
    )
    return
  }
  // 兜底：若无缓存但已选实例，补一次请求
  if (!Number.isFinite(instanceId) || instanceId <= 0) return
  try {
    const res = await getInstancePoints(instanceId)
    const data = res?.data || {}
    instancePointsCache.value[idx] = data
    varPointOptions.value[idx] = buildPointOptionsFromData(
      data,
      v.pointType === 'property' ? 'property' : 'measurement',
    )
  } catch {
    delete instancePointsCache.value[idx]
    varPointOptions.value[idx] = []
  }
}
const operatorOptions = ref<string[]>(['==', '!=', '>', '>=', '<', '<='])
const relationOptions = ref<string[]>(['And'])
const arithmeticOperatorOptions = ref<string[]>(['+', '-', '*', '/'])
// const ruleTypeOptions = ref<string[]>(['alone', 'default', 'custom'])
const ruleTypeOptions = ref<string[]>(['default', 'custom'])

const variableNameOptions = computed(() => {
  const list = cardData.value?.config?.variables
  const arr = Array.isArray(list) ? list : []
  return arr.map((v: any) => v?.name).filter(Boolean)
})

// 取消监听，改为在 validateForm 中统一同步 wires

// 变量设置：新增 / 删除
function addVariable() {
  const nextIdx = getNextVariableIndex()
  cardData.value.config.variables.push({
    name: `X${nextIdx}`,
    type: 'single',
    instance: '',
    pointType: '',
    point: '',
  })
}
function removeVariable(idx: number) {
  const vars = cardData.value.config.variables
  if (!Array.isArray(vars)) return
  const removed = vars[idx]
  vars.splice(idx, 1)
  // 清理所有规则中对已删除变量名的引用
  clearInvalidVariableReferences()
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

// 移除无效的变量引用：当规则里引用了已不存在的变量名时，将其置空
function clearInvalidVariableReferences() {
  const list = cardData.value?.config?.variables
  const validNames = new Set(
    (Array.isArray(list) ? list : []).map((v: any) => v?.name).filter(Boolean),
  )
  const rules = cardData.value?.config?.rule
  if (!Array.isArray(rules)) return
  for (const r of rules) {
    if (r?.type === 'alone' && r.rule) {
      if (r.rule.variables && !validNames.has(r.rule.variables)) {
        r.rule.variables = ''
      }
      if (r.rule.value && !validNames.has(r.rule.value)) {
        r.rule.value = ''
      }
    } else if (r?.type === 'default' && Array.isArray(r.rule)) {
      for (const sub of r.rule) {
        if (sub && sub.type === 'variable') {
          if (sub.variables && !validNames.has(sub.variables)) {
            sub.variables = ''
          }
          if (sub.value && !validNames.has(sub.value)) {
            sub.value = ''
          }
        }
      }
    }
  }
}

function onCombinedOperandChange(v: any, index: number, val: any) {
  const s = String(val ?? '').trim()
  const n = Number(s)
  // 仅当为数值时才保留数值；否则需要为已有变量名，若既非变量名又非数值则置空
  const isVar = variableNameOptions.value.includes(s)
  v.formula[index] = Number.isFinite(n) ? n : isVar ? s : ''
}
function onCombinedExtraOperandChange(v: any, index: number, val: any) {
  onCombinedOperandChange(v, index, val)
}
function onRuleValueChange(r: any, val: any) {
  const s = String(val ?? '').trim()
  const n = Number(s)
  r.rule.value = Number.isFinite(n) ? n : s
}
function onRuleSubValueChange(sub: any, val: any) {
  const s = String(val ?? '').trim()
  const n = Number(s)
  sub.value = Number.isFinite(n) ? n : s
}

// 规则配置：新增 / 删除顶层规则项
function addRule() {
  const nextName = getNextOutName()
  cardData.value.config.rule.push({
    name: nextName,
    type: 'default',
    rule: [
      {
        type: 'variable',
        variables: variableNameOptions.value[0] || '',
        operator: operatorOptions.value[0],
        value: variableNameOptions.value[0] || '',
      },
    ],
  })
}
function removeRule(idx: number) {
  cardData.value.config.rule.splice(idx, 1)
}
function getNextOutName(): string {
  const names: string[] = (cardData.value?.config?.rule || []).map((r: any) => r.name)
  let max = 0
  for (const n of names) {
    const m = /^out(\d+)$/.exec(n)
    if (m) max = Math.max(max, Number(m[1]))
  }
  const next = String(max + 1).padStart(3, '0')
  return `out${next}`
}

// 当类型切换时，初始化对应结构
function onRuleTypeChange(r: any) {
  if (r.type === 'alone') {
    r.rule = {
      variables: variableNameOptions.value[0] || '',
      operator: operatorOptions.value[0],
      value: variableNameOptions.value[0] || '',
    }
  } else if (r.type === 'default') {
    r.rule = [
      {
        type: 'variable',
        variables: variableNameOptions.value[0] || '',
        value: variableNameOptions.value[0] || '',
        operator: operatorOptions.value[0],
      },
    ]
  } else if (r.type === 'custom') {
    r.rule = ''
  }
}

// default：添加 / 删除条件
function adddefaultCondition(r: any) {
  if (!Array.isArray(r.rule)) r.rule = []
  r.rule.push({ type: 'relation', value: '&&' })
  r.rule.push({
    type: 'variable',
    variables: variableNameOptions.value[0] || '',
    value: variableNameOptions.value[0] || '',
    operator: operatorOptions.value[0],
  })
}
function removedefaultCondition(r: any, variableRuleIndex: number) {
  if (!Array.isArray(r.rule)) return
  // 先移除前置 relation（如果存在）
  if (
    variableRuleIndex - 1 >= 0 &&
    r.rule[variableRuleIndex - 1] &&
    r.rule[variableRuleIndex - 1].type === 'relation'
  ) {
    r.rule.splice(variableRuleIndex - 1, 1)
    r.rule.splice(variableRuleIndex - 1, 1) // 删除后，原变量项索引前移一位
  } else {
    r.rule.splice(variableRuleIndex, 1)
  }
}
function afterValidSync() {
  const names: string[] = cardData.value.config.rule.map((r: any) => r?.name).filter(Boolean)
  Object.keys(cardData.value.config.wires).forEach((k) => {
    if (k !== 'default' && !names.includes(k)) delete cardData.value?.config?.wires[k]
  })
  for (const n of names) {
    if (!Array.isArray(cardData.value?.config.wires[n])) cardData.value.config.wires[n] = []
  }

  // 变量规范化：
  const vars = Array.isArray(cardData.value?.config?.variables)
    ? cardData.value.config.variables
    : []
  for (const v of vars) {
    if (!v) continue
    if (v.type === 'combined') {
      // 仅保留 type/name/formula（保留已填值，如缺失则补齐前三项）
      v.formula = Array.isArray(v.formula) && v.formula.length >= 3 ? v.formula : ['', '', '']
      delete v.instance
      delete v.pointType
      delete v.point
    } else {
      // single：仅保留 type/name/instance/pointType/point（保留用户输入）
      v.type = 'single'
      if (v.instance == null) v.instance = ''
      if (v.pointType == null) v.pointType = ''
      if (v.point == null) v.point = ''
      delete v.formula
    }
    // 导出前数值归一化：formula 中可转为数值的字符串统一转 number
    if (Array.isArray(v.formula)) {
      v.formula = v.formula.map((x: any) => {
        const nx = Number(String(x))
        return String(x).trim() !== '' && Number.isFinite(nx) ? nx : x
      })
    }
  }

  // 规则值归一化：value 尝试数值化，variables 不动
  const rules = cardData.value?.config?.rule
  if (Array.isArray(rules)) {
    for (const r of rules) {
      if (r?.type === 'alone' && r.rule) {
        const n = Number(String(r.rule.value))
        if (String(r.rule.value).trim() !== '' && Number.isFinite(n)) r.rule.value = n
      } else if (r?.type === 'default' && Array.isArray(r.rule)) {
        for (const sub of r.rule) {
          if (sub && sub.type === 'variable') {
            const n = Number(String(sub.value))
            if (String(sub.value).trim() !== '' && Number.isFinite(n)) sub.value = n
          }
        }
      }
    }
  }
}
// variable: 类型切换
function onVariableTypeChange(v: any, idx: number) {
  if (v.type === 'combined') {
    // 初始化 formula 结构
    if (!Array.isArray(v.formula)) v.formula = ['', '', '']
    // 清空 single 相关字段，仅保留 type/name/formula
    v.instance = ''
    v.pointType = 'measurement'
    v.point = ''
  } else {
    // single 模式仅保留 type/name/instance/pointType/point
    v.formula = []
  }
}

// variable: 追加一行 (op, operand)
function addCombinedRow(v: any) {
  if (!Array.isArray(v.formula)) v.formula = ['', '', '']
  v.formula.push('')
  v.formula.push('')
}

// variable: 删除指定的 (op, operand) 行（不动首行的三元素）
function removeCombinedRow(v: any, pairIndex: number) {
  if (!Array.isArray(v.formula)) return
  const start = 3 + pairIndex * 2
  if (start <= v.formula.length - 2) {
    v.formula.splice(start, 2)
  }
}

// variable: 额外的 (op, operand) 对数量
function getCombinedExtraPairCount(v: any) {
  if (!Array.isArray(v.formula) || v.formula.length <= 3) return 0
  return Math.floor((v.formula.length - 3) / 2)
}

// 变量名选项（排除自己）
function getVariableNameOptionsExcluding(selfIdx: number) {
  const list = Array.isArray(cardData.value?.config?.variables)
    ? cardData.value.config.variables
    : []
  return list
    .map((it: any, i: number) => (i === selfIdx ? '' : it?.name))
    .filter((x: string) => !!x)
}
// 使用 el-form 内置校验
function validateForm(): Promise<{ valid: boolean; data: any }> {
  return new Promise((resolve) => {
    const form = formRef.value as any
    if (form && typeof form.validate === 'function') {
      form.validate((valid: boolean) => {
        if (valid) afterValidSync()
        console.log(cardData.value)

        resolve({ valid: !!valid, data: cardData.value })
      })
    } else {
      afterValidSync()
      resolve({ valid: true, data: cardData.value })
    }
  })
}

defineExpose({ validateForm })

onMounted(() => {
  fetchInstances().then(() => {
    normalizeVariables()
    preloadVarOptions()
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
.voltage-class,
.function-switch-form {
  .main-section {
    display: flex;
    gap: 0.1rem;
  }
  .section {
    &.variable-section {
      padding-right: 0.1rem;
      width: 50%;
      border-right: 1px solid rgba(255, 255, 255, 0.1);
    }
    &.rule-section {
      width: calc(50% - 0.1rem);
    }
    &.basic-section {
      width: 100%;
      padding-bottom: 0.2rem;
      border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    }
    .section__header {
      margin-top: 0.2rem;
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
        padding: 0 0.04rem;
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
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    // padding: 0.12rem;
    &:last-child {
      padding-bottom: 0;
      border-bottom: none;
    }
  }
  .rule-row__head,
  .variable-row {
    display: flex;
    // align-items: center;
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
    // margin-left: 0.12rem;
  }
  .rule-row__body {
    display: flex;
    align-items: center;
    gap: 0.08rem;
    // margin-left: 1.36rem;
  }

  .rule-row__select {
    width: 1.2rem;
  }
  :deep(.el-input),
  :deep(.el-select) {
    max-width: 100%;
  }
  .rule-row__default,
  .variable-row__combined {
    display: flex;
    flex-direction: column;
    gap: 0.08rem;
    /* keep space for nested rows */
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
  .rule-row__default-add {
    min-height: 0.2rem;
  }
}

// // 统一输入组件宽度
// .voltage-class .function-switch-form :deep(.el-input),
// .voltage-class .function-switch-form :deep(.el-select) {
//   width: 1.2rem;
// }
</style>
