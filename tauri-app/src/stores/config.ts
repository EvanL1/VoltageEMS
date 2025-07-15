import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useConfigStore = defineStore('config', () => {
  // Theme
  const theme = ref<'light' | 'dark'>('light')
  const primaryColor = ref('#409EFF')
  
  // Language
  const language = ref<'en' | 'zh'>('en')
  
  // Layout
  const sidebarCollapsed = ref(false)
  const showBreadcrumb = ref(true)
  const showFooter = ref(true)
  
  // API Configuration
  const apiBaseUrl = ref(import.meta.env.VITE_API_BASE_URL || 'http://localhost:8080')
  const wsUrl = ref(import.meta.env.VITE_WS_URL || 'ws://localhost:8080/ws')
  
  // Actions
  function setTheme(newTheme: 'light' | 'dark') {
    theme.value = newTheme
    document.documentElement.setAttribute('data-theme', newTheme)
    localStorage.setItem('theme', newTheme)
  }
  
  function setLanguage(newLang: 'en' | 'zh') {
    language.value = newLang
    localStorage.setItem('language', newLang)
  }
  
  function toggleSidebar() {
    sidebarCollapsed.value = !sidebarCollapsed.value
    localStorage.setItem('sidebarCollapsed', String(sidebarCollapsed.value))
  }
  
  // Initialize from localStorage
  function initializeConfig() {
    const savedTheme = localStorage.getItem('theme') as 'light' | 'dark'
    if (savedTheme) {
      setTheme(savedTheme)
    }
    
    const savedLang = localStorage.getItem('language') as 'en' | 'zh'
    if (savedLang) {
      setLanguage(savedLang)
    }
    
    const savedSidebar = localStorage.getItem('sidebarCollapsed')
    if (savedSidebar !== null) {
      sidebarCollapsed.value = savedSidebar === 'true'
    }
  }
  
  return {
    // State
    theme,
    primaryColor,
    language,
    sidebarCollapsed,
    showBreadcrumb,
    showFooter,
    apiBaseUrl,
    wsUrl,
    
    // Actions
    setTheme,
    setLanguage,
    toggleSidebar,
    initializeConfig
  }
})