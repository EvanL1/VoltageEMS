<template>
  <div class="point-table-manager">
    <el-card>
      <template #header>
        <div class="card-header">
          <span>点表管理</span>
          <el-button type="primary" @click="showCreateDialog = true">
            <el-icon><Plus /></el-icon>
            创建点表
          </el-button>
        </div>
      </template>

      <el-table
        :data="pointTableStore.tables"
        v-loading="pointTableStore.loading"
        style="width: 100%"
      >
        <el-table-column prop="name" label="名称" />
        <el-table-column prop="protocol_type" label="协议类型" width="120" />
        <el-table-column prop="channel_id" label="通道ID" width="100">
          <template #default="scope">
            {{ scope.row.channel_id || '-' }}
          </template>
        </el-table-column>
        <el-table-column label="点位数量" width="300">
          <template #default="scope">
            <div class="point-counts">
              <el-tag type="info">遥测: {{ scope.row.point_counts.telemetry }}</el-tag>
              <el-tag type="info">遥信: {{ scope.row.point_counts.signal }}</el-tag>
              <el-tag type="info">遥控: {{ scope.row.point_counts.control }}</el-tag>
              <el-tag type="info">遥调: {{ scope.row.point_counts.adjustment }}</el-tag>
            </div>
          </template>
        </el-table-column>
        <el-table-column prop="updated_at" label="更新时间" width="180">
          <template #default="scope">
            {{ formatDate(scope.row.updated_at) }}
          </template>
        </el-table-column>
        <el-table-column label="操作" width="200" fixed="right">
          <template #default="scope">
            <el-button size="small" @click="editTable(scope.row)">编辑</el-button>
            <el-button size="small" type="success" @click="exportTable(scope.row)">导出</el-button>
            <el-button size="small" type="danger" @click="deleteTable(scope.row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 创建点表对话框 -->
    <el-dialog
      v-model="showCreateDialog"
      title="创建点表"
      width="500px"
    >
      <el-form :model="createForm" label-width="100px">
        <el-form-item label="点表名称" required>
          <el-input v-model="createForm.name" placeholder="请输入点表名称" />
        </el-form-item>
        <el-form-item label="协议类型" required>
          <el-select v-model="createForm.protocolType" placeholder="请选择协议类型">
            <el-option label="Modbus TCP" value="modbus_tcp" />
            <el-option label="Modbus RTU" value="modbus_rtu" />
            <el-option label="IEC60870-5-104" value="iec104" />
            <el-option label="IEC60870-5-101" value="iec101" />
            <el-option label="CAN" value="can" />
          </el-select>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showCreateDialog = false">取消</el-button>
        <el-button type="primary" @click="createTable">创建</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessageBox } from 'element-plus'
import { Plus } from '@element-plus/icons-vue'
import { usePointTableStore } from '@/stores/pointTable'

const router = useRouter()
const pointTableStore = usePointTableStore()

const showCreateDialog = ref(false)
const createForm = ref({
  name: '',
  protocolType: ''
})

onMounted(() => {
  pointTableStore.fetchTables()
})

function formatDate(dateStr) {
  return new Date(dateStr).toLocaleString('zh-CN')
}

async function createTable() {
  if (!createForm.value.name || !createForm.value.protocolType) {
    ElMessage.error('请填写完整信息')
    return
  }

  try {
    await pointTableStore.createTable(createForm.value.name, createForm.value.protocolType)
    showCreateDialog.value = false
    createForm.value = { name: '', protocolType: '' }
  } catch (error) {
    console.error('创建点表失败:', error)
  }
}

function editTable(table) {
  router.push(`/point-table/${table.id}`)
}

async function exportTable(table) {
  try {
    await pointTableStore.exportToComsrv(table.id)
  } catch (error) {
    console.error('导出失败:', error)
  }
}

async function deleteTable(table) {
  try {
    await ElMessageBox.confirm(
      `确定要删除点表 "${table.name}" 吗？此操作不可恢复。`,
      '删除确认',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )
    
    await pointTableStore.deleteTable(table.id)
  } catch (error) {
    if (error !== 'cancel') {
      console.error('删除失败:', error)
    }
  }
}
</script>

<style scoped>
.point-table-manager {
  height: 100%;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.point-counts {
  display: flex;
  gap: 8px;
}

.point-counts .el-tag {
  font-size: 12px;
}
</style>