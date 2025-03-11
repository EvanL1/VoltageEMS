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
        <el-button type="primary" size="small">Detail</el-button>
      </div>
    </div>

    <!-- 仪表盘卡片 -->
    <div class="dashboard-cards">
      <div class="card-row">
        <!-- 功率仪表 -->
        <el-card class="dashboard-card">
          <div class="gauge-container">
            <div class="gauge power-gauge">
              <div class="gauge-indicator" style="transform: rotate(45deg)"></div>
              <div class="gauge-center"></div>
            </div>
            <div class="gauge-label">
              <span>Power: </span>
              <span class="gauge-value">2.3 KW</span>
            </div>
          </div>
        </el-card>

        <!-- SOC仪表 -->
        <el-card class="dashboard-card">
          <div class="gauge-container">
            <div class="gauge soc-gauge">
              <div class="gauge-indicator" style="transform: rotate(160deg)"></div>
              <div class="gauge-center"></div>
            </div>
            <div class="gauge-label">
              <span>SOC: </span>
              <span class="gauge-value">99.9%</span>
            </div>
          </div>
        </el-card>

        <!-- 温度计 -->
        <el-card class="dashboard-card">
          <div class="temperature-container">
            <div class="thermometer">
              <div class="thermometer-tube">
                <div class="thermometer-fill"></div>
              </div>
              <div class="thermometer-bulb"></div>
            </div>
            <div class="temperature-details">
              <div class="temp-header">Temperature Details</div>
              <div class="temp-values">
                <div class="temp-row">
                  <span class="temp-label max">Max</span>
                  <span class="temp-value">18°C</span>
                </div>
                <div class="temp-row">
                  <span class="temp-label min">Min</span>
                  <span class="temp-value">17°C</span>
                </div>
                <div class="temp-row">
                  <span class="temp-label current">Current</span>
                  <span class="temp-value">0°C</span>
                </div>
              </div>
            </div>
          </div>
        </el-card>
      </div>

      <!-- 系统状态表格 -->
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
            <span class="condition-tag discharging">DisCharging</span>
            <span class="condition-tag">System Condition</span>
          </div>
        </div>
        <el-table :data="systemData" style="width: 100%" size="large">
          <el-table-column prop="name" label="Parameter" width="220" />
          <el-table-column prop="value" label="Value" width="120" align="right" />
          <el-table-column prop="cumulateCharge" label="CumulateCharge" align="center" />
          <el-table-column prop="cumulateDischarge" label="CumulateDischarge" align="center" />
        </el-table>
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
    },
    formatDate(date) {
      const year = date.getFullYear();
      const month = String(date.getMonth() + 1).padStart(2, '0');
      const day = String(date.getDate()).padStart(2, '0');
      const hours = String(date.getHours()).padStart(2, '0');
      const minutes = String(date.getMinutes()).padStart(2, '0');
      const seconds = String(date.getSeconds()).padStart(2, '0');
      
      return `${month}/${day}/${year} ${hours}:${minutes}:${seconds}`;
    }
  },
  mounted() {
    // 更新时间间隔
    setInterval(this.refreshData, 60000);
  }
}
</script>

<style scoped>
.dashboard {
  padding: 0;
}

/* 顶部操作栏 */
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

/* 项目选择器 */
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

/* 仪表盘卡片 */
.dashboard-cards {
  margin-bottom: 20px;
}

.card-row {
  display: flex;
  gap: 20px;
  margin-bottom: 20px;
}

.dashboard-card {
  flex: 1;
  height: 250px;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* 仪表样式 */
.gauge-container {
  text-align: center;
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}

.gauge {
  width: 200px;
  height: 200px;
  border-radius: 50%;
  position: relative;
  background: #f5f5f5;
  overflow: hidden;
}

.power-gauge {
  background: conic-gradient(
    #f3f3f3 0deg, 
    #f3f3f3 120deg, 
    #ff6b6b 121deg, 
    #ff6b6b 240deg, 
    #f3f3f3 241deg, 
    #f3f3f3 360deg
  );
}

.soc-gauge {
  background: conic-gradient(
    #3f87f5 0deg, 
    #3f87f5 180deg, 
    #f3f3f3 181deg, 
    #f3f3f3 360deg
  );
}

.gauge::before {
  content: '';
  position: absolute;
  top: 10%;
  left: 10%;
  width: 80%;
  height: 80%;
  background: white;
  border-radius: 50%;
}

.gauge-indicator {
  position: absolute;
  top: 50%;
  left: 50%;
  width: 90px;
  height: 4px;
  background: #333;
  transform-origin: 0 50%;
  z-index: 10;
}

.gauge-center {
  position: absolute;
  top: 50%;
  left: 50%;
  width: 20px;
  height: 20px;
  background: #333;
  border-radius: 50%;
  transform: translate(-50%, -50%);
  z-index: 11;
}

.gauge-label {
  margin-top: 10px;
  font-size: 18px;
  font-weight: bold;
}

.gauge-value {
  color: #409EFF;
}

/* 温度计样式 */
.temperature-container {
  display: flex;
  width: 100%;
  height: 100%;
  align-items: center;
  justify-content: space-around;
}

.thermometer {
  width: 60px;
  height: 180px;
  position: relative;
}

.thermometer-tube {
  position: absolute;
  top: 0;
  left: 50%;
  transform: translateX(-50%);
  width: 16px;
  height: 150px;
  background: white;
  border-radius: 10px;
  overflow: hidden;
  border: 2px solid #ddd;
}

.thermometer-fill {
  position: absolute;
  bottom: 0;
  width: 100%;
  height: 50%;
  background: linear-gradient(to top, #f00, #0f0);
}

.thermometer-bulb {
  position: absolute;
  bottom: 0;
  left: 50%;
  transform: translateX(-50%);
  width: 40px;
  height: 40px;
  background: #f00;
  border-radius: 50%;
  border: 2px solid #ddd;
}

.temperature-details {
  width: 120px;
}

.temp-header {
  font-weight: bold;
  margin-bottom: 15px;
  text-align: center;
}

.temp-values {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.temp-row {
  display: flex;
  justify-content: space-between;
}

.temp-label {
  font-weight: bold;
}

.temp-label.max {
  color: red;
}

.temp-label.min {
  color: green;
}

.temp-label.current {
  color: #409EFF;
}

/* 状态表格 */
.status-table-card {
  width: 100%;
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
</style> 