#!/usr/bin/env node
/**
 * InfluxDB Writer for VoltageEMS
 * 直接将模拟数据写入 InfluxDB 2.x
 */

const { InfluxDB, Point } = require('@influxdata/influxdb-client');

// InfluxDB 配置
const url = process.env.INFLUX_URL || 'http://localhost:8086';
const token = process.env.INFLUX_TOKEN || 'voltage-super-secret-auth-token';
const org = process.env.INFLUX_ORG || 'voltageems';
const bucket = process.env.INFLUX_BUCKET || 'history';

// 创建 InfluxDB 客户端
const influxDB = new InfluxDB({ url, token });
const writeApi = influxDB.getWriteApi(org, bucket, 'ms');

// 设备列表
const devices = [
  { id: 'sensor_01', name: 'Temperature Sensor 01' },
  { id: 'sensor_02', name: 'Temperature Sensor 02' },
  { id: 'meter_01', name: 'Power Meter 01' },
  { id: 'meter_02', name: 'Power Meter 02' }
];

// 生成温度数据
function generateTemperature(baseTemp = 25) {
  return baseTemp + (Math.random() - 0.5) * 10;
}

// 生成电力数据
function generatePowerData() {
  const voltage = 220 + (Math.random() - 0.5) * 20;
  const current = 10 + Math.random() * 20;
  const power = voltage * current;
  const powerFactor = 0.85 + Math.random() * 0.15;
  
  return {
    voltage: voltage,
    current: current,
    power: power,
    powerFactor: powerFactor,
    frequency: 50 + (Math.random() - 0.5) * 0.2
  };
}

// 写入数据到 InfluxDB
function writeData() {
  const timestamp = new Date();
  
  // 温度数据
  devices.filter(d => d.id.startsWith('sensor')).forEach(device => {
    const temperature = generateTemperature();
    const point = new Point('temperature')
      .tag('device_id', device.id)
      .tag('device_name', device.name)
      .floatField('value', temperature)
      .timestamp(timestamp);
    
    writeApi.writePoint(point);
    console.log(`[TEMP] ${device.name}: ${temperature.toFixed(2)}°C`);
  });
  
  // 电力数据
  devices.filter(d => d.id.startsWith('meter')).forEach(device => {
    const powerData = generatePowerData();
    
    // 电压数据点
    const voltagePoint = new Point('voltage')
      .tag('device_id', device.id)
      .tag('device_name', device.name)
      .floatField('value', powerData.voltage)
      .timestamp(timestamp);
    
    // 功率数据点
    const powerPoint = new Point('power')
      .tag('device_id', device.id)
      .tag('device_name', device.name)
      .floatField('voltage', powerData.voltage)
      .floatField('current', powerData.current)
      .floatField('power', powerData.power)
      .floatField('powerFactor', powerData.powerFactor)
      .floatField('frequency', powerData.frequency)
      .timestamp(timestamp);
    
    writeApi.writePoint(voltagePoint);
    writeApi.writePoint(powerPoint);
    
    console.log(`[POWER] ${device.name}: ${powerData.voltage.toFixed(2)}V, ${powerData.current.toFixed(2)}A, ${powerData.power.toFixed(2)}W`);
  });
  
  // 强制刷新数据
  writeApi.flush();
  console.log('---');
}

// 主循环
async function main() {
  console.log('Starting to write data to InfluxDB...');
  console.log('URL:', url);
  console.log('Bucket:', bucket);
  console.log('');
  
  // 每秒写入一次数据
  setInterval(() => {
    try {
      writeData();
    } catch (error) {
      console.error('Write error:', error);
    }
  }, 1000);
}

// 优雅退出
process.on('SIGINT', async () => {
  console.log('\nShutting down...');
  try {
    await writeApi.close();
  } catch (error) {
    console.error('Close error:', error);
  }
  process.exit(0);
});

// 启动
main().catch(console.error);