<template>
  <div class="voltage-class rule-chain-editor" :class="{ 'is-fullscreen': isFullscreen }">
    <div class="rule-chain-editor__header">
      <div class="rule-chain-editor__header-left">
        <div class="rule-chain-editor__header-left-back" @click="goBackToList">
          <img :src="backIconSrc" />
          Back
        </div>
        <div class="rule-chain-editor__header-left-fenge"></div>
        <div class="rule-chain-editor__header-right-name">
          {{ ruleChainStore.currentRuleChain?.name }}
        </div>
      </div>
      <div class="rule-chain-editor__header-right">
        <el-button type="primary" @click="toggleFullscreen" class="custom-button">
          <el-icon>
            <FullScreen />
          </el-icon>
          {{ isFullscreen ? 'Exit Fullscreen' : 'Fullscreen' }}
        </el-button>

        <el-button
          v-if="!isMonitorMode"
          type="primary"
          @click="handleImportClick"
          class="custom-button"
        >
          <el-icon>
            <Download />
          </el-icon>
          Import
        </el-button>
        <el-button v-if="isMonitorMode" type="primary" @click="handleExport" class="custom-button">
          <el-icon>
            <Upload />
          </el-icon>
          Export
        </el-button>
        <el-button v-if="isMonitorMode" type="primary" @click="enterEditMode" class="custom-button">
          <el-icon>
            <Edit />
          </el-icon>
          Edit
        </el-button>
        <el-button
          v-if="!isMonitorMode"
          type="warning"
          @click="handleExitEdit"
          class="custom-button"
          >Cancel Edit</el-button
        >
      </div>
    </div>

    <div class="rule-chain-editor__content">
      <div v-if="!isMonitorMode" class="rule-chain-editor__left-panel">
        <div class="rule-chain-editor__card-categories">
          <el-collapse v-model="activeCategories">
            <el-collapse-item
              v-for="category in cardCategories"
              :key="category.type"
              :name="category.type"
              :title="category.title"
              style="margin-bottom: 0.2rem"
            >
              <template #title>
                <div class="rule-chain-editor__category-title">
                  <span>{{ category.title }}</span>
                </div>
              </template>

              <div class="rule-chain-editor__cards">
                <div
                  v-for="card in category.cards"
                  :key="card.id"
                  class="rule-chain-editor__card"
                  :data-type="card.type"
                  draggable="true"
                  @dragstart="onDragStart($event, card as unknown as RuleCard)"
                >
                  <div class="rule-chain-editor__card-icon" :class="`icon--${card.type}`">
                    <img :src="card.icon" v-if="card.type === 'function-switch'" />
                    <img :src="card.icon" v-else-if="card.type === 'action-changeValue'" />
                  </div>
                  <div class="rule-chain-editor__card-content">
                    <div class="rule-chain-editor__card-name">{{ card.name }}</div>
                    <div class="rule-chain-editor__card-description">
                      {{ card.description }}
                    </div>
                  </div>
                </div>
              </div>
            </el-collapse-item>
          </el-collapse>
        </div>
      </div>

      <div ref="centerPanelRef" class="rule-chain-editor__center-panel" @drop="handleDropGuard">
        <VueFlow
          fit-view-on-init
          :connection-mode="ConnectionMode.Strict"
          :only-render-visible-elements="true"
          :nodes="nodes"
          :edges="edges"
          class="rule-chain-editor__flow"
          :default-viewport="{ zoom: 1, x: 0, y: 0 }"
          :min-zoom="0.1"
          :max-zoom="4"
          :snap-to-grid="true"
          :snap-grid="[1, 1]"
          :node-types="nodeTypes"
          :connection-line-style="{ stroke: '#ff8a00', strokeWidth: 2.5 }"
          :nodes-draggable="true"
          :nodes-connectable="!isMonitorMode"
          :elements-selectable="!isMonitorMode"
          @connect="handleConnectGuard"
          @node-double-click="handleNodeClick"
          @dragover="handleDragOverGuard"
          @dragleave="handleDragLeaveGuard"
        >
          <template #node-custom="nodeProps">
            <div class="rf-node-with-vars">
              <CustomNode v-bind="nodeProps" :is-monitor-mode="isMonitorMode" />
            </div>
          </template>
          <template #node-start="nodeProps">
            <StartNode v-bind="nodeProps" />
          </template>
          <template #node-end="nodeProps">
            <EndNode v-bind="nodeProps" />
          </template>
          <Background variant="lines" :gap="20" color="rgba(255, 255, 255, 0.1)" />
          <MiniMap
            class="rf-minimap-custom"
            :node-stroke-color="'#74b9ff'"
            :node-color="'#ddd'"
            :node-border-radius="2"
            position="top-right"
          />

          <Controls position="bottom-right" />
        </VueFlow>
        <Teleport to="body">
          <template v-for="nodeId in Array.from(visibleVarsNodes)" :key="nodeId">
            <div
              v-if="isMonitorMode && nodeVarsPositions.has(nodeId) && getNodeVarsData(nodeId)"
              class="node-vars-bubble-fixed"
              :style="getBubbleStyle(nodeId)"
            >
              <div
                class="node-vars-bubble__row"
                v-for="v in getNodeVarsData(nodeId)"
                :key="`${nodeId}-${v.name}-${v.instance}-${v.point}`"
              >
                <div class="node-vars-bubble__left">
                  <span class="var-item">{{
                    `${v.instance_name || v.instance || '-'}/${v.point_name || v.point || '-'}`
                  }}</span>
                </div>
                <div class="node-vars-bubble__right">
                  <span>{{ v.value !== undefined ? v.value : '-' }}</span>
                  <span v-if="v.unit" class="node-vars-bubble__unit">{{ v.unit }}</span>
                </div>
              </div>
            </div>
          </template>
        </Teleport>
      </div>
    </div>

    <div v-if="!isMonitorMode" class="rule-chain-editor__floating-actions">
      <el-button
        circle
        type="warning"
        class="floating-btn floating-btn--cancel"
        @click="handleCancel"
        :disabled="!hasUnsavedChanges"
        title="Cancel"
      >
        ×
      </el-button>
      <el-button
        circle
        type="primary"
        class="floating-btn floating-btn--submit"
        @click="handleSave"
        :disabled="!hasUnsavedChanges"
        title="Submit"
      >
        √
      </el-button>
    </div>

    <CardEditDialog
      v-model:visible="cardEditDialogVisible"
      :card="editingCard"
      @save="handleCardEditConfirm"
    />

    <input
      ref="importFileInput"
      type="file"
      accept="application/json"
      style="display: none"
      @change="handleImportChange"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch, onUnmounted, nextTick, Teleport } from 'vue'
import { getCurrentFontSize } from '@/utils/responsive'
import { useRoute, useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { getRuleDetail } from '@/api/rulesManagement'
import backIcon from '@/assets/icons/button-back.svg'
const backIconSrc = backIcon as string
import { Background } from '@vue-flow/background'
import { Controls } from '@vue-flow/controls'
import { FullScreen, Download, Upload, Edit } from '@element-plus/icons-vue'
import {
  VueFlow,
  ConnectionMode,
  useVueFlow,
  type Node as FlowNode,
  type Edge as FlowEdge,
  type Connection,
  type NodeChange,
} from '@vue-flow/core'
import changeIcon from '@/assets/icons/button-change.svg'
import fenzhiIcon from '@/assets/icons/button-fenzhi.svg'
import CustomNode from './components/customCard/CustomNode.vue'
import StartNode from './components/customCard/StartNode.vue'
import EndNode from './components/customCard/EndNode.vue'
// import { Background } from '@vue-flow/background'
import { MiniMap } from '@vue-flow/minimap'
// import { Controls } from '@vue-flow/controls'
import { useRuleChainStore } from '@/stores/ruleChain.ts'
import type {
  RuleCard,
  RuleChain,
  Node as AppNode,
  Edge as AppEdge,
  Rule,
} from '@/types/ruleConfiguration'
import type { RuleChainPayload } from '@/types/ruleConfiguration'
import useDragAndDrop from '@/utils/useDnd'
import CardEditDialog from './components/CardEditDialog.vue'
import { updateRule } from '@/api/rulesManagement'
import wsManager from '@/utils/websocket'
const {
  updateNode,
  toObject,
  addEdges,
  onNodesChange,
  onEdgesChange,
  onConnect,
  onEdgeUpdate,
  onNodeDragStop,
  setNodes,
  setEdges,
  applyEdgeChanges,
  removeEdges,
  fitView,
  viewport,
  findNode,
  flowToScreenCoordinate,
} = useVueFlow()
const { onDragStart, onDragOver, onDragLeave, onDrop } = useDragAndDrop()
// 路由
const route = useRoute()
const router = useRouter()

const ruleChainStore = useRuleChainStore()

const currentChainId = ref('')
const isMonitorMode = ref(true)
const activeCategories = ref(['function', 'action'])
const cardEditDialogVisible = ref(false)
const editingCard = ref<RuleCard | null>(null)
const importFileInput = ref<HTMLInputElement | null>(null)
const subscriptionId = ref<string>('')
const visibleVarsNodes = ref<Set<string>>(new Set())
const lastActiveNodeIds = ref<Set<string>>(new Set())
const nodeVarsPositions = ref<Map<string, { top: number; left: number }>>(new Map())
const centerPanelRef = ref<HTMLElement | null>(null)
const nodeVariablesData = ref<Map<string, Record<string, number>>>(new Map())

const nodeTypes: any = {
  custom: CustomNode,
  start: StartNode,
  end: EndNode,
}

const nodes = computed(() => {
  return isMonitorMode.value ? ruleChainStore.monitorNodes : ruleChainStore.nodes
})
const edges = computed(() => {
  return isMonitorMode.value ? ruleChainStore.monitorEdges : ruleChainStore.edges
})
const isFullscreen = computed(() => ruleChainStore.isFullscreen)
const hasUnsavedChanges = computed(() => ruleChainStore.hasUnsavedChanges)

// 规则卡片分类
const cardCategories = ref([
  {
    type: 'function',
    title: 'Funtcion',
    icon: 'Filter',
    cards: [
      {
        id: 'function-2',
        name: 'Switch Function',
        type: 'function-switch',
        description: 'Switch function',
        icon: fenzhiIcon,
        config: {
          variables: [],
          rule: [],
          wires: {},
        },
      },
    ],
  },
  {
    type: 'action',
    title: 'Action',
    icon: 'Operation',
    tooltip: 'action cards are used to perform specific actions',
    cards: [
      {
        id: 'action-1',
        name: 'Change Value',
        type: 'action-changeValue',
        description: 'change value of a point',
        icon: changeIcon,
        config: { rule: [], wires: {} },
      },
    ],
  },
])

// 确保 start 和 end 节点不可删除
function ensureStartEndNodesUndeletable(nodes: FlowNode[]) {
  return nodes.map((node) => {
    if (node.id === 'start' || node.id === 'end') {
      return {
        ...node,
        deletable: false,
      }
    }
    return node
  })
}

let isInitNodes = true
onNodesChange((changes: NodeChange[]) => {
  if (isInitNodes) {
    isInitNodes = false
    return
  }

  // 拦截删除 start 和 end 节点的操作
  const removeChanges = changes.filter((change) => change.type === 'remove')
  if (removeChanges.length > 0 && !isMonitorMode.value) {
    const protectedNodeIds = new Set(['start', 'end'])
    const attemptedDeletes = removeChanges
      .map((change) => (change as any).id)
      .filter((id: string) => protectedNodeIds.has(id))

    if (attemptedDeletes.length > 0) {
      ElMessage.warning('Start and End nodes cannot be deleted')
      // 阻止删除操作：从 changes 中移除这些删除操作
      const filteredChanges = changes.filter((change) => {
        if (change.type === 'remove') {
          const nodeId = (change as any).id
          return !protectedNodeIds.has(nodeId)
        }
        return true
      })
      // 应用过滤后的 changes
      if (filteredChanges.length !== changes.length) {
        // 重新设置节点以确保 start 和 end 节点存在
        const currentNodes = toObject().nodes as FlowNode[]
        const updatedNodes = ensureStartEndNodesUndeletable(currentNodes)
        setNodes(updatedNodes)
        return
      }
    }
  }

  // 只在实际修改节点数据、位置、添加或删除时触发，过滤掉视图变化（如选中、尺寸变化等）
  const meaningfulChanges = changes.filter((change) => {
    // 过滤掉 select（选中）和 dimensions（尺寸变化，可能是视图变化导致的）
    if (change.type === 'select' || change.type === 'dimensions') {
      return false
    }
    // 保留 add（添加节点）、remove（删除节点）、position（位置变化，即拖拽）
    return change.type === 'add' || change.type === 'remove' || change.type === 'position'
  })
  if (meaningfulChanges.length > 0) {
    // 监视模式下：VueFlow 内部已维护状态，只需更新变量浮层位置
    if (isMonitorMode.value) {
      const positionChanges = meaningfulChanges.filter((c) => c.type === 'position')
      if (positionChanges.length > 0 && visibleVarsNodes.value.size > 0) {
        requestAnimationFrame(() => {
          updateAllBubblePositions()
        })
      }
    } else {
      ruleChainStore.hasUnsavedChanges = true
    }
  }
})

onConnect(() => {
  ruleChainStore.hasUnsavedChanges = true
})
onEdgeUpdate(() => {
  ruleChainStore.hasUnsavedChanges = true
})
onNodeDragStop(() => {
  if (isMonitorMode.value) {
    if (visibleVarsNodes.value.size > 0) {
      nextTick(() => {
        updateAllBubblePositions()
      })
    }
  } else {
    ruleChainStore.hasUnsavedChanges = true
  }
})
onEdgesChange((changes: any[]) => {
  // 过滤掉选择操作，选择操作不应该触发 hasUnsavedChanges
  const meaningfulChanges = (changes || []).filter((change) => {
    // 过滤掉 select（选中）操作
    return change.type !== 'select'
  })

  if (meaningfulChanges.length === 0) {
    // 如果没有有意义的变更，直接应用 changes（用于选择状态更新）但不设置 hasUnsavedChanges
    applyEdgeChanges && applyEdgeChanges(changes)
    return
  }

  const currentEdges = (toObject().edges as any[]) || []
  for (const change of meaningfulChanges) {
    if (change.type === 'remove') {
      const sourceId = change.source
      const allNodes = (toObject().nodes as any[]) || []
      const sourceNode = allNodes.find((n) => n.id === sourceId)
      const key =
        sourceNode?.data?.type === 'function-switch' ? change.sourceHandle || '' : 'default'
      if (sourceNode && sourceNode.data?.config?.wires) {
        const wires = sourceNode.data.config.wires as Record<string, string[]>
        const arr = wires[key]
        if (Array.isArray(arr)) {
          const idx = arr.indexOf(change.target)
          if (idx > -1) arr.splice(idx, 1)
        }
      }
      updateNode(sourceNode?.id, {
        data: {
          ...sourceNode?.data,
        },
      })
    }
  }
  ruleChainStore.hasUnsavedChanges = true
  applyEdgeChanges && applyEdgeChanges(changes)
})

const fitFlowToViewport = () => {
  const flowObj = toObject()
  if ((flowObj.nodes as FlowNode[] | undefined)?.length) {
    fitView({
      includeHiddenNodes: true,
      padding: 0.2,
      duration: 0,
    })
  }
}

let resizeTimer: number | null = null
const handleWindowResize = () => {
  if (resizeTimer) {
    window.clearTimeout(resizeTimer)
  }
  resizeTimer = window.setTimeout(() => {
    fitFlowToViewport()
  }, 160)
}

const toggleFullscreen = () => {
  ruleChainStore.toggleFullscreen()
  nextTick(() => fitFlowToViewport())
}

const handleExitEdit = async () => {
  if (!hasUnsavedChanges.value) {
    isMonitorMode.value = true
    fitFlowToViewport()
    return
  }
  try {
    await ElMessageBox.confirm('Save your changes before exiting?', 'Unsaved Changes', {
      confirmButtonText: 'Save & Exit',
      cancelButtonText: 'Discard',
      type: 'warning',
    })
    await handleSave()
    isMonitorMode.value = true
  } catch {
    handleCancel()
    isMonitorMode.value = true
  }
  nextTick(() => fitFlowToViewport())
}

const handleConnectGuard = (connection: Connection) => {
  if (isMonitorMode.value) return
  handleConnect(connection)
}
const handleDragOverGuard = (e: DragEvent) => {
  if (isMonitorMode.value) return
  onDragOver(e)
}
const handleDragLeaveGuard = () => {
  if (isMonitorMode.value) return
  onDragLeave()
}
const handleDropGuard = (e: DragEvent) => {
  if (isMonitorMode.value) return
  onDrop(e)
}

const handleNodeClick = (event: any) => {
  if (isMonitorMode.value) return
  const node = event.node || event
  if (node.id === 'start' || node.id === 'end') return
  if (node && node.data) {
    editingCard.value = { ...node.data }
    cardEditDialogVisible.value = true
  }
}

const handleCardEditConfirm = (newCard: any) => {
  if (!newCard?.id) return
  updateNode(newCard.id, {
    data: {
      ...newCard,
    },
  })
  pruneInvalidIncomingEdges(newCard.id)
  ruleChainStore.hasUnsavedChanges = true
  cardEditDialogVisible.value = false
}

function pruneInvalidIncomingEdges(targetNodeId: string) {
  const obj = toObject()
  const allEdges = (obj.edges as any[]) || []
  const allNodes = (obj.nodes as any[]) || []
  const incoming = allEdges.filter((e) => e.source === targetNodeId)
  const removeChanges: any[] = []
  for (const edge of incoming) {
    const sourceNode = allNodes.find((n) => n.id === edge.source)
    if (!sourceNode) continue
    if (sourceNode.data?.type === 'function-switch') {
      const wires = sourceNode.data?.config?.wires
      const keys = wires && typeof wires === 'object' ? Object.keys(wires) : []
      const allowed = new Set<string>([...keys, ...keys.map((k) => `right${k}`)])
      if (!allowed.has(edge.sourceHandle)) {
        removeChanges.push(edge.id)
      }
    }
  }

  if (removeChanges.length) {
    removeEdges(removeChanges)
    ruleChainStore.hasUnsavedChanges = true
  }
}

const handleConnect = (connection: Connection) => {
  const newEdge: FlowEdge = {
    id: `edge-${Date.now()}`,
    source: connection.source!,
    target: connection.target!,
    sourceHandle: connection.sourceHandle || 'right',
    targetHandle: connection.targetHandle || 'left',
    style: {
      stroke: '#6F3381',
      strokeWidth: 3,
    },
  }

  const allNodes = (toObject().nodes as any[]) || []
  const sourceNode = allNodes.find((n) => n.id === newEdge.source)
  const key = sourceNode?.data?.type === 'function-switch' ? newEdge.sourceHandle || '' : 'default'
  if (sourceNode) {
    if (!sourceNode.data.config) sourceNode.data.config = {}
    if (!sourceNode.data.config.wires || typeof sourceNode.data.config.wires !== 'object') {
      sourceNode.data.config.wires = {}
    }
    const wires = sourceNode.data.config.wires as Record<string, string[]>
    const wireKey = key || 'default'
    if (!Array.isArray(wires[wireKey])) wires[wireKey] = []
    if (!wires[wireKey].includes(newEdge.target)) {
      wires[wireKey].push(newEdge.target)
    }
    // 使用传回的新数据更新节点
    updateNode(sourceNode.id, {
      data: {
        ...sourceNode.data,
      },
    })
  }
  ruleChainStore.hasUnsavedChanges = true
  addEdges(newEdge)
}

const handleSave = async () => {
  const flowObj = toObject()
  const newNodes = flowObj.nodes as unknown as AppNode[]
  const newEdges = flowObj.edges as unknown as AppEdge[]

  // 使用当前 VueFlow 中的数据构建 payload，而不是 store 中的旧数据
  const payload = ruleChainStore.exportRuleChain(
    flowObj.nodes as FlowNode[],
    flowObj.edges as FlowEdge[],
  ) as RuleChainPayload
  try {
    await updateRule(payload)
    ruleChainStore.saveChanges(newNodes, newEdges)
    ElMessage.success('Submitted successfully')
    nextTick(() => {
      fitFlowToViewport()
    })
  } catch (error) {
    ruleChainStore.hasUnsavedChanges = true
    ElMessage.error('Submit failed')
  }
}

const handleCancel = () => {
  ruleChainStore.discardChanges()
  setNodes(nodes.value as unknown as FlowNode[])
  setEdges(edges.value as unknown as FlowEdge[])
  ElMessage.success('Changes discarded')
}

const handleExport = () => {
  // 使用当前 VueFlow 中的数据导出，确保导出的是当前显示的数据
  const flowObj = toObject()
  const ruleChainData = ruleChainStore.exportRuleChain(
    flowObj.nodes as FlowNode[],
    flowObj.edges as FlowEdge[],
  )
  const dataStr = JSON.stringify(ruleChainData, null, 2)
  const dataBlob = new Blob([dataStr], { type: 'application/json' })
  const url = URL.createObjectURL(dataBlob)
  const link = document.createElement('a')
  link.href = url
  link.download = `${ruleChainData.name}.json`
  link.click()
  URL.revokeObjectURL(url)
  ElMessage.success('Rule chain exported successfully')
}

const goBackToList = () => {
  router.push({ name: 'ruleConfiguration' })
}

function applyRuntimeUpdate(data: any) {
  if (!data || typeof data !== 'object') return

  if (data.rule_id && Array.isArray(data.execution_path)) {
    const executionPath = data.execution_path
    const activeNodeIds = executionPath
      .map((item: any) => String(item.id))
      .filter((id: string) => id && id !== 'undefined' && id !== 'null')

    const varsNodeIds = new Set<string>()
    const variablesMap = new Map<string, Record<string, number>>()
    executionPath.forEach((item: any) => {
      if (
        item.id &&
        item.id !== 'start' &&
        item.id !== 'end' &&
        item.variables &&
        typeof item.variables === 'object'
      ) {
        varsNodeIds.add(item.id)
        variablesMap.set(item.id, item.variables)
      }
    })

    visibleVarsNodes.value = varsNodeIds
    nodeVariablesData.value = variablesMap

    const currentActiveSet = new Set<string>(activeNodeIds)
    const lastActiveSet = lastActiveNodeIds.value
    const hasChanged =
      currentActiveSet.size !== lastActiveSet.size ||
      !Array.from(currentActiveSet).every((id: string) => lastActiveSet.has(id))

    if (hasChanged) {
      lastActiveNodeIds.value = currentActiveSet
      applyActiveRuntime(activeNodeIds)
    }

    if (visibleVarsNodes.value.size > 0) {
      nextTick(() => {
        updateAllBubblePositions()
      })
    }
  }
}

function startMonitorSubscription() {
  if (!isMonitorMode.value) return
  try {
    subscriptionId.value = wsManager.subscribe(
      {
        source: 'rule',
        channels: [Number(currentChainId.value)],
        interval: 1000,
      },
      {
        onBatchDataUpdate: (payload: any) => {
          try {
            applyRuntimeUpdate(payload)
          } catch (error) {
            console.error('[RuleChainEditor] 处理规则执行数据失败:', error)
          }
        },
      } as any,
    )
  } catch (error) {
    console.error('[RuleChainEditor] 订阅规则失败:', error)
  }
}
function stopMonitorSubscription() {
  try {
    wsManager.unsubscribe(subscriptionId.value)
  } catch {}
}

function enterEditMode() {
  isMonitorMode.value = false
  stopMonitorSubscription()
  clearSimulation()
  resetRuntimeVisuals()
  visibleVarsNodes.value.clear()
}

function getNodeVarsData(nodeId: string) {
  const node = findNode(nodeId)
  if (!node || node.type === 'start' || node.type === 'end') return null

  const realtimeVars = nodeVariablesData.value.get(nodeId)
  if (realtimeVars) {
    const varDefinitions = Array.isArray(node.data?.config?.variables)
      ? node.data.config.variables
      : []

    return varDefinitions.map((varDef: any) => {
      const varName = varDef.name || varDef.point || ''
      const value = realtimeVars[varName] !== undefined ? realtimeVars[varName] : undefined
      return {
        ...varDef,
        value: value,
      }
    })
  }

  return Array.isArray(node.data?.config?.variables) ? node.data.config.variables : null
}

function updateBubblePosition(nodeId: string) {
  const node = findNode(nodeId)
  if (!node || !centerPanelRef.value) {
    visibleVarsNodes.value.delete(nodeId)
    nodeVarsPositions.value.delete(nodeId)
    return
  }

  try {
    let nodeElement = document.querySelector(`[data-id="${nodeId}"]`) as HTMLElement
    if (!nodeElement) {
      nodeElement = document.querySelector(`.vue-flow__node[data-id="${nodeId}"]`) as HTMLElement
    }

    if (!nodeElement) {
      visibleVarsNodes.value.delete(nodeId)
      nodeVarsPositions.value.delete(nodeId)
      return
    }

    const nodeRect = nodeElement.getBoundingClientRect()
    if (nodeRect.width === 0 || nodeRect.height === 0) {
      visibleVarsNodes.value.delete(nodeId)
      nodeVarsPositions.value.delete(nodeId)
      return
    }

    const containerRect = centerPanelRef.value.getBoundingClientRect()
    const isFullscreen = ruleChainStore.isFullscreen
    const headerHeight = isFullscreen ? 0.6 * getCurrentFontSize() : 0

    const maxLeft = isFullscreen ? window.innerWidth : containerRect.right
    const maxTop = isFullscreen ? window.innerHeight - headerHeight : containerRect.bottom
    const minLeft = isFullscreen ? 0 : containerRect.left
    const minTop = isFullscreen ? containerRect.top : containerRect.top

    if (
      nodeRect.right < minLeft ||
      nodeRect.left > maxLeft ||
      nodeRect.bottom < minTop ||
      nodeRect.top > maxTop
    ) {
      visibleVarsNodes.value.delete(nodeId)
      nodeVarsPositions.value.delete(nodeId)
      return
    }

    const left = nodeRect.left
    const top = nodeRect.bottom
    nodeVarsPositions.value.set(nodeId, { top, left })
  } catch (error) {
    console.error('Failed to update bubble position:', error)
    visibleVarsNodes.value.delete(nodeId)
    nodeVarsPositions.value.delete(nodeId)
  }
}

function getBubbleStyle(nodeId: string): Record<string, string> {
  const position = nodeVarsPositions.value.get(nodeId)
  if (!position || !centerPanelRef.value) return { display: 'none' }

  const node = findNode(nodeId)
  if (!node) {
    visibleVarsNodes.value.delete(nodeId)
    nodeVarsPositions.value.delete(nodeId)
    return { display: 'none' }
  }

  const currentZoom = viewport.value?.zoom || 1
  const scale = currentZoom
  const bubbleWidth = 2.5 * getCurrentFontSize() * scale
  const bubbleHeight = 3 * getCurrentFontSize() * scale

  let left = position.left
  let top = position.top

  const containerRect = centerPanelRef.value.getBoundingClientRect()
  const isFullscreen = ruleChainStore.isFullscreen
  const headerHeight = isFullscreen ? 0.6 * getCurrentFontSize() : 0
  console.log(containerRect, '////')

  const maxLeft = isFullscreen ? window.innerWidth : containerRect.right
  const maxTop = isFullscreen ? window.innerHeight - headerHeight : containerRect.bottom
  const minLeft = isFullscreen ? 0 : containerRect.left
  const minTop = isFullscreen ? containerRect.top : containerRect.top
  console.log(maxLeft, 'maxLeft', maxTop, 'maxTop', minLeft, 'minLeft', minTop, 'minTop')
  console.log(bubbleWidth, 'bubbleWidth', bubbleHeight, 'bubbleHeight', scale, 'scale')

  if (left + bubbleWidth > maxLeft) {
    left = Math.max(minLeft, left)
  }
  if (left + bubbleWidth < minLeft) {
    left = minLeft + bubbleWidth
  }

  if (top > maxTop + bubbleHeight) {
    top = maxTop - bubbleHeight
  }
  if (top < minTop) {
    top = minTop
  }

  if (left < minLeft || left + bubbleWidth > maxLeft || top < minTop || top > maxTop) {
    visibleVarsNodes.value.delete(nodeId)
    nodeVarsPositions.value.delete(nodeId)
    return { display: 'none' }
  }

  return {
    position: 'fixed',
    top: `${top}px`,
    left: `${left}px`,
    zIndex: '99999',
    transform: `scale(${scale})`,
    transformOrigin: 'top left',
  }
}

function updateAllBubblePositions() {
  visibleVarsNodes.value.forEach((nodeId) => {
    updateBubblePosition(nodeId)
  })
}

let simTimer: any = null

function clearSimulation() {
  if (simTimer) {
    clearInterval(simTimer)
    simTimer = null
  }
}

function resetRuntimeVisuals() {
  setEdges((prev: any[]) =>
    prev.map((e: any) => ({
      ...e,
      class: '',
      data: { ...(e.data || {}), isAnimating: false },
    })),
  )
  setNodes((prev: any[]) =>
    prev.map((n: any) => ({
      ...n,
      class: '',
      data: { ...(n.data || {}), status: '' },
    })),
  )
}

function applyActiveRuntime(activeNodeIds: string[]) {
  const activeNodes = new Set(activeNodeIds)
  const currentEdges = edges.value
  const activeEdges = currentEdges
    .filter((e) => activeNodes.has(String(e.source)) && activeNodes.has(String(e.target)))
    .map((e) => String(e.id))
  const activeEdgeSet = new Set(activeEdges)

  setNodes((prev: any[]) =>
    prev.map((n: any) => ({
      ...n,
      class: activeNodes.has(String(n.id)) ? 'active-node' : '',
    })),
  )
  setEdges((prev: any[]) =>
    prev.map((e: any) => ({
      ...e,
      class: activeEdgeSet.has(String(e.id)) ? 'active-edge' : '',
      data: { ...(e.data || {}), isAnimating: activeEdgeSet.has(String(e.id)) },
    })),
  )

  if (isMonitorMode.value) {
    const updatedNodes = toObject().nodes as any[]
    const updatedEdges = toObject().edges as any[]
    ruleChainStore.updateMonitorNodes(updatedNodes as unknown as FlowNode[])
    ruleChainStore.updateMonitorEdges(updatedEdges as unknown as FlowEdge[])
  }
}

function startSimulation() {
  if (!isMonitorMode.value) return
  clearSimulation()
  resetRuntimeVisuals()
}

const handleImportClick = () => {
  importFileInput.value?.click()
}

const handleImportChange = (event: Event) => {
  const target = event.target as HTMLInputElement
  const file = target.files?.[0]
  if (!file) return

  const reader = new FileReader()
  reader.onload = () => {
    try {
      const text = reader.result as string
      const parsed = JSON.parse(text || '{}')
      const flow = parsed?.flow_json || parsed
      const nextNodes = Array.isArray(flow?.nodes) ? flow.nodes : []
      const nextEdges = Array.isArray(flow?.edges) ? flow.edges : []
      if (!nextNodes.length && !nextEdges.length) {
        ElMessage.error('Import failed: no nodes or edges found')
        return
      }
      // 确保 start 和 end 节点不可删除
      const protectedNodes = ensureStartEndNodesUndeletable(nextNodes as FlowNode[])
      setNodes(protectedNodes)
      setEdges(nextEdges as unknown as FlowEdge[])
      ruleChainStore.hasUnsavedChanges = true
      ElMessage.success('Imported successfully')
    } catch (error) {
      ElMessage.error('Import failed: invalid JSON structure')
    } finally {
      target.value = ''
    }
  }
  reader.readAsText(file)
}
const RuleDetail = async (chainId: string) => {
  const res = await getRuleDetail(chainId)
  if (res.success && res?.data) {
    const payload = res.data
    const flow = payload.flow_json
    const nextNodes = Array.isArray(flow?.nodes) ? flow.nodes : []
    const nextEdges = Array.isArray(flow?.edges) ? flow.edges : []
    if (nextNodes.length || nextEdges.length) {
      // 确保 start 和 end 节点不可删除
      const protectedNodes = ensureStartEndNodesUndeletable(nextNodes as FlowNode[])
      ruleChainStore.saveChanges(
        protectedNodes as unknown as AppNode[],
        nextEdges as unknown as AppEdge[],
      )
      ruleChainStore.setCurrentRuleChain({
        id: chainId,
        name: payload.name || '',
        description: payload.description || '',
        priority: payload.priority || 10,
        enabled: payload.enabled || true,
        cooldown_ms: payload.cooldown_ms || 5000,
      } as unknown as RuleChain)
      setNodes(protectedNodes)
      setEdges(nextEdges as unknown as FlowEdge[])
    } else {
      ruleChainStore.initDefaultGraph()
      setNodes(ruleChainStore.nodes as unknown as FlowNode[])
      setEdges([] as unknown as FlowEdge[])
      ruleChainStore.setCurrentRuleChain({
        id: chainId,
        name: payload.name || '',
        description: payload.description || '',
        priority: payload.priority || 10,
        enabled: payload.enabled || true,
        cooldown_ms: payload.cooldown_ms || 5000,
      })
    }
  }
}

onMounted(async () => {
  ruleChainStore.isFullscreen = false
  const chainId = route.params.id as string
  if (chainId) {
    try {
      currentChainId.value = chainId
      await RuleDetail(chainId)
    } catch (error) {
      console.error(error)
    }
  }
  window.addEventListener('resize', handleWindowResize)
  fitFlowToViewport()
  if (isMonitorMode.value) {
    startMonitorSubscription()
  }
  startSimulation()
})
onUnmounted(() => {
  window.removeEventListener('resize', handleWindowResize)
  ruleChainStore.hasUnsavedChanges = false
  stopMonitorSubscription()
  clearSimulation()
})

watch(
  () => isMonitorMode.value,
  (val) => {
    if (val) {
      ruleChainStore.createMonitorSnapshot()
      nextTick(() => {
        const monitorNodes = ensureStartEndNodesUndeletable(
          ruleChainStore.monitorNodes as unknown as FlowNode[],
        )
        setNodes(monitorNodes)
        setEdges(ruleChainStore.monitorEdges as unknown as FlowEdge[])
        resetRuntimeVisuals()
        startSimulation()
        startMonitorSubscription()
        requestAnimationFrame(() => {
          requestAnimationFrame(() => {
            fitFlowToViewport()
          })
        })
      })
    } else {
      clearSimulation()
      const currentNodes = toObject().nodes as any[]
      const currentEdges = toObject().edges as any[]
      ruleChainStore.updateMonitorNodes(currentNodes as unknown as FlowNode[])
      ruleChainStore.updateMonitorEdges(currentEdges as unknown as FlowEdge[])
      nextTick(() => {
        const editNodes = ensureStartEndNodesUndeletable(
          ruleChainStore.nodes as unknown as FlowNode[],
        )
        setNodes(editNodes)
        setEdges(ruleChainStore.edges as unknown as FlowEdge[])
        resetRuntimeVisuals()
        visibleVarsNodes.value.clear()
        nodeVarsPositions.value.clear()
        nodeVariablesData.value.clear()
        lastActiveNodeIds.value.clear()
        requestAnimationFrame(() => {
          requestAnimationFrame(() => {
            fitFlowToViewport()
          })
        })
      })
    }
  },
)

watch(
  () => [nodes.value, viewport.value],
  () => {
    if (isMonitorMode.value && visibleVarsNodes.value.size > 0) {
      requestAnimationFrame(() => {
        updateAllBubblePositions()
      })
    }
  },
  { deep: true },
)

watch(
  () => viewport.value,
  (newViewport, oldViewport) => {
    if (!isMonitorMode.value || visibleVarsNodes.value.size === 0) return

    if (
      newViewport &&
      oldViewport &&
      (newViewport.zoom !== oldViewport.zoom ||
        newViewport.x !== oldViewport.x ||
        newViewport.y !== oldViewport.y)
    ) {
      requestAnimationFrame(() => {
        updateAllBubblePositions()
      })
    }
  },
  { deep: true, immediate: false },
)
</script>

<style lang="scss" scoped>
.voltage-class {
  .rule-chain-editor {
    // background-color: ;
    box-sizing: border-box;
    height: 100%;
    width: 100%;
    display: flex;
    flex-direction: column;
    position: relative;

    &.is-fullscreen {
      position: fixed;
      top: 0;
      left: 0;
      z-index: 9999;
      height: 100vh;
      width: 100vw;
      background-image: url('@/assets/images/simple-bg.png');
      background-size: cover;
      background-position: center;
      background-repeat: no-repeat;
    }

    .rule-chain-editor__header {
      height: 0.6rem;
      width: 100%;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 0.12rem;
      padding: 0.12rem;
      background-color: #132c54;
      border-bottom: 0.01rem solid #435678;
      .rule-chain-editor__header-left,
      .rule-chain-editor__header-right {
        display: flex;
        align-items: center;
        gap: 0.12rem;
      }
      .rule-chain-editor__header-left {
        .rule-chain-editor__header-left-back {
          display: flex;
          align-items: center;
          gap: 0.04rem;
          cursor: pointer;
          font-size: 0.14rem;
          font-weight: 600;
          img {
            width: 0.2rem;
            height: 0.2rem;
          }
        }
        .rule-chain-editor__header-left-fenge {
          height: 0.29rem;
          width: 0.02rem;
          background-color: #435678;
          margin: 0 0.04rem;
        }
        .rule-chain-editor__header-right-name {
          font-size: 0.2rem;
          font-weight: 700;
          color: #ffffff;
        }
      }
    }

    .rule-chain-editor__content {
      display: flex;
      height: calc(100% - 0.6rem);

      .rule-chain-editor__left-panel {
        width: 3rem;
        background-color: rgba(19, 44, 84, 0.2);
        border-right: 0.01rem solid #435678;
        display: flex;
        flex-direction: column;
        transition: width 0.3s ease;
        box-shadow: var(--shadow-light);

        .rule-chain-editor__chain-selector {
          padding: 0.16rem;
          position: relative;
        }

        .rule-chain-editor__card-categories {
          flex: 1;
          overflow-y: auto;
          padding: 0.16rem;

          :deep(.el-collapse-item__content) {
            padding-top: 0.1rem;
          }

          .rule-chain-editor__category-title {
            display: flex;
            align-items: center;
            gap: 0.08rem;

            .rule-chain-editor__tooltip-icon {
              margin-left: auto;
              cursor: help;
              color: #909399;
            }
          }
        }

        .rule-chain-editor__cards {
          display: flex;
          flex-direction: column;
          gap: 0.08rem;

          .rule-chain-editor__card {
            display: flex;
            align-items: center;
            padding: 0.12rem;
            border-radius: 0.08rem;
            cursor: grab;
            transition: all 0.2s ease;
            min-width: 2rem;
            box-shadow: 0 0.02rem 0.04rem rgba(0, 0, 0, 0.1);

            &:hover {
              transform: translateZ(0);
            }

            &:active {
              cursor: grabbing;
            }

            &[data-type='function-switch'] {
              background-color: #81c784; // soft green
              box-shadow: 0 0.04rem 0.12rem rgba(129, 199, 132, 0.35);
            }
            &[data-type='action-changeValue'] {
              background-color: #4fc3f7; // soft sky blue
              box-shadow: 0 0.04rem 0.12rem rgba(79, 195, 247, 0.35);
            }
            &[data-type='function-switch'] .rule-chain-editor__card-name,
            &[data-type='function-switch'] .rule-chain-editor__card-description,
            &[data-type='action-changeValue'] .rule-chain-editor__card-name,
            &[data-type='action-changeValue'] .rule-chain-editor__card-description {
              color: #ffffff;
            }

            .rule-chain-editor__card-icon {
              width: 0.36rem;
              height: 0.36rem;
              display: flex;
              align-items: center;
              justify-content: center;
              background: linear-gradient(135deg, #e8f4fd 0%, #d1e7f5 100%);
              border-radius: 0.06rem;
              margin-right: 0.12rem;
              font-size: 0.2rem;
              .el-icon {
                font-size: 0.2rem;
              }
              img {
                width: 0.2rem;
                height: 0.2rem;
                object-fit: contain;
              }
              &.icon--function-switch {
                background: #66bb6a;
                .el-icon {
                  color: #ffffff;
                }
              }
              &.icon--action-changeValue {
                background: #29b6f6;
                .el-icon {
                  color: #ffffff;
                }
              }
            }

            .rule-chain-editor__card-content {
              flex: 1;
            }

            .rule-chain-editor__card-name {
              font-weight: 600;
              color: #2c3e50;
              margin-bottom: 0.04rem;
              font-size: 0.14rem;
            }

            .rule-chain-editor__card-description {
              font-size: 0.12rem;
              color: #909399;
              line-height: 1.4;
            }
          }
        }
      }

      .rule-chain-editor__center-panel {
        flex: 1;
        height: 100%;
        position: relative;
        z-index: 1;

        .rule-chain-editor__flow {
          width: 100%;
          height: 100%;
        }

        @keyframes active-edge-dash {
          from {
            stroke-dashoffset: 0;
          }
          to {
            stroke-dashoffset: -80;
          }
        }
        :deep(.active-edge .vue-flow__edge-path) {
          stroke: #ffd166 !important;
          stroke-width: 3 !important;
          stroke-linecap: round;
          vector-effect: non-scaling-stroke;
          shape-rendering: geometricPrecision;
          stroke-dasharray: 8 12 !important;
          animation: active-edge-dash 2s linear infinite;
          will-change: stroke-dashoffset;
          filter: drop-shadow(0 0 0.03rem rgba(255, 209, 102, 0.6));
          backface-visibility: hidden;
          transform: translateZ(0);
        }
        :deep(.vue-flow__edge) {
          backface-visibility: hidden;
          transform: translateZ(0);
        }

        :deep(.vue-flow__edge.selected .vue-flow__edge-path) {
          stroke-width: 4.5 !important;
          stroke: #ff5722 !important;
          filter: drop-shadow(0 0 0.08rem rgba(255, 87, 34, 0.9));
        }

        :deep(.vue-flow__node.selected) {
          box-shadow:
            0 0 0.16rem rgba(255, 87, 34, 0.7),
            0 0 0.24rem rgba(255, 87, 34, 0.5),
            0.02rem 0.02rem 0.08rem rgba(0, 0, 0, 0.15) !important;
        }

        @keyframes active-node-pulse {
          0% {
            filter: drop-shadow(0 0 0.02rem rgba(255, 209, 102, 0.25));
          }
          50% {
            filter: drop-shadow(0 0 0.1rem rgba(255, 209, 102, 0.95));
          }
          100% {
            filter: drop-shadow(0 0 0.02rem rgba(255, 209, 102, 0.25));
          }
        }
        :deep(.active-node) {
          animation: active-node-pulse 1.2s ease-in-out infinite;
        }

        .rf-node-with-vars {
          position: relative;
          display: flex;
          flex-direction: column;
        }
        .node-vars-bubble__row {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 0.08rem;
          + .node-vars-bubble__row {
            margin-top: 0.06rem;
          }
        }
        .node-vars-bubble__left {
          display: flex;
          align-items: center;
          gap: 0.06rem;
          flex: 1;
          min-width: 0;
          .var-item {
            font-size: 0.12rem;
            color: #cfe2ff;
            opacity: 0.9;
            word-break: break-word;
            word-wrap: break-word;
            overflow-wrap: break-word;
            white-space: normal;
          }
        }
        .node-vars-bubble__right {
          font-size: 0.12rem;
          color: #fff;
          opacity: 0.95;
        }
      }
    }

    .rule-chain-editor__floating-actions {
      position: fixed;
      right: 1rem;
      bottom: 0.24rem;
      display: flex;
      gap: 0.16rem;
      z-index: 10;
      .floating-btn {
        width: 0.8rem !important;
        height: 0.8rem !important;
        font-size: 0.28rem !important;
        border-radius: 50% !important;
        display: flex;
        align-items: center;
        justify-content: center;
        box-shadow: 0 0.04rem 0.16rem rgba(0, 0, 0, 0.35);
      }
    }
  }
  :deep(.custom-button .el-icon) {
    margin-right: 0.08rem;
  }
}
</style>
