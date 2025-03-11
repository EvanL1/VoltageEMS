// 响应式配置工具

// 设计稿尺寸（基准尺寸）
const DESIGN_WIDTH = 1920
const DESIGN_HEIGHT = 1080

// 基准字体大小（设计稿在1920px宽度下的字体大小）
const BASE_FONT_SIZE = 100 // 100px = 1rem

/**
 * 设置根字体大小，实现响应式布局
 */
export function setRem(): void {
  const html = document.documentElement
  const screenWidth = window.innerWidth

  const scale = screenWidth / DESIGN_WIDTH
  // 设置根字体大小
  const fontSize = BASE_FONT_SIZE * scale
  html.style.fontSize = fontSize + 'px'
}

/**
 * 初始化响应式配置
 */
export function initResponsive(): void {
  // 初始化
  setRem()

  // 监听窗口大小变化
  window.addEventListener('resize', function () {
    // 防抖处理
    clearTimeout((window as any).remResizeTimer)
    ;(window as any).remResizeTimer = setTimeout(setRem, 100)
  })
}

/**
 * 将设计稿中的px值转换为当前响应式下的px值
 * @param designPx 设计稿中的px值
 * @returns 当前响应式下的px值
 */
export function pxToResponsive(designPx: number): number {
  const screenWidth = window.innerWidth
  const scale = screenWidth / DESIGN_WIDTH
  return designPx * scale
}

/**
 * 将设计稿中的px值转换为rem值
 * @param designPx 设计稿中的px值
 * @returns 对应的rem值
 */
export function pxToRem(designPx: number): string {
  const screenWidth = window.innerWidth
  const scale = screenWidth / DESIGN_WIDTH
  const currentBaseFontSize = BASE_FONT_SIZE * scale
  const remValue = designPx / currentBaseFontSize
  return `${remValue}rem`
}

/**
 * 获取当前屏幕的缩放比例
 * @returns 当前屏幕相对于设计稿的缩放比例
 */
export function getCurrentScale(): number {
  const screenWidth = window.innerWidth
  return screenWidth / DESIGN_WIDTH
}

/**
 * 获取当前根字体大小
 * @returns 当前根字体大小（px）
 */
export function getCurrentFontSize(): number {
  const html = document.documentElement
  const fontSize = parseFloat(getComputedStyle(html).fontSize)
  return fontSize || BASE_FONT_SIZE
}
