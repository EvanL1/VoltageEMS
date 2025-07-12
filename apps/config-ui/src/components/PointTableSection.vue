<template>
  <div class="point-table-section">
    <div class="section-header">
      <div class="header-left">
        <h3>点位定义</h3>
        <el-button-group>
          <el-button size="small" @click="showUploadDialog('definition')">
            <el-icon><Upload /></el-icon>
            上传CSV
          </el-button>
          <el-button size="small" @click="exportCsv('definition')">
            <el-icon><Download /></el-icon>
            导出CSV
          </el-button>
          <el-button size="small" @click="downloadTemplate('definition')">
            <el-icon><DocumentCopy /></el-icon>
            下载模板
          </el-button>
        </el-button-group>
      </div>
      <div class="header-right">
        <h3>协议映射</h3>
        <el-button-group>
          <el-button size="small" @click="showUploadDialog('mapping')">
            <el-icon><Upload /></el-icon>
            上传CSV
          </el-button>
          <el-button size="small" @click="exportCsv('mapping')">
            <el-icon><Download /></el-icon>
            导出CSV
          </el-button>
          <el-button size="small" @click="downloadTemplate('mapping')">
            <el-icon><DocumentCopy /></el-icon>
            下载模板
          </el-button>
        </el-button-group>
      </div>
    </div>

    <div class="tables-container">
      <!-- 点位定义表格 -->
      <div class="table-section">
        <el-table :data="points" height="400" style="width: 100%">
          <el-table-column prop="point_id" label="点位ID" width="80" />
          <el-table-column prop="signal_name" label="信号名称" />
          <el-table-column prop="chinese_name" label="中文名称" />
          <el-table-column prop="data_type" label="数据类型" width="100" />
          <el-table-column prop="scale" label="缩放" width="80" />
          <el-table-column prop="offset" label="偏移" width="80" />
          <el-table-column prop="unit" label="单位" width="80" />
          <el-table-column label="操作" width="100" fixed="right">
            <template #default="scope">
              <el-button size="small" type="text" @click="editPoint(scope.row)">编辑</el-button>
              <el-button size="small" type="text" @click="deletePoint(scope.row)">删除</el-button>
            </template>
          </el-table-column>
        </el-table>
      </div>

      <!-- 协议映射表格 -->
      <div class="table-section">
        <ProtocolMappingTable
          :mappings="formattedMappings"
          :protocol-type="protocolType"
          @edit="editMapping"
          @delete="deleteMapping"
        />
      </div>
    </div>

    <!-- CSV上传对话框 -->
    <el-dialog
      v-model="uploadDialog.visible"
      :title="`上传${uploadDialog.type === 'definition' ? '点位定义' : '协议映射'}CSV`"
      width="600px"
    >
      <el-upload
        ref="uploadRef"
        :auto-upload="false"
        :limit="1"
        accept=".csv"
        :on-change="handleFileChange"
        drag
      >
        <el-icon class="el-icon--upload"><upload-filled /></el-icon>
        <div class="el-upload__text">
          拖拽文件到此处或 <em>点击上传</em>
        </div>
        <template #tip>
          <div class="el-upload__tip">仅支持CSV格式文件</div>
        </template>
      </el-upload>
      
      <div v-if="csvPreview" class="csv-preview">
        <h4>预览:</h4>
        <el-input
          v-model="csvPreview"
          type="textarea"
          :rows="10"
          readonly
        />
      </div>

      <template #footer>
        <el-button @click="uploadDialog.visible = false">取消</el-button>
        <el-button type="primary" @click="uploadCsv" :disabled="!csvContent">上传</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Upload, Download, UploadFilled, DocumentCopy } from '@element-plus/icons-vue'
import { usePointTableStore } from '@/stores/pointTable'
import { invoke } from '@tauri-apps/api/core'
import ProtocolMappingTable from './ProtocolMappingTable.vue'

const props = defineProps({
  tableId: String,
  pointType: String,
  points: Array,
  mappings: Array,
  csvType: String,
  mappingCsvType: String,
  protocolType: String
})

const pointTableStore = usePointTableStore()
const uploadDialog = ref({ visible: false, type: '' })
const csvContent = ref('')
const csvPreview = ref('')

// 格式化映射数据，从枚举中提取实际数据
const formattedMappings = computed(() => {
  return props.mappings.map(mapping => {
    // 如果是枚举类型，提取内部数据
    if (mapping.Modbus) return mapping.Modbus
    if (mapping.IEC60870) return mapping.IEC60870
    if (mapping.CAN) return mapping.CAN
    // 如果已经是普通对象，直接返回
    return mapping
  })
})

function showUploadDialog(type) {
  uploadDialog.value = { visible: true, type }
  csvContent.value = ''
  csvPreview.value = ''
}

function handleFileChange(file) {
  const reader = new FileReader()
  reader.onload = (e) => {
    csvContent.value = e.target.result
    csvPreview.value = csvContent.value.substring(0, 1000) + (csvContent.value.length > 1000 ? '...' : '')
  }
  reader.readAsText(file.raw)
}

async function uploadCsv() {
  if (!csvContent.value) {
    ElMessage.error('请选择文件')
    return
  }

  try {
    const csvType = uploadDialog.value.type === 'definition' ? props.csvType : props.mappingCsvType
    await pointTableStore.uploadCsv(props.tableId, csvType, csvContent.value)
    uploadDialog.value.visible = false
  } catch (error) {
    console.error('上传失败:', error)
  }
}

async function exportCsv(type) {
  try {
    const csvType = type === 'definition' ? props.csvType : props.mappingCsvType
    const content = await pointTableStore.exportCsv(props.tableId, csvType)
    
    // 创建下载链接
    const blob = new Blob([content], { type: 'text/csv;charset=utf-8;' })
    const link = document.createElement('a')
    link.href = URL.createObjectURL(blob)
    link.download = `${props.pointType}_${type}.csv`
    link.click()
    URL.revokeObjectURL(link.href)
    
    ElMessage.success('导出成功')
  } catch (error) {
    console.error('导出失败:', error)
  }
}

function editPoint(point) {
  // TODO: 实现点位编辑功能
  ElMessage.info('点位编辑功能开发中...')
}

async function deletePoint(point) {
  try {
    await ElMessageBox.confirm(
      `确定要删除点位 "${point.signal_name}" 吗？`,
      '删除确认',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }
    )
    
    await pointTableStore.deletePoint(props.tableId, props.pointType, point.point_id)
  } catch (error) {
    if (error !== 'cancel') {
      console.error('删除失败:', error)
    }
  }
}

function editMapping(mapping) {
  // TODO: 实现映射编辑功能
  ElMessage.info('映射编辑功能开发中...')
}

function deleteMapping(mapping) {
  // 映射数据通过CSV管理，不单独删除
  ElMessage.warning('映射数据请通过上传新的CSV文件来更新')
}

async function downloadTemplate(type) {
  try {
    const csvType = type === 'definition' ? props.csvType : props.mappingCsvType
    const content = await invoke('get_protocol_csv_template', {
      protocolType: props.protocolType,
      csvType
    })
    
    // 创建下载链接
    const blob = new Blob([content], { type: 'text/csv;charset=utf-8;' })
    const link = document.createElement('a')
    link.href = URL.createObjectURL(blob)
    link.download = `${props.protocolType}_${props.pointType}_${type}_template.csv`
    link.click()
    URL.revokeObjectURL(link.href)
    
    ElMessage.success('模板下载成功')
  } catch (error) {
    ElMessage.error(`下载模板失败: ${error}`)
  }
}
</script>

<style scoped>
.point-table-section {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.section-header {
  display: flex;
  justify-content: space-between;
  margin-bottom: 16px;
}

.header-left, .header-right {
  display: flex;
  align-items: center;
  gap: 16px;
}

.header-left h3, .header-right h3 {
  margin: 0;
  font-size: 16px;
}

.tables-container {
  display: flex;
  gap: 16px;
  flex: 1;
}

.table-section {
  flex: 1;
}

.csv-preview {
  margin-top: 16px;
}

.csv-preview h4 {
  margin: 0 0 8px 0;
}
</style>