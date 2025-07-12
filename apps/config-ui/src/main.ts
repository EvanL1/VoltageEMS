import { createApp } from "vue";
import { createPinia } from "pinia";
import ElementPlus from "element-plus";
import "element-plus/dist/index.css";
import "element-plus/theme-chalk/dark/css-vars.css";
import * as ElementPlusIconsVue from "@element-plus/icons-vue";

import App from "./App.vue";
import router from "./router";
import "./styles/theme.scss";
import "./styles/interactions.scss";
import "./styles/cyberpunk-effects.scss";
import { logger, info } from './utils/logger';

const app = createApp(App);
const pinia = createPinia();

// 初始化日志系统
info('VoltageEMS Config UI starting...', {
  version: '0.1.0',
  environment: import.meta.env.MODE,
  timestamp: new Date().toISOString()
});

// Vue 错误处理
app.config.errorHandler = (err, instance, info) => {
  logger.error('Vue error', {
    error: err?.toString(),
    component: instance?.$options?.name || 'Unknown',
    info,
    stack: err instanceof Error ? err.stack : undefined
  }, err instanceof Error ? err.stack : undefined);
};

// 注册所有 Element Plus 图标
for (const [key, component] of Object.entries(ElementPlusIconsVue)) {
  app.component(key, component);
}

app.use(pinia);
app.use(router);
app.use(ElementPlus);

app.mount("#app");

// 应用挂载后的日志
info('VoltageEMS Config UI mounted successfully');
