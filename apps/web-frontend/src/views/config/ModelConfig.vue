<template>
  <div class="model-config">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>模型配置管理</span>
          <el-button type="primary" @click="handleAdd">新建模型</el-button>
        </div>
      </template>

      <!-- 搜索区域 -->
      <el-form :inline="true" :model="searchForm" class="search-form">
        <el-form-item label="模型名称">
          <el-input v-model="searchForm.name" placeholder="请输入模型名称" clearable />
        </el-form-item>
        <el-form-item label="模型类型">
          <el-select v-model="searchForm.type" placeholder="请选择模型类型" clearable>
            <el-option label="计算模型" value="calculation" />
            <el-option label="控制模型" value="control" />
            <el-option label="优化模型" value="optimization" />
            <el-option label="预测模型" value="prediction" />
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

      <!-- 表格 -->
      <el-table :data="tableData" stripe style="width: 100%" v-loading="loading">
        <el-table-column prop="id" label="模型ID" width="100" />
        <el-table-column prop="name" label="模型名称" />
        <el-table-column prop="type" label="模型类型">
          <template #default="scope">
            <el-tag :type="getModelTypeTag(scope.row.type)">
              {{ getModelTypeLabel(scope.row.type) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="version" label="版本" width="100" />
        <el-table-column prop="status" label="状态" width="100">
          <template #default="scope">
            <el-tag :type="getStatusTag(scope.row.status)">
              {{ getStatusLabel(scope.row.status) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="executionInterval" label="执行周期" width="120">
          <template #default="scope">
            {{ scope.row.executionInterval }}秒
          </template>
        </el-table-column>
        <el-table-column prop="lastRunTime" label="最后运行时间" width="180" />
        <el-table-column prop="description" label="描述" show-overflow-tooltip />
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
            <el-button type="primary" link @click="handleConfig(scope.row)">配置</el-button>
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
    <el-dialog v-model="dialogVisible" :title="dialogTitle" width="700px">
      <el-form :model="form" :rules="rules" ref="formRef" label-width="120px">
        <el-form-item label="模型名称" prop="name">
          <el-input v-model="form.name" placeholder="请输入模型名称" />
        </el-form-item>
        <el-form-item label="模型类型" prop="type">
          <el-select v-model="form.type" placeholder="请选择模型类型">
            <el-option label="计算模型" value="calculation" />
            <el-option label="控制模型" value="control" />
            <el-option label="优化模型" value="optimization" />
            <el-option label="预测模型" value="prediction" />
          </el-select>
        </el-form-item>
        <el-form-item label="执行周期" prop="executionInterval">
          <el-input-number 
            v-model="form.executionInterval" 
            :min="1" 
            :max="3600" 
            :step="1"
          />
          <span style="margin-left: 10px">秒</span>
        </el-form-item>
        <el-form-item label="输入点位" prop="inputPoints">
          <el-select 
            v-model="form.inputPoints" 
            multiple 
            placeholder="请选择输入点位"
            style="width: 100%"
          >
            <el-option label="电压_相A" value="1001" />
            <el-option label="电压_相B" value="1002" />
            <el-option label="电压_相C" value="1003" />
            <el-option label="电流_相A" value="1004" />
            <el-option label="电流_相B" value="1005" />
            <el-option label="电流_相C" value="1006" />
          </el-select>
        </el-form-item>
        <el-form-item label="输出点位" prop="outputPoints">
          <el-select 
            v-model="form.outputPoints" 
            multiple 
            placeholder="请选择输出点位"
            style="width: 100%"
          >
            <el-option label="总功率" value="2001" />
            <el-option label="功率因数" value="2002" />
            <el-option label="谐波畸变率" value="2003" />
          </el-select>
        </el-form-item>
        <el-form-item label="计算公式" prop="formula" v-if="form.type === 'calculation'">
          <el-input 
            v-model="form.formula" 
            type="textarea" 
            rows="4" 
            placeholder="请输入计算公式，例如: P = U * I * cos(φ)"
          />
        </el-form-item>
        <el-form-item label="描述" prop="description">
          <el-input 
            v-model="form.description" 
            type="textarea" 
            rows="3" 
            placeholder="请输入模型描述"
          />
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
  type: '',
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
  type: '',
  executionInterval: 60,
  inputPoints: [],
  outputPoints: [],
  formula: '',
  description: ''
})

// 表单验证规则
const rules = {
  name: [
    { required: true, message: '请输入模型名称', trigger: 'blur' }
  ],
  type: [
    { required: true, message: '请选择模型类型', trigger: 'change' }
  ],
  executionInterval: [
    { required: true, message: '请输入执行周期', trigger: 'blur' }
  ],
  inputPoints: [
    { required: true, message: '请选择输入点位', trigger: 'change' }
  ],
  outputPoints: [
    { required: true, message: '请选择输出点位', trigger: 'change' }
  ]
}

// 获取模型类型标签
const getModelTypeLabel = (type) => {
  const typeMap = {
    'calculation': '计算模型',
    'control': '控制模型',
    'optimization': '优化模型',
    'prediction': '预测模型'
  }
  return typeMap[type] || type
}

const getModelTypeTag = (type) => {
  const typeMap = {
    'calculation': 'primary',
    'control': 'success',
    'optimization': 'warning',
    'prediction': 'danger'
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

// 查询
const handleSearch = () => {
  currentPage.value = 1
  fetchData()
}

// 重置
const handleReset = () => {
  searchForm.name = ''
  searchForm.type = ''
  searchForm.status = ''
  handleSearch()
}

// 新增
const handleAdd = () => {
  dialogTitle.value = '新建模型'
  Object.assign(form, {
    name: '',
    type: '',
    executionInterval: 60,
    inputPoints: [],
    outputPoints: [],
    formula: '',
    description: ''
  })
  dialogVisible.value = true
}

// 编辑
const handleEdit = (row) => {
  dialogTitle.value = '编辑模型'
  Object.assign(form, row)
  dialogVisible.value = true
}

// 配置
const handleConfig = (row) => {
  ElMessage.info(`配置模型：${row.name}`)
}

// 启动
const handleStart = async (row) => {
  try {
    await ElMessageBox.confirm(
      `确定要启动模型"${row.name}"吗？`,
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
      `确定要停止模型"${row.name}"吗？`,
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
      `确定要删除模型"${row.name}"吗？`,
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
    const types = ['calculation', 'control', 'optimization', 'prediction']
    const statuses = ['running', 'stopped', 'error']
    
    for (let i = 0; i < 8; i++) {
      mockData.push({
        id: `M${1000 + i}`,
        name: `${getModelTypeLabel(types[i % 4])}${i + 1}`,
        type: types[i % 4],
        version: '1.0.0',
        status: statuses[i % 3],
        executionInterval: [60, 120, 300, 600][i % 4],
        lastRunTime: new Date().toLocaleString(),
        inputPoints: ['1001', '1002', '1003'],
        outputPoints: ['2001'],
        description: `${getModelTypeLabel(types[i % 4])}的示例模型`
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
.model-config {
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