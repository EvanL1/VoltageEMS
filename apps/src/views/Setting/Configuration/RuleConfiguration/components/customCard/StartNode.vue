<template>
  <div class="voltage-class start-node">
    <!-- 卡片内容 -->
    <div class="start-node__content">
      <div class="start-node__icon">
        <el-icon><CaretRight /></el-icon>
      </div>
      <div class="start-node__info">
        <div class="start-node__name">START</div>
        <div class="start-node__description">Rule chain entry point</div>
      </div>
    </div>

    <!-- 右侧连接点（只能发起连接） -->
    <Handle
      type="source"
      :position="Position.Right"
      id="right"
      class="start-node__handle start-node__handle--right"
    />
  </div>
</template>

<script setup lang="ts">
import { Handle, Position } from '@vue-flow/core'
import { CaretRight } from '@element-plus/icons-vue'

defineProps<{
  data: any
}>()
</script>

<style lang="scss" scoped>
.voltage-class {
  .start-node {
    display: flex;
    align-items: center;
    padding: 0.12rem;
    background-color: #ffcc80; // light orange
    border-radius: 0.08rem;
    cursor: grab;
    transition: all 0.2s ease;
    min-width: 2rem;
    border-left: 0.04rem solid rgba(255, 138, 0, 0.4); // orange accent
    border-top: 0.02rem solid rgba(255, 138, 0, 0.4);
    box-shadow: 0.02rem 0.02rem 0.08rem rgba(0, 0, 0, 0.15);
    &:hover {
      border-color: #ff8a00;
      box-shadow: 0.04rem 0.04rem 0.12rem rgba(255, 138, 0, 0.25);
    }

    &:active {
      cursor: grabbing;
    }

    &__handle {
      width: 0.12rem;
      height: 0.12rem;
      background-color: #ff8a00;
      border: 0.02rem solid white;
      border-radius: 50%;
      position: absolute;
      top: 50%;
      transform: translateY(-50%);
      z-index: 10;
      cursor: crosshair;

      &--right {
        right: -0.06rem;
      }

      &:hover {
        background-color: #ffb74d;
        transform: translateY(-50%) scale(1.2);
      }
    }

    &__content {
      display: flex;
      align-items: center;
      flex: 1;
      padding: 0 0.08rem;
    }

    &__icon {
      width: 0.36rem;
      height: 0.36rem;
      display: flex;
      align-items: center;
      justify-content: center;
      background-color: #ffa726; // orange tile for contrast
      border-radius: 0.06rem;
      margin-right: 0.12rem;
      .el-icon {
        font-size: 0.2rem;
        color: #ffffff;
      }
    }

    &__info {
      flex: 1;
    }

    &__name {
      font-weight: 600;
      color: #ffffff;
      margin-bottom: 0.04rem;
      font-size: 0.14rem;
    }

    &__description {
      font-size: 0.12rem;
      color: #ffffff;
      line-height: 1.4;
    }
  }
}
</style>
