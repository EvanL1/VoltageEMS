<template>
  <div class="dashboard">
    <!-- Top Operation Bar -->
    <div class="dashboard-header">
      <div class="header-left">
        <span class="view-label">View by:</span>
        <el-select v-model="viewType" placeholder="Select View" size="default" style="width: 120px">
          <el-option label="Overview" value="overview" />
          <el-option label="Detail" value="detail" />
        </el-select>
      </div>
      <div class="header-right">
        <span>Last Update: {{ currentTime }}</span>
        <el-button type="text" @click="refreshData">
          <el-icon><el-icon-refresh /></el-icon>
        </el-button>
      </div>
    </div>

    <!-- Project Selector -->
    <div class="project-selector">
      <div class="selector-left">
        <span>Select Project:</span>
        <el-select v-model="selectedProject" placeholder="Select Project" size="default" style="width: 160px">
          <el-option label="Voltage-SYS" value="voltage-sys" />
          <el-option label="EMS Project" value="ems-project" />
        </el-select>
      </div>
      <div class="selector-right">
        <div class="project-info">
          <span class="project-label">Project Capacity:</span>
          <span class="project-value">100kW / 200kWh</span>
        </div>
        <el-button type="primary" size="small">Detail</el-button>
      </div>
    </div>

    <!-- Main Dashboard Content -->
    <div class="main-dashboard-container">
      <!-- Center: System Topology Diagram -->
      <el-card class="topology-card">
        <div class="topology-header">
          <h3>System Topology with Real-time Data</h3>
        </div>
        <div class="topology-diagram">
          <div class="topology-grid">
            <div class="topology-node pv">
              <div class="node-icon">
                <el-icon><el-icon-sunny /></el-icon>
              </div>
              <div class="node-label">PV</div>
              <div class="node-data">{{ pvData.power }} kW</div>
            </div>

            <div class="topology-connection h-line"></div>

            <div class="topology-node converter">
              <div class="node-icon">
                <el-icon><el-icon-set-up /></el-icon>
              </div>
              <div class="node-label">Converter</div>
              <div class="node-data">{{ converterData.efficiency }}% eff</div>
            </div>

            <div class="topology-connection h-line"></div>

            <div class="topology-node battery">
              <div class="node-icon">
                <el-icon><el-icon-lightning /></el-icon>
              </div>
              <div class="node-label">Battery</div>
              <div class="node-data">{{ socValue }}% SOC</div>
            </div>

            <div class="topology-connection down-line"></div>

            <div class="topology-node load">
              <div class="node-icon">
                <el-icon><el-icon-house /></el-icon>
              </div>
              <div class="node-label">Load</div>
              <div class="node-data">{{ loadData.power }} kW</div>
            </div>

            <div class="topology-connection h-line"></div>

            <div class="topology-node grid">
              <div class="node-icon">
                <el-icon><el-icon-grid /></el-icon>
              </div>
              <div class="node-label">Grid</div>
              <div class="node-data status">{{ gridData.status }}</div>
              <div class="node-data">{{ gridData.power }} kW</div>
            </div>
          </div>
        </div>
      </el-card>

      <div class="bottom-row">
        <!-- Bottom Left: Power and SOC -->
        <div class="bottom-left">
          <!-- Power Bar -->
          <el-card class="dashboard-card">
            <div class="bar-container">
              <h3 class="bar-title">Power</h3>
              <div class="power-bars">
                <div class="bar-item">
                  <div class="bar-label">Charge</div>
                  <div class="bar-track">
                    <div class="bar-fill charge" :style="{ width: getPowerPercentage(chargePower) + '%' }"></div>
                  </div>
                  <div class="bar-value">{{ chargePower }} kW</div>
                </div>
                <div class="bar-item">
                  <div class="bar-label">Discharge</div>
                  <div class="bar-track">
                    <div class="bar-fill discharge" :style="{ width: getPowerPercentage(dischargePower) + '%' }"></div>
                  </div>
                  <div class="bar-value">{{ dischargePower }} kW</div>
                </div>
              </div>
            </div>
          </el-card>

          <!-- SOC Bar -->
          <el-card class="dashboard-card">
            <div class="bar-container">
              <h3 class="bar-title">State of Charge</h3>
              <div class="soc-bar">
                <div class="soc-track">
                  <div 
                    class="soc-fill" 
                    :class="getSocClass(socValue)" 
                    :style="{ width: socValue + '%' }"
                  ></div>
                </div>
                <div class="soc-value">{{ socValue }}%</div>
              </div>
              <div class="soc-legend">
                <div class="legend-item">
                  <div class="legend-color green"></div>
                  <div class="legend-label">Normal</div>
                </div>
                <div class="legend-item">
                  <div class="legend-color yellow"></div>
                  <div class="legend-label">Low (≤ 10%)</div>
                </div>
                <div class="legend-item">
                  <div class="legend-color red"></div>
                  <div class="legend-label">Critical (≤ 5%)</div>
                </div>
              </div>
            </div>
          </el-card>
        </div>

        <!-- Bottom Right: Current Alerts -->
        <div class="bottom-right">
          <el-card class="alert-card">
            <template #header>
              <div class="alert-header">
                <span>Current Alerts</span>
                <el-tag type="danger" v-if="currentAlerts.length > 0">{{ currentAlerts.length }}</el-tag>
              </div>
            </template>
            <div v-if="currentAlerts.length > 0">
              <el-table :data="currentAlerts" stripe style="width: 100%">
                <el-table-column prop="time" label="Time" width="180" />
                <el-table-column prop="type" label="Type" width="100">
                  <template #default="scope">
                    <el-tag :type="getAlertType(scope.row.type)">{{ scope.row.type }}</el-tag>
                  </template>
                </el-table-column>
                <el-table-column prop="message" label="Message" show-overflow-tooltip />
              </el-table>
            </div>
            <div v-else class="no-alerts">
              <el-icon><el-icon-check /></el-icon>
              <span>No active alerts</span>
            </div>
          </el-card>
        </div>
      </div>

      <!-- System Status Table -->
      <el-card class="status-table-card">
        <div class="status-header">
          <div class="system-icon">
            <el-icon class="grid-icon"><el-icon-grid /></el-icon>
            <div class="system-type">Grid</div>
          </div>
          <div class="system-icon active">
            <el-icon class="container-icon"><el-icon-box /></el-icon>
            <div class="system-type">Container</div>
          </div>
          <div class="system-condition">
            <span class="condition-tag" :class="systemState.toLowerCase()">{{ systemState }}</span>
            <span class="condition-tag">System Condition</span>
          </div>
        </div>
        <el-table :data="systemData" style="width: 100%" size="large">
          <el-table-column prop="name" label="Parameter" width="220" />
          <el-table-column prop="value" label="Value" width="120" align="right" />
          <el-table-column prop="cumulateCharge" label="Cumulate Charge" align="center" />
          <el-table-column prop="cumulateDischarge" label="Cumulate Discharge" align="center" />
        </el-table>
      </el-card>

      <!-- Energy & SOC Trends -->
      <el-card class="charts-card">
        <div class="charts-header">
          <h3>Daily Energy & SOC Trends</h3>
        </div>
        <div class="charts-container">
          <div class="chart power-chart">
            <div class="chart-title">Power (24h)</div>
            <div class="chart-placeholder">
              <!-- In real implementation, replace with actual chart component -->
              <div class="chart-mock">
                <div class="chart-line" v-for="i in 10" :key="i" 
                  :style="{ height: Math.random() * 70 + 10 + '%', left: (i-1) * 10 + '%' }"></div>
              </div>
            </div>
          </div>
          <div class="chart soc-chart">
            <div class="chart-title">SOC (24h)</div>
            <div class="chart-placeholder">
              <!-- In real implementation, replace with actual chart component -->
              <div class="chart-mock">
                <div class="chart-line soc" v-for="i in 10" :key="i" 
                  :style="{ height: 60 + Math.random() * 30 + '%', left: (i-1) * 10 + '%' }"></div>
              </div>
            </div>
          </div>
        </div>
      </el-card>
    </div>
  </div>
</template>

<script>
export default {
  name: 'HomeView',
  data() {
    return {
      viewType: 'overview',
      selectedProject: 'voltage-sys',
      currentTime: this.formatDate(new Date()),
      // Power data
      chargePower: 0,
      dischargePower: 2.3,
      maxPower: 100, // 100kW system
      // SOC data
      socValue: 88,
      // System state
      systemState: 'Discharging',
      // Topology data
      pvData: {
        power: 15.4
      },
      converterData: {
        efficiency: 98.5
      },
      loadData: {
        power: 18.7
      },
      gridData: {
        status: 'Connected',
        power: 1.2
      },
      // Current alerts
      currentAlerts: [
        {
          time: '2025-03-15 09:23:45',
          type: 'WARNING',
          message: 'Grid frequency fluctuation detected'
        },
        {
          time: '2025-03-15 08:17:32',
          type: 'INFO',
          message: 'Battery cooling system activated'
        }
      ],
      // System data table
      systemData: [
        {
          name: 'Charge power',
          value: '0 kW',
          cumulateCharge: '',
          cumulateDischarge: ''
        },
        {
          name: 'Discharge power',
          value: '2.3 kW',
          cumulateCharge: '',
          cumulateDischarge: ''
        },
        {
          name: 'Charge current',
          value: '0 A',
          cumulateCharge: '2.47 MWh',
          cumulateDischarge: ''
        },
        {
          name: 'Discharge current',
          value: '3.3 A',
          cumulateCharge: '',
          cumulateDischarge: '2.18 MWh'
        }
      ]
    }
  },
  methods: {
    refreshData() {
      this.currentTime = this.formatDate(new Date());
      // In real application, would fetch updated data from the server
    },
    formatDate(date) {
      const year = date.getFullYear();
      const month = String(date.getMonth() + 1).padStart(2, '0');
      const day = String(date.getDate()).padStart(2, '0');
      const hours = String(date.getHours()).padStart(2, '0');
      const minutes = String(date.getMinutes()).padStart(2, '0');
      const seconds = String(date.getSeconds()).padStart(2, '0');
      
      return `${month}/${day}/${year} ${hours}:${minutes}:${seconds}`;
    },
    getPowerPercentage(power) {
      return Math.min((power / this.maxPower) * 100, 100);
    },
    getSocClass(soc) {
      if (soc <= 5) return 'critical';
      if (soc <= 10) return 'warning';
      return 'normal';
    },
    getAlertType(type) {
      const typeMap = {
        'INFO': 'info',
        'WARNING': 'warning',
        'ERROR': 'danger'
      };
      return typeMap[type] || 'info';
    }
  },
  mounted() {
    // Update data every minute
    this.refreshData();
    this.updateInterval = setInterval(this.refreshData, 60000);
  },
  beforeUnmount() {
    // Clear interval when component is destroyed
    clearInterval(this.updateInterval);
  }
}
</script>

<style scoped>
.dashboard {
  padding: 0;
}

/* Top Operation Bar */
.dashboard-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px;
  background-color: #f9f9f9;
  border-radius: 4px;
  margin-bottom: 15px;
}

.view-label {
  margin-right: 10px;
  font-weight: 500;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 10px;
  color: #666;
  font-size: 14px;
}

/* Project Selector */
.project-selector {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px;
  background-color: #f6f8e8;
  border-radius: 4px;
  margin-bottom: 20px;
}

.selector-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.selector-right {
  display: flex;
  align-items: center;
  gap: 15px;
}

.project-info {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
}

.project-label {
  font-size: 0.8rem;
  color: #666;
}

.project-value {
  font-weight: bold;
  color: #333;
}

/* Main Dashboard Container */
.main-dashboard-container {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

/* Bottom Row Layout */
.bottom-row {
  display: flex;
  gap: 20px;
  margin-bottom: 20px;
}

.bottom-left {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.bottom-right {
  flex: 1;
}

/* Dashboard Cards */
.dashboard-card {
  height: 180px;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* Bar Styles */
.bar-container {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 10px;
}

.bar-title {
  margin-bottom: 15px;
  text-align: center;
}

.power-bars {
  display: flex;
  flex-direction: column;
  height: 100%;
  justify-content: space-around;
}

.bar-item {
  display: flex;
  align-items: center;
  margin-bottom: 15px;
}

.bar-label {
  width: 100px;
  font-weight: bold;
}

.bar-track {
  flex-grow: 1;
  height: 25px;
  background-color: #eee;
  border-radius: 4px;
  overflow: hidden;
  margin: 0 10px;
}

.bar-fill {
  height: 100%;
  transition: width 0.5s ease-in-out;
}

.bar-fill.charge {
  background-color: #409EFF;
}

.bar-fill.discharge {
  background-color: #F56C6C;
}

.bar-value {
  width: 60px;
  text-align: right;
  font-weight: bold;
}

/* SOC Bar */
.soc-bar {
  display: flex;
  align-items: center;
  margin: 20px 0;
}

.soc-track {
  flex-grow: 1;
  height: 40px;
  background-color: #eee;
  border-radius: 4px;
  overflow: hidden;
  margin-right: 15px;
}

.soc-fill {
  height: 100%;
  transition: width 0.5s ease-in-out;
}

.soc-fill.normal {
  background-color: #67C23A;
}

.soc-fill.warning {
  background-color: #E6A23C;
}

.soc-fill.critical {
  background-color: #F56C6C;
}

.soc-value {
  width: 60px;
  text-align: right;
  font-weight: bold;
  font-size: 1.2rem;
}

.soc-legend {
  display: flex;
  justify-content: space-around;
  margin-top: 15px;
}

.legend-item {
  display: flex;
  align-items: center;
}

.legend-color {
  width: 15px;
  height: 15px;
  border-radius: 3px;
  margin-right: 5px;
}

.legend-color.green {
  background-color: #67C23A;
}

.legend-color.yellow {
  background-color: #E6A23C;
}

.legend-color.red {
  background-color: #F56C6C;
}

.legend-label {
  font-size: 0.8rem;
}

/* Alert Card */
.alert-card {
  height: 100%;
}

.alert-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.no-alerts {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 150px;
  color: #67C23A;
  font-size: 16px;
}

.no-alerts .el-icon {
  font-size: 40px;
  margin-bottom: 10px;
}

/* Topology Diagram */
.topology-card {
  width: 100%;
  min-height: 400px;
}

.topology-header {
  margin-bottom: 20px;
  text-align: center;
}

.topology-diagram {
  height: 350px;
  display: flex;
  justify-content: center;
  align-items: center;
}

.topology-grid {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr 1fr 1fr;
  grid-template-rows: 1fr 1fr 1fr;
  gap: 5px;
  width: 100%;
  height: 100%;
  padding: 20px;
}

.topology-node {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  border: 2px solid #ddd;
  border-radius: 8px;
  padding: 10px;
  background-color: #f9f9f9;
}

.node-icon {
  font-size: 2rem;
  margin-bottom: 5px;
}

.node-label {
  font-weight: bold;
  margin-bottom: 5px;
}

.node-data {
  font-size: 0.9rem;
  color: #555;
}

.node-data.status {
  color: #67C23A;
  font-weight: bold;
}

.topology-node.pv {
  grid-column: 1;
  grid-row: 1;
  color: #E6A23C;
}

.topology-node.converter {
  grid-column: 3;
  grid-row: 1;
  color: #409EFF;
}

.topology-node.battery {
  grid-column: 5;
  grid-row: 1;
  color: #67C23A;
}

.topology-node.load {
  grid-column: 3;
  grid-row: 3;
  color: #F56C6C;
}

.topology-node.grid {
  grid-column: 5;
  grid-row: 3;
  color: #909399;
}

.topology-connection {
  position: relative;
}

.h-line::after {
  content: '';
  position: absolute;
  top: 50%;
  left: 0;
  width: 100%;
  height: 2px;
  background-color: #ddd;
}

.down-line::after {
  content: '';
  position: absolute;
  top: 0;
  left: 50%;
  width: 2px;
  height: 100%;
  background-color: #ddd;
}

/* Status Table */
.status-table-card {
  width: 100%;
  margin-bottom: 20px;
}

.status-header {
  display: flex;
  margin-bottom: 20px;
  border-bottom: 1px solid #eee;
  padding-bottom: 10px;
}

.system-icon {
  display: flex;
  flex-direction: column;
  align-items: center;
  margin-right: 40px;
  opacity: 0.5;
  cursor: pointer;
}

.system-icon.active {
  opacity: 1;
  color: #409EFF;
}

.grid-icon, .container-icon {
  font-size: 40px;
  margin-bottom: 5px;
}

.system-condition {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 10px;
}

.condition-tag {
  padding: 5px 15px;
  border-radius: 4px;
  background: #eee;
}

.condition-tag.discharging {
  background: #67C23A;
  color: white;
}

.condition-tag.charging {
  background: #409EFF;
  color: white;
}

.condition-tag.idle {
  background: #909399;
  color: white;
}

/* Charts Card */
.charts-card {
  width: 100%;
}

.charts-header {
  margin-bottom: 20px;
  text-align: center;
}

.charts-container {
  display: flex;
  flex-direction: column;
  height: 320px;
  gap: 20px;
}

.chart {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.chart-title {
  font-weight: bold;
  margin-bottom: 10px;
  text-align: center;
}

.chart-placeholder {
  flex: 1;
  background-color: #f9f9f9;
  border-radius: 4px;
  border: 1px solid #eee;
  position: relative;
}

/* Mock chart for demonstration */
.chart-mock {
  position: relative;
  width: 100%;
  height: 100%;
}

.chart-line {
  position: absolute;
  bottom: 0;
  width: 6px;
  background-color: #409EFF;
  border-radius: 3px 3px 0 0;
}

.chart-line.soc {
  background-color: #67C23A;
}
</style> 