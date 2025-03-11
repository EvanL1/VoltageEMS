const express = require('express');
const cors = require('cors');
const mqtt = require('mqtt');
const moment = require('moment');

// Create Express app
const app = express();
const port = process.env.PORT || 3000;

// Middleware
app.use(cors());
app.use(express.json());

// MQTT client (uncomment and configure when MQTT broker is available)
/*
const mqttClient = mqtt.connect('mqtt://mosquitto:1883', {
  clientId: `api-server-${Math.random().toString(16).slice(2, 8)}`,
  clean: true,
  connectTimeout: 4000,
  reconnectPeriod: 1000
});

mqttClient.on('connect', () => {
  console.log('Connected to MQTT broker');
  mqttClient.subscribe('voltage/+/data', (err) => {
    if (!err) {
      console.log('Subscribed to device data topics');
    }
  });
});

mqttClient.on('message', (topic, message) => {
  // Handle incoming messages
  console.log(`Received message from ${topic}: ${message.toString()}`);
});
*/

// Simulated data for demonstration
const systemData = {
  power: {
    charge: 0,
    discharge: 2.3
  },
  soc: 88,
  temperature: {
    current: 25,
    min: 17,
    max: 30
  },
  devices: {
    pv: { power: 15.4 },
    converter: { efficiency: 98.5 },
    battery: { soc: 88 },
    load: { power: 18.7 },
    grid: { status: 'Connected', power: 1.2 }
  },
  alerts: [
    {
      id: 1,
      time: '2025-03-15 09:23:45',
      type: 'WARNING',
      message: 'Grid frequency fluctuation detected'
    },
    {
      id: 2,
      time: '2025-03-15 08:17:32',
      type: 'INFO',
      message: 'Battery cooling system activated'
    }
  ],
  history: {
    power: generateRandomData(24),
    soc: generateRandomData(24, 60, 90)
  }
};

// Generate random data points for charts
function generateRandomData(count, min = 0, max = 100) {
  const data = [];
  const now = moment();
  
  for (let i = 0; i < count; i++) {
    data.push({
      time: moment(now).subtract(i, 'hours').format('YYYY-MM-DD HH:mm:ss'),
      value: min + Math.random() * (max - min)
    });
  }
  
  return data.reverse();
}

// API Routes
app.get('/system/status', (req, res) => {
  res.json(systemData);
});

app.get('/system/alerts', (req, res) => {
  res.json(systemData.alerts);
});

app.get('/system/history', (req, res) => {
  res.json(systemData.history);
});

// Configuration related endpoints
app.get('/config/:service', (req, res) => {
  const { service } = req.params;
  res.json({
    service,
    status: 'active',
    configPath: `/etc/${service.toLowerCase()}`
  });
});

// Start the server
app.listen(port, () => {
  console.log(`API server running on port ${port}`);
}); 