import { useVueFlow } from '@vue-flow/core'
import { ref, watch } from 'vue'
import type { RuleCard } from '@/types/index'

let id = 0

/**
 * 生成唯一节点ID
 * @returns {string} - 返回唯一的节点ID字符串
 */
function getId() {
  return `dndnode_${id++}`
}

/**
 * 拖拽相关的全局响应式状态
 * 注意：实际项目中不建议在全局作用域创建ref，可能导致内存泄漏
 * @type {{ isDragOver: Ref<boolean>, isDragging: Ref<boolean>}}
 */
const state = {
  /**
   * 当前正在被拖拽的卡片
   */
  draggedCard: ref<RuleCard | null>(null),
  /**
   * 鼠标是否悬停在可放置区域
   */
  isDragOver: ref(false),
  /**
   * 是否正在拖拽中
   */
  isDragging: ref(false),
}

/**
 * 拖拽与放置的组合式函数
 * 提供拖拽相关的响应式状态和方法
 */
export default function useDragAndDrop() {
  // 解构全局状态
  const { draggedCard, isDragOver, isDragging } = state
  // 获取VueFlow提供的核心方法
  const { addNodes, screenToFlowCoordinate, onNodesInitialized, updateNode } = useVueFlow()

  /**
   * 监听拖拽状态，拖拽时禁止页面文本选择，提升用户体验
   */
  watch(isDragging, (dragging) => {
    document.body.style.userSelect = dragging ? 'none' : ''
  })

  /**
   * 拖拽开始事件
   * @param {DragEvent} event - 拖拽事件对象
   * @param {RuleCard} card - 拖拽的卡片
   */
  function onDragStart(event: DragEvent, card: RuleCard) {
    if (event.dataTransfer) {
      // 设置拖拽数据类型和内容
      event.dataTransfer.setData('application/vueflow', JSON.stringify(card))
      event.dataTransfer.effectAllowed = 'move'
    }
    draggedCard.value = card
    // 标记为正在拖拽
    isDragging.value = true

    // 监听全局drop事件，便于拖拽结束时清理状态
    document.addEventListener('drop', onDragEnd)
  }

  /**
   * 拖拽经过可放置区域事件
   * @param {DragEvent} event - 拖拽事件对象
   */
  function onDragOver(event: DragEvent) {
    event.preventDefault() // 阻止默认行为，允许放置

    if (draggedCard.value) {
      // 标记鼠标悬停在可放置区域
      isDragOver.value = true

      if (event.dataTransfer) {
        // 设置拖拽效果为move
        event.dataTransfer.dropEffect = 'move'
      }
    }
  }

  /**
   * 拖拽离开可放置区域事件
   */
  function onDragLeave() {
    isDragOver.value = false
  }

  /**
   * 拖拽结束事件（无论是否成功放下）
   */
  function onDragEnd() {
    isDragging.value = false
    isDragOver.value = false
    draggedCard.value = null
    // 移除全局drop事件监听，防止内存泄漏
    document.removeEventListener('drop', onDragEnd)
  }

  /**
   * 放下节点到画布事件
   * @param {DragEvent} event - 拖拽事件对象
   */
  function onDrop(event: DragEvent) {
    // 将屏幕坐标转换为VueFlow画布坐标
    const position = screenToFlowCoordinate({
      x: event.clientX,
      y: event.clientY,
    })

    // 构造新节点对象
    const time = Date.now()
    if (!draggedCard.value) return

    // 深拷贝来自 cardCategories 的源卡片数据，确保每次初始化都以源数据为准且互不影响
    const sourceCard = deepClone(draggedCard.value)

    const newNode = {
      id: `node-${time}`,
      type: 'custom',
      position,
      data: {
        id: `node-${time}`,
        cardId: sourceCard.id,
        label: sourceCard.name,
        description: sourceCard.description,
        type: sourceCard.type,
        // 使用深拷贝后的默认配置，来源于 RuleChainEditorView 中的 cardCategories
        config: sourceCard?.config,
      },
    }

    /**
     * 节点放下后，自动将节点居中对齐到鼠标位置
     * 监听节点初始化事件，获取节点尺寸后调整位置
     */
    const { off } = onNodesInitialized(() => {
      updateNode(newNode.id, (node) => ({
        position: {
          x: node.position.x - node.dimensions.width / 2,
          y: node.position.y - node.dimensions.height / 2,
        },
      }))
      // 移除监听，避免重复触发
      off()
    })

    // 添加新节点到画布
    addNodes(newNode)
  }

  // 返回所有拖拽相关的响应式状态和方法，供组件使用
  return {
    isDragOver,
    isDragging,
    onDragStart,
    onDragLeave,
    onDragOver,
    onDrop,
  }
}

// 简单深拷贝，保证初始化不污染 cardCategories 源数据
function deepClone<T>(obj: T): T {
  try {
    // @ts-ignore
    if (typeof structuredClone === 'function') return structuredClone(obj)
  } catch {}
  return JSON.parse(JSON.stringify(obj))
}
