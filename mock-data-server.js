const express = require('express');
const app = express();
const cors = require('cors');

// 启用 CORS
app.use(cors());
app.use(express.json());

// 模拟设备数据
const devices = ['device_001', 'device_002', 'device_003'];
const metrics = ['temperature', 'voltage', 'current', 'power'];

// SimpleJSON 数据源接口实现

// 测试连接
app.get('/', (req, res) => {
  res.send('OK');
});

// 搜索接口 - 返回可用的指标
app.post('/search', (req, res) => {
  const targets = [];
  devices.forEach(device => {
    metrics.forEach(metric => {
      targets.push(`${device}.${metric}`);
    });
  });
  res.json(targets);
});

// 查询接口 - 返回时间序列数据
app.post('/query', (req, res) => {
  const { targets, range } = req.body;
  const from = new Date(range.from);
  const to = new Date(range.to);
  
  const response = targets.map(target => {
    const { target: targetName } = target;
    const [device, metric] = targetName.split('.');
    
    // 生成模拟数据点
    const datapoints = [];
    const interval = 60000; // 1分钟间隔
    
    for (let time = from.getTime(); time <= to.getTime(); time += interval) {
      let value;
      const baseValue = {
        temperature: 25 + Math.random() * 10,
        voltage: 220 + Math.random() * 20,
        current: 10 + Math.random() * 5,
        power: 2000 + Math.random() * 1000
      };
      
      value = baseValue[metric] || Math.random() * 100;
      
      // 添加一些周期性变化
      const hour = new Date(time).getHours();
      if (metric === 'power') {
        // 功率在白天更高
        value += (hour >= 8 && hour <= 18) ? 500 : -200;
      }
      
      datapoints.push([value, time]);
    }
    
    return {
      target: targetName,
      datapoints
    };
  });
  
  res.json(response);
});

// 标签键接口
app.post('/tag-keys', (req, res) => {
  res.json([
    { type: 'string', text: 'device' },
    { type: 'string', text: 'metric' }
  ]);
});

// 标签值接口
app.post('/tag-values', (req, res) => {
  const { key } = req.body;
  
  if (key === 'device') {
    res.json(devices.map(d => ({ text: d })));
  } else if (key === 'metric') {
    res.json(metrics.map(m => ({ text: m })));
  } else {
    res.json([]);
  }
});

const PORT = 3001;
app.listen(PORT, () => {
  console.log(`Mock data server running on http://localhost:${PORT}`);
  console.log('Available metrics:');
  devices.forEach(device => {
    metrics.forEach(metric => {
      console.log(`  - ${device}.${metric}`);
    });
  });
});