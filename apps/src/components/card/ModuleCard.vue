<template>
  <div class="card">
    <div class="card__header">
      <div class="card__header-icon"></div>
      <div class="card__header-title">{{ props.title }}</div>
    </div>
    <div class="card__content" :class="{ 'no-footer': !props.isShowFooter }">
      <slot></slot>
    </div>
    <div class="card__content-footer" v-if="props.isShowFooter">
      <el-button type="primary" class="card__content-footer-button" @click="handleSubmitClick"
        ><img :src="submitIcon" class="card__content-footer-button-icon" />Submit</el-button
      >
    </div>
  </div>
</template>

<script lang="ts" setup>
import submitIcon from '@/assets/icons/btn-submit.svg'
const props = defineProps({
  title: {
    type: String,
    required: true,
  },
  isShowFooter: {
    type: Boolean,
    default: false,
  },
})

const emit = defineEmits<{
  (e: 'submit'): void
}>()

function handleSubmitClick() {
  emit('submit')
}
</script>

<style lang="scss" scoped>
.card {
  width: 100%;
  height: 100%;
  min-height: 0;
  background: rgba(84, 98, 140, 0.2);
  padding: 20px;
  border: 1px solid;
  border-image: linear-gradient(
      117.31deg,
      rgba(148, 166, 197, 0.3) 2.77%,
      rgba(148, 166, 197, 0) 32.18%,
      rgba(148, 166, 197, 0.103266) 72.63%,
      rgba(148, 166, 197, 0.3) 99.05%
    )
    1;
  backdrop-filter: blur(10px);
  display: flex;
  flex-direction: column;
  .card__header {
    display: flex;
    align-items: center;
    .card__header-icon {
      width: 20px;
      height: 20px;
      background-image: url('@/assets/images/card-icon.png');
      background-size: cover;
      background-position: center;
      background-repeat: no-repeat;
    }
    .card__header-title {
      margin-left: 3px;
      font-family: Arimo;
      font-weight: 700;
      font-style: Bold;
      font-size: 18px;
      line-height: 1em;
      color: #ffffff;
    }
  }
  .card__content {
    flex: 1 1 auto;
    min-height: 0;
    width: 100%;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    &.no-footer {
      height: calc(100% - 20px);
    }
  }
  .card__content-footer {
    flex: none;
    width: 100%;
    display: flex;
    flex-direction: row-reverse;
    border-top: 1px solid rgba(255, 255, 255, 0.1);
    .card__content-footer-button {
      margin: 30px 0 10px 0;
      .card__content-footer-button-icon {
        width: 16px;
        height: 16px;
        margin-right: 8px;
        object-position: center;
        object-fit: contain;
      }
    }
  }
}
</style>
