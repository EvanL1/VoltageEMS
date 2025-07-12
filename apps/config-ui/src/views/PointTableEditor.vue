<template>
  <div class="point-table-editor">
    <el-page-header @back="goBack">
      <template #content>
        <span class="text-large font-600 mr-3">
          {{ pointTableStore.currentTable?.name || '点表编辑' }}
        </span>
      </template>
      <template #extra>
        <el-button @click="validateTable">验证</el-button>
        <el-button type="primary" @click="exportToComsrv">导出为Comsrv格式</el-button>
      </template>
    </el-page-header>

    <el-tabs v-model="activeTab" class="editor-tabs">
      <el-tab-pane label="遥测 (YC)" name="telemetry">
        <PointTableSection
          :table-id="tableId"
          point-type="telemetry"
          :points="pointTableStore.currentTable?.telemetry || []"
          :mappings="pointTableStore.currentTable?.telemetry_mapping || []"
          csv-type="Telemetry"
          mapping-csv-type="TelemetryMapping"
          :protocol-type="pointTableStore.currentTable?.protocol_type || 'modbus_tcp'"
        />
      </el-tab-pane>
      
      <el-tab-pane label="遥信 (YX)" name="signal">
        <PointTableSection
          :table-id="tableId"
          point-type="signal"
          :points="pointTableStore.currentTable?.signal || []"
          :mappings="pointTableStore.currentTable?.signal_mapping || []"
          csv-type="Signal"
          mapping-csv-type="SignalMapping"
          :protocol-type="pointTableStore.currentTable?.protocol_type || 'modbus_tcp'"
        />
      </el-tab-pane>
      
      <el-tab-pane label="遥控 (YK)" name="control">
        <PointTableSection
          :table-id="tableId"
          point-type="control"
          :points="pointTableStore.currentTable?.control || []"
          :mappings="pointTableStore.currentTable?.control_mapping || []"
          csv-type="Control"
          mapping-csv-type="ControlMapping"
          :protocol-type="pointTableStore.currentTable?.protocol_type || 'modbus_tcp'"
        />
      </el-tab-pane>
      
      <el-tab-pane label="遥调 (YT)" name="adjustment">
        <PointTableSection
          :table-id="tableId"
          point-type="adjustment"
          :points="pointTableStore.currentTable?.adjustment || []"
          :mappings="pointTableStore.currentTable?.adjustment_mapping || []"
          csv-type="Adjustment"
          mapping-csv-type="AdjustmentMapping"
          :protocol-type="pointTableStore.currentTable?.protocol_type || 'modbus_tcp'"
        />
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { usePointTableStore } from '@/stores/pointTable'
import PointTableSection from '@/components/PointTableSection.vue'

const route = useRoute()
const router = useRouter()
const pointTableStore = usePointTableStore()

const tableId = ref('')
const activeTab = ref('telemetry')

onMounted(async () => {
  tableId.value = route.params.id
  await pointTableStore.fetchTable(tableId.value)
})

function goBack() {
  router.push('/point-table')
}

async function validateTable() {
  try {
    const result = await pointTableStore.validateTable(tableId.value)
    if (result.warnings.length > 0) {
      ElMessage.warning(`验证完成，有 ${result.warnings.length} 个警告`)
    }
  } catch (error) {
    console.error('验证失败:', error)
  }
}

async function exportToComsrv() {
  try {
    await pointTableStore.exportToComsrv(tableId.value)
  } catch (error) {
    console.error('导出失败:', error)
  }
}
</script>

<style scoped>
.point-table-editor {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.el-page-header {
  padding: 16px;
  background: white;
  border-bottom: 1px solid #e4e7ed;
}

.editor-tabs {
  flex: 1;
  padding: 16px;
  background: white;
}

:deep(.el-tabs__content) {
  height: calc(100% - 55px);
}

:deep(.el-tab-pane) {
  height: 100%;
}
</style>