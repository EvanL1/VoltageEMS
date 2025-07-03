const { defineConfig } = require('@vue/cli-service')

module.exports = defineConfig({
  transpileDependencies: true,
  devServer: {
    port: 8082,
    open: false,
    allowedHosts: 'all',
    headers: {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, PATCH, OPTIONS',
      'Access-Control-Allow-Headers': 'X-Requested-With, content-type, Authorization'
    },
    client: {
      webSocketURL: 'auto://0.0.0.0:0/ws'
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
        onProxyReq: function(proxyReq, req, res) {
          // 保持 /grafana 路径，因为 Grafana 配置了子路径
          proxyReq.setHeader('Origin', 'http://localhost:3050');
        }
      }
    }
  },
  configureWebpack: {
    cache: {
      type: 'filesystem'
    },
    optimization: {
      splitChunks: {
        chunks: 'all'
      }
    }
  }
}) 