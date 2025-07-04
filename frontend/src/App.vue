<template>
  <router-view />
</template>

<script setup>
import { onMounted } from 'vue'
import { useUserStore } from '@/stores/user'

const userStore = useUserStore()

onMounted(async () => {
  // 如果有token但没有用户信息，尝试获取用户信息
  if (userStore.isLoggedIn && !userStore.userInfo) {
    try {
      await userStore.fetchUserInfo()
    } catch (error) {
      console.error('Failed to fetch user info:', error)
    }
  }
})
</script>

<style lang="scss">
/* Import Design System */
@import '@/styles/global.scss';
@import '@/styles/components/index.scss';

/* App Specific Styles */
#app {
  height: 100%;
  background: var(--color-background);
}
</style>