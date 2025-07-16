<template>
  <div class="storage-policy">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>存储策略管理</span>
          <el-button type="primary" @click="handleAdd">新建策略</el-button>
        </div>
      </template>

      <!-- 搜索区域 -->
      <el-form :inline="true" :model="searchForm" class="search-form">
        <el-form-item label="策略名称">
          <el-input v-model="searchForm.name" placeholder="请输入策略名称" clearable />
        </el-form-item>
        <el-form-item label="存储类型">
          <el-select v-model="searchForm.storageType" placeholder="请选择存储类型" clearable>
            <el-option label="实时存储" value="realtime" />
            <el-option label="历史存储" value="history" />
            <el-option label="归档存储" value="archive" />
          </el-select>
        </el-form-item>
        <el-form-item label="状态">
          <el-select v-model="searchForm.enabled" placeholder="请选择状态" clearable>
            <el-option label="启用" :value="true" />
            <el-option label="禁用" :value="false" />
          </el-select>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="handleSearch">查询</el-button>
          <el-button @click="handleReset">重置</el-button>
        </el-form-item>
      </el-form>

      <!-- 表格 -->
      <el-table :data="tableData" stripe style="width: 100%" v-loading="loading">
        <el-table-column prop="id" label="策略ID" width="100" />
        <el-table-column prop="name" label="策略名称" />
        <el-table-column prop="storageType" label="存储类型" width="120">
          <template #default="scope">
            <el-tag :type="getStorageTypeTag(scope.row.storageType)">
              {{ getStorageTypeLabel(scope.row.storageType) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="interval" label="存储间隔" width="120">
          <template #default="scope">
            {{ formatInterval(scope.row.interval) }}
          </template>
        </el-table-column>
        <el-table-column prop="retention" label="保留时间" width="120">
          <template #default="scope">
            {{ formatRetention(scope.row.retention) }}
          </template>
        </el-table-column>
        <el-table-column prop="compression" label="压缩" width="80">
          <template #default="scope">
            <el-tag v-if="scope.row.compression" type="success">启用</el-tag>
            <el-tag v-else type="info">禁用</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="pointCount" label="关联点位数" width="100" />
        <el-table-column prop="dataSize" label="数据量" width="100">
          <template #default="scope">
            {{ formatDataSize(scope.row.dataSize) }}
          </template>
        </el-table-column>
        <el-table-column prop="enabled" label="状态" width="80">
          <template #default="scope">
            <el-switch
              v-model="scope.row.enabled"
              @change="handleStatusChange(scope.row)"
            />
          </template>
        </el-table-column>
        <el-table-column label="操作" width="200" fixed="right">
          <template #default="scope">
            <el-button type="primary" link @click="handleEdit(scope.row)">编辑</el-button>
            <el-button type="primary" link @click="handleView(scope.row)">查看点位</el-button>
            <el-button type="danger" link @click="handleDelete(scope.row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>

      <!-- 分页 -->
      <el-pagination
        v-model:current-page="currentPage"
        v-model:page-size="pageSize"
        :page-sizes="[10, 20, 50, 100]"
        :total="total"
        layout="total, sizes, prev, pager, next, jumper"
        @size-change="handleSizeChange"
        @current-change="handleCurrentChange"
      />
    </el-card>

    <!-- 新增/编辑对话框 -->
    <el-dialog v-model="dialogVisible" :title="dialogTitle" width="800px">
      <el-form :model="form" :rules="rules" ref="formRef" label-width="120px">
        <el-form-item label="策略名称" prop="name">
          <el-input v-model="form.name" placeholder="请输入策略名称" />
        </el-form-item>
        
        <el-form-item label="存储类型" prop="storageType">
          <el-radio-group v-model="form.storageType">
            <el-radio value="realtime">实时存储</el-radio>
            <el-radio value="history">历史存储</el-radio>
            <el-radio value="archive">归档存储</el-radio>
          </el-radio-group>
        </el-form-item>

        <el-row :gutter="20">
          <el-col :span="12">
            <el-form-item label="存储间隔" prop="interval">
              <el-input-number v-model="form.interval" :min="1" :max="3600" />
              <el-select v-model="form.intervalUnit" style="margin-left: 10px; width: 80px">
                <el-option label="秒" value="s" />
                <el-option label="分钟" value="m" />
                <el-option label="小时" value="h" />
              </el-select>
            </el-form-item>
          </el-col>
          <el-col :span="12">
            <el-form-item label="保留时间" prop="retention">
              <el-input-number v-model="form.retention" :min="1" :max="365" />
              <el-select v-model="form.retentionUnit" style="margin-left: 10px; width: 80px">
                <el-option label="天" value="d" />
                <el-option label="月" value="M" />
                <el-option label="年" value="y" />
              </el-select>
            </el-form-item>
          </el-col>
        </el-row>

        <el-form-item label="存储位置" prop="location">
          <el-select v-model="form.location" placeholder="请选择存储位置">
            <el-option label="InfluxDB - 主库" value="influxdb_primary" />
            <el-option label="InfluxDB - 备库" value="influxdb_backup" />
            <el-option label="TimescaleDB" value="timescaledb" />
            <el-option label="对象存储" value="object_storage" />
          </el-select>
        </el-form-item>

        <el-form-item label="选择点位" prop="points">
          <el-transfer
            v-model="form.selectedPoints"
            :data="allPoints"
            :titles="['可选点位', '已选点位']"
            :props="{
              key: 'id',
              label: 'name'
            }"
            filterable
            filter-placeholder="搜索点位"
            style="height: 400px"
          />
        </el-form-item>

        <el-form-item label="高级设置">
          <el-checkbox v-model="form.compression">启用数据压缩</el-checkbox>
          <el-checkbox v-model="form.downsampling" style="margin-left: 20px">启用降采样</el-checkbox>
        </el-form-item>

        <template v-if="form.downsampling">
          <el-form-item label="降采样规则" prop="downsamplingRules">
            <el-button type="primary" size="small" @click="addDownsamplingRule">添加规则</el-button>
            <div v-for="(rule, index) in form.downsamplingRules" :key="index" class="downsampling-rule">
              <el-row :gutter="10" align="middle">
                <el-col :span="6">
                  <el-select v-model="rule.after" placeholder="时间范围">
                    <el-option label="7天后" value="7d" />
                    <el-option label="30天后" value="30d" />
                    <el-option label="90天后" value="90d" />
                    <el-option label="180天后" value="180d" />
                  </el-select>
                </el-col>
                <el-col :span="6">
                  <el-select v-model="rule.interval" placeholder="聚合间隔">
                    <el-option label="5分钟" value="5m" />
                    <el-option label="15分钟" value="15m" />
                    <el-option label="1小时" value="1h" />
                    <el-option label="1天" value="1d" />
                  </el-select>
                </el-col>
                <el-col :span="6">
                  <el-select v-model="rule.function" placeholder="聚合函数">
                    <el-option label="平均值" value="avg" />
                    <el-option label="最大值" value="max" />
                    <el-option label="最小值" value="min" />
                    <el-option label="求和" value="sum" />
                  </el-select>
                </el-col>
                <el-col :span="6">
                  <el-button type="danger" link @click="removeDownsamplingRule(index)">删除</el-button>
                </el-col>
              </el-row>
            </div>
          </el-form-item>
        </template>

        <el-form-item label="启用策略" prop="enabled">
          <el-switch v-model="form.enabled" />
        </el-form-item>
      </el-form>
      <template #footer>
        <span class="dialog-footer">
          <el-button @click="dialogVisible = false">取消</el-button>
          <el-button type="primary" @click="handleSubmit">确定</el-button>
        </span>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'

// 搜索表单
const searchForm = reactive({
  name: '',
  storageType: '',
  enabled: null
})

// 表格数据
const tableData = ref([])
const loading = ref(false)
const currentPage = ref(1)
const pageSize = ref(20)
const total = ref(0)

// 对话框
const dialogVisible = ref(false)
const dialogTitle = ref('')
const formRef = ref()
const form = reactive({
  name: '',
  storageType: 'realtime',
  interval: 10,
  intervalUnit: 's',
  retention: 30,
  retentionUnit: 'd',
  location: 'influxdb_primary',
  selectedPoints: [],
  compression: false,
  downsampling: false,
  downsamplingRules: [],
  enabled: true
})

// 所有可选点位
const allPoints = ref([
  { id: '1001', name: '电压_相A' },
  { id: '1002', name: '电压_相B' },
  { id: '1003', name: '电压_相C' },
  { id: '1004', name: '电流_相A' },
  { id: '1005', name: '电流_相B' },
  { id: '1006', name: '电流_相C' },
  { id: '2001', name: '总功率' },
  { id: '2002', name: '功率因数' },
  { id: '2003', name: '谐波畸变率' }
])

// 表单验证规则
const rules = {
  name: [
    { required: true, message: '请输入策略名称', trigger: 'blur' }
  ],
  storageType: [
    { required: true, message: '请选择存储类型', trigger: 'change' }
  ],
  interval: [
    { required: true, message: '请输入存储间隔', trigger: 'blur' }
  ],
  retention: [
    { required: true, message: '请输入保留时间', trigger: 'blur' }
  ],
  location: [
    { required: true, message: '请选择存储位置', trigger: 'change' }
  ]
}

// 获取存储类型标签
const getStorageTypeLabel = (type) => {
  const typeMap = {
    'realtime': '实时存储',
    'history': '历史存储',
    'archive': '归档存储'
  }
  return typeMap[type] || type
}

const getStorageTypeTag = (type) => {
  const typeMap = {
    'realtime': 'primary',
    'history': 'success',
    'archive': 'warning'
  }
  return typeMap[type] || 'info'
}

// 格式化间隔
const formatInterval = (interval) => {
  if (interval < 60) return `${interval}秒`
  if (interval < 3600) return `${interval / 60}分钟`
  return `${interval / 3600}小时`
}

// 格式化保留时间
const formatRetention = (days) => {
  if (days < 30) return `${days}天`
  if (days < 365) return `${Math.floor(days / 30)}个月`
  return `${Math.floor(days / 365)}年`
}

// 格式化数据大小
const formatDataSize = (bytes) => {
  if (bytes < 1024) return `${bytes}B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)}MB`
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)}GB`
}

// 添加降采样规则
const addDownsamplingRule = () => {
  form.downsamplingRules.push({
    after: '7d',
    interval: '5m',
    function: 'avg'
  })
}

// 删除降采样规则
const removeDownsamplingRule = (index) => {
  form.downsamplingRules.splice(index, 1)
}

// 查询
const handleSearch = () => {
  currentPage.value = 1
  fetchData()
}

// 重置
const handleReset = () => {
  searchForm.name = ''
  searchForm.storageType = ''
  searchForm.enabled = null
  handleSearch()
}

// 新增
const handleAdd = () => {
  dialogTitle.value = '新建存储策略'
  Object.assign(form, {
    name: '',
    storageType: 'realtime',
    interval: 10,
    intervalUnit: 's',
    retention: 30,
    retentionUnit: 'd',
    location: 'influxdb_primary',
    selectedPoints: [],
    compression: false,
    downsampling: false,
    downsamplingRules: [],
    enabled: true
  })
  dialogVisible.value = true
}

// 编辑
const handleEdit = (row) => {
  dialogTitle.value = '编辑存储策略'
  Object.assign(form, row)
  dialogVisible.value = true
}

// 查看点位
const handleView = (row) => {
  ElMessage.info(`查看策略"${row.name}"关联的点位`)
}

// 删除
const handleDelete = async (row) => {
  try {
    await ElMessageBox.confirm(
      `确定要删除存储策略"${row.name}"吗？`,
      '提示',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )
    // TODO: 调用删除API
    ElMessage.success('删除成功')
    fetchData()
  } catch (error) {
    console.log('取消删除')
  }
}

// 状态变更
const handleStatusChange = (row) => {
  // TODO: 调用API更新状态
  ElMessage.success(`策略${row.enabled ? '启用' : '禁用'}成功`)
}

// 提交表单
const handleSubmit = async () => {
  await formRef.value.validate()
  // TODO: 调用新增/编辑API
  ElMessage.success(dialogTitle.value.includes('新建') ? '新建成功' : '编辑成功')
  dialogVisible.value = false
  fetchData()
}

// 分页变化
const handleSizeChange = () => {
  fetchData()
}

const handleCurrentChange = () => {
  fetchData()
}

// 获取数据
const fetchData = async () => {
  loading.value = true
  try {
    // TODO: 调用API获取数据
    // 模拟数据
    const mockData = []
    const types = ['realtime', 'history', 'archive']
    
    for (let i = 0; i < 8; i++) {
      mockData.push({
        id: `P${1000 + i}`,
        name: `${getStorageTypeLabel(types[i % 3])}_策略${i + 1}`,
        storageType: types[i % 3],
        interval: [10, 60, 300, 3600][i % 4],
        retention: [7, 30, 90, 365][i % 4],
        compression: i % 2 === 0,
        pointCount: Math.floor(Math.random() * 100) + 10,
        dataSize: Math.floor(Math.random() * 1024 * 1024 * 1024),
        enabled: i % 4 !== 0,
        location: 'influxdb_primary',
        selectedPoints: ['1001', '1002'],
        downsampling: i % 3 === 0,
        downsamplingRules: []
      })
    }
    tableData.value = mockData
    total.value = 50
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  fetchData()
})
</script>

<style scoped>
.storage-policy {
  padding: 20px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.search-form {
  margin-bottom: 20px;
}

.el-pagination {
  margin-top: 20px;
  display: flex;
  justify-content: flex-end;
}

.downsampling-rule {
  margin-top: 10px;
  padding: 10px;
  background-color: #f5f7fa;
  border-radius: 4px;
}

:deep(.el-transfer) {
  display: flex;
  align-items: center;
}

:deep(.el-transfer-panel) {
  width: 300px;
}
</style>