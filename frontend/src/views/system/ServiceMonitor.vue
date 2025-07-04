<template>
  <div class="page-container">
    <div class="page-header">
      <h1>{{ $t('menu.pageName') }}</h1>
      <div class="header-actions">
        <el-button type="primary">
          <el-icon><Plus /></el-icon>
          {{ $t('common.add') }}
        </el-button>
      </div>
    </div>

    <el-card>
      <div class="coming-soon">
        <el-icon :size="64" color="#409eff">
          <Tools />
        </el-icon>
        <h2>{{ $t('common.developing', '功能开发中') }}</h2>
        <p>{{ $t('common.developingDesc', '该功能正在开发中，敬请期待') }}</p>
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { Plus, Tools } from '@element-plus/icons-vue'
</script>

<style lang="scss" scoped>
@import '@/styles/design-tokens.scss';

.page-container {
  padding: var(--page-padding);
  background: var(--color-background);
  min-height: 100vh;
}

/* Apple Style Page Header */
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--spacing-xxl);
  
  h1 {
    font-size: var(--font-size-4xl);
    font-weight: var(--font-weight-bold);
    color: var(--color-text-primary);
    margin: 0;
    letter-spacing: -0.02em;
    
    // Apple's distinctive text rendering
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }
}

.header-actions {
  display: flex;
  gap: var(--spacing-md);
}

/* Tesla Style Card */
:deep(.el-card) {
  background: var(--color-surface);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  border: 1px solid var(--color-border);
  transition: all 0.3s ease;
  
  &:hover {
    box-shadow: var(--shadow-md);
  }
}

:deep(.el-card__body) {
  padding: var(--spacing-xxl);
}

/* Coming Soon */
.coming-soon {
  text-align: center;
  padding: calc(var(--spacing-xxl) * 3) var(--spacing-xl);
  
  :deep(.el-icon) {
    color: var(--color-primary);
    margin-bottom: var(--spacing-lg);
  }
  
  h2 {
    margin: var(--spacing-lg) 0 var(--spacing-md);
    font-size: var(--font-size-2xl);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    letter-spacing: -0.5px;
  }
  
  p {
    color: var(--color-text-secondary);
    font-size: var(--font-size-base);
    font-weight: var(--font-weight-medium);
    line-height: 1.6;
  }
}

/* Enhanced Buttons */
:deep(.el-button) {
  border-radius: var(--radius-md);
  font-weight: var(--font-weight-medium);
  padding: var(--spacing-sm) var(--spacing-lg);
  transition: all 0.3s ease;
  border: none;
  box-shadow: var(--shadow-sm);
}

:deep(.el-button--primary) {
  background: var(--gradient-primary);
  color: white;
  
  &:hover {
    transform: translateY(-1px);
    box-shadow: var(--shadow-md);
    opacity: 0.9;
  }
  
  &:active {
    transform: translateY(0);
  }
}

/* Responsive Design */
@media (max-width: 768px) {
  .page-container {
    padding: var(--spacing-md);
  }
  
  .page-header {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--spacing-md);
    
    h1 {
      font-size: var(--font-size-2xl);
    }
  }
  
  .header-actions {
    width: 100%;
    
    .el-button {
      flex: 1;
    }
  }
}

/* Icon Styling */
:deep(.el-icon) {
  margin-right: var(--spacing-xs);
}
</style>