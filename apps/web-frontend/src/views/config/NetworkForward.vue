<template>
  <div class="network-forward">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>网络转发管理</span>
          <el-button type="primary" @click="handleAdd">新建转发规则</el-button>
        </div>
      </template>

      <!-- 搜索区域 -->
      <el-form :inline="true" :model="searchForm" class="search-form">
        <el-form-item label="规则名称">
          <el-input v-model="searchForm.name" placeholder="请输入规则名称" clearable />
        </el-form-item>
        <el-form-item label="转发类型">
          <el-select v-model="searchForm.forwardType" placeholder="请选择转发类型" clearable>
            <el-option label="MQTT" value="mqtt" />
            <el-option label="HTTP/HTTPS" value="http" />
            <el-option label="WebSocket" value="websocket" />
            <el-option label="TCP" value="tcp" />
            <el-option label="AWS IoT" value="aws_iot" />
            <el-option label="阿里云IoT" value="aliyun_iot" />
          </el-select>
        </el-form-item>
        <el-form-item label="状态">
          <el-select v-model="searchForm.status" placeholder="请选择状态" clearable>
            <el-option label="运行中" value="running" />
            <el-option label="已停止" value="stopped" />
            <el-option label="错误" value="error" />
          </el-select>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="handleSearch">查询</el-button>
          <el-button @click="handleReset">重置</el-button>
        </el-form-item>
      </el-form>

      <!-- 统计信息 -->
      <el-row :gutter="20" class="stats-row">
        <el-col :span="6">
          <el-statistic title="运行中规则" :value="runningCount" />
        </el-col>
        <el-col :span="6">
          <el-statistic title="今日转发量" :value="todayForwardCount" suffix="条" />
        </el-col>
        <el-col :span="6">
          <el-statistic title="转发成功率" :value="successRate" suffix="%" />
        </el-col>
        <el-col :span="6">
          <el-statistic title="平均延迟" :value="avgLatency" suffix="ms" />
        </el-col>
      </el-row>

      <!-- 表格 -->
      <el-table :data="tableData" stripe style="width: 100%" v-loading="loading">
        <el-table-column prop="id" label="规则ID" width="100" />
        <el-table-column prop="name" label="规则名称" show-overflow-tooltip />
        <el-table-column prop="forwardType" label="转发类型" width="120">
          <template #default="scope">
            <el-tag :type="getForwardTypeTag(scope.row.forwardType)">
              {{ getForwardTypeLabel(scope.row.forwardType) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="destination" label="目标地址" show-overflow-tooltip />
        <el-table-column prop="dataSource" label="数据源" show-overflow-tooltip />
        <el-table-column prop="status" label="状态" width="100">
          <template #default="scope">
            <el-tag :type="getStatusTag(scope.row.status)">
              {{ getStatusLabel(scope.row.status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="forwardCount" label="转发次数" width="100" />
        <el-table-column prop="errorCount" label="错误次数" width="100" />
        <el-table-column prop="lastForwardTime" label="最后转发时间" width="180" />
        <el-table-column label="操作" width="250" fixed="right">
          <template #default="scope">
            <el-button 
              v-if="scope.row.status === 'stopped'" 
              type="success" 
              link 
              @click="handleStart(scope.row)"
            >
              启动
            </el-button>
            <el-button 
              v-else-if="scope.row.status === 'running'" 
              type="warning" 
              link 
              @click="handleStop(scope.row)"
            >
              停止
            </el-button>
            <el-button type="primary" link @click="handleEdit(scope.row)">编辑</el-button>
            <el-button type="primary" link @click="handleTest(scope.row)">测试</el-button>
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
        <el-form-item label="规则名称" prop="name">
          <el-input v-model="form.name" placeholder="请输入规则名称" />
        </el-form-item>
        
        <el-form-item label="转发类型" prop="forwardType">
          <el-select v-model="form.forwardType" placeholder="请选择转发类型" @change="handleTypeChange">
            <el-option label="MQTT" value="mqtt" />
            <el-option label="HTTP/HTTPS" value="http" />
            <el-option label="WebSocket" value="websocket" />
            <el-option label="TCP" value="tcp" />
            <el-option label="AWS IoT" value="aws_iot" />
            <el-option label="阿里云IoT" value="aliyun_iot" />
          </el-select>
        </el-form-item>

        <!-- MQTT配置 -->
        <template v-if="form.forwardType === 'mqtt'">
          <el-form-item label="Broker地址" prop="brokerAddress">
            <el-input v-model="form.brokerAddress" placeholder="例如: mqtt://broker.example.com:1883" />
          </el-form-item>
          <el-form-item label="主题" prop="topic">
            <el-input v-model="form.topic" placeholder="例如: device/data" />
          </el-form-item>
          <el-form-item label="QoS" prop="qos">
            <el-radio-group v-model="form.qos">
              <el-radio :value="0">QoS 0</el-radio>
              <el-radio :value="1">QoS 1</el-radio>
              <el-radio :value="2">QoS 2</el-radio>
            </el-radio-group>
          </el-form-item>
          <el-form-item label="用户名" prop="username">
            <el-input v-model="form.username" placeholder="可选" />
          </el-form-item>
          <el-form-item label="密码" prop="password">
            <el-input v-model="form.password" type="password" placeholder="可选" show-password />
          </el-form-item>
        </template>

        <!-- HTTP配置 -->
        <template v-if="form.forwardType === 'http'">
          <el-form-item label="URL" prop="url">
            <el-input v-model="form.url" placeholder="例如: https://api.example.com/data" />
          </el-form-item>
          <el-form-item label="请求方法" prop="method">
            <el-select v-model="form.method">
              <el-option label="POST" value="POST" />
              <el-option label="PUT" value="PUT" />
              <el-option label="PATCH" value="PATCH" />
            </el-select>
          </el-form-item>
          <el-form-item label="请求头" prop="headers">
            <el-input 
              v-model="form.headers" 
              type="textarea" 
              rows="3" 
              placeholder="JSON格式，例如: {&quot;Authorization&quot;: &quot;Bearer token&quot;}"
            />
          </el-form-item>
        </template>

        <!-- AWS IoT配置 -->
        <template v-if="form.forwardType === 'aws_iot'">
          <el-form-item label="Endpoint" prop="awsEndpoint">
            <el-input v-model="form.awsEndpoint" placeholder="例如: xxx.iot.region.amazonaws.com" />
          </el-form-item>
          <el-form-item label="Thing Name" prop="thingName">
            <el-input v-model="form.thingName" placeholder="AWS IoT Thing名称" />
          </el-form-item>
          <el-form-item label="证书路径" prop="certPath">
            <el-input v-model="form.certPath" placeholder="设备证书路径" />
          </el-form-item>
          <el-form-item label="私钥路径" prop="keyPath">
            <el-input v-model="form.keyPath" placeholder="私钥路径" />
          </el-form-item>
        </template>

        <el-form-item label="数据源" prop="dataSource">
          <el-select 
            v-model="form.dataSource" 
            multiple 
            placeholder="请选择数据源点位"
            style="width: 100%"
          >
            <el-option label="全部遥测点" value="all_yc" />
            <el-option label="全部遥信点" value="all_yx" />
            <el-option label="电压_相A" value="1001" />
            <el-option label="电压_相B" value="1002" />
            <el-option label="电压_相C" value="1003" />
            <el-option label="电流_相A" value="1004" />
            <el-option label="电流_相B" value="1005" />
            <el-option label="电流_相C" value="1006" />
            <el-option label="总功率" value="2001" />
            <el-option label="功率因数" value="2002" />
          </el-select>
        </el-form-item>

        <el-form-item label="数据格式" prop="dataFormat">
          <el-radio-group v-model="form.dataFormat">
            <el-radio value="json">JSON</el-radio>
            <el-radio value="csv">CSV</el-radio>
            <el-radio value="xml">XML</el-radio>
            <el-radio value="custom">自定义</el-radio>
          </el-radio-group>
        </el-form-item>

        <el-form-item label="转发条件" prop="condition">
          <el-input 
            v-model="form.condition" 
            type="textarea" 
            rows="2" 
            placeholder="可选，例如: value > 100 或者留空表示全部转发"
          />
        </el-form-item>

        <el-form-item label="转发间隔" prop="interval">
          <el-input-number v-model="form.interval" :min="1" :max="3600" />
          <span style="margin-left: 10px">秒</span>
        </el-form-item>

        <el-form-item label="重试策略">
          <el-checkbox v-model="form.retryEnabled">启用重试</el-checkbox>
          <template v-if="form.retryEnabled">
            <el-input-number v-model="form.retryCount" :min="1" :max="10" style="margin-left: 20px" />
            <span style="margin-left: 10px">次</span>
          </template>
        </el-form-item>

        <el-form-item label="启用规则" prop="enabled">
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
import { ref, reactive, onMounted, computed } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'

// 搜索表单
const searchForm = reactive({
  name: '',
  forwardType: '',
  status: ''
})

// 表格数据
const tableData = ref([])
const loading = ref(false)
const currentPage = ref(1)
const pageSize = ref(20)
const total = ref(0)

// 统计数据
const runningCount = computed(() => tableData.value.filter(item => item.status === 'running').length)
const todayForwardCount = ref(12580)
const successRate = ref(99.2)
const avgLatency = ref(23)

// 对话框
const dialogVisible = ref(false)
const dialogTitle = ref('')
const formRef = ref()
const form = reactive({
  name: '',
  forwardType: '',
  brokerAddress: '',
  topic: '',
  qos: 1,
  username: '',
  password: '',
  url: '',
  method: 'POST',
  headers: '',
  awsEndpoint: '',
  thingName: '',
  certPath: '',
  keyPath: '',
  dataSource: [],
  dataFormat: 'json',
  condition: '',
  interval: 60,
  retryEnabled: true,
  retryCount: 3,
  enabled: true
})

// 表单验证规则
const rules = {
  name: [
    { required: true, message: '请输入规则名称', trigger: 'blur' }
  ],
  forwardType: [
    { required: true, message: '请选择转发类型', trigger: 'change' }
  ],
  dataSource: [
    { required: true, message: '请选择数据源', trigger: 'change' }
  ],
  interval: [
    { required: true, message: '请输入转发间隔', trigger: 'blur' }
  ]
}

// 获取转发类型标签
const getForwardTypeLabel = (type) => {
  const typeMap = {
    'mqtt': 'MQTT',
    'http': 'HTTP/HTTPS',
    'websocket': 'WebSocket',
    'tcp': 'TCP',
    'aws_iot': 'AWS IoT',
    'aliyun_iot': '阿里云IoT'
  }
  return typeMap[type] || type
}

const getForwardTypeTag = (type) => {
  const typeMap = {
    'mqtt': 'primary',
    'http': 'success',
    'websocket': 'warning',
    'tcp': 'danger',
    'aws_iot': 'primary',
    'aliyun_iot': 'success'
  }
  return typeMap[type] || 'info'
}

// 获取状态标签
const getStatusLabel = (status) => {
  const statusMap = {
    'running': '运行中',
    'stopped': '已停止',
    'error': '错误'
  }
  return statusMap[status] || status
}

const getStatusTag = (status) => {
  const statusMap = {
    'running': 'success',
    'stopped': 'info',
    'error': 'danger'
  }
  return statusMap[status] || 'info'
}

// 类型变化处理
const handleTypeChange = (value) => {
  // 根据转发类型重置特定字段
  if (value === 'mqtt') {
    form.brokerAddress = ''
    form.topic = ''
    form.qos = 1
  } else if (value === 'http') {
    form.url = ''
    form.method = 'POST'
    form.headers = ''
  } else if (value === 'aws_iot') {
    form.awsEndpoint = ''
    form.thingName = ''
    form.certPath = ''
    form.keyPath = ''
  }
}

// 查询
const handleSearch = () => {
  currentPage.value = 1
  fetchData()
}

// 重置
const handleReset = () => {
  searchForm.name = ''
  searchForm.forwardType = ''
  searchForm.status = ''
  handleSearch()
}

// 新增
const handleAdd = () => {
  dialogTitle.value = '新建转发规则'
  Object.assign(form, {
    name: '',
    forwardType: '',
    brokerAddress: '',
    topic: '',
    qos: 1,
    username: '',
    password: '',
    url: '',
    method: 'POST',
    headers: '',
    awsEndpoint: '',
    thingName: '',
    certPath: '',
    keyPath: '',
    dataSource: [],
    dataFormat: 'json',
    condition: '',
    interval: 60,
    retryEnabled: true,
    retryCount: 3,
    enabled: true
  })
  dialogVisible.value = true
}

// 编辑
const handleEdit = (row) => {
  dialogTitle.value = '编辑转发规则'
  Object.assign(form, row)
  dialogVisible.value = true
}

// 测试
const handleTest = (row) => {
  ElMessage.info(`正在测试转发规则：${row.name}`)
  // TODO: 调用测试API
  setTimeout(() => {
    ElMessage.success('连接测试成功')
  }, 1500)
}

// 启动
const handleStart = async (row) => {
  try {
    await ElMessageBox.confirm(
      `确定要启动转发规则"${row.name}"吗？`,
      '提示',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'info'
      }
    )
    // TODO: 调用启动API
    row.status = 'running'
    ElMessage.success('启动成功')
  } catch (error) {
    console.log('取消启动')
  }
}

// 停止
const handleStop = async (row) => {
  try {
    await ElMessageBox.confirm(
      `确定要停止转发规则"${row.name}"吗？`,
      '提示',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )
    // TODO: 调用停止API
    row.status = 'stopped'
    ElMessage.success('停止成功')
  } catch (error) {
    console.log('取消停止')
  }
}

// 删除
const handleDelete = async (row) => {
  try {
    await ElMessageBox.confirm(
      `确定要删除转发规则"${row.name}"吗？`,
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
    const types = ['mqtt', 'http', 'websocket', 'tcp', 'aws_iot', 'aliyun_iot']
    const statuses = ['running', 'stopped', 'error']
    
    for (let i = 0; i < 10; i++) {
      mockData.push({
        id: `F${1000 + i}`,
        name: `${getForwardTypeLabel(types[i % 6])}_转发规则${i + 1}`,
        forwardType: types[i % 6],
        destination: i % 2 === 0 ? 'mqtt://broker.emqx.io:1883' : 'https://api.example.com/data',
        dataSource: ['1001', '1002', '1003'],
        status: statuses[i % 3],
        forwardCount: Math.floor(Math.random() * 10000),
        errorCount: Math.floor(Math.random() * 100),
        lastForwardTime: new Date().toLocaleString(),
        brokerAddress: 'mqtt://broker.emqx.io:1883',
        topic: 'device/data',
        qos: 1,
        dataFormat: 'json',
        interval: 60,
        retryEnabled: true,
        retryCount: 3,
        enabled: true
      })
    }
    tableData.value = mockData
    total.value = 100
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  fetchData()
})
</script>

<style scoped>
.network-forward {
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

.stats-row {
  margin-bottom: 20px;
  padding: 20px;
  background-color: #f5f7fa;
  border-radius: 4px;
}

.el-pagination {
  margin-top: 20px;
  display: flex;
  justify-content: flex-end;
}
</style>