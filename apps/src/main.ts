import './assets/main.css'

import { createApp, type DirectiveBinding } from 'vue'
import { createPinia } from 'pinia'
import piniaPluginPersistedstate from 'pinia-plugin-persistedstate'
import ElementPlus from 'element-plus'

import App from './App.vue'
import router from './router'
import { permissionDirective } from './utils/directives'
import { initResponsive } from './utils/responsive'

const app = createApp(App)
const pinia = createPinia()

// 配置Pinia持久化插件
pinia.use(piniaPluginPersistedstate)

app.use(pinia)
app.use(router)
app.use(ElementPlus)

// 注册自定义指令 v-permission
app.directive('permission', permissionDirective)

// 初始化响应式配置
initResponsive()

app.mount('#app')
