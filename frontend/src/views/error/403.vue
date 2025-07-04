<template>
  <div class="error-page">
    <div class="error-content">
      <div class="error-icon">
        <el-icon :size="120" color="#f56c6c">
          <WarnTriangleFilled />
        </el-icon>
      </div>
      <h1 class="error-code">403</h1>
      <h2 class="error-title">{{ $t('error.forbidden') }}</h2>
      <p class="error-description">
        {{ errorMessage }}
      </p>
      <div class="error-actions">
        <el-button type="primary" @click="goBack">
          {{ $t('common.goBack', '返回上一页') }}
        </el-button>
        <el-button @click="goHome">
          {{ $t('common.goHome', '返回首页') }}
        </el-button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useUserStore } from '@/stores/user'

const router = useRouter()
const { t } = useI18n()
const userStore = useUserStore()

const errorMessage = computed(() => {
  const roleTexts = {
    operator: t('error.operatorForbidden', '操作员没有权限访问此页面'),
    engineer: t('error.engineerForbidden', '工程师没有权限访问此页面'),
    admin: t('error.adminForbidden', '管理员权限异常')
  }
  
  return roleTexts[userStore.role] || t('error.defaultForbidden', '您没有权限访问此页面，请联系管理员')
})

const goBack = () => {
  router.go(-1)
}

const goHome = () => {
  router.push('/')
}
</script>

<style lang="scss" scoped>
.error-page {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: #f5f7fa;
}

.error-content {
  text-align: center;
  padding: 40px;
  background: white;
  border-radius: 8px;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.1);
  max-width: 500px;

  .error-icon {
    margin-bottom: 20px;
  }

  .error-code {
    font-size: 72px;
    font-weight: 700;
    color: #f56c6c;
    margin: 0 0 10px 0;
  }

  .error-title {
    font-size: 24px;
    color: #333;
    margin: 0 0 20px 0;
  }

  .error-description {
    font-size: 16px;
    color: #666;
    margin: 0 0 30px 0;
    line-height: 1.5;
  }

  .error-actions {
    display: flex;
    gap: 12px;
    justify-content: center;
  }
}
</style>