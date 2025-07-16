<template>
  <div class="error-page">
    <div class="error-content">
      <div class="error-icon">
        <el-icon :size="120">
          <Lock />
        </el-icon>
      </div>
      <h1 class="error-code">403</h1>
      <h2 class="error-title">{{ $t('error.forbidden', '访问被拒绝') }}</h2>
      <p class="error-description">
        {{ errorMessage }}
      </p>
      <div class="error-info">
        <p class="current-role">
          {{ $t('error.currentRole', '当前角色') }}: 
          <el-tag type="warning">{{ userRoleName }}</el-tag>
        </p>
      </div>
      <div class="error-actions">
        <el-button type="primary" @click="goBack">
          <el-icon><Back /></el-icon>
          {{ $t('common.goBack', '返回上一页') }}
        </el-button>
        <el-button @click="goHome">
          <el-icon><House /></el-icon>
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
import { usePermission } from '@/composables/usePermission'
import { Lock, Back, House } from '@element-plus/icons-vue'

const router = useRouter()
const { t } = useI18n()
const userStore = useUserStore()
const { userRoleName } = usePermission()

const errorMessage = computed(() => {
  const roleTexts = {
    super_admin: t('error.superAdminForbidden', '超级管理员权限异常，请联系技术支持'),
    system_admin: t('error.systemAdminForbidden', '系统管理员没有权限访问此页面'),
    ops_engineer: t('error.engineerForbidden', '运维工程师没有权限访问此页面'),
    monitor: t('error.monitorForbidden', '监控人员没有权限访问此页面'),
    guest: t('error.guestForbidden', '访客没有权限访问此页面')
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
@import '@/styles/design-tokens.scss';

.error-page {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--color-background);
}

.error-content {
  text-align: center;
  padding: var(--space-10);
  background: var(--color-background-elevated);
  border-radius: var(--radius-xl);
  border: 1px solid var(--color-border-light);
  box-shadow: var(--shadow-lg);
  max-width: 500px;

  .error-icon {
    margin-bottom: var(--space-6);
    color: var(--color-accent-orange);
  }

  .error-code {
    font-size: 96px;
    font-weight: var(--font-weight-bold);
    margin: 0 0 var(--space-4) 0;
    line-height: 1;
    background: linear-gradient(135deg, var(--color-accent-orange), var(--color-accent-gold));
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
  }

  .error-title {
    font-size: var(--font-size-3xl);
    color: var(--color-text-primary);
    margin: 0 0 var(--space-6) 0;
    font-weight: var(--font-weight-semibold);
  }

  .error-description {
    font-size: var(--font-size-lg);
    color: var(--color-text-secondary);
    margin: 0 0 var(--space-6) 0;
    line-height: 1.6;
  }

  .error-info {
    margin-bottom: var(--space-8);

    .current-role {
      font-size: var(--font-size-base);
      color: var(--color-text-secondary);
      
      .el-tag {
        margin-left: var(--space-2);
      }
    }
  }

  .error-actions {
    display: flex;
    gap: var(--space-4);
    justify-content: center;

    .el-button {
      min-width: 140px;
    }
  }
}
</style>