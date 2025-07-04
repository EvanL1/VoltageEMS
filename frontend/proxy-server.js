const http = require('http');
const httpProxy = require('http-proxy');

// 创建代理服务器
const proxy = httpProxy.createProxyServer({
  target: 'http://localhost:8082',
  changeOrigin: true,
  ws: true, // 支持WebSocket
  xfwd: true
});

// 处理代理错误
proxy.on('error', (err, req, res) => {
  console.error('Proxy error:', err);
  if (res.writeHead) {
    res.writeHead(500, {
      'Content-Type': 'text/plain'
    });
    res.end('Proxy error: ' + err.message);
  }
});

// 创建服务器
const server = http.createServer((req, res) => {
  // 设置CORS headers
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, PUT, DELETE, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization');
  
  // 处理OPTIONS请求
  if (req.method === 'OPTIONS') {
    res.writeHead(200);
    res.end();
    return;
  }
  
  // 代理请求
  proxy.web(req, res);
});

// 处理WebSocket连接
server.on('upgrade', (req, socket, head) => {
  proxy.ws(req, socket, head);
});

const PORT = 8083;
server.listen(PORT, () => {
  console.log(`Reverse proxy server running on http://localhost:${PORT}`);
  console.log(`Proxying to http://localhost:8082`);
});