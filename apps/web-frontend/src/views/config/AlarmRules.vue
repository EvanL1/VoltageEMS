<template>
  <div class="alarm-rules">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>告警规则管理</span>
          <el-button type="primary" @click="handleAdd">新建规则</el-button>
        </div>
      </template>

      <!-- 搜索区域 -->
      <el-form :inline="true" :model="searchForm" class="search-form">
        <el-form-item label="规则名称">
          <el-input v-model="searchForm.name" placeholder="请输入规则名称" clearable />
        </el-form-item>
        <el-form-item label="告警级别">
          <el-select v-model="searchForm.level" placeholder="请选择告警级别" clearable>
            <el-option label="紧急" value="critical" />
            <el-option label="重要" value="major" />
            <el-option label="次要" value="minor" />
            <el-option label="提示" value="info" />
          </el-select>
        </el-form-item>
        <el-form-item label="规则类型">
          <el-select v-model="searchForm.type" placeholder="请选择规则类型" clearable>
            <el-option label="阈值告警" value="threshold" />
            <el-option label="变化率告警" value="rate" />
            <el-option label="状态告警" value="status" />
            <el-option label="组合告警" value="composite" />
          </el-select>
        </el-form-item>
        <el-form-item label="启用状态">
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
        <el-table-column prop="id" label="规则ID" width="100" />
        <el-table-column prop="name" label="规则名称" show-overflow-tooltip />
        <el-table-column prop="type" label="规则类型" width="120">
          <template #default="scope">
            <el-tag>{{ getRuleTypeLabel(scope.row.type) }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="level" label="告警级别" width="100">
          <template #default="scope">
            <el-tag :type="getLevelTag(scope.row.level)">
              {{ getLevelLabel(scope.row.level) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="targetPoint" label="监测点位" />
        <el-table-column prop="condition" label="触发条件" show-overflow-tooltip />
        <el-table-column prop="enabled" label="状态" width="100">
          <template #default="scope">
            <el-switch
              v-model="scope.row.enabled"
              @change="handleStatusChange(scope.row)"
            />
          </template>
        </el-table-column>
        <el-table-column prop="triggerCount" label="触发次数" width="100" />
        <el-table-column prop="lastTriggerTime" label="最后触发时间" width="180" />
        <el-table-column label="操作" width="200" fixed="right">
          <template #default="scope">
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
        <el-row :gutter="20">
          <el-col :span="12">
            <el-form-item label="规则名称" prop="name">
              <el-input v-model="form.name" placeholder="请输入规则名称" />
            </el-form-item>
          </el-col>
          <el-col :span="12">
            <el-form-item label="规则类型" prop="type">
              <el-select v-model="form.type" placeholder="请选择规则类型" @change="handleTypeChange">
                <el-option label="阈值告警" value="threshold" />
                <el-option label="变化率告警" value="rate" />
                <el-option label="状态告警" value="status" />
                <el-option label="组合告警" value="composite" />
              </el-select>
            </el-form-item>
          </el-col>
        </el-row>
        
        <el-row :gutter="20">
          <el-col :span="12">
            <el-form-item label="告警级别" prop="level">
              <el-select v-model="form.level" placeholder="请选择告警级别">
                <el-option label="紧急" value="critical" />
                <el-option label="重要" value="major" />
                <el-option label="次要" value="minor" />
                <el-option label="提示" value="info" />
              </el-select>
            </el-form-item>
          </el-col>
          <el-col :span="12">
            <el-form-item label="监测点位" prop="targetPoint">
              <el-select v-model="form.targetPoint" placeholder="请选择监测点位">
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
          </el-col>
        </el-row>

        <!-- 阈值告警配置 -->
        <template v-if="form.type === 'threshold'">
          <el-row :gutter="20">
            <el-col :span="12">
              <el-form-item label="比较方式" prop="operator">
                <el-select v-model="form.operator" placeholder="请选择比较方式">
                  <el-option label="大于" value=">" />
                  <el-option label="大于等于" value=">=" />
                  <el-option label="小于" value="<" />
                  <el-option label="小于等于" value="<=" />
                  <el-option label="等于" value="=" />
                  <el-option label="不等于" value="!=" />
                </el-select>
              </el-form-item>
            </el-col>
            <el-col :span="12">
              <el-form-item label="阈值" prop="threshold">
                <el-input-number v-model="form.threshold" :precision="2" :step="0.1" />
              </el-form-item>
            </el-col>
          </el-row>
        </template>

        <!-- 变化率告警配置 -->
        <template v-if="form.type === 'rate'">
          <el-row :gutter="20">
            <el-col :span="12">
              <el-form-item label="时间窗口" prop="timeWindow">
                <el-input-number v-model="form.timeWindow" :min="1" :max="3600" />
                <span style="margin-left: 10px">秒</span>
              </el-form-item>
            </el-col>
            <el-col :span="12">
              <el-form-item label="变化率阈值" prop="rateThreshold">
                <el-input-number v-model="form.rateThreshold" :precision="2" :step="0.1" />
                <span style="margin-left: 10px">%</span>
              </el-form-item>
            </el-col>
          </el-row>
        </template>

        <el-row :gutter="20">
          <el-col :span="12">
            <el-form-item label="持续时间" prop="duration">
              <el-input-number v-model="form.duration" :min="0" :max="3600" />
              <span style="margin-left: 10px">秒</span>
            </el-form-item>
          </el-col>
          <el-col :span="12">
            <el-form-item label="死区时间" prop="deadband">
              <el-input-number v-model="form.deadband" :min="0" :max="3600" />
              <span style="margin-left: 10px">秒</span>
            </el-form-item>
          </el-col>
        </el-row>

        <el-form-item label="告警消息" prop="message">
          <el-input 
            v-model="form.message" 
            type="textarea" 
            rows="2" 
            placeholder="请输入告警消息模板，支持变量：{value}, {threshold}, {point_name}"
          />
        </el-form-item>

        <el-form-item label="通知方式" prop="notifications">
          <el-checkbox-group v-model="form.notifications">
            <el-checkbox label="web">Web通知</el-checkbox>
            <el-checkbox label="email">邮件通知</el-checkbox>
            <el-checkbox label="sms">短信通知</el-checkbox>
            <el-checkbox label="webhook">Webhook</el-checkbox>
          </el-checkbox-group>
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
import { ref, reactive, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'

// 搜索表单
const searchForm = reactive({
  name: '',
  level: '',
  type: '',
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
  type: '',
  level: '',
  targetPoint: '',
  operator: '',
  threshold: 0,
  timeWindow: 60,
  rateThreshold: 10,
  duration: 0,
  deadband: 300,
  message: '',
  notifications: ['web'],
  enabled: true
})

// 表单验证规则
const rules = {
  name: [
    { required: true, message: '请输入规则名称', trigger: 'blur' }
  ],
  type: [
    { required: true, message: '请选择规则类型', trigger: 'change' }
  ],
  level: [
    { required: true, message: '请选择告警级别', trigger: 'change' }
  ],
  targetPoint: [
    { required: true, message: '请选择监测点位', trigger: 'change' }
  ],
  message: [
    { required: true, message: '请输入告警消息', trigger: 'blur' }
  ]
}

// 获取规则类型标签
const getRuleTypeLabel = (type) => {
  const typeMap = {
    'threshold': '阈值告警',
    'rate': '变化率告警',
    'status': '状态告警',
    'composite': '组合告警'
  }
  return typeMap[type] || type
}

// 获取级别标签
const getLevelLabel = (level) => {
  const levelMap = {
    'critical': '紧急',
    'major': '重要',
    'minor': '次要',
    'info': '提示'
  }
  return levelMap[level] || level
}

const getLevelTag = (level) => {
  const levelMap = {
    'critical': 'danger',
    'major': 'warning',
    'minor': '',
    'info': 'info'
  }
  return levelMap[level] || 'info'
}

// 规则类型变化
const handleTypeChange = (value) => {
  // 根据规则类型调整表单
  if (value === 'threshold') {
    form.operator = '>'
    form.threshold = 0
  } else if (value === 'rate') {
    form.timeWindow = 60
    form.rateThreshold = 10
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
  searchForm.level = ''
  searchForm.type = ''
  searchForm.enabled = null
  handleSearch()
}

// 新增
const handleAdd = () => {
  dialogTitle.value = '新建告警规则'
  Object.assign(form, {
    name: '',
    type: '',
    level: '',
    targetPoint: '',
    operator: '',
    threshold: 0,
    timeWindow: 60,
    rateThreshold: 10,
    duration: 0,
    deadband: 300,
    message: '',
    notifications: ['web'],
    enabled: true
  })
  dialogVisible.value = true
}

// 编辑
const handleEdit = (row) => {
  dialogTitle.value = '编辑告警规则'
  Object.assign(form, row)
  dialogVisible.value = true
}

// 测试
const handleTest = (row) => {
  ElMessage.info(`正在测试规则：${row.name}`)
  // TODO: 调用测试API
  setTimeout(() => {
    ElMessage.success('规则测试通过')
  }, 1500)
}

// 删除
const handleDelete = async (row) => {
  try {
    await ElMessageBox.confirm(
      `确定要删除告警规则"${row.name}"吗？`,
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
  ElMessage.success(`规则${row.enabled ? '启用' : '禁用'}成功`)
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
    const types = ['threshold', 'rate', 'status', 'composite']
    const levels = ['critical', 'major', 'minor', 'info']
    
    for (let i = 0; i < 10; i++) {
      mockData.push({
        id: `R${1000 + i}`,
        name: `${getRuleTypeLabel(types[i % 4])}_${i + 1}`,
        type: types[i % 4],
        level: levels[i % 4],
        targetPoint: `${['电压_相A', '电流_相A', '总功率', '功率因数'][i % 4]}`,
        condition: i % 2 === 0 ? '> 220V' : '< 0.9',
        enabled: i % 3 !== 0,
        triggerCount: Math.floor(Math.random() * 100),
        lastTriggerTime: i % 2 === 0 ? new Date().toLocaleString() : '-',
        operator: '>',
        threshold: 220,
        message: '{point_name}当前值{value}超过阈值{threshold}',
        notifications: ['web', 'email']
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
.alarm-rules {
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