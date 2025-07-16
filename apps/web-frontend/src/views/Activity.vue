<template>
  <div class="activity-page">
    <el-card class="activity-card">
      <template #header>
        <div class="card-header">
          <h2>System Activity Logs</h2>
          <div class="header-actions">
            <el-button type="primary" size="small" @click="refreshLogs">Refresh</el-button>
            <el-button type="info" size="small" @click="exportLogs">Export</el-button>
          </div>
        </div>
      </template>
      
      <div class="filter-bar">
        <el-row :gutter="20">
          <el-col :span="6">
            <el-select v-model="filter.service" placeholder="Select Service" clearable style="width: 100%">
              <el-option label="All Services" value="" />
              <el-option label="modsrv" value="modsrv" />
              <el-option label="netsrv" value="netsrv" />
              <el-option label="comsrv" value="comsrv" />
              <el-option label="hissrv" value="hissrv" />
              <el-option label="mosquitto" value="mosquitto" />
            </el-select>
          </el-col>
          <el-col :span="6">
            <el-select v-model="filter.level" placeholder="Log Level" clearable style="width: 100%">
              <el-option label="All Levels" value="" />
              <el-option label="INFO" value="info" />
              <el-option label="WARNING" value="warning" />
              <el-option label="ERROR" value="error" />
            </el-select>
          </el-col>
          <el-col :span="12">
            <el-date-picker
              v-model="filter.timeRange"
              type="datetimerange"
              range-separator="to"
              start-placeholder="Start Time"
              end-placeholder="End Time"
              format="YYYY-MM-DD HH:mm:ss"
              value-format="YYYY-MM-DD HH:mm:ss"
              style="width: 100%"
            />
          </el-col>
        </el-row>
      </div>
      
      <el-table :data="filteredLogs" style="width: 100%" max-height="500">
        <el-table-column prop="timestamp" label="Time" width="180" sortable />
        <el-table-column prop="service" label="Service" width="120" />
        <el-table-column prop="level" label="Level" width="100">
          <template #default="scope">
            <el-tag :type="getLogLevelType(scope.row.level)">{{ scope.row.level }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="message" label="Message" show-overflow-tooltip />
        <el-table-column prop="user" label="Operator" width="120" />
      </el-table>
      
      <div class="pagination-container">
        <el-pagination
          background
          layout="prev, pager, next, sizes, total"
          :page-sizes="[10, 20, 50, 100]"
          :page-size="pageSize"
          :current-page="currentPage"
          :total="filteredLogs.length"
          @size-change="handleSizeChange"
          @current-change="handleCurrentChange"
        />
      </div>
    </el-card>
  </div>
</template>

<script>
export default {
  name: 'ActivityView',
  data() {
    return {
      filter: {
        service: '',
        level: '',
        timeRange: null
      },
      pageSize: 20,
      currentPage: 1,
      logs: [
        {
          timestamp: '2025-03-11 13:25:30',
          service: 'modsrv',
          level: 'INFO',
          message: 'System started successfully',
          user: 'system'
        },
        {
          timestamp: '2025-03-11 13:26:45',
          service: 'netsrv',
          level: 'INFO',
          message: 'Connected to MQTT server',
          user: 'system'
        },
        {
          timestamp: '2025-03-11 13:30:22',
          service: 'comsrv',
          level: 'WARNING',
          message: 'Device DEV001 connection timeout',
          user: 'system'
        },
        {
          timestamp: '2025-03-11 13:45:11',
          service: 'hissrv',
          level: 'INFO',
          message: 'Historical data stored successfully',
          user: 'system'
        },
        {
          timestamp: '2025-03-11 14:01:34',
          service: 'mosquitto',
          level: 'ERROR',
          message: 'Client authentication failed',
          user: 'system'
        },
        {
          timestamp: '2025-03-11 14:15:27',
          service: 'modsrv',
          level: 'INFO',
          message: 'Configuration file updated',
          user: 'admin'
        },
        {
          timestamp: '2025-03-11 14:30:56',
          service: 'netsrv',
          level: 'INFO',
          message: 'Data sent to external system successfully',
          user: 'system'
        }
      ]
    }
  },
  computed: {
    filteredLogs() {
      let result = [...this.logs];
      
      // Filter by service
      if (this.filter.service) {
        result = result.filter(log => log.service === this.filter.service);
      }
      
      // Filter by log level
      if (this.filter.level) {
        result = result.filter(log => log.level.toLowerCase() === this.filter.level);
      }
      
      // Filter by time range
      if (this.filter.timeRange && this.filter.timeRange.length === 2) {
        const [start, end] = this.filter.timeRange;
        result = result.filter(log => {
          const timestamp = new Date(log.timestamp);
          return timestamp >= new Date(start) && timestamp <= new Date(end);
        });
      }
      
      return result;
    }
  },
  methods: {
    getLogLevelType(level) {
      const map = {
        'INFO': '',
        'WARNING': 'warning',
        'ERROR': 'danger'
      };
      return map[level] || '';
    },
    refreshLogs() {
      // In a real application, this would fetch the latest logs from the server
      this.$message.success('Logs refreshed');
    },
    exportLogs() {
      // In a real application, this would export the logs
      this.$message.success('Logs exported successfully');
    },
    handleSizeChange(val) {
      this.pageSize = val;
    },
    handleCurrentChange(val) {
      this.currentPage = val;
    }
  }
}
</script>

<style scoped>
.activity-page {
  padding: 0;
}

.activity-card {
  margin-bottom: 20px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.header-actions {
  display: flex;
  gap: 10px;
}

.filter-bar {
  margin-bottom: 20px;
}

.pagination-container {
  margin-top: 20px;
  text-align: right;
}
</style> 