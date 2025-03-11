const { defineConfig } = require('@vue/cli-service')

module.exports = defineConfig({
  transpileDependencies: true,
  devServer: {
    port: 8080,
    open: true,
    proxy: {
      '/api': {
        target: 'http://localhost:3001',
        changeOrigin: true
      },
      '/grafana': {
        target: 'http://localhost:3000',
        changeOrigin: true,
        pathRewrite: {
          '^/grafana': ''
        }
      }
    }
  }
}) 