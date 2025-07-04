import { createApp } from 'vue'
import App from './App.vue'
import router from './router'
import pinia from './stores'
import i18n from './i18n'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import * as ElementPlusIconsVue from '@element-plus/icons-vue'
import { permissionDirective } from './directives/permission'

// 导入新的设计系统（在 Element Plus 之后导入以覆盖样式）
import '@/styles/global.scss'
import '@/styles/components/index.scss'
import '@/styles/element-overrides.scss'

const app = createApp(App)

// 注册所有图标
for (const [key, component] of Object.entries(ElementPlusIconsVue)) {
  app.component(`ElIcon${key}`, component)
}

// 注册权限指令
app.directive('permission', permissionDirective)

app.use(pinia)
app.use(router)
app.use(ElementPlus)
app.use(i18n)

app.mount('#app') 