#!/usr/bin/env node
/**
 * Mock Data Generator for VoltageEMS
 * 生成模拟数据到 Redis，供 hissrv 写入 InfluxDB
 */

const redis = require('redis');

// 创建 Redis 客户端
const client = redis.createClient({
  host: '127.0.0.1',
  port: 6379
});

// 错误处理
client.on('error', (err) => {
  console.error('Redis 错误:', err);
});

// 连接成功
client.on('connect', () => {
  console.log('已连接到 Redis');
  console.log('开始生成模拟数据...');
  console.log('按 Ctrl+C 停止');
});

// 设备列表
const devices = [
  { id: 'sensor_01', name: '1号温度传感器', type: 'temperature' },
  { id: 'sensor_02', name: '2号温度传感器', type: 'temperature' },
  { id: 'meter_01', name: '1号电表', type: 'power' },
  { id: 'meter_02', name: '2号电表', type: 'power' }
];

// 生成温度数据
function generateTemperature(baseTemp = 25) {
  return baseTemp + (Math.random() - 0.5) * 10; // ±5度波动
}

// 生成电力数据
function generatePowerData() {
  const voltage = 220 + (Math.random() - 0.5) * 20; // 220V ±10V
  const current = 10 + Math.random() * 20; // 10-30A
  const power = voltage * current;
  const powerFactor = 0.85 + Math.random() * 0.15; // 0.85-1.0
  
  return {
    voltage: voltage.toFixed(2),
    current: current.toFixed(2),
    power: power.toFixed(2),
    powerFactor: powerFactor.toFixed(3),
    frequency: (50 + (Math.random() - 0.5) * 0.2).toFixed(2) // 50Hz ±0.1Hz
  };
}

// 发布数据到 Redis
async function publishData() {
  const timestamp = new Date().toISOString();
  
  // 温度传感器数据
  for (const device of devices.filter(d => d.type === 'temperature')) {
    const data = {
      device_id: device.id,
      device_name: device.name,
      value: generateTemperature(),
      unit: '°C',
      timestamp: timestamp
    };
    
    // 发布到 channel
    await client.publish(`temperature:${device.id}`, JSON.stringify(data));
    
    // 同时设置 key（供查询）
    await client.set(`temperature:${device.id}:current`, JSON.stringify(data), 'EX', 3600);
    
    console.log(`[温度] ${device.name}: ${data.value.toFixed(2)}°C`);
  }
  
  // 电表数据
  for (const device of devices.filter(d => d.type === 'power')) {
    const powerData = generatePowerData();
    const data = {
      device_id: device.id,
      device_name: device.name,
      ...powerData,
      timestamp: timestamp
    };
    
    // 发布到 channel
    await client.publish(`power:${device.id}`, JSON.stringify(data));
    
    // 设置电压数据
    await client.publish(`voltage:${device.id}`, JSON.stringify({
      device_id: device.id,
      device_name: device.name,
      value: powerData.voltage,
      unit: 'V',
      timestamp: timestamp
    }));
    
    // 同时设置 keys
    await client.set(`power:${device.id}:current`, JSON.stringify(data), 'EX', 3600);
    await client.set(`voltage:${device.id}:current`, powerData.voltage, 'EX', 3600);
    
    console.log(`[电力] ${device.name}: ${powerData.voltage}V, ${powerData.current}A, ${powerData.power}W`);
  }
  
  // 发布汇总数据
  const summaryData = {
    total_power: devices
      .filter(d => d.type === 'power')
      .reduce((sum, d) => {
        const power = generatePowerData().power;
        return sum + parseFloat(power);
      }, 0).toFixed(2),
    avg_temperature: devices
      .filter(d => d.type === 'temperature')
      .reduce((sum, d, i, arr) => {
        const temp = generateTemperature();
        return i === arr.length - 1 ? (sum + temp) / arr.length : sum + temp;
      }, 0).toFixed(2),
    timestamp: timestamp
  };
  
  await client.publish('data:summary', JSON.stringify(summaryData));
  console.log(`[汇总] 总功率: ${summaryData.total_power}W, 平均温度: ${summaryData.avg_temperature}°C`);
  console.log('---');
}

// 主循环
async function main() {
  // 连接 Redis
  await client.connect();
  
  // 每秒发布一次数据
  setInterval(async () => {
    try {
      await publishData();
    } catch (error) {
      console.error('发布数据错误:', error);
    }
  }, 1000);
}

// 优雅退出
process.on('SIGINT', async () => {
  console.log('\n正在关闭...');
  await client.quit();
  process.exit(0);
});

// 启动
main().catch(console.error);