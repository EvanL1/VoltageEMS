import { createI18n } from 'vue-i18n'
import en from '../locales/en'
import zh from '../locales/zh'

// 获取浏览器语言设置，但默认为英文
function getDefaultLocale() {
  // 始终默认为英文
  const defaultLocale = 'en'
  
  // 从 localStorage 获取用户选择的语言
  const savedLocale = localStorage.getItem('voltage-locale')
  if (savedLocale && ['en', 'zh'].includes(savedLocale)) {
    return savedLocale
  }
  
  return defaultLocale
}

const i18n = createI18n({
  legacy: false, // 使用 Composition API 模式
  locale: getDefaultLocale(), // 默认语言为英文
  fallbackLocale: 'en', // 回退语言为英文
  messages: {
    en,
    zh
  },
  globalInjection: true // 全局注入 $t 函数
})

export default i18n

// 导出切换语言的函数
export function setLocale(locale) {
  if (['en', 'zh'].includes(locale)) {
    i18n.global.locale.value = locale
    localStorage.setItem('voltage-locale', locale)
    document.documentElement.lang = locale
  }
}

// 导出获取当前语言的函数
export function getCurrentLocale() {
  return i18n.global.locale.value
}

// 导出支持的语言列表
export const supportedLocales = [
  { code: 'en', name: 'English', flag: '🇺🇸' },
  { code: 'zh', name: '中文', flag: '🇨🇳' }
]