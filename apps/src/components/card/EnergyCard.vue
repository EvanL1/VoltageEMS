<template>
  <div class="card">
    <div class="card__container">
      <div class="card__left">
        <div class="card__left-icon">
          <img :src="iconUrl" class="card__left-iconImg" />
        </div>
      </div>
      <div class="card__right">
        <div class="card__right-top">
          <div class="card__right-title">{{ props.title }}</div>
        </div>
        <div class="card__right-bottom">
          <span class="card__right-value">
            {{ props.value }}
            <span v-if="props.unit" class="card__right-unit">&nbsp;{{ props.unit }}</span>
          </span>
        </div>
      </div>
    </div>
  </div>
</template>

<script lang="ts" setup>
import { computed } from 'vue'

const props = defineProps<{
  title: string
  icon: string
  value: string
  unit?: string
}>()

// 使用import.meta.globEager批量导入图片资源，确保打包后图片路径正确
const images = import.meta.glob('@/assets/images/*.png', { eager: true, import: 'default' })

const iconUrl = computed(() => {
  // 兼容@和/src两种写法，确保开发和生产环境都能正常加载
  const key1 = `/src/assets/images/${props.icon}.png`
  const key2 = `@/assets/images/${props.icon}.png`
  const result = images[key1] || images[key2] || ''
  return typeof result === 'string' ? result : ''
})
</script>

<style lang="scss" scoped>
.card {
  width: 240px;
  height: 70px;
  display: flex;
  align-items: center;
  .card__container {
    display: flex;
    .card__left {
      display: flex;
      align-items: center;
      margin-right: 23px;

      .card__left-icon {
        width: 53.6px;
        height: 58.6px;
        display: flex;
        align-items: center;
        justify-content: center;
        .card__left-iconImg {
          width: 100%;
          height: 100%;
          object-fit: contain;
        }
      }
    }
    .card__right {
      height: 100%;
      flex: 1;
      display: flex;
      flex-direction: column;
      justify-content: space-between;
      color: rgba(255, 255, 255, 0.6);
      .card__right-top {
        .card__right-title {
          font-weight: 400;
          font-size: 16px;
          line-height: 22px;
        }
      }
      .card__right-bottom {
        margin-top: 5px;
        height: 32px;
        .card__right-value {
          font-weight: 700;
          font-size: 30px;
          line-height: 32px;
          color: #ffffff;
          .card__right-unit {
            font-weight: 700;
            font-size: 14px;
            color: rgba(255, 255, 255, 0.6);
          }
        }
      }
    }
  }
}
</style>
