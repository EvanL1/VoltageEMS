<template>
  <div class="alarms-page">
    <!-- 告警统计 -->
    <div class="alarm-statistics">
      <el-row :gutter="20">
        <el-col :span="4">
          <el-card class="stat-card critical">
            <div class="stat-content">
              <div class="stat-icon">
                <el-icon :size="32"><el-icon-warning-filled /></el-icon>
              </div>
              <div class="stat-info">
                <div class="stat-value">{{ statistics.critical }}</div>
                <div class="stat-label">紧急告警</div>
              </div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="4">
          <el-card class="stat-card major">
            <div class="stat-content">
              <div class="stat-icon">
                <el-icon :size="32"><el-icon-warning /></el-icon>
              </div>
              <div class="stat-info">
                <div class="stat-value">{{ statistics.major }}</div>
                <div class="stat-label">重要告警</div>
              </div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="4">
          <el-card class="stat-card minor">
            <div class="stat-content">
              <div class="stat-icon">
                <el-icon :size="32"><el-icon-info-filled /></el-icon>
              </div>
              <div class="stat-info">
                <div class="stat-value">{{ statistics.minor }}</div>
                <div class="stat-label">次要告警</div>
              </div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="4">
          <el-card class="stat-card warning">
            <div class="stat-content">
              <div class="stat-icon">
                <el-icon :size="32"><el-icon-bell /></el-icon>
              </div>
              <div class="stat-info">
                <div class="stat-value">{{ statistics.warning }}</div>
                <div class="stat-label">提示告警</div>
              </div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="4">
          <el-card class="stat-card total">
            <div class="stat-content">
              <div class="stat-icon">
                <el-icon :size="32"><el-icon-document /></el-icon>
              </div>
              <div class="stat-info">
                <div class="stat-value">{{ statistics.total }}</div>
                <div class="stat-label">活跃告警</div>
              </div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="4">
          <el-card class="stat-card handled">
            <div class="stat-content">
              <div class="stat-icon">
                <el-icon :size="32"><el-icon-circle-check /></el-icon>
              </div>
              <div class="stat-info">
                <div class="stat-value">{{ statistics.handled }}</div>
                <div class="stat-label">今日处理</div>
              </div>
            </div>
          </el-card>
        </el-col>
      </el-row>
    </div>

    <!-- 告警列表 -->
    <div class="alarm-list">
      <el-card>
        <template #header>
          <div class="card-header">
            <span>告警列表</span>
            <div class="header-actions">
              <el-button type="primary" @click="confirmSelected" :disabled="selectedAlarms.length === 0">
                批量确认 ({{ selectedAlarms.length }})
              </el-button>
              <el-button @click="exportAlarms">导出</el-button>
              <el-button @click="refreshAlarms">
                <el-icon><el-icon-refresh /></el-icon>
                刷新
              </el-button>
            </div>
          </div>
        </template>

        <!-- 筛选条件 -->
        <div class="filter-section">
          <el-row :gutter="20">
            <el-col :span="6">
              <el-select v-model="filter.level" placeholder="告警级别" clearable style="width: 100%">
                <el-option label="全部级别" value="" />
                <el-option label="紧急" value="critical" />
                <el-option label="重要" value="major" />
                <el-option label="次要" value="minor" />
                <el-option label="提示" value="warning" />
              </el-select>
            </el-col>
            <el-col :span="6">
              <el-select v-model="filter.category" placeholder="告警类型" clearable style="width: 100%">
                <el-option label="全部类型" value="" />
                <el-option label="环境告警" value="environmental" />
                <el-option label="电力告警" value="power" />
                <el-option label="通信告警" value="communication" />
                <el-option label="系统告警" value="system" />
                <el-option label="安全告警" value="security" />
              </el-select>
            </el-col>
            <el-col :span="6">
              <el-select v-model="filter.status" placeholder="处理状态" clearable style="width: 100%">
                <el-option label="全部状态" value="" />
                <el-option label="未确认" value="unconfirmed" />
                <el-option label="已确认" value="confirmed" />
                <el-option label="处理中" value="processing" />
                <el-option label="已清除" value="cleared" />
              </el-select>
            </el-col>
            <el-col :span="6">
              <el-input v-model="filter.keyword" placeholder="搜索告警内容" clearable>
                <template #prefix>
                  <el-icon><el-icon-search /></el-icon>
                </template>
              </el-input>
            </el-col>
          </el-row>
        </div>

        <!-- 告警表格 -->
        <el-table 
          :data="filteredAlarms" 
          style="width: 100%" 
          :row-class-name="tableRowClassName"
          @selection-change="handleSelectionChange">
          <el-table-column type="selection" width="55" />
          <el-table-column prop="id" label="告警ID" width="100" />
          <el-table-column prop="level" label="级别" width="80">
            <template #default="scope">
              <el-tag :type="getLevelType(scope.row.level)" size="small">
                {{ getLevelText(scope.row.level) }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="category" label="类型" width="100">
            <template #default="scope">
              <el-tag size="small">{{ getCategoryText(scope.row.category) }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="device" label="告警设备" min-width="120" />
          <el-table-column prop="message" label="告警内容" min-width="200" show-overflow-tooltip />
          <el-table-column prop="occurTime" label="发生时间" width="180" sortable />
          <el-table-column prop="status" label="状态" width="100">
            <template #default="scope">
              <el-tag :type="getStatusType(scope.row.status)" size="small">
                {{ getStatusText(scope.row.status) }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="duration" label="持续时间" width="100">
            <template #default="scope">
              <span>{{ formatDuration(scope.row.duration) }}</span>
            </template>
          </el-table-column>
          <el-table-column label="操作" width="180" fixed="right">
            <template #default="scope">
              <el-button v-if="scope.row.status === 'unconfirmed'" 
                size="small" type="primary" @click="confirmAlarm(scope.row)">
                确认
              </el-button>
              <el-button size="small" @click="viewAlarmDetail(scope.row)">详情</el-button>
              <el-dropdown trigger="click">
                <el-button size="small">
                  更多 <el-icon><el-icon-arrow-down /></el-icon>
                </el-button>
                <template #dropdown>
                  <el-dropdown-menu>
                    <el-dropdown-item @click="assignAlarm(scope.row)">分配处理</el-dropdown-item>
                    <el-dropdown-item @click="addRemark(scope.row)">添加备注</el-dropdown-item>
                    <el-dropdown-item @click="viewHistory(scope.row)">查看历史</el-dropdown-item>
                  </el-dropdown-menu>
                </template>
              </el-dropdown>
            </template>
          </el-table-column>
        </el-table>

        <!-- 分页 -->
        <div class="pagination-container">
          <el-pagination
            background
            layout="total, prev, pager, next, sizes"
            :total="alarms.length"
            :page-size="pageSize"
            :current-page="currentPage"
            @size-change="handleSizeChange"
            @current-change="handleCurrentChange" />
        </div>
      </el-card>
    </div>

    <!-- 告警详情对话框 -->
    <el-dialog v-model="detailDialogVisible" title="告警详情" width="700px">
      <div v-if="selectedAlarm" class="alarm-detail">
        <el-descriptions :column="2" border>
          <el-descriptions-item label="告警ID">{{ selectedAlarm.id }}</el-descriptions-item>
          <el-descriptions-item label="告警级别">
            <el-tag :type="getLevelType(selectedAlarm.level)">
              {{ getLevelText(selectedAlarm.level) }}
            </el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="告警类型">{{ getCategoryText(selectedAlarm.category) }}</el-descriptions-item>
          <el-descriptions-item label="告警设备">{{ selectedAlarm.device }}</el-descriptions-item>
          <el-descriptions-item label="发生时间">{{ selectedAlarm.occurTime }}</el-descriptions-item>
          <el-descriptions-item label="处理状态">
            <el-tag :type="getStatusType(selectedAlarm.status)">
              {{ getStatusText(selectedAlarm.status) }}
            </el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="告警内容" :span="2">{{ selectedAlarm.message }}</el-descriptions-item>
          <el-descriptions-item label="告警值">{{ selectedAlarm.value }}</el-descriptions-item>
          <el-descriptions-item label="阈值">{{ selectedAlarm.threshold }}</el-descriptions-item>
          <el-descriptions-item label="处理建议" :span="2">{{ selectedAlarm.suggestion }}</el-descriptions-item>
        </el-descriptions>

        <el-divider />

        <h4>处理记录</h4>
        <el-timeline>
          <el-timeline-item 
            v-for="record in selectedAlarm.records" 
            :key="record.time"
            :timestamp="record.time"
            placement="top">
            <el-card>
              <p>{{ record.action }}</p>
              <p style="margin-top: 5px; color: #909399; font-size: 12px;">
                操作人: {{ record.operator }}
              </p>
            </el-card>
          </el-timeline-item>
        </el-timeline>
      </div>
    </el-dialog>
  </div>
</template>

<script>
export default {
  name: 'AlarmsView',
  data() {
    return {
      filter: {
        level: '',
        category: '',
        status: '',
        keyword: ''
      },
      pageSize: 10,
      currentPage: 1,
      selectedAlarms: [],
      detailDialogVisible: false,
      selectedAlarm: null,
      statistics: {
        critical: 2,
        major: 5,
        minor: 8,
        warning: 12,
        total: 27,
        handled: 45
      },
      alarms: [
        {
          id: 'ALM001',
          level: 'critical',
          category: 'power',
          device: 'PCS-01',
          message: '逆变器过载，当前负载率达到105%',
          occurTime: '2025-01-07 10:12:30',
          status: 'unconfirmed',
          duration: 180,
          value: '105%',
          threshold: '100%',
          suggestion: '请立即降低负载或启动备用设备',
          records: [
            { time: '2025-01-07 10:12:30', action: '系统自动生成告警', operator: '系统' }
          ]
        },
        {
          id: 'ALM002',
          level: 'major',
          category: 'environmental',
          device: 'Battery-BMS',
          message: '电池温度偏高，当前温度42°C',
          occurTime: '2025-01-07 10:05:15',
          status: 'confirmed',
          duration: 485,
          value: '42°C',
          threshold: '40°C',
          suggestion: '检查冷却系统运行状态，必要时降低充放电功率',
          records: [
            { time: '2025-01-07 10:05:15', action: '系统自动生成告警', operator: '系统' },
            { time: '2025-01-07 10:08:00', action: '确认告警', operator: '张三' }
          ]
        },
        {
          id: 'ALM003',
          level: 'minor',
          category: 'communication',
          device: 'Grid-Meter',
          message: '通信延迟增加，当前延迟150ms',
          occurTime: '2025-01-07 09:50:00',
          status: 'processing',
          duration: 1030,
          value: '150ms',
          threshold: '100ms',
          suggestion: '检查网络连接质量',
          records: [
            { time: '2025-01-07 09:50:00', action: '系统自动生成告警', operator: '系统' },
            { time: '2025-01-07 09:52:00', action: '确认告警并开始处理', operator: '李四' }
          ]
        }
      ]
    }
  },
  computed: {
    filteredAlarms() {
      let result = [...this.alarms];
      
      if (this.filter.level) {
        result = result.filter(alarm => alarm.level === this.filter.level);
      }
      
      if (this.filter.category) {
        result = result.filter(alarm => alarm.category === this.filter.category);
      }
      
      if (this.filter.status) {
        result = result.filter(alarm => alarm.status === this.filter.status);
      }
      
      if (this.filter.keyword) {
        result = result.filter(alarm => 
          alarm.message.toLowerCase().includes(this.filter.keyword.toLowerCase()) ||
          alarm.device.toLowerCase().includes(this.filter.keyword.toLowerCase())
        );
      }
      
      // 分页
      const start = (this.currentPage - 1) * this.pageSize;
      const end = start + this.pageSize;
      return result.slice(start, end);
    }
  },
  methods: {
    getLevelType(level) {
      const types = {
        critical: 'danger',
        major: 'warning',
        minor: '',
        warning: 'info'
      };
      return types[level] || '';
    },
    getLevelText(level) {
      const texts = {
        critical: '紧急',
        major: '重要',
        minor: '次要',
        warning: '提示'
      };
      return texts[level] || level;
    },
    getCategoryText(category) {
      const texts = {
        environmental: '环境',
        power: '电力',
        communication: '通信',
        system: '系统',
        security: '安全'
      };
      return texts[category] || category;
    },
    getStatusType(status) {
      const types = {
        unconfirmed: 'danger',
        confirmed: 'warning',
        processing: '',
        cleared: 'success'
      };
      return types[status] || 'info';
    },
    getStatusText(status) {
      const texts = {
        unconfirmed: '未确认',
        confirmed: '已确认',
        processing: '处理中',
        cleared: '已清除'
      };
      return texts[status] || status;
    },
    formatDuration(seconds) {
      if (seconds < 60) return `${seconds}秒`;
      if (seconds < 3600) return `${Math.floor(seconds / 60)}分钟`;
      return `${Math.floor(seconds / 3600)}小时`;
    },
    tableRowClassName({ row }) {
      if (row.level === 'critical') return 'critical-row';
      if (row.level === 'major') return 'major-row';
      return '';
    },
    handleSelectionChange(val) {
      this.selectedAlarms = val;
    },
    handleSizeChange(val) {
      this.pageSize = val;
    },
    handleCurrentChange(val) {
      this.currentPage = val;
    },
    confirmAlarm(alarm) {
      this.$message.success(`告警 ${alarm.id} 已确认`);
      alarm.status = 'confirmed';
    },
    confirmSelected() {
      this.$message.success(`已批量确认 ${this.selectedAlarms.length} 条告警`);
    },
    viewAlarmDetail(alarm) {
      this.selectedAlarm = alarm;
      this.detailDialogVisible = true;
    },
    assignAlarm(alarm) {
      this.$message.info(`分配告警 ${alarm.id}`);
    },
    addRemark(alarm) {
      this.$message.info(`为告警 ${alarm.id} 添加备注`);
    },
    viewHistory(alarm) {
      this.$message.info(`查看告警 ${alarm.id} 的历史记录`);
    },
    exportAlarms() {
      this.$message.success('告警数据已导出');
    },
    refreshAlarms() {
      this.$message.success('告警列表已刷新');
    }
  }
}
</script>

<style scoped>
.alarms-page {
  padding: 20px;
}

/* 告警统计 */
.alarm-statistics {
  margin-bottom: 20px;
}

.stat-card {
  height: 90px;
}

.stat-card.critical { border-left: 4px solid #F56C6C; }
.stat-card.major { border-left: 4px solid #E6A23C; }
.stat-card.minor { border-left: 4px solid #909399; }
.stat-card.warning { border-left: 4px solid #409EFF; }
.stat-card.total { border-left: 4px solid #303133; }
.stat-card.handled { border-left: 4px solid #67C23A; }

.stat-content {
  display: flex;
  align-items: center;
  gap: 15px;
}

.stat-icon {
  color: #909399;
}

.stat-info {
  flex: 1;
}

.stat-value {
  font-size: 24px;
  font-weight: bold;
  color: #303133;
}

.stat-label {
  font-size: 12px;
  color: #909399;
  margin-top: 2px;
}

/* 告警列表 */
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.header-actions {
  display: flex;
  gap: 10px;
}

.filter-section {
  margin-bottom: 20px;
}

:deep(.critical-row) {
  background-color: #fef0f0;
}

:deep(.major-row) {
  background-color: #fdf6ec;
}

.pagination-container {
  margin-top: 20px;
  display: flex;
  justify-content: flex-end;
}

/* 告警详情 */
.alarm-detail h4 {
  margin: 20px 0 10px 0;
  color: #303133;
}
</style>