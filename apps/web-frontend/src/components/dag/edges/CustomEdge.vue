<template>
  <g>
    <!-- 箭头标记定义 -->
    <defs>
      <marker
        id="arrowclosed"
        markerWidth="10"
        markerHeight="10"
        refX="9"
        refY="3"
        orient="auto"
        markerUnits="strokeWidth"
      >
        <path d="M0,0 L0,6 L9,3 z" :fill="arrowColor" />
      </marker>
      
      <marker
        id="arrow-success"
        markerWidth="10"
        markerHeight="10"
        refX="9"
        refY="3"
        orient="auto"
        markerUnits="strokeWidth"
      >
        <path d="M0,0 L0,6 L9,3 z" fill="#52c41a" />
      </marker>
      
      <marker
        id="arrow-error"
        markerWidth="10"
        markerHeight="10"
        refX="9"
        refY="3"
        orient="auto"
        markerUnits="strokeWidth"
      >
        <path d="M0,0 L0,6 L9,3 z" fill="#f5222d" />
      </marker>
    </defs>
    
    <!-- 主连接线 -->
    <path
      :id="id"
      :d="edgePath"
      :marker-end="markerEnd"
      :style="edgeStyle"
      class="vue-flow__edge-path"
    />
    
    <!-- 连接线标签 -->
    <foreignObject
      v-if="label"
      :width="labelX"
      :height="labelY" 
      :x="labelX - labelWidth / 2"
      :y="labelY - labelHeight / 2"
      class="vue-flow__edge-label"
      requiredExtensions="http://www.w3.org/1999/xhtml"
    >
      <div class="edge-label" :class="labelClass">
        {{ label }}
      </div>
    </foreignObject>
    
    <!-- 删除按钮 -->
    <foreignObject
      v-if="selected"
      :width="20"
      :height="20"
      :x="labelX - 10"
      :y="labelY - 30"
      class="vue-flow__edge-button"
    >
      <button 
        class="edge-delete-btn"
        @click="deleteEdge"
        title="删除连接"
      >
        ×
      </button>
    </foreignObject>
  </g>
</template>

<script>
import { computed } from 'vue'
import { getBezierPath, getSimpleEdgeCenter } from '@vue-flow/core'

export default {
  name: 'CustomEdge',
  props: {
    id: String,
    sourceX: Number,
    sourceY: Number,
    targetX: Number,
    targetY: Number,
    sourcePosition: String,
    targetPosition: String,
    data: Object,
    style: Object,
    selected: Boolean,
  },
  emits: ['edge-click', 'edge-delete'],
  setup(props, { emit }) {
    // 计算贝塞尔曲线路径
    const edgePath = computed(() => {
      const [path] = getBezierPath({
        sourceX: props.sourceX,
        sourceY: props.sourceY,
        sourcePosition: props.sourcePosition,
        targetX: props.targetX,
        targetY: props.targetY,
        targetPosition: props.targetPosition,
        curvature: 0.2, // 曲线弯曲度
      })
      return path
    })

    // 计算标签位置
    const { labelX, labelY } = computed(() => {
      const [, labelX, labelY] = getSimpleEdgeCenter({
        sourceX: props.sourceX,
        sourceY: props.sourceY,
        targetX: props.targetX,
        targetY: props.targetY,
      })
      return { labelX, labelY }
    }).value

    // 箭头颜色
    const arrowColor = computed(() => {
      return props.data?.color || '#b1b1b7'
    })

    // 箭头标记
    const computedMarkerEnd = computed(() => {
      if (props.data?.type === 'condition-true') {
        return 'url(#arrow-success)'
      } else if (props.data?.type === 'condition-false') {
        return 'url(#arrow-error)'
      } else {
        return 'url(#arrowclosed)'
      }
    })

    // 边样式
    const edgeStyle = computed(() => {
      const baseStyle = {
        stroke: props.data?.color || '#b1b1b7',
        strokeWidth: props.selected ? 3 : 2,
        fill: 'none',
      }
      
      // 根据边类型设置样式
      if (props.data?.type === 'condition-true') {
        baseStyle.stroke = '#52c41a'
        baseStyle.strokeDasharray = '0'
      } else if (props.data?.type === 'condition-false') {
        baseStyle.stroke = '#f5222d'
        baseStyle.strokeDasharray = '5,5'
      } else if (props.data?.type === 'error') {
        baseStyle.stroke = '#faad14'
        baseStyle.strokeDasharray = '3,3'
      }
      
      return baseStyle
    })

    // 标签内容
    const label = computed(() => {
      if (props.data?.label) return props.data.label
      if (props.data?.type === 'condition-true') return 'True'
      if (props.data?.type === 'condition-false') return 'False'
      if (props.data?.type === 'error') return 'Error'
      return ''
    })

    // 标签样式类
    const labelClass = computed(() => {
      const classes = []
      if (props.data?.type === 'condition-true') classes.push('label-success')
      if (props.data?.type === 'condition-false') classes.push('label-error')
      if (props.data?.type === 'error') classes.push('label-warning')
      if (props.selected) classes.push('label-selected')
      return classes
    })

    const labelWidth = 60
    const labelHeight = 20

    // 删除边
    const deleteEdge = () => {
      emit('edge-delete', props.id)
    }

    return {
      edgePath,
      edgeStyle,
      markerEnd: computedMarkerEnd,
      arrowColor,
      labelX,
      labelY,
      labelWidth,
      labelHeight,
      label,
      labelClass,
      deleteEdge,
    }
  },
}
</script>

<style scoped>
.vue-flow__edge-path {
  transition: stroke-width 0.2s ease, stroke 0.2s ease;
}

.vue-flow__edge-path:hover {
  stroke-width: 3 !important;
}

.edge-label {
  background: white;
  border: 1px solid #d9d9d9;
  border-radius: 4px;
  padding: 2px 6px;
  font-size: 11px;
  font-weight: 500;
  text-align: center;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  white-space: nowrap;
}

.label-success {
  background: #f6ffed;
  border-color: #b7eb8f;
  color: #52c41a;
}

.label-error {
  background: #fff1f0;
  border-color: #ffccc7;
  color: #f5222d;
}

.label-warning {
  background: #fffbe6;
  border-color: #ffe58f;
  color: #faad14;
}

.label-selected {
  border-color: #1890ff;
  box-shadow: 0 0 0 2px rgba(24, 144, 255, 0.2);
}

.edge-delete-btn {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  border: 1px solid #ff4d4f;
  background: #fff1f0;
  color: #ff4d4f;
  font-size: 12px;
  font-weight: bold;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
}

.edge-delete-btn:hover {
  background: #ff4d4f;
  color: white;
  transform: scale(1.1);
}
</style>