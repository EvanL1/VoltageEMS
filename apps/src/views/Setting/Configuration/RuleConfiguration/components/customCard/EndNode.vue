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
    padding: 12px;
    background-color: #f57c00; // deep orange
    border-radius: 8px;
    cursor: grab;
    transition: all 0.2s ease;
    min-width: 200px;
    border-left: 4px solid rgba(255, 138, 0, 0.4); // orange accent
    border-top: 2px solid rgba(255, 138, 0, 0.4);

    &:hover {
      border-color: #ff8a00;
      box-shadow: 4px 4px 12px rgba(255, 138, 0, 0.25);
    }

    &:active {
      cursor: grabbing;
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

      &:hover {
        background-color: #ffb74d;
        transform: translateY(-50%) scale(1.2);
      }
    }

    &__content {
      display: flex;
      align-items: center;
      flex: 1;
      padding: 0 8px;
    }

    &__icon {
      width: 36px;
      height: 36px;
      display: flex;
      align-items: center;
      justify-content: center;
      background-color: #ef6c00; // deeper orange tile
      border-radius: 6px;
      margin-right: 12px;
      .el-icon {
        font-size: 20px;
        color: #ffffff;
      }
    }

    &__info {
      flex: 1;
    }

    &__name {
      font-weight: 600;
      color: #ffffff;
      margin-bottom: 4px;
      font-size: 14px;
    }

    &__description {
      font-size: 12px;
      color: #ffffff;
      line-height: 1.4;
    }
  }
}
</style>
