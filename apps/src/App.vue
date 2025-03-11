<script setup lang="ts">
import en from 'element-plus/es/locale/lang/en'
const locale = en
</script>

<template>
  <el-config-provider :locale="locale">
    <router-view />
  </el-config-provider>
</template>

<style>
body {
  margin: 0;
  padding: 0;
  overflow: hidden;
  box-sizing: border-box;
}

#app {
  height: 100vh;
  width: 100vw;
  margin: 0;
  padding: 0;
  overflow: hidden;
  position: relative;
}
</style>
