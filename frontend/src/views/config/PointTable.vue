<template>
  <div class="point-table">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>点表管理</span>
          <div>
            <el-button type="primary" @click="handleImport">导入点表</el-button>
            <el-button type="success" @click="handleExport">导出点表</el-button>
            <el-button type="primary" @click="handleAdd">新增点位</el-button>
          </div>
        </div>
      </template>

      <!-- 搜索区域 -->
      <el-form :inline="true" :model="searchForm" class="search-form">
        <el-form-item label="点位名称">
          <el-input v-model="searchForm.name" placeholder="请输入点位名称" clearable />
        </el-form-item>
        <el-form-item label="点位ID">
          <el-input v-model="searchForm.pointId" placeholder="请输入点位ID" clearable />
        </el-form-item>
        <el-form-item label="点位类型">
          <el-select v-model="searchForm.type" placeholder="请选择点位类型" clearable>
            <el-option label="遥测(YC)" value="YC" />
            <el-option label="遥信(YX)" value="YX" />
            <el-option label="遥控(YK)" value="YK" />
            <el-option label="遥调(YT)" value="YT" />
          </el-select>
        </el-form-item>
        <el-form-item label="所属通道">
          <el-select v-model="searchForm.channelId" placeholder="请选择通道" clearable>
            <el-option label="通道1" value="1" />
            <el-option label="通道2" value="2" />
            <el-option label="通道3" value="3" />
          </el-select>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="handleSearch">查询</el-button>
          <el-button @click="handleReset">重置</el-button>
        </el-form-item>
      </el-form>

      <!-- 表格 -->
      <el-table :data="tableData" stripe style="width: 100%" v-loading="loading">
        <el-table-column type="selection" width="55" />
        <el-table-column prop="pointId" label="点位ID" width="100" sortable />
        <el-table-column prop="name" label="点位名称" show-overflow-tooltip />
        <el-table-column prop="type" label="点位类型" width="100">
          <template #default="scope">
            <el-tag :type="getTypeTagType(scope.row.type)">{{ scope.row.type }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="channelName" label="所属通道" />
        <el-table-column prop="address" label="寄存器地址" />
        <el-table-column prop="dataType" label="数据类型" />
        <el-table-column prop="unit" label="单位" width="80" />
        <el-table-column prop="scale" label="比例系数" width="100" />
        <el-table-column prop="offset" label="偏移量" width="100" />
        <el-table-column prop="description" label="描述" show-overflow-tooltip />
        <el-table-column label="操作" width="150" fixed="right">
          <template #default="scope">
            <el-button type="primary" link @click="handleEdit(scope.row)">编辑</el-button>
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
        <el-row :gutter="20">
          <el-col :span="12">
            <el-form-item label="点位ID" prop="pointId">
              <el-input-number v-model="form.pointId" :min="1" :max="999999" />
            </el-form-item>
          </el-col>
          <el-col :span="12">
            <el-form-item label="点位名称" prop="name">
              <el-input v-model="form.name" placeholder="请输入点位名称" />
            </el-form-item>
          </el-col>
        </el-row>
        <el-row :gutter="20">
          <el-col :span="12">
            <el-form-item label="点位类型" prop="type">
              <el-select v-model="form.type" placeholder="请选择点位类型">
                <el-option label="遥测(YC)" value="YC" />
                <el-option label="遥信(YX)" value="YX" />
                <el-option label="遥控(YK)" value="YK" />
                <el-option label="遥调(YT)" value="YT" />
              </el-select>
            </el-form-item>
          </el-col>
          <el-col :span="12">
            <el-form-item label="所属通道" prop="channelId">
              <el-select v-model="form.channelId" placeholder="请选择通道">
                <el-option label="通道1" value="1" />
                <el-option label="通道2" value="2" />
                <el-option label="通道3" value="3" />
              </el-select>
            </el-form-item>
          </el-col>
        </el-row>
        <el-row :gutter="20">
          <el-col :span="12">
            <el-form-item label="寄存器地址" prop="address">
              <el-input v-model="form.address" placeholder="例如: 40001" />
            </el-form-item>
          </el-col>
          <el-col :span="12">
            <el-form-item label="数据类型" prop="dataType">
              <el-select v-model="form.dataType" placeholder="请选择数据类型">
                <el-option label="INT16" value="int16" />
                <el-option label="UINT16" value="uint16" />
                <el-option label="INT32" value="int32" />
                <el-option label="UINT32" value="uint32" />
                <el-option label="FLOAT32" value="float32" />
                <el-option label="BOOL" value="bool" />
              </el-select>
            </el-form-item>
          </el-col>
        </el-row>
        <el-row :gutter="20">
          <el-col :span="12">
            <el-form-item label="单位" prop="unit">
              <el-input v-model="form.unit" placeholder="例如: kW, A, V" />
            </el-form-item>
          </el-col>
          <el-col :span="12">
            <el-form-item label="比例系数" prop="scale">
              <el-input-number v-model="form.scale" :precision="4" :step="0.1" />
            </el-form-item>
          </el-col>
        </el-row>
        <el-row :gutter="20">
          <el-col :span="12">
            <el-form-item label="偏移量" prop="offset">
              <el-input-number v-model="form.offset" :precision="4" :step="0.1" />
            </el-form-item>
          </el-col>
        </el-row>
        <el-form-item label="描述" prop="description">
          <el-input v-model="form.description" type="textarea" rows="3" placeholder="请输入描述信息" />
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
  pointId: '',
  type: '',
  channelId: ''
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
  pointId: 1,
  name: '',
  type: '',
  channelId: '',
  address: '',
  dataType: '',
  unit: '',
  scale: 1,
  offset: 0,
  description: ''
})

// 表单验证规则
const rules = {
  pointId: [
    { required: true, message: '请输入点位ID', trigger: 'blur' }
  ],
  name: [
    { required: true, message: '请输入点位名称', trigger: 'blur' }
  ],
  type: [
    { required: true, message: '请选择点位类型', trigger: 'change' }
  ],
  channelId: [
    { required: true, message: '请选择所属通道', trigger: 'change' }
  ],
  address: [
    { required: true, message: '请输入寄存器地址', trigger: 'blur' }
  ],
  dataType: [
    { required: true, message: '请选择数据类型', trigger: 'change' }
  ]
}

// 获取类型标签样式
const getTypeTagType = (type) => {
  const typeMap = {
    'YC': 'primary',
    'YX': 'success',
    'YK': 'warning',
    'YT': 'danger'
  }
  return typeMap[type] || 'info'
}

// 查询
const handleSearch = () => {
  currentPage.value = 1
  fetchData()
}

// 重置
const handleReset = () => {
  searchForm.name = ''
  searchForm.pointId = ''
  searchForm.type = ''
  searchForm.channelId = ''
  handleSearch()
}

// 导入
const handleImport = () => {
  ElMessage.info('导入点表功能待实现')
}

// 导出
const handleExport = () => {
  ElMessage.info('导出点表功能待实现')
}

// 新增
const handleAdd = () => {
  dialogTitle.value = '新增点位'
  Object.assign(form, {
    pointId: 1,
    name: '',
    type: '',
    channelId: '',
    address: '',
    dataType: '',
    unit: '',
    scale: 1,
    offset: 0,
    description: ''
  })
  dialogVisible.value = true
}

// 编辑
const handleEdit = (row) => {
  dialogTitle.value = '编辑点位'
  Object.assign(form, row)
  dialogVisible.value = true
}

// 删除
const handleDelete = async (row) => {
  try {
    await ElMessageBox.confirm(
      `确定要删除点位"${row.name}"吗？`,
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
    for (let i = 0; i < 10; i++) {
      mockData.push({
        pointId: 1000 + i,
        name: `电压_相${['A', 'B', 'C'][i % 3]}`,
        type: ['YC', 'YX', 'YK', 'YT'][i % 4],
        channelId: (i % 3 + 1).toString(),
        channelName: `通道${i % 3 + 1}`,
        address: `4000${i}`,
        dataType: ['float32', 'uint16', 'bool'][i % 3],
        unit: ['V', 'A', 'kW', ''][i % 4],
        scale: 0.1,
        offset: 0,
        description: `测量${['A', 'B', 'C'][i % 3]}相电压值`
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
.point-table {
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