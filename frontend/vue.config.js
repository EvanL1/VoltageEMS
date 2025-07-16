const { defineConfig } = require('@vue/cli-service')

module.exports = defineConfig({
  transpileDependencies: true,
  
  // 生产环境构建优化
  productionSourceMap: process.env.NODE_ENV === 'development',
  
  // 开发服务器配置
  devServer: {
    port: 8083,
    open: false,
    hot: true,
    compress: true,
    allowedHosts: 'all',
    headers: {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, PATCH, OPTIONS',
      'Access-Control-Allow-Headers': 'X-Requested-With, content-type, Authorization'
    },
    client: {
      webSocketURL: 'auto://0.0.0.0:0/ws',
      overlay: {
        warnings: false,
        errors: true
      }
    },
    proxy: {
      '/api': {
        target: 'http://localhost:3001',
        changeOrigin: true,
        secure: false
      },
      '/grafana': {
        target: 'http://localhost:3050',
        changeOrigin: true,
        secure: false,
        ws: true,
        onProxyReq: function(proxyReq) {
          // 保持 /grafana 路径，因为 Grafana 配置了子路径
          proxyReq.setHeader('Origin', 'http://localhost:3050');
        }
      }
    }
  },
  
  // Webpack 配置
  configureWebpack: {
    // 缓存配置，提高构建速度
    cache: {
      type: 'filesystem',
      buildDependencies: {
        config: [__filename]
      }
    },
    
    // 性能优化
    performance: {
      hints: false
    },
    
    // 优化配置
    optimization: {
      splitChunks: {
        chunks: 'all',
        cacheGroups: {
          libs: {
            name: 'chunk-libs',
            test: /[\\/]node_modules[\\/]/,
            priority: 10,
            chunks: 'initial'
          },
          elementUI: {
            name: 'chunk-elementPlus',
            priority: 20,
            test: /[\\/]node_modules[\\/]_?element-plus(.*)/
          },
          echarts: {
            name: 'chunk-echarts',
            priority: 20,
            test: /[\\/]node_modules[\\/]_?echarts(.*)/
          },
          commons: {
            name: 'chunk-commons',
            test: /[\\/]src[\\/]components/,
            minChunks: 3,
            priority: 5,
            reuseExistingChunk: true
          }
        }
      },
      runtimeChunk: 'single'
    }
  },
  
  // CSS 相关配置
  css: {
    extract: process.env.NODE_ENV === 'production',
    sourceMap: false,
    loaderOptions: {
      scss: {
        additionalData: `@import "@/styles/design-tokens.scss";`,
        sassOptions: {
          outputStyle: 'compressed'
        }
      }
    }
  },
  
  // 链式操作
  chainWebpack: config => {
    // 开发环境禁用一些不必要的插件以提高构建速度
    if (process.env.NODE_ENV === 'development') {
      config.plugins.delete('preload')
      config.plugins.delete('prefetch')
    }
    
    // 配置别名
    config.resolve.alias
      .set('@', require('path').resolve(__dirname, 'src'))
    
    // 优化图片处理
    config.module
      .rule('images')
      .test(/\.(png|jpe?g|gif|webp)(\?.*)?$/)
      .use('image-webpack-loader')
      .loader('image-webpack-loader')
      .options({
        bypassOnDebug: true
      })
      .end()
  }
}) 