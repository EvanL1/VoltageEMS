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
      <div class="topology-section">
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

            <div class="topology-node pcs">
              <div class="node-icon">
                <el-icon><el-icon-set-up /></el-icon>
              </div>
              <div class="node-label">PCS</div>
              <div class="node-data">{{ pcsData.efficiency }}% eff</div>
            </div>

            <div class="topology-connection h-line"></div>

            <div class="topology-node battery">
              <div class="node-icon">
                <el-icon><el-icon-lightning /></el-icon>
              </div>
              <div class="node-label">Battery</div>
              <div class="node-data soc" :class="getSocClass(socValue)">SOC: {{ socValue }}%</div>
              <div class="node-data power-data">
                <div class="power-item" v-if="chargePower > 0">
                  <span class="power-label charge">Charge:</span>
                  <span class="power-value">{{ chargePower }} kW</span>
                </div>
                <div class="power-item" v-if="dischargePower > 0">
                  <span class="power-label discharge">Discharge:</span>
                  <span class="power-value">{{ dischargePower }} kW</span>
                </div>
                <div class="power-item" v-if="chargePower === 0 && dischargePower === 0">
                  <span class="power-label idle">Idle</span>
                </div>
              </div>
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

            <div class="topology-node generator">
              <div class="node-icon">
                <el-icon><el-icon-cpu /></el-icon>
              </div>
              <div class="node-label">Diesel Generator</div>
              <div class="node-data status">{{ generatorData.status }}</div>
              <div class="node-data">{{ generatorData.power }} kW</div>
            </div>
          </div>
        </div>
      </div>

      <!-- Alerts and Charts Row -->
      <div class="bottom-row">
        <!-- Current Alerts -->
        <div class="alert-section">
          <div class="alert-header">
            <span>Current Alerts</span>
            <el-tag type="danger" v-if="currentAlerts.length > 0">{{ currentAlerts.length }}</el-tag>
          </div>
          <div v-if="currentAlerts.length > 0" class="alert-content">
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
        </div>

        <!-- Energy & SOC Trends -->
        <div class="charts-section">
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
        </div>
      </div>
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
      pcsData: {
        efficiency: 98.5
      },
      loadData: {
        power: 18.7
      },
      generatorData: {
        status: 'Standby',
        power: 0
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
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
}

/* Top Operation Bar */
.dashboard-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px;
  background-color: #f9f9f9;
  margin-bottom: 0;
  border-bottom: 1px solid #eee;
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
  margin-bottom: 0;
  border-bottom: 1px solid #e8ecd0;
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
  flex: 1;
  overflow: hidden;
}

/* Topology Section */
.topology-section {
  padding: 10px;
  background-color: white;
  border-bottom: 1px solid #eee;
  height: 280px; /* Reduced height */
}

.topology-header {
  margin-bottom: 5px;
  text-align: center;
}

.topology-header h3 {
  margin: 0;
  font-size: 16px;
}

.topology-diagram {
  height: 230px; /* Reduced height */
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
  padding: 5px;
}

/* Bottom Row Layout */
.bottom-row {
  display: flex;
  flex: 1;
  overflow: hidden;
}

/* Alert Section */
.alert-section {
  flex: 1;
  padding: 15px;
  background-color: white;
  border-right: 1px solid #eee;
  display: flex;
  flex-direction: column;
}

.alert-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding-bottom: 10px;
  border-bottom: 1px solid #eee;
  font-weight: bold;
  font-size: 16px;
}

.alert-content {
  flex: 1;
  overflow: auto;
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

/* Charts Section */
.charts-section {
  flex: 1;
  padding: 15px;
  background-color: white;
  display: flex;
  flex-direction: column;
}

.charts-header {
  margin-bottom: 10px;
  text-align: center;
  padding-bottom: 10px;
  border-bottom: 1px solid #eee;
}

.charts-container {
  display: flex;
  flex-direction: column;
  flex: 1;
  gap: 15px;
}

.chart {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.chart-title {
  font-weight: bold;
  margin-bottom: 5px;
  text-align: center;
}

.chart-placeholder {
  flex: 1;
  background-color: #f9f9f9;
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

/* Topology Node Styles - Keep existing styles */
.topology-node {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  border: 2px solid #ddd;
  border-radius: 8px;
  padding: 8px;
  background-color: #f9f9f9;
  font-size: 0.9rem;
}

.node-icon {
  font-size: 1.6rem;
  margin-bottom: 3px;
}

.node-label {
  font-weight: bold;
  margin-bottom: 3px;
  font-size: 0.85rem;
}

.node-data {
  font-size: 0.8rem;
  color: #555;
  margin-bottom: 2px;
}

.node-data.status {
  color: #67C23A;
  font-weight: bold;
}

.node-data.soc {
  font-weight: bold;
  font-size: 0.95rem;
}

.node-data.soc.normal {
  color: #67C23A;
}

.node-data.soc.warning {
  color: #E6A23C;
}

.node-data.soc.critical {
  color: #F56C6C;
}

.power-data {
  display: flex;
  flex-direction: column;
  align-items: center;
  margin-top: 3px;
  width: 100%;
}

.power-item {
  display: flex;
  justify-content: space-between;
  width: 100%;
  margin-bottom: 2px;
}

.power-label {
  font-weight: 500;
  margin-right: 5px;
}

.power-label.charge {
  color: #409EFF;
}

.power-label.discharge {
  color: #F56C6C;
}

.power-label.idle {
  color: #909399;
}

.power-value {
  font-weight: bold;
}

.topology-node.pv {
  grid-column: 1;
  grid-row: 1;
  color: #E6A23C;
}

.topology-node.pcs {
  grid-column: 3;
  grid-row: 1;
  color: #409EFF;
}

.topology-node.battery {
  grid-column: 5;
  grid-row: 1;
  color: #67C23A;
  padding: 10px;
}

.topology-node.load {
  grid-column: 3;
  grid-row: 3;
  color: #F56C6C;
}

.topology-node.generator {
  grid-column: 5;
  grid-row: 3;
  color: #606266;
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
</style> 