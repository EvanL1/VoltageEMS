import { createPinia } from 'pinia'

const pinia = createPinia()

export default pinia

// 导出各个store
export * from './user'
export * from './config'
export * from './realtime'
export * from './alarm'