<template>
  <div class="login-container">
    <!-- 背景视频或渐变 -->
    <div class="login-background">
      <div class="gradient-overlay"></div>
    </div>

    <!-- 主内容 -->
    <div class="login-content">
      <!-- Logo -->
      <div class="logo-section">
        <div class="logo-wrapper">
          <svg width="60" height="60" viewBox="0 0 60 60" fill="none" xmlns="http://www.w3.org/2000/svg">
            <circle cx="30" cy="30" r="28" stroke="currentColor" stroke-width="4"/>
            <path d="M20 30L25 20L30 40L35 15L40 30" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </div>
        <h1 class="brand-name">Monarch Hub</h1>
        <p class="brand-tagline">{{ $t('login.subtitle') }}</p>
      </div>

      <!-- 登录表单 -->
      <form class="login-form" @submit.prevent="handleLogin">
        <div class="form-group">
          <input
            v-model="loginForm.username"
            type="text"
            class="form-input"
            :placeholder="$t('login.username')"
            :class="{ 'has-value': loginForm.username }"
            required
          >
          <label class="form-label">{{ $t('login.username') }}</label>
        </div>

        <div class="form-group">
          <input
            v-model="loginForm.password"
            :type="showPassword ? 'text' : 'password'"
            class="form-input"
            :placeholder="$t('login.password')"
            :class="{ 'has-value': loginForm.password }"
            required
          >
          <label class="form-label">{{ $t('login.password') }}</label>
          <button
            type="button"
            class="password-toggle"
            @click="showPassword = !showPassword"
          >
            <span v-if="showPassword">👁</span>
            <span v-else>👁‍🗨</span>
          </button>
        </div>

        <div class="form-options">
          <label class="checkbox-wrapper">
            <input
              v-model="loginForm.remember"
              type="checkbox"
              class="checkbox-input"
            >
            <span class="checkbox-label">{{ $t('login.remember') }}</span>
          </label>
        </div>

        <button
          type="submit"
          class="submit-button"
          :disabled="loading"
        >
          <span v-if="!loading">{{ $t('login.loginButton') }}</span>
          <div v-else class="loading-spinner">
            <div class="spinner"></div>
          </div>
        </button>
      </form>

      <!-- 快速登录 -->
      <div class="quick-login">
        <p class="quick-login-title">{{ $t('login.demoUsers') }}</p>
        <div class="quick-login-buttons">
          <button
            v-for="user in demoUsers"
            :key="user.username"
            class="quick-login-btn"
            :class="`role-${user.role}`"
            @click="quickLogin(user)"
          >
            <span class="role-icon">
              <el-icon><User /></el-icon>
            </span>
            <span class="role-name">{{ $t(`roles.${user.role}`) }}</span>
          </button>
        </div>
      </div>
    </div>

    <!-- 语言切换 -->
    <div class="language-switcher">
      <button
        v-for="lang in ['zh', 'en']"
        :key="lang"
        class="lang-btn"
        :class="{ active: currentLang === lang }"
        @click="changeLanguage(lang)"
      >
        {{ lang === 'zh' ? '中文' : 'EN' }}
      </button>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, reactive, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { useUserStore } from '@/stores/user'
import { User } from '@element-plus/icons-vue'

const router = useRouter()
const { t, locale } = useI18n()
const userStore = useUserStore()

// 响应式数据
const loading = ref(false)
const showPassword = ref(false)
const currentLang = computed({
  get: () => locale.value,
  set: (val) => locale.value = val
})

// 登录表单
const loginForm = reactive({
  username: '',
  password: '',
  remember: false
})

// 演示用户
const demoUsers = [
  {
    username: 'operator',
    password: 'operator123',
    role: 'operator',
    color: '#52c41a',
    tagType: 'success'
  },
  {
    username: 'engineer',
    password: 'engineer123',
    role: 'engineer',
    color: '#faad14',
    tagType: 'warning'
  },
  {
    username: 'admin',
    password: 'admin123',
    role: 'admin',
    color: '#f5222d',
    tagType: 'danger'
  }
]

// 方法
const handleLogin = async () => {
  if (!loginForm.username || !loginForm.password) {
    ElMessage.error(t('login.pleaseEnterCredentials'))
    return
  }

  loading.value = true
  try {
    const result = await userStore.login({
      username: loginForm.username,
      password: loginForm.password
    })

    if (result.success) {
      ElMessage.success(t('login.loginSuccess'))
      router.push('/')
    } else {
      ElMessage.error(result.error || t('login.loginFailed'))
    }
  } catch (error) {
    ElMessage.error(t('login.loginFailed'))
  } finally {
    loading.value = false
  }
}

const quickLogin = (user) => {
  loginForm.username = user.username
  loginForm.password = user.password
  handleLogin()
}

const changeLanguage = (lang) => {
  locale.value = lang
  localStorage.setItem('language', lang)
}

// 生命周期
onMounted(() => {
  // 从本地存储恢复语言设置
  const savedLang = localStorage.getItem('language')
  if (savedLang) {
    locale.value = savedLang
  }
})
</script>

<style lang="scss" scoped>
@import '@/styles/design-tokens.scss';

.login-container {
  position: relative;
  width: 100vw;
  height: 100vh;
  overflow: hidden;
}

/* 背景 */
.login-background {
  position: absolute;
  inset: 0;
  background: linear-gradient(135deg, #1e3c72 0%, #2a5298 100%);
  
  .gradient-overlay {
    position: absolute;
    inset: 0;
    background: 
      radial-gradient(circle at 20% 50%, rgba(120, 119, 198, 0.3), transparent 50%),
      radial-gradient(circle at 80% 80%, rgba(255, 119, 198, 0.3), transparent 50%),
      radial-gradient(circle at 40% 20%, rgba(255, 219, 112, 0.2), transparent 50%);
    animation: gradientShift 30s ease infinite;
  }
}

@keyframes gradientShift {
  0%, 100% { transform: rotate(0deg) scale(1); }
  33% { transform: rotate(120deg) scale(1.1); }
  66% { transform: rotate(240deg) scale(0.9); }
}

/* 主内容 */
.login-content {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  padding: var(--space-6);
}

/* Logo 部分 */
.logo-section {
  text-align: center;
  margin-bottom: var(--space-10);
  animation: fadeInUp 0.8s ease-out;
  
  .logo-wrapper {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 100px;
    height: 100px;
    margin-bottom: var(--space-6);
    background: rgba(255, 255, 255, 0.1);
    backdrop-filter: blur(10px);
    border-radius: 30px;
    color: white;
    animation: logoFloat 6s ease-in-out infinite;
  }
  
  .brand-name {
    font-size: var(--font-size-4xl);
    font-weight: var(--font-weight-bold);
    color: white;
    margin-bottom: var(--space-2);
    letter-spacing: -0.02em;
  }
  
  .brand-tagline {
    font-size: var(--font-size-lg);
    color: rgba(255, 255, 255, 0.8);
    font-weight: var(--font-weight-light);
  }
}

@keyframes logoFloat {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-10px); }
}

@keyframes fadeInUp {
  from {
    opacity: 0;
    transform: translateY(30px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

/* 登录表单 */
.login-form {
  width: 100%;
  max-width: 380px;
  animation: fadeInUp 0.8s ease-out 0.2s both;
  
  .form-group {
    position: relative;
    margin-bottom: var(--space-6);
    
    .form-input {
      width: 100%;
      height: 56px;
      padding: 0 var(--space-4);
      background: rgba(255, 255, 255, 0.1);
      border: 1px solid rgba(255, 255, 255, 0.2);
      border-radius: var(--radius-lg);
      font-size: var(--font-size-base);
      color: white;
      transition: all var(--duration-normal) var(--ease-in-out);
      
      &::placeholder {
        color: transparent;
      }
      
      &:focus,
      &.has-value {
        background: rgba(255, 255, 255, 0.15);
        border-color: rgba(255, 255, 255, 0.5);
        outline: none;
        
        + .form-label {
          top: -10px;
          font-size: var(--font-size-sm);
          background: linear-gradient(135deg, #1e3c72 0%, #2a5298 100%);
          padding: 0 var(--space-2);
        }
      }
    }
    
    .form-label {
      position: absolute;
      left: var(--space-4);
      top: 50%;
      transform: translateY(-50%);
      color: rgba(255, 255, 255, 0.7);
      font-size: var(--font-size-base);
      pointer-events: none;
      transition: all var(--duration-fast) var(--ease-in-out);
    }
    
    .password-toggle {
      position: absolute;
      right: var(--space-4);
      top: 50%;
      transform: translateY(-50%);
      background: none;
      border: none;
      color: rgba(255, 255, 255, 0.7);
      cursor: pointer;
      padding: var(--space-2);
      
      &:hover {
        color: white;
      }
    }
  }
  
  .form-options {
    margin-bottom: var(--space-6);
    
    .checkbox-wrapper {
      display: flex;
      align-items: center;
      color: rgba(255, 255, 255, 0.8);
      cursor: pointer;
      font-size: var(--font-size-sm);
      
      .checkbox-input {
        width: 18px;
        height: 18px;
        margin-right: var(--space-2);
        accent-color: white;
      }
    }
  }
  
  .submit-button {
    width: 100%;
    height: 56px;
    background: rgba(255, 255, 255, 0.2);
    border: 1px solid rgba(255, 255, 255, 0.3);
    backdrop-filter: blur(10px);
    border-radius: var(--radius-full);
    font-size: var(--font-size-base);
    font-weight: var(--font-weight-semibold);
    color: white;
    cursor: pointer;
    transition: all var(--duration-normal) var(--ease-in-out);
    position: relative;
    overflow: hidden;
    
    &::before {
      content: '';
      position: absolute;
      inset: 0;
      background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.2), transparent);
      transform: translateX(-100%);
      transition: transform 0.6s;
    }
    
    &:hover:not(:disabled) {
      background: rgba(255, 255, 255, 0.3);
      transform: translateY(-1px);
      box-shadow: 0 10px 30px rgba(0, 0, 0, 0.2);
      
      &::before {
        transform: translateX(100%);
      }
    }
    
    &:active:not(:disabled) {
      transform: translateY(0);
    }
    
    &:disabled {
      opacity: 0.7;
      cursor: not-allowed;
    }
    
    .loading-spinner {
      display: flex;
      align-items: center;
      justify-content: center;
      
      .spinner {
        width: 20px;
        height: 20px;
        border: 2px solid rgba(255, 255, 255, 0.3);
        border-top-color: white;
        border-radius: 50%;
        animation: spin 0.8s linear infinite;
      }
    }
  }
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* 快速登录 */
.quick-login {
  margin-top: var(--space-8);
  text-align: center;
  animation: fadeInUp 0.8s ease-out 0.4s both;
  
  .quick-login-title {
    font-size: var(--font-size-sm);
    color: rgba(255, 255, 255, 0.7);
    margin-bottom: var(--space-4);
  }
  
  .quick-login-buttons {
    display: flex;
    gap: var(--space-3);
    justify-content: center;
    
    .quick-login-btn {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: var(--space-2);
      padding: var(--space-3) var(--space-4);
      background: rgba(255, 255, 255, 0.1);
      border: 1px solid rgba(255, 255, 255, 0.2);
      border-radius: var(--radius-lg);
      color: white;
      cursor: pointer;
      transition: all var(--duration-fast) var(--ease-in-out);
      
      .role-icon {
        width: 40px;
        height: 40px;
        display: flex;
        align-items: center;
        justify-content: center;
        background: rgba(255, 255, 255, 0.2);
        border-radius: var(--radius-full);
      }
      
      .role-name {
        font-size: var(--font-size-sm);
        font-weight: var(--font-weight-medium);
      }
      
      &:hover {
        background: rgba(255, 255, 255, 0.2);
        transform: translateY(-2px);
      }
      
      &.role-operator {
        border-color: rgba(52, 199, 89, 0.5);
        &:hover { background: rgba(52, 199, 89, 0.2); }
      }
      
      &.role-engineer {
        border-color: rgba(255, 149, 0, 0.5);
        &:hover { background: rgba(255, 149, 0, 0.2); }
      }
      
      &.role-admin {
        border-color: rgba(255, 59, 48, 0.5);
        &:hover { background: rgba(255, 59, 48, 0.2); }
      }
    }
  }
}

/* 语言切换 */
.language-switcher {
  position: absolute;
  top: var(--space-6);
  right: var(--space-6);
  display: flex;
  gap: 2px;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(10px);
  border-radius: var(--radius-full);
  padding: 2px;
  
  .lang-btn {
    padding: var(--space-2) var(--space-4);
    background: transparent;
    border: none;
    border-radius: var(--radius-full);
    color: rgba(255, 255, 255, 0.7);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    cursor: pointer;
    transition: all var(--duration-fast) var(--ease-in-out);
    
    &:hover {
      color: white;
    }
    
    &.active {
      background: rgba(255, 255, 255, 0.2);
      color: white;
    }
  }
}

/* 响应式 */
@media (max-width: 640px) {
  .logo-section {
    .brand-name {
      font-size: var(--font-size-3xl);
    }
  }
  
  .login-form {
    max-width: 100%;
  }
  
  .quick-login-buttons {
    flex-direction: column;
    width: 100%;
    
    .quick-login-btn {
      flex-direction: row;
      width: 100%;
    }
  }
}
</style>