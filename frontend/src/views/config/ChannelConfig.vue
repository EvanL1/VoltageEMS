<template>
  <div class="channel-config">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>通道配置管理</span>
          <el-button type="primary" @click="handleAdd">新增通道</el-button>
        </div>
      </template>

      <!-- 搜索区域 -->
      <el-form :inline="true" :model="searchForm" class="search-form">
        <el-form-item label="通道名称">
          <el-input v-model="searchForm.name" placeholder="请输入通道名称" clearable />
        </el-form-item>
        <el-form-item label="协议类型">
          <el-select v-model="searchForm.protocol" placeholder="请选择协议类型" clearable>
            <el-option label="Modbus TCP" value="modbus_tcp" />
            <el-option label="Modbus RTU" value="modbus_rtu" />
            <el-option label="IEC60870" value="iec60870" />
            <el-option label="CAN Bus" value="can" />
          </el-select>
        </el-form-item>
        <el-form-item label="状态">
          <el-select v-model="searchForm.status" placeholder="请选择状态" clearable>
            <el-option label="启用" value="enabled" />
            <el-option label="禁用" value="disabled" />
          </el-select>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="handleSearch">查询</el-button>
          <el-button @click="handleReset">重置</el-button>
        </el-form-item>
      </el-form>

      <!-- 表格 -->
      <el-table :data="tableData" stripe style="width: 100%" v-loading="loading">
        <el-table-column prop="id" label="ID" width="80" />
        <el-table-column prop="name" label="通道名称" />
        <el-table-column prop="protocol" label="协议类型">
          <template #default="scope">
            <el-tag>{{ getProtocolLabel(scope.row.protocol) }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="transport" label="传输方式" />
        <el-table-column prop="address" label="地址" />
        <el-table-column prop="status" label="状态">
          <template #default="scope">
            <el-switch
              v-model="scope.row.status"
              active-value="enabled"
              inactive-value="disabled"
              @change="handleStatusChange(scope.row)"
            />
          </template>
        </el-table-column>
        <el-table-column prop="createTime" label="创建时间" width="180" />
        <el-table-column label="操作" width="200" fixed="right">
          <template #default="scope">
            <el-button type="primary" link @click="handleEdit(scope.row)">编辑</el-button>
            <el-button type="primary" link @click="handleView(scope.row)">查看</el-button>
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
    <el-dialog v-model="dialogVisible" :title="dialogTitle" width="600px">
      <el-form :model="form" :rules="rules" ref="formRef" label-width="100px">
        <el-form-item label="通道名称" prop="name">
          <el-input v-model="form.name" placeholder="请输入通道名称" />
        </el-form-item>
        <el-form-item label="协议类型" prop="protocol">
          <el-select v-model="form.protocol" placeholder="请选择协议类型">
            <el-option label="Modbus TCP" value="modbus_tcp" />
            <el-option label="Modbus RTU" value="modbus_rtu" />
            <el-option label="IEC60870" value="iec60870" />
            <el-option label="CAN Bus" value="can" />
          </el-select>
        </el-form-item>
        <el-form-item label="传输方式" prop="transport">
          <el-select v-model="form.transport" placeholder="请选择传输方式">
            <el-option label="TCP" value="tcp" />
            <el-option label="Serial" value="serial" />
            <el-option label="CAN" value="can" />
          </el-select>
        </el-form-item>
        <el-form-item label="地址" prop="address">
          <el-input v-model="form.address" placeholder="请输入地址" />
        </el-form-item>
        <el-form-item label="端口" prop="port" v-if="form.transport === 'tcp'">
          <el-input-number v-model="form.port" :min="1" :max="65535" />
        </el-form-item>
        <el-form-item label="状态" prop="status">
          <el-radio-group v-model="form.status">
            <el-radio value="enabled">启用</el-radio>
            <el-radio value="disabled">禁用</el-radio>
          </el-radio-group>
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
  protocol: '',
  status: ''
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
  protocol: '',
  transport: '',
  address: '',
  port: 502,
  status: 'enabled'
})

// 表单验证规则
const rules = {
  name: [
    { required: true, message: '请输入通道名称', trigger: 'blur' }
  ],
  protocol: [
    { required: true, message: '请选择协议类型', trigger: 'change' }
  ],
  transport: [
    { required: true, message: '请选择传输方式', trigger: 'change' }
  ],
  address: [
    { required: true, message: '请输入地址', trigger: 'blur' }
  ]
}

// 获取协议标签
const getProtocolLabel = (protocol) => {
  const protocolMap = {
    'modbus_tcp': 'Modbus TCP',
    'modbus_rtu': 'Modbus RTU',
    'iec60870': 'IEC60870',
    'can': 'CAN Bus'
  }
  return protocolMap[protocol] || protocol
}

// 查询
const handleSearch = () => {
  currentPage.value = 1
  fetchData()
}

// 重置
const handleReset = () => {
  searchForm.name = ''
  searchForm.protocol = ''
  searchForm.status = ''
  handleSearch()
}

// 新增
const handleAdd = () => {
  dialogTitle.value = '新增通道'
  Object.assign(form, {
    name: '',
    protocol: '',
    transport: '',
    address: '',
    port: 502,
    status: 'enabled'
  })
  dialogVisible.value = true
}

// 编辑
const handleEdit = (row) => {
  dialogTitle.value = '编辑通道'
  Object.assign(form, row)
  dialogVisible.value = true
}

// 查看
const handleView = (row) => {
  ElMessage.info(`查看通道：${row.name}`)
}

// 删除
const handleDelete = async (row) => {
  try {
    await ElMessageBox.confirm(
      `确定要删除通道"${row.name}"吗？`,
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
  ElMessage.success(`通道${row.status === 'enabled' ? '启用' : '禁用'}成功`)
}

// 提交表单
const handleSubmit = async () => {
  await formRef.value.validate()
  // TODO: 调用新增/编辑API
  ElMessage.success(dialogTitle.value.includes('新增') ? '新增成功' : '编辑成功')
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
    for (let i = 0; i < 5; i++) {
      mockData.push({
        id: i + 1,
        name: `通道${i + 1}`,
        protocol: ['modbus_tcp', 'modbus_rtu', 'iec60870', 'can'][i % 4],
        transport: ['tcp', 'serial', 'can'][i % 3],
        address: i % 2 === 0 ? '192.168.1.' + (100 + i) : '/dev/ttyS' + i,
        status: i % 3 === 0 ? 'disabled' : 'enabled',
        createTime: new Date().toLocaleString()
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
.channel-config {
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
</style>