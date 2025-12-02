import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import type { RuleChain } from '@/types/ruleConfiguration'
import type { Node, Edge } from '@vue-flow/core'

export const useRuleChainStore = defineStore('ruleChain', () => {
  const ruleChains = ref<RuleChain[]>([])
  const nodes = ref<Node[]>([
    // {
    //   id: 'start',
    //   type: 'start',
    //   position: { x: 100, y: 100 },
    //   data: {
    //     id: 'start',
    //     type: 'start',
    //     label: 'START',
    //     description: 'START',
    //     config: { wires: { default: [] } },
    //   },
    //   deletable: false,
    // },
    // {
    //   id: 'end',
    //   type: 'end',
    //   position: { x: 500, y: 100 },
    //   data: {
    //     id: 'end',
    //     type: 'end',
    //     label: 'END',
    //     description: 'END',
    //     config: { wires: { default: [] } },
    //   },
    //   deletable: false,
    // },
  ])
  const edges = ref<Edge[]>([])
  const currentRuleChain = ref<RuleChain | null>(null)
  const isFullscreen = ref(false)
  const isLeftPanelCollapsed = ref(false)
  const hasUnsavedChanges = ref(false)

  const addRuleChain = (ruleChainData: Omit<RuleChain, 'id' | 'createdAt' | 'updatedAt'>) => {
    const newRuleChain: RuleChain = {
      id: `chain-${Date.now()}`,
      ...ruleChainData,
    }
    ruleChains.value.push(newRuleChain)
    return newRuleChain
  }

  const updateRuleChain = (id: string, updates: Partial<Omit<RuleChain, 'id' | 'createdAt'>>) => {
    const index = ruleChains.value.findIndex((chain) => chain.id === id)
    if (index !== -1) {
      ruleChains.value[index] = {
        ...ruleChains.value[index],
        ...updates,
      }
      hasUnsavedChanges.value = true
    }
  }

  watch(hasUnsavedChanges, (newVal) => {
    console.log('hasUnsavedChanges', newVal)
  })

  const deleteRuleChain = (id: string) => {
    const index = ruleChains.value.findIndex((chain) => chain.id === id)
    if (index !== -1) {
      ruleChains.value.splice(index, 1)
      if (currentRuleChain.value?.id === id) {
        currentRuleChain.value = null
      }
    }
  }

  const getRuleChain = (id: string) => ruleChains.value.find((chain) => chain.id === id)

  const setCurrentRuleChain = (ruleChain: RuleChain) => {
    currentRuleChain.value = ruleChain
  }

  const addNodes = (node: Node[]) => {
    nodes.value.push(...node)
  }

  const addEdges = (edge: Edge[]) => {
    edges.value.push(...edge)
  }

  const toggleFullscreen = () => {
    isFullscreen.value = !isFullscreen.value
  }

  const toggleLeftPanel = () => {
    isLeftPanelCollapsed.value = !isLeftPanelCollapsed.value
  }

  const saveChanges = (newNodes: Node[], newEdges: Edge[]) => {
    hasUnsavedChanges.value = false
    nodes.value = newNodes
    edges.value = newEdges
  }

  const discardChanges = () => {
    hasUnsavedChanges.value = false
  }

  // 当规则详情为空时的初始化：仅包含 START 与 END 节点，edges 为空
  const initDefaultGraph = () => {
    nodes.value = [
      {
        id: 'start',
        type: 'start',
        position: { x: 100, y: 100 },
        data: {
          id: 'start',
          type: 'start',
          label: 'START',
          description: 'START',
          config: { wires: { default: [] } },
        },
        deletable: false,
      },
      {
        id: 'end',
        type: 'end',
        position: { x: 500, y: 100 },
        data: {
          id: 'end',
          type: 'end',
          label: 'END',
          description: 'END',
          config: { wires: { default: [] } },
        },
        deletable: false,
      },
    ]
    edges.value = []
    hasUnsavedChanges.value = false
  }

  const exportRuleChain = () => {
    const buildNodeData = (node: Node) => {
      if (!node.data || typeof node.data !== 'object') return null
      const data: Record<string, any> = {}
      if (node.data.config) data.config = node.data.config
      if (node.type !== 'start' && node.type !== 'end') {
        if (node.data.label) data.label = node.data.label
        if (node.data.type) data.type = node.data.type
      }
      if (node.data.id) data.id = node.data.id
      if (node.data.description) data.description = node.data.description
      // 始终保留 description（包括 start/end 节点）
      if ((node.data as any).description) data.description = (node.data as any).description
      return Object.keys(data).length ? data : null
    }

    const flow_json = {
      edges: edges.value.map((edge) => ({
        id: edge.id,
        source: edge.source,
        target: edge.target,
        ...(edge.sourceHandle ? { sourceHandle: edge.sourceHandle } : {}),
        ...(edge.targetHandle ? { targetHandle: edge.targetHandle } : {}),
      })),
      nodes: nodes.value.map((node) => {
        const data = buildNodeData(node)
        return {
          id: node.id,
          type: node.type,
          position: node.position,
          ...(data ? { data } : {}),
        }
      }),
    }
    return {
      cooldown_ms: currentRuleChain.value?.cooldown_ms || 5000,
      description: currentRuleChain.value?.description || '',
      enabled: currentRuleChain.value?.enabled || true,
      flow_json,
      format: 'vue-flow',
      id: currentRuleChain.value?.id || `chain-${Date.now()}`,
      name: currentRuleChain.value?.name || 'Untitled Rule Chain',
      priority: currentRuleChain.value?.priority || 100,
    }
  }

  const clearAll = () => {
    ruleChains.value = []
    nodes.value = []
    edges.value = []
    currentRuleChain.value = null
    hasUnsavedChanges.value = false
  }

  return {
    ruleChains,
    nodes,
    edges,
    currentRuleChain,
    isFullscreen,
    isLeftPanelCollapsed,
    hasUnsavedChanges,
    addRuleChain,
    updateRuleChain,
    deleteRuleChain,
    getRuleChain,
    setCurrentRuleChain,
    addNodes,
    addEdges,
    toggleFullscreen,
    toggleLeftPanel,
    saveChanges,
    discardChanges,
    initDefaultGraph,
    exportRuleChain,
    clearAll,
  }
})
