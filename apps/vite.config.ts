import { fileURLToPath, URL } from 'node:url'

import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import vueDevTools from 'vite-plugin-vue-devtools'
import AutoImport from 'unplugin-auto-import/vite'
import Components from 'unplugin-vue-components/vite'
import { ElementPlusResolver } from 'unplugin-vue-components/resolvers'
import viteCompression from 'vite-plugin-compression'
import { visualizer } from 'rollup-plugin-visualizer'
import autoprefixer from 'autoprefixer'
// @ts-ignore
import pxtorem from 'postcss-pxtorem'

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    vue(),
    // vueDevTools(), // 暂时关闭 Vue DevTools 调试工具
    AutoImport({
      imports: ['vue', 'vue-router', 'pinia'],
      resolvers: [ElementPlusResolver()],
      dts: true,
    }),
    Components({
      resolvers: [ElementPlusResolver()],
      dts: true,
    }),
    // Gzip压缩插件（不压缩图片文件）
    viteCompression({
      verbose: true,
      disable: false,
      threshold: 10240, // 10KB以上才压缩
      algorithm: 'gzip',
      ext: '.gz',
      // 只压缩非图片文件
      filter: (file) => {
        // 不压缩常见图片格式
        return !/\.(png|jpe?g|gif|svg|webp|avif|bmp|ico)$/i.test(file)
      },
    }),
    // 打包分析插件 - 生成分析报告到 dist/stats.html
    visualizer({
      open: true, // 构建后自动打开分析报告
      gzipSize: true,
      brotliSize: true,
      filename: 'dist/stats.html', // 生成分析报告文件
      emitFile: true,
    }),
  ],
  server: {
    host: '0.0.0.0', // 允许外部访问
    port: 5173, // 指定端口号
    open: true, // 自动打开浏览器
    proxy: {
      '/api': {
        target: 'http://192.168.30.62:6005',
        changeOrigin: true,
        // rewrite: (path) => path.replace(/^\/api/, ''),
      },
      '/hisApi': {
        target: 'http://192.168.30.62:6004',
        changeOrigin: true,
        // rewrite: (path) => path.replace(/^\/api/, ''),
      },
      '/alarmApi': {
        target: 'http://192.168.30.62:6002',
        changeOrigin: true,
        // rewrite: (path) => path.replace(/^\/api/, ''),
      },
      '/netApi': {
        target: 'http://192.168.30.62:6006',
        changeOrigin: true,
        // rewrite: (path) => path.replace(/^\/api/, ''),
      },
      '/comApi': {
        target: 'http://192.168.30.62:6001',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/comApi/, ''),
      },
      '/ruleApi': {
        target: 'http://192.168.30.62:6002',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/ruleApi/, ''),
      },
      '/modApi': {
        target: 'http://192.168.30.62:6002',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/modApi/, ''),
      },
      // WebSocket 代理：将前端的 /ws 转发到本机 127.0.0.1:6005
      '/ws': {
        target: 'ws://192.168.30.62:6005',
        changeOrigin: true,
        ws: true,
        // 不做 path 重写，保持 /ws 直通后端 /ws
        // 如需后端根路径接收，可启用以下重写：
        // rewrite: (path) => path.replace(/^\/ws/, ''),
      },
    },
  },
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  css: {
    preprocessorOptions: {
      scss: {
        // 如果需要全局 SCSS 变量，可以在这里添加
        // additionalData: `@use "@/assets/styles/variables.scss" as *;`,
      },
    },
    postcss: {
      plugins: [
        // 自动添加浏览器前缀
        autoprefixer({
          overrideBrowserslist: [
            'last 2 versions',
            '> 1%',
            'iOS 7',
            'last 3 iOS versions',
            'Android >= 4.0',
          ],
          flexbox: 'no-2009',
        }),
        // // px 转 rem
        pxtorem({
          rootValue: 100, // 根元素字体大小，与HTML中的设置保持一致
          unitPrecision: 5, // 转换后的小数点位数
          propList: ['*'], // 需要转换的属性，*表示所有属性
          selectorBlackList: [
            /^\.no-rem/, // 不转换的类名
          ],
          replace: true, // 是否替换原来的值
          mediaQuery: false, // 是否转换媒体查询中的 px
          minPixelValue: 1, // 小于这个值的 px 不转换
          exclude: /EnergyBg\.vue/i, // 排除EnergyBg.vue 文件
        }),
      ],
    },
  },
  // 优化依赖预构建
  optimizeDeps: {
    include: ['vue', 'vue-router', 'pinia', 'element-plus', 'axios', 'echarts'],
  },
  // 构建配置
  build: {
    // 确保CSS正确处理
    cssCodeSplit: true,
    // 生成sourcemap用于调试
    sourcemap: false,
    // chunk 大小警告限制（KB）
    chunkSizeWarningLimit: 1000,
    // 压缩配置
    minify: 'terser',
    terserOptions: {
      compress: {
        drop_console: true, // 移除console
        drop_debugger: true, // 移除debugger
      },
    },
    // Rollup 配置 - 代码分割优化
    rollupOptions: {
      output: {
        // 手动分割代码块
        manualChunks: (id) => {
          // node_modules 中的依赖单独打包
          if (id.includes('node_modules')) {
            // Vue 核心库单独打包
            if (id.includes('vue') && !id.includes('vue-router') && !id.includes('pinia')) {
              return 'vue-core'
            }
            // Vue Router 单独打包
            if (id.includes('vue-router')) {
              return 'vue-router'
            }
            // Pinia 单独打包
            if (id.includes('pinia')) {
              return 'pinia'
            }
            // Element Plus 单独打包（体积较大）
            if (id.includes('element-plus')) {
              return 'element-plus'
            }
            // ECharts 单独打包（体积很大，约 700KB）
            if (id.includes('echarts')) {
              return 'echarts'
            }
            // Vue Flow 相关包单独打包
            if (id.includes('@vue-flow')) {
              return 'vue-flow'
            }
            // 其他 node_modules 依赖
            return 'vendor'
          }
        },
        // 输出文件命名规则
        chunkFileNames: 'js/[name]-[hash].js',
        entryFileNames: 'js/[name]-[hash].js',
        assetFileNames: (assetInfo) => {
          // 图片资源
          if (/\.(png|jpe?g|gif|svg|webp|avif|bmp|ico)$/i.test(assetInfo.name || '')) {
            return 'images/[name]-[hash][extname]'
          }
          // 字体资源
          if (/\.(woff2?|eot|ttf|otf)$/i.test(assetInfo.name || '')) {
            return 'fonts/[name]-[hash][extname]'
          }
          // CSS 文件
          if (/\.css$/i.test(assetInfo.name || '')) {
            return 'css/[name]-[hash][extname]'
          }
          // 其他资源
          return 'assets/[name]-[hash][extname]'
        },
      },
    },
  },
})
