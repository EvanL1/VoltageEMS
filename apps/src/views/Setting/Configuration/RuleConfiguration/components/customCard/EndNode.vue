<template>
  <div class="voltage-class end-node">
    <!-- 左侧连接点（只能接收连接） -->
    <Handle
      type="target"
      :position="Position.Left"
      id="left"
      class="end-node__handle end-node__handle--left"
    />

    <!-- 卡片内容 -->
    <div class="end-node__content">
      <div class="end-node__icon">
        <el-icon><CircleCheck /></el-icon>
      </div>
      <div class="end-node__info">
        <div class="end-node__name">END</div>
        <div class="end-node__description">Rule chain exit point</div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Handle, Position } from '@vue-flow/core'
import { CircleCheck } from '@element-plus/icons-vue'

defineProps<{
  data: any
}>()
</script>

<style lang="scss" scoped>
.voltage-class {
  .end-node {
    display: flex;
    align-items: center;
    padding: 0.12rem;
    background-color: #f57c00; // deep orange
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

      &--left {
        left: -0.06rem;
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
      background-color: #ef6c00; // deeper orange tile
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
