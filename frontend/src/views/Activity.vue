<template>
  <div class="activity-page">
    <el-card class="activity-card">
      <template #header>
        <div class="card-header">
          <h2>系统活动记录</h2>
          <div class="header-actions">
            <el-button type="primary" size="small" @click="refreshLogs">刷新</el-button>
            <el-button type="info" size="small" @click="exportLogs">导出</el-button>
          </div>
        </div>
      </template>
      
      <div class="filter-bar">
        <el-row :gutter="20">
          <el-col :span="6">
            <el-select v-model="filter.service" placeholder="选择服务" clearable>
              <el-option label="所有服务" value="" />
              <el-option label="modsrv" value="modsrv" />
              <el-option label="netsrv" value="netsrv" />
              <el-option label="comsrv" value="comsrv" />
              <el-option label="hissrv" value="hissrv" />
              <el-option label="mosquitto" value="mosquitto" />
            </el-select>
          </el-col>
          <el-col :span="6">
            <el-select v-model="filter.level" placeholder="日志级别" clearable>
              <el-option label="所有级别" value="" />
              <el-option label="INFO" value="info" />
              <el-option label="WARNING" value="warning" />
              <el-option label="ERROR" value="error" />
            </el-select>
          </el-col>
          <el-col :span="12">
            <el-date-picker
              v-model="filter.timeRange"
              type="datetimerange"
              range-separator="至"
              start-placeholder="开始时间"
              end-placeholder="结束时间"
              format="YYYY-MM-DD HH:mm:ss"
              value-format="YYYY-MM-DD HH:mm:ss"
              style="width: 100%"
            />
          </el-col>
        </el-row>
      </div>
      
      <el-table :data="filteredLogs" style="width: 100%" max-height="500">
        <el-table-column prop="timestamp" label="时间" width="180" sortable />
        <el-table-column prop="service" label="服务" width="120" />
        <el-table-column prop="level" label="级别" width="100">
          <template #default="scope">
            <el-tag :type="getLogLevelType(scope.row.level)">{{ scope.row.level }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="message" label="消息" show-overflow-tooltip />
        <el-table-column prop="user" label="操作员" width="120" />
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
          message: '系统启动成功',
          user: 'system'
        },
        {
          timestamp: '2025-03-11 13:26:45',
          service: 'netsrv',
          level: 'INFO',
          message: '连接到 MQTT 服务器',
          user: 'system'
        },
        {
          timestamp: '2025-03-11 13:30:22',
          service: 'comsrv',
          level: 'WARNING',
          message: '设备 DEV001 连接超时',
          user: 'system'
        },
        {
          timestamp: '2025-03-11 13:45:11',
          service: 'hissrv',
          level: 'INFO',
          message: '历史数据存储成功',
          user: 'system'
        },
        {
          timestamp: '2025-03-11 14:01:34',
          service: 'mosquitto',
          level: 'ERROR',
          message: '客户端认证失败',
          user: 'system'
        },
        {
          timestamp: '2025-03-11 14:15:27',
          service: 'modsrv',
          level: 'INFO',
          message: '配置文件已更新',
          user: 'admin'
        },
        {
          timestamp: '2025-03-11 14:30:56',
          service: 'netsrv',
          level: 'INFO',
          message: '发送数据到外部系统成功',
          user: 'system'
        }
      ]
    }
  },
  computed: {
    filteredLogs() {
      let result = [...this.logs];
      
      // 过滤服务
      if (this.filter.service) {
        result = result.filter(log => log.service === this.filter.service);
      }
      
      // 过滤日志级别
      if (this.filter.level) {
        result = result.filter(log => log.level.toLowerCase() === this.filter.level);
      }
      
      // 过滤时间范围
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
      // 在实际应用中，这里会从服务器获取最新日志
      this.$message.success('日志已刷新');
    },
    exportLogs() {
      // 在实际应用中，这里会导出日志
      this.$message.success('日志导出成功');
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