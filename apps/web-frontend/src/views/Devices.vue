<template>
  <div class="devices-page">
    <!-- 设备概览 -->
    <div class="devices-overview">
      <el-row :gutter="20">
        <el-col :span="6">
          <el-card class="stat-card">
            <div class="stat-item">
              <el-icon :size="30" color="#409EFF"><el-icon-monitor /></el-icon>
              <div class="stat-content">
                <div class="stat-value">{{ totalDevices }}</div>
                <div class="stat-label">设备总数</div>
              </div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="6">
          <el-card class="stat-card">
            <div class="stat-item">
              <el-icon :size="30" color="#67C23A"><el-icon-circle-check /></el-icon>
              <div class="stat-content">
                <div class="stat-value">{{ onlineDevices }}</div>
                <div class="stat-label">在线设备</div>
              </div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="6">
          <el-card class="stat-card">
            <div class="stat-item">
              <el-icon :size="30" color="#E6A23C"><el-icon-warning /></el-icon>
              <div class="stat-content">
                <div class="stat-value">{{ warningDevices }}</div>
                <div class="stat-label">异常设备</div>
              </div>
            </div>
          </el-card>
        </el-col>
        <el-col :span="6">
          <el-card class="stat-card">
            <div class="stat-item">
              <el-icon :size="30" color="#F56C6C"><el-icon-circle-close /></el-icon>
              <div class="stat-content">
                <div class="stat-value">{{ offlineDevices }}</div>
                <div class="stat-label">离线设备</div>
              </div>
            </div>
          </el-card>
        </el-col>
      </el-row>
    </div>

    <!-- 设备列表 -->
    <div class="devices-list">
      <el-card>
        <template #header>
          <div class="card-header">
            <span>设备列表</span>
            <div class="header-actions">
              <el-input
                v-model="searchText"
                placeholder="搜索设备名称或地址"
                style="width: 300px"
                @input="handleSearch">
                <template #prefix>
                  <el-icon><el-icon-search /></el-icon>
                </template>
              </el-input>
              <el-button type="primary" @click="addDevice">添加设备</el-button>
              <el-button @click="refreshDevices">
                <el-icon><el-icon-refresh /></el-icon>
                刷新
              </el-button>
            </div>
          </div>
        </template>

        <!-- 筛选条件 -->
        <div class="filter-bar">
          <el-select v-model="filterProtocol" placeholder="协议类型" clearable style="width: 150px">
            <el-option label="全部协议" value="" />
            <el-option label="Modbus TCP" value="modbus-tcp" />
            <el-option label="Modbus RTU" value="modbus-rtu" />
            <el-option label="CAN" value="can" />
            <el-option label="IEC60870-104" value="iec104" />
            <el-option label="GPIO" value="gpio" />
          </el-select>
          <el-select v-model="filterStatus" placeholder="状态" clearable style="width: 120px">
            <el-option label="全部状态" value="" />
            <el-option label="在线" value="online" />
            <el-option label="离线" value="offline" />
            <el-option label="异常" value="warning" />
          </el-select>
          <el-select v-model="filterChannel" placeholder="通道" clearable style="width: 150px">
            <el-option label="全部通道" value="" />
            <el-option v-for="channel in channels" :key="channel" :label="channel" :value="channel" />
          </el-select>
        </div>

        <!-- 设备表格 -->
        <el-table 
          :data="filteredDevices" 
          style="width: 100%" 
          @selection-change="handleSelectionChange">
          <el-table-column type="selection" width="55" />
          <el-table-column prop="name" label="设备名称" min-width="150">
            <template #default="scope">
              <el-button type="text" @click="viewDevice(scope.row)">{{ scope.row.name }}</el-button>
            </template>
          </el-table-column>
          <el-table-column prop="protocol" label="协议类型" width="120">
            <template #default="scope">
              <el-tag size="small">{{ scope.row.protocol }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="address" label="设备地址" width="150" />
          <el-table-column prop="channel" label="所属通道" width="120" />
          <el-table-column prop="status" label="状态" width="100">
            <template #default="scope">
              <el-tag :type="getStatusType(scope.row.status)">
                {{ getStatusText(scope.row.status) }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="points" label="点位数" width="100">
            <template #default="scope">
              <span>{{ scope.row.points.total }}</span>
            </template>
          </el-table-column>
          <el-table-column prop="lastUpdate" label="最后更新" width="180" />
          <el-table-column label="实时数据" width="200">
            <template #default="scope">
              <div class="realtime-data">
                <span v-for="(value, key) in scope.row.realtimeData" :key="key" class="data-item">
                  {{ key }}: <strong>{{ value }}</strong>
                </span>
              </div>
            </template>
          </el-table-column>
          <el-table-column label="操作" width="200" fixed="right">
            <template #default="scope">
              <el-button size="small" @click="viewDevice(scope.row)">查看</el-button>
              <el-button size="small" @click="editDevice(scope.row)">编辑</el-button>
              <el-button size="small" type="danger" @click="deleteDevice(scope.row)">删除</el-button>
            </template>
          </el-table-column>
        </el-table>

        <!-- 分页 -->
        <div class="pagination-container">
          <el-pagination
            background
            layout="total, prev, pager, next, sizes"
            :total="devices.length"
            :page-size="pageSize"
            :current-page="currentPage"
            @size-change="handleSizeChange"
            @current-change="handleCurrentChange" />
        </div>
      </el-card>
    </div>

    <!-- 设备详情对话框 -->
    <el-dialog v-model="deviceDetailVisible" title="设备详情" width="800px">
      <div v-if="selectedDevice" class="device-detail">
        <el-descriptions :column="2" border>
          <el-descriptions-item label="设备名称">{{ selectedDevice.name }}</el-descriptions-item>
          <el-descriptions-item label="协议类型">{{ selectedDevice.protocol }}</el-descriptions-item>
          <el-descriptions-item label="设备地址">{{ selectedDevice.address }}</el-descriptions-item>
          <el-descriptions-item label="所属通道">{{ selectedDevice.channel }}</el-descriptions-item>
          <el-descriptions-item label="状态">
            <el-tag :type="getStatusType(selectedDevice.status)">
              {{ getStatusText(selectedDevice.status) }}
            </el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="最后更新">{{ selectedDevice.lastUpdate }}</el-descriptions-item>
        </el-descriptions>

        <el-divider />

        <h4>点位信息</h4>
        <el-row :gutter="20">
          <el-col :span="6">
            <div class="point-stat">
              <div class="point-value">{{ selectedDevice.points.telemetry }}</div>
              <div class="point-label">遥测点</div>
            </div>
          </el-col>
          <el-col :span="6">
            <div class="point-stat">
              <div class="point-value">{{ selectedDevice.points.signal }}</div>
              <div class="point-label">遥信点</div>
            </div>
          </el-col>
          <el-col :span="6">
            <div class="point-stat">
              <div class="point-value">{{ selectedDevice.points.control }}</div>
              <div class="point-label">遥控点</div>
            </div>
          </el-col>
          <el-col :span="6">
            <div class="point-stat">
              <div class="point-value">{{ selectedDevice.points.adjustment }}</div>
              <div class="point-label">遥调点</div>
            </div>
          </el-col>
        </el-row>

        <el-divider />

        <h4>实时数据</h4>
        <el-table :data="selectedDeviceData" max-height="300">
          <el-table-column prop="name" label="点位名称" />
          <el-table-column prop="type" label="类型" width="100" />
          <el-table-column prop="value" label="当前值" width="120" />
          <el-table-column prop="unit" label="单位" width="80" />
          <el-table-column prop="updateTime" label="更新时间" width="180" />
        </el-table>
      </div>
    </el-dialog>
  </div>
</template>

<script>
export default {
  name: 'DevicesView',
  data() {
    return {
      searchText: '',
      filterProtocol: '',
      filterStatus: '',
      filterChannel: '',
      pageSize: 10,
      currentPage: 1,
      selectedDevices: [],
      deviceDetailVisible: false,
      selectedDevice: null,
      selectedDeviceData: [],
      channels: ['Channel-1', 'Channel-2', 'Channel-3'],
      devices: [
        {
          id: 1,
          name: 'PCS-01',
          protocol: 'Modbus TCP',
          address: '192.168.1.100:502',
          channel: 'Channel-1',
          status: 'online',
          points: { total: 45, telemetry: 20, signal: 15, control: 5, adjustment: 5 },
          lastUpdate: '2025-01-07 10:15:30',
          realtimeData: { 功率: '25.5kW', SOC: '85%' }
        },
        {
          id: 2,
          name: 'Battery-BMS',
          protocol: 'CAN',
          address: 'CAN0:0x100',
          channel: 'Channel-2',
          status: 'online',
          points: { total: 62, telemetry: 35, signal: 20, control: 4, adjustment: 3 },
          lastUpdate: '2025-01-07 10:15:28',
          realtimeData: { 电压: '380V', 电流: '65A' }
        },
        {
          id: 3,
          name: 'Grid-Meter',
          protocol: 'IEC60870-104',
          address: '192.168.1.110:2404',
          channel: 'Channel-3',
          status: 'warning',
          points: { total: 38, telemetry: 25, signal: 10, control: 2, adjustment: 1 },
          lastUpdate: '2025-01-07 10:10:15',
          realtimeData: { 频率: '49.8Hz', 功率因数: '0.95' }
        },
        {
          id: 4,
          name: 'DI/DO-Module',
          protocol: 'GPIO',
          address: 'GPIO:Board1',
          channel: 'Channel-1',
          status: 'offline',
          points: { total: 24, telemetry: 0, signal: 16, control: 8, adjustment: 0 },
          lastUpdate: '2025-01-07 09:45:00',
          realtimeData: {}
        }
      ],
      totalDevices: 12,
      onlineDevices: 8,
      warningDevices: 2,
      offlineDevices: 2
    }
  },
  computed: {
    filteredDevices() {
      let result = [...this.devices];
      
      if (this.searchText) {
        result = result.filter(device => 
          device.name.toLowerCase().includes(this.searchText.toLowerCase()) ||
          device.address.toLowerCase().includes(this.searchText.toLowerCase())
        );
      }
      
      if (this.filterProtocol) {
        result = result.filter(device => 
          device.protocol.toLowerCase().replace(' ', '-') === this.filterProtocol
        );
      }
      
      if (this.filterStatus) {
        result = result.filter(device => device.status === this.filterStatus);
      }
      
      if (this.filterChannel) {
        result = result.filter(device => device.channel === this.filterChannel);
      }
      
      // 分页
      const start = (this.currentPage - 1) * this.pageSize;
      const end = start + this.pageSize;
      return result.slice(start, end);
    }
  },
  methods: {
    getStatusType(status) {
      const types = {
        online: 'success',
        offline: 'danger',
        warning: 'warning'
      };
      return types[status] || 'info';
    },
    getStatusText(status) {
      const texts = {
        online: '在线',
        offline: '离线',
        warning: '异常'
      };
      return texts[status] || status;
    },
    handleSearch() {
      this.currentPage = 1;
    },
    handleSelectionChange(val) {
      this.selectedDevices = val;
    },
    handleSizeChange(val) {
      this.pageSize = val;
    },
    handleCurrentChange(val) {
      this.currentPage = val;
    },
    addDevice() {
      this.$message.info('添加设备功能开发中');
    },
    refreshDevices() {
      this.$message.success('设备列表已刷新');
    },
    viewDevice(device) {
      this.selectedDevice = device;
      // 模拟加载设备实时数据
      this.selectedDeviceData = [
        { name: '有功功率', type: '遥测', value: '25.5', unit: 'kW', updateTime: '2025-01-07 10:15:30' },
        { name: '无功功率', type: '遥测', value: '12.3', unit: 'kVar', updateTime: '2025-01-07 10:15:30' },
        { name: '电网频率', type: '遥测', value: '50.02', unit: 'Hz', updateTime: '2025-01-07 10:15:30' },
        { name: '运行状态', type: '遥信', value: '运行', unit: '-', updateTime: '2025-01-07 10:15:30' },
        { name: '故障状态', type: '遥信', value: '正常', unit: '-', updateTime: '2025-01-07 10:15:30' }
      ];
      this.deviceDetailVisible = true;
    },
    editDevice(device) {
      this.$message.info(`编辑设备: ${device.name}`);
    },
    deleteDevice(device) {
      this.$confirm(`确定要删除设备 ${device.name} 吗?`, '警告', {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }).then(() => {
        this.$message.success(`设备 ${device.name} 已删除`);
      });
    }
  }
}
</script>

<style scoped>
.devices-page {
  padding: 20px;
}

/* 设备概览 */
.devices-overview {
  margin-bottom: 20px;
}

.stat-card {
  height: 100px;
}

.stat-item {
  display: flex;
  align-items: center;
  gap: 15px;
}

.stat-content {
  flex: 1;
}

.stat-value {
  font-size: 28px;
  font-weight: bold;
  color: #303133;
}

.stat-label {
  font-size: 14px;
  color: #909399;
  margin-top: 5px;
}

/* 设备列表 */
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
  display: flex;
  gap: 10px;
}

.realtime-data {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.data-item {
  font-size: 12px;
  color: #606266;
}

.data-item strong {
  color: #409EFF;
}

.pagination-container {
  margin-top: 20px;
  display: flex;
  justify-content: flex-end;
}

/* 设备详情 */
.device-detail h4 {
  margin: 20px 0 10px 0;
  color: #303133;
}

.point-stat {
  text-align: center;
  padding: 15px;
  background: #f5f7fa;
  border-radius: 4px;
}

.point-value {
  font-size: 24px;
  font-weight: bold;
  color: #409EFF;
}

.point-label {
  font-size: 14px;
  color: #909399;
  margin-top: 5px;
}
</style>