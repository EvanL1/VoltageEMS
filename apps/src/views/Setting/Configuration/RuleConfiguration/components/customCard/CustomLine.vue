<script setup>
import { computed, ref } from 'vue'

const props = defineProps({
  // 起点X坐标（必填）
  sourceX: { type: Number, required: true },
  // 起点Y坐标（必填）
  sourceY: { type: Number, required: true },
  // 终点X坐标（必填）
  targetX: { type: Number, required: true },
  // 终点Y坐标（必填）
  targetY: { type: Number, required: true },
  // 起点方向（默认'right'，可选'left'、'top'、'bottom'等）
  sourcePosition: { type: String, default: 'right' },
  // 终点方向（默认'left'，可选'right'、'top'、'bottom'等）
  targetPosition: { type: String, default: 'left' },
  // 线条附加数据对象
  data: { type: Object, default: () => ({}) },
  // 是否被选中
  selected: { type: Boolean, default: false },
  // 是否显示动画（如虚线流动）
  animated: { type: Boolean, default: false },
  // 线条自定义样式
  style: { type: Object, default: () => ({}) },
  // 线条自定义class
  class: { type: String, default: '' },
  // 线条标签内容
  label: { type: String, default: '123' },
  // 标签文字样式
  labelStyle: { type: Object, default: () => ({}) },
  // 标签背景样式
  labelBgStyle: { type: Object, default: () => ({}) },
  // 标签背景内边距（数字或数组）
  labelBgPadding: { type: [Number, Array], default: 2 },
  // 标签背景圆角
  labelBgBorderRadius: { type: Number, default: 0 },
  // 终点箭头marker id
  markerEnd: { type: String, default: undefined },
  // 起点箭头marker id
  markerStart: { type: String, default: undefined },
  // 路径额外配置
  pathOptions: { type: Object, default: () => ({}) },
  // 交互宽度（鼠标可点击区域宽度）
  interactionWidth: { type: Number, default: 20 },
  
  // 自定义样式 props
  // 线条颜色
  color: { type: String, default: '#6F3381' },
  // 线条宽度
  strokeWidth: { type: Number, default: 2 },
  // 虚线样式（如"6 6"表示6px实线6px空隙）
  dash: { type: [String, Number], default: '6 6' }, // 虚线节奏：间隔为 "线段 间隙"
  // 是否让虚线"流动"动画
  animated: { type: Boolean, default: true },       // 是否让虚线"流动"
  // 是否为曲线（true=贝塞尔曲线，false=直线）
  curved: { type: Boolean, default: true },         // true=贝塞尔曲线, false=直线
})
const hoverColor = 'red'
const hoverStrokeWidth = computed(() => props.strokeWidth * 2)
// 定义事件
const emit = defineEmits(['click', 'edge-click'])

// 防止 <marker> id 冲突
const markerId = `arrow-${Math.random().toString(36).slice(2)}`

const pathD = computed(() => {
  const sx = props.sourceX, sy = props.sourceY
  const tx = props.targetX, ty = props.targetY

  if (!props.curved) return `M${sx},${sy} L${tx},${ty}`

  // 水平中点做双控制点：平滑、简单且好看
  const cx = (sx + tx) / 2
  return `M${sx},${sy} C ${cx},${sy} ${cx},${ty} ${tx},${ty}`
})

const handleClick = (event) => {
  event.stopPropagation()
  // 同时触发两种事件以确保兼容性
  emit('click', event)
  emit('edge-click', event)
}
</script>

<template>
  <g class="custom-line" @click="handleClick">
    <!-- 箭头定义：随线宽缩放，自动按切线方向旋转 -->
    <defs>
      <marker
        :id="markerId"
        markerWidth="14"
        markerHeight="7"
        refX="10"
        refY="3.5"
        orient="auto"
        :markerUnits="strokeWidth"
      >
        <!-- 更长但整体缩小的三角形：x最大为11，y最大为7 -->
        <path d="M0,0 L11,3.5 L0,7 Z" :fill="color" />
      </marker>
    </defs>

    <!-- 可见的路径 -->
    <path
      class="vue-flow__connection vue-flow__edge-path"
      :class="{ animated }"
      :d="pathD"
      fill="none"
      :stroke="color"
      :stroke-width="strokeWidth"
      :stroke-dasharray="dash"
      vector-effect="non-scaling-stroke"
      :marker-end="`url(#${markerId})`"
    />
    
    <!-- 不可见的点击区域，增加点击范围 -->
    <!-- <path
      class="vue-flow__edge-interaction"
      :d="pathD"
      fill="none"
      :stroke="'transparent'"
      :stroke-width="interactionWidth"
      vector-effect="non-scaling-stroke"
      style="cursor: pointer;"
    /> -->
  </g>
</template>

<style scoped>
/* 让虚线流动（可按需调整速度与位移） */
.animated {
  animation: dash-move 1.2s linear infinite;
  /* 初始偏移可不设，由动画控制 */
}

@keyframes dash-move {
  to { stroke-dashoffset: -24; }
}


.vue-flow__edge-interaction {
  /* cursor: pointer; */
  pointer-events: all;
}

.vue-flow__edge-path {
  pointer-events: none;
  stroke:v-bind(color);
  stroke-width:v-bind(strokeWidth);
  &:hover {
    stroke:v-bind(hoverColor);
    stroke-width:v-bind(hoverStrokeWidth);
  }
}
</style>
