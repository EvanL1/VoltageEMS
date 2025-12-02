<template>
  <div
    class="voltage-class custom-node"
    :class="[`custom-node--${data.type || 'default'}`]"
    ref="nodeRef"
    :style="nodeStyle"
  >
    <!-- 左侧连接点 -->
    <Handle
      type="target"
      :position="Position.Left"
      id="left"
      class="custom-node__handle custom-node__handle--left"
      :style="{ top: '50%' }"
    />

    <!-- 卡片内容 -->
    <div
      class="custom-node__content"
      :style="{
        marginRight: data.type === 'function-switch' && data.config.rule.length > 0 ? '26px' : '0',
      }"
    >
      <div class="custom-node__icon" :class="`icon--${data.type}`">
        <img :src="fenzhiIcon" v-if="data.type === 'function-switch'" />
        <img :src="chongzhiIcon" v-else-if="data.type === 'action-changeValue'" />
      </div>
      <div class="custom-node__info">
        <div class="custom-node__name">{{ data.label }}</div>
        <div class="custom-node__description">{{ data.description || '' }}</div>
      </div>
    </div>

    <!-- 右侧连接点 -->
    <Handle
      v-if="data.type !== 'function-switch'"
      type="source"
      :position="Position.Right"
      id="right1"
      class="custom-node__handle custom-node__handle--right"
      :style="{ top: '50%' }"
    ></Handle>
    <template v-if="data.type === 'function-switch'">
      <Handle
        v-for="(item, idx) in data.config.rule"
        :key="item.name"
        type="source"
        :position="Position.Right"
        :id="item.name"
        class="custom-node__handle custom-node__handle--right"
        :style="{ top: `calc(${((idx + 1) / (data.config.rule.length + 1)) * 100}% - 6px)` }"
      >
        <div class="custom-node__handle__text">{{ item.name }}</div>
      </Handle>
    </template>
  </div>
</template>

<script setup lang="ts">
import { Handle, Position } from '@vue-flow/core'
import { ref, onMounted, nextTick, watch, computed } from 'vue'
import { QuestionFilled, Switch, Pointer } from '@element-plus/icons-vue'
import fenzhiIcon from '@/assets/icons/button-fenzhi.svg'
import chongzhiIcon from '@/assets/icons/button-change.svg'
const props = defineProps<{
  data: any
}>()

const nodeRef = ref<HTMLElement | null>(null)
const baseHeight = ref<number>(0)

const handlesCount = computed(() => {
  const rules = props.data?.config?.rule
  return Array.isArray(rules) ? rules.length : 0
})

const nodeStyle = computed(() => {
  if (!baseHeight.value) return {}
  // 最小相邻 Handle 间距固定为 8px
  const minGapPx = 20
  const requiredMinHeight = Math.max(baseHeight.value, (handlesCount.value + 1) * minGapPx)
  return { minHeight: requiredMinHeight + 'px' }
})

function measureBaseHeight() {
  nextTick(() => {
    const el = nodeRef.value
    // const h = nodeRef.value?.offsetHeight || nodeRef.value?.clientHeight  || 0
    const h = nodeRef.value?.offsetHeight || nodeRef.value?.clientHeight || 0
    // 取一次基准高度；若首次测量不到，则使用一个保守默认值
    if (!baseHeight.value) baseHeight.value = h || 80
  })
}
watch(
  () => props.data?.config?.rule?.length,
  () => {
    // 规则数量变化后重新测量以得到合理的基线
    measureBaseHeight()
  },
)
</script>

<style lang="scss" scoped>
.voltage-class {
  .custom-node {
    display: flex;
    align-items: center;
    padding: 12px;
    // background-color: white;
    border-radius: 8px;
    cursor: grab;
    transition: all 0.2s ease;
    min-width: 250px;
    border-left: 4px solid rgba(255, 138, 0, 0.4);
    border-top: 2px solid rgba(255, 138, 0, 0.4);
    &:hover {
      border-color: #ff8a00;
      box-shadow: 4px 4px 12px rgba(255, 138, 0, 0.25);
    }

    &:active {
      cursor: grabbing;
    }

    &--function-switch {
      background-color: #81c784; // soft green
    }
    &--action-changeValue {
      background-color: #4fc3f7; // soft sky blue
    }

    &--function-switch .custom-node__name,
    &--function-switch .custom-node__description,
    &--action-changeValue .custom-node__name,
    &--action-changeValue .custom-node__description {
      color: #ffffff;
    }

    .icon--function-switch {
      background-color: #66bb6a; // green tile
    }
    .icon--action-changeValue {
      background-color: #29b6f6; // sky blue tile
    }

    &__handle {
      width: 12px;
      height: 12px;
      background-color: #ff8a00;
      border: 2px solid white;
      border-radius: 50%;
      position: absolute;
      top: 50%;
      transform: translateY(-50%);
      z-index: 10;
      cursor: crosshair;

      &--left {
        left: -6px;
      }

      &--right {
        right: -6px;

        .custom-node__handle__text {
          font-size: 10px;
          color: rgba(255, 105, 0, 1);
          position: absolute;
          top: 50%;
          right: 16px;
          text-align: right;
          transform: translateY(-50%);
        }
      }

      &:hover {
        background-color: #ffb74d;
        transform: translateY(-50%) scale(1.2);
      }
    }

    &__content {
      display: flex;
      align-items: center;
      width: 100%;
      padding: 0 8px;
    }

    &__icon {
      width: 36px;
      height: 36px;
      display: flex;
      align-items: center;
      justify-content: center;
      background-color: #fff2e6;
      border-radius: 6px;
      margin-right: 12px;
      .el-icon {
        font-size: 20px;
        color: #ffffff !important;
      }
      img {
        width: 0.2rem;
        height: 0.2rem;
      }
    }

    &__info {
      flex: 1;
    }

    &__name {
      font-weight: 600;
      color: #2c3e50;
      margin-bottom: 4px;
      font-size: 14px;
    }

    &__description {
      font-size: 12px;
      color: #909399;
      line-height: 1.4;
      // white-space: nowrap;
      // overflow: hidden;
      // text-overflow: ellipsis;
      word-break: break-all;
      width: 100%;
    }
  }
}
</style>
