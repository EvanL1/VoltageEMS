const express = require('express');
const cors = require('cors');
const morgan = require('morgan');
const path = require('path');
const fs = require('fs-extra');
const winston = require('winston');

// 创建 logger
const logger = winston.createLogger({
  level: 'info',
  format: winston.format.combine(
    winston.format.timestamp(),
    winston.format.json()
  ),
  transports: [
    new winston.transports.Console(),
    new winston.transports.File({ filename: 'api.log' })
  ]
});

// 创建 Express 应用
const app = express();
const PORT = process.env.PORT || 3001;

// 中间件
app.use(cors());
app.use(morgan('dev'));
app.use(express.json({ limit: '10mb' }));
app.use(express.urlencoded({ extended: true }));

// 配置文件根目录
const CONFIG_ROOT = path.resolve(__dirname, '../../config');

// 确保配置目录存在
fs.ensureDirSync(CONFIG_ROOT);

// 路由
app.get('/api/config/:service', async (req, res) => {
  try {
    const { service } = req.params;
    let configPath;
    let configContent;

    // 根据服务类型确定配置文件路径
    switch (service) {
      case 'modsrv':
        configPath = path.join(CONFIG_ROOT, 'modsrv', 'modsrv.toml');
        break;
      case 'netsrv':
        configPath = path.join(CONFIG_ROOT, 'netsrv', 'netsrv.json');
        break;
      case 'comsrv':
        // comsrv 可能有多个配置文件，这里简化处理
        configPath = path.join(CONFIG_ROOT, 'comsrv');
        break;
      case 'hissrv':
        configPath = path.join(CONFIG_ROOT, 'hissrv');
        break;
      case 'mosquitto':
        configPath = path.join(CONFIG_ROOT, 'mosquitto', 'mosquitto.conf');
        break;
      default:
        return res.status(404).json({ error: '未知的服务类型' });
    }

    // 检查文件是否存在
    if (!await fs.pathExists(configPath)) {
      return res.status(404).json({ error: '配置文件不存在' });
    }

    // 读取配置文件
    if (service === 'comsrv' || service === 'hissrv') {
      // 目录类型的配置，返回目录下所有文件
      const files = await fs.readdir(configPath);
      const configs = {};
      
      for (const file of files) {
        const filePath = path.join(configPath, file);
        const stat = await fs.stat(filePath);
        
        if (stat.isFile()) {
          configs[file] = await fs.readFile(filePath, 'utf8');
        }
      }
      
      configContent = configs;
    } else {
      // 单文件类型的配置
      configContent = await fs.readFile(configPath, 'utf8');
    }

    res.json(configContent);
  } catch (error) {
    logger.error('获取配置文件失败', { error: error.message, stack: error.stack });
    res.status(500).json({ error: '获取配置文件失败', details: error.message });
  }
});

app.post('/api/config/:service', async (req, res) => {
  try {
    const { service } = req.params;
    const { config } = req.body;
    let configPath;

    // 根据服务类型确定配置文件路径
    switch (service) {
      case 'modsrv':
        configPath = path.join(CONFIG_ROOT, 'modsrv', 'modsrv.toml');
        break;
      case 'netsrv':
        configPath = path.join(CONFIG_ROOT, 'netsrv', 'netsrv.json');
        break;
      case 'comsrv':
        configPath = path.join(CONFIG_ROOT, 'comsrv');
        break;
      case 'hissrv':
        configPath = path.join(CONFIG_ROOT, 'hissrv');
        break;
      case 'mosquitto':
        configPath = path.join(CONFIG_ROOT, 'mosquitto', 'mosquitto.conf');
        break;
      default:
        return res.status(404).json({ error: '未知的服务类型' });
    }

    // 确保目录存在
    await fs.ensureDir(path.dirname(configPath));

    // 写入配置文件
    if (service === 'comsrv' || service === 'hissrv') {
      // 目录类型的配置，写入多个文件
      for (const [file, content] of Object.entries(config)) {
        const filePath = path.join(configPath, file);
        await fs.writeFile(filePath, content);
      }
    } else {
      // 单文件类型的配置
      await fs.writeFile(configPath, config);
    }

    res.json({ success: true, message: '配置保存成功' });
  } catch (error) {
    logger.error('保存配置文件失败', { error: error.message, stack: error.stack });
    res.status(500).json({ error: '保存配置文件失败', details: error.message });
  }
});

// 启动服务器
app.listen(PORT, () => {
  logger.info(`API 服务已启动，监听端口 ${PORT}`);
  console.log(`API 服务已启动，监听端口 ${PORT}`);
}); 