<template>
  <div class="point-table">
    <!-- Toolbar -->
    <div class="table-toolbar">
      <el-select v-model="selectedChannel" placeholder="Select Channel" @change="loadPoints">
        <el-option
          v-for="channel in channels"
          :key="channel.id"
          :label="channel.name"
          :value="channel.id"
        />
      </el-select>
      
      <el-select v-model="selectedType" placeholder="Point Type" clearable>
        <el-option label="Measurements (YC)" value="YC" />
        <el-option label="Signals (YX)" value="YX" />
        <el-option label="Controls (YK)" value="YK" />
        <el-option label="Adjustments (YT)" value="YT" />
      </el-select>
      
      <el-button type="primary" @click="addPoint" :disabled="!selectedChannel">
        <el-icon><Plus /></el-icon>
        Add Point
      </el-button>
      
      <el-button @click="importPoints" :disabled="!selectedChannel">
        <el-icon><Upload /></el-icon>
        Import CSV
      </el-button>
      
      <el-button @click="exportPoints" :disabled="!selectedChannel || points.length === 0">
        <el-icon><Download /></el-icon>
        Export CSV
      </el-button>
      
      <el-button type="danger" @click="deleteSelected" :disabled="selectedPoints.length === 0">
        <el-icon><Delete /></el-icon>
        Delete Selected
      </el-button>
      
      <div style="flex: 1"></div>
      
      <el-input
        v-model="searchQuery"
        placeholder="Search points..."
        :prefix-icon="Search"
        clearable
        style="width: 300px"
      />
    </div>
    
    <!-- Point Table -->
    <el-card>
      <el-table
        :data="filteredPoints"
        v-loading="loading"
        @selection-change="handleSelectionChange"
        row-key="id"
        style="width: 100%"
      >
        <el-table-column type="selection" width="55" />
        
        <el-table-column prop="point_id" label="Point ID" width="100" sortable>
          <template #default="{ row }">
            <el-input
              v-if="row.editing"
              v-model.number="row.point_id"
              size="small"
              @blur="validatePointId(row)"
            />
            <span v-else>{{ row.point_id }}</span>
          </template>
        </el-table-column>
        
        <el-table-column prop="type" label="Type" width="80">
          <template #default="{ row }">
            <el-select v-if="row.editing" v-model="row.type" size="small">
              <el-option label="YC" value="YC" />
              <el-option label="YX" value="YX" />
              <el-option label="YK" value="YK" />
              <el-option label="YT" value="YT" />
            </el-select>
            <el-tag v-else :type="getTypeColor(row.type)">{{ row.type }}</el-tag>
          </template>
        </el-table-column>
        
        <el-table-column prop="description" label="Description" min-width="200">
          <template #default="{ row }">
            <el-input
              v-if="row.editing"
              v-model="row.description"
              size="small"
            />
            <span v-else>{{ row.description }}</span>
          </template>
        </el-table-column>
        
        <el-table-column prop="address" label="Address" width="150">
          <template #default="{ row }">
            <el-input
              v-if="row.editing"
              v-model="row.address"
              size="small"
              placeholder="e.g., 1:3:30001"
            />
            <span v-else>{{ row.address }}</span>
          </template>
        </el-table-column>
        
        <el-table-column prop="scale" label="Scale" width="100">
          <template #default="{ row }">
            <el-input-number
              v-if="row.editing"
              v-model="row.scale"
              size="small"
              :precision="4"
              :step="0.1"
            />
            <span v-else>{{ row.scale }}</span>
          </template>
        </el-table-column>
        
        <el-table-column prop="unit" label="Unit" width="80">
          <template #default="{ row }">
            <el-input
              v-if="row.editing"
              v-model="row.unit"
              size="small"
            />
            <span v-else>{{ row.unit || '-' }}</span>
          </template>
        </el-table-column>
        
        <el-table-column prop="min" label="Min" width="100">
          <template #default="{ row }">
            <el-input-number
              v-if="row.editing"
              v-model="row.min"
              size="small"
              :precision="2"
            />
            <span v-else>{{ row.min !== null ? row.min : '-' }}</span>
          </template>
        </el-table-column>
        
        <el-table-column prop="max" label="Max" width="100">
          <template #default="{ row }">
            <el-input-number
              v-if="row.editing"
              v-model="row.max"
              size="small"
              :precision="2"
            />
            <span v-else>{{ row.max !== null ? row.max : '-' }}</span>
          </template>
        </el-table-column>
        
        <el-table-column prop="enabled" label="Enabled" width="80">
          <template #default="{ row }">
            <el-switch v-model="row.enabled" @change="togglePoint(row)" />
          </template>
        </el-table-column>
        
        <el-table-column label="Actions" width="150" fixed="right">
          <template #default="{ row }">
            <el-button
              v-if="!row.editing"
              type="primary"
              size="small"
              text
              @click="editPoint(row)"
            >
              Edit
            </el-button>
            <el-button
              v-else
              type="success"
              size="small"
              text
              @click="savePoint(row)"
            >
              Save
            </el-button>
            <el-button
              v-if="row.editing"
              size="small"
              text
              @click="cancelEdit(row)"
            >
              Cancel
            </el-button>
            <el-button
              v-else
              type="danger"
              size="small"
              text
              @click="deletePoint(row)"
            >
              Delete
            </el-button>
          </template>
        </el-table-column>
      </el-table>
      
      <el-pagination
        v-model:current-page="currentPage"
        v-model:page-size="pageSize"
        :page-sizes="[50, 100, 200, 500]"
        :total="filteredPoints.length"
        layout="total, sizes, prev, pager, next, jumper"
        style="margin-top: 20px"
      />
    </el-card>
    
    <!-- Add/Edit Point Dialog -->
    <el-dialog
      v-model="showAddDialog"
      title="Add New Point"
      width="600px"
    >
      <el-form :model="newPoint" :rules="pointRules" ref="pointFormRef" label-width="120px">
        <el-form-item label="Point ID" prop="point_id">
          <el-input-number v-model="newPoint.point_id" :min="1" :max="999999" />
        </el-form-item>
        
        <el-form-item label="Type" prop="type">
          <el-radio-group v-model="newPoint.type">
            <el-radio label="YC">Measurement (YC)</el-radio>
            <el-radio label="YX">Signal (YX)</el-radio>
            <el-radio label="YK">Control (YK)</el-radio>
            <el-radio label="YT">Adjustment (YT)</el-radio>
          </el-radio-group>
        </el-form-item>
        
        <el-form-item label="Description" prop="description">
          <el-input v-model="newPoint.description" placeholder="Point description" />
        </el-form-item>
        
        <el-form-item label="Address" prop="address">
          <el-input v-model="newPoint.address" placeholder="e.g., 1:3:30001">
            <template #append>
              <el-tooltip content="Format: slave_id:function_code:register">
                <el-icon><InfoFilled /></el-icon>
              </el-tooltip>
            </template>
          </el-input>
        </el-form-item>
        
        <el-form-item label="Scale Factor" prop="scale">
          <el-input-number v-model="newPoint.scale" :precision="4" :step="0.1" />
        </el-form-item>
        
        <el-form-item label="Unit">
          <el-select v-model="newPoint.unit" filterable allow-create>
            <el-option label="V" value="V" />
            <el-option label="A" value="A" />
            <el-option label="kW" value="kW" />
            <el-option label="kWh" value="kWh" />
            <el-option label="Hz" value="Hz" />
            <el-option label="°C" value="°C" />
            <el-option label="%" value="%" />
          </el-select>
        </el-form-item>
        
        <el-form-item label="Range">
          <el-col :span="11">
            <el-input-number v-model="newPoint.min" placeholder="Min" :precision="2" style="width: 100%" />
          </el-col>
          <el-col :span="2" style="text-align: center">to</el-col>
          <el-col :span="11">
            <el-input-number v-model="newPoint.max" placeholder="Max" :precision="2" style="width: 100%" />
          </el-col>
        </el-form-item>
        
        <el-form-item label="Enabled">
          <el-switch v-model="newPoint.enabled" />
        </el-form-item>
      </el-form>
      
      <template #footer>
        <el-button @click="showAddDialog = false">Cancel</el-button>
        <el-button type="primary" @click="confirmAddPoint">Add Point</el-button>
      </template>
    </el-dialog>
    
    <!-- Import Dialog -->
    <el-dialog
      v-model="showImportDialog"
      title="Import Points from CSV"
      width="700px"
    >
      <el-alert
        title="CSV Format"
        type="info"
        :closable="false"
        show-icon
        style="margin-bottom: 20px"
      >
        <template #default>
          Required columns: point_id, type, description, address<br>
          Optional columns: scale, unit, min, max, enabled
        </template>
      </el-alert>
      
      <el-upload
        class="upload-demo"
        drag
        action="#"
        :before-upload="handleImportFile"
        accept=".csv"
      >
        <el-icon class="el-icon--upload"><UploadFilled /></el-icon>
        <div class="el-upload__text">
          Drop CSV file here or <em>click to upload</em>
        </div>
      </el-upload>
      
      <div v-if="importPreview.length > 0" style="margin-top: 20px">
        <h4>Preview (first 5 rows):</h4>
        <el-table :data="importPreview" size="small" max-height="300">
          <el-table-column prop="point_id" label="ID" width="80" />
          <el-table-column prop="type" label="Type" width="60" />
          <el-table-column prop="description" label="Description" />
          <el-table-column prop="address" label="Address" width="120" />
          <el-table-column prop="scale" label="Scale" width="80" />
          <el-table-column prop="unit" label="Unit" width="60" />
        </el-table>
      </div>
      
      <template #footer>
        <el-button @click="cancelImport">Cancel</el-button>
        <el-button 
          type="primary" 
          @click="confirmImport" 
          :disabled="importPreview.length === 0"
        >
          Import {{ importPreview.length }} Points
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, reactive } from 'vue'
import {
  Plus,
  Upload,
  Download,
  Delete,
  Search,
  InfoFilled,
  UploadFilled
} from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'

// Mock channel data
const channels = ref([
  { id: 1, name: 'Main Power System' },
  { id: 2, name: 'Solar Panel Controller' },
  { id: 3, name: 'Energy Storage System' },
  { id: 4, name: 'Diesel Generator' }
])

// State
const selectedChannel = ref<number | null>(null)
const selectedType = ref('')
const searchQuery = ref('')
const loading = ref(false)
const points = ref<any[]>([])
const selectedPoints = ref<any[]>([])
const currentPage = ref(1)
const pageSize = ref(100)

// Dialog state
const showAddDialog = ref(false)
const showImportDialog = ref(false)
const pointFormRef = ref<FormInstance>()
const importPreview = ref<any[]>([])

// New point form
const newPoint = reactive({
  point_id: null as number | null,
  type: 'YC',
  description: '',
  address: '',
  scale: 1.0,
  unit: '',
  min: null as number | null,
  max: null as number | null,
  enabled: true
})

// Form rules
const pointRules = reactive<FormRules>({
  point_id: [
    { required: true, message: 'Point ID is required', trigger: 'blur' },
    {
      validator: (rule: any, value: any, callback: any) => {
        if (points.value.some(p => p.point_id === value && !p.editing)) {
          callback(new Error('Point ID already exists'))
        } else {
          callback()
        }
      },
      trigger: 'blur'
    }
  ],
  type: [
    { required: true, message: 'Point type is required', trigger: 'change' }
  ],
  description: [
    { required: true, message: 'Description is required', trigger: 'blur' }
  ],
  address: [
    { required: true, message: 'Address is required', trigger: 'blur' },
    {
      pattern: /^\d+:\d+:\d+$/,
      message: 'Invalid address format (use slave:function:register)',
      trigger: 'blur'
    }
  ],
  scale: [
    { required: true, message: 'Scale is required', trigger: 'blur' }
  ]
})

// Computed
const filteredPoints = computed(() => {
  let filtered = points.value
  
  if (selectedType.value) {
    filtered = filtered.filter(p => p.type === selectedType.value)
  }
  
  if (searchQuery.value) {
    const search = searchQuery.value.toLowerCase()
    filtered = filtered.filter(p => 
      p.point_id.toString().includes(search) ||
      p.description.toLowerCase().includes(search) ||
      p.address.toLowerCase().includes(search)
    )
  }
  
  // Pagination
  const start = (currentPage.value - 1) * pageSize.value
  const end = start + pageSize.value
  return filtered.slice(start, end)
})

// Methods
async function loadPoints() {
  if (!selectedChannel.value) return
  
  loading.value = true
  
  try {
    // TODO: Load points from API
    // Mock data for now
    points.value = generateMockPoints(selectedChannel.value)
    ElMessage.success('Points loaded successfully')
  } catch (error) {
    ElMessage.error('Failed to load points')
  } finally {
    loading.value = false
  }
}

function generateMockPoints(channelId: number) {
  const basePoints = [
    // YC - Measurements
    { type: 'YC', description: 'Voltage Phase A', address: '1:4:30001', scale: 0.1, unit: 'V', min: 0, max: 500 },
    { type: 'YC', description: 'Voltage Phase B', address: '1:4:30002', scale: 0.1, unit: 'V', min: 0, max: 500 },
    { type: 'YC', description: 'Voltage Phase C', address: '1:4:30003', scale: 0.1, unit: 'V', min: 0, max: 500 },
    { type: 'YC', description: 'Current Phase A', address: '1:4:30004', scale: 0.01, unit: 'A', min: 0, max: 1000 },
    { type: 'YC', description: 'Current Phase B', address: '1:4:30005', scale: 0.01, unit: 'A', min: 0, max: 1000 },
    { type: 'YC', description: 'Active Power', address: '1:4:30010', scale: 0.001, unit: 'kW', min: 0, max: 10000 },
    { type: 'YC', description: 'Reactive Power', address: '1:4:30011', scale: 0.001, unit: 'kVar', min: -5000, max: 5000 },
    { type: 'YC', description: 'Power Factor', address: '1:4:30012', scale: 0.001, unit: '', min: -1, max: 1 },
    { type: 'YC', description: 'Frequency', address: '1:4:30013', scale: 0.01, unit: 'Hz', min: 45, max: 55 },
    { type: 'YC', description: 'Temperature', address: '1:4:30020', scale: 0.1, unit: '°C', min: -40, max: 125 },
    
    // YX - Signals
    { type: 'YX', description: 'Circuit Breaker Status', address: '1:2:10001', scale: 1, unit: '', min: 0, max: 1 },
    { type: 'YX', description: 'Alarm Status', address: '1:2:10002', scale: 1, unit: '', min: 0, max: 1 },
    { type: 'YX', description: 'Communication Status', address: '1:2:10003', scale: 1, unit: '', min: 0, max: 1 },
    { type: 'YX', description: 'Fault Status', address: '1:2:10004', scale: 1, unit: '', min: 0, max: 1 },
    
    // YK - Controls
    { type: 'YK', description: 'Circuit Breaker Control', address: '1:5:20001', scale: 1, unit: '', min: 0, max: 1 },
    { type: 'YK', description: 'Reset Alarm', address: '1:5:20002', scale: 1, unit: '', min: 0, max: 1 },
    
    // YT - Adjustments
    { type: 'YT', description: 'Power Setpoint', address: '1:6:40001', scale: 0.001, unit: 'kW', min: 0, max: 1000 },
    { type: 'YT', description: 'Voltage Setpoint', address: '1:6:40002', scale: 0.1, unit: 'V', min: 200, max: 240 }
  ]
  
  return basePoints.map((point, index) => ({
    id: `${channelId}-${index + 1}`,
    point_id: (channelId * 10000) + index + 1,
    ...point,
    enabled: true,
    editing: false,
    _original: null
  }))
}

function handleSelectionChange(selection: any[]) {
  selectedPoints.value = selection
}

function addPoint() {
  // Reset form
  Object.assign(newPoint, {
    point_id: null,
    type: 'YC',
    description: '',
    address: '',
    scale: 1.0,
    unit: '',
    min: null,
    max: null,
    enabled: true
  })
  
  showAddDialog.value = true
}

async function confirmAddPoint() {
  const valid = await pointFormRef.value?.validate()
  if (!valid) return
  
  const newId = `${selectedChannel.value}-${Date.now()}`
  points.value.push({
    id: newId,
    ...newPoint,
    editing: false,
    _original: null
  })
  
  showAddDialog.value = false
  ElMessage.success('Point added successfully')
}

function editPoint(point: any) {
  point._original = { ...point }
  point.editing = true
}

function cancelEdit(point: any) {
  if (point._original) {
    Object.assign(point, point._original)
    point._original = null
  }
  point.editing = false
}

async function savePoint(point: any) {
  // Validate
  if (!point.point_id || !point.description || !point.address) {
    ElMessage.error('Please fill in all required fields')
    return
  }
  
  // Check for duplicate point ID
  const duplicate = points.value.find(p => 
    p.id !== point.id && p.point_id === point.point_id
  )
  if (duplicate) {
    ElMessage.error('Point ID already exists')
    return
  }
  
  point.editing = false
  point._original = null
  ElMessage.success('Point saved successfully')
}

function validatePointId(point: any) {
  const duplicate = points.value.find(p => 
    p.id !== point.id && p.point_id === point.point_id
  )
  if (duplicate) {
    ElMessage.error('Point ID already exists')
  }
}

async function deletePoint(point: any) {
  await ElMessageBox.confirm(
    `Delete point ${point.point_id} - ${point.description}?`,
    'Confirm Delete',
    {
      confirmButtonText: 'Delete',
      cancelButtonText: 'Cancel',
      type: 'warning'
    }
  )
  
  const index = points.value.findIndex(p => p.id === point.id)
  if (index > -1) {
    points.value.splice(index, 1)
    ElMessage.success('Point deleted')
  }
}

async function deleteSelected() {
  if (selectedPoints.value.length === 0) return
  
  await ElMessageBox.confirm(
    `Delete ${selectedPoints.value.length} selected points?`,
    'Confirm Delete',
    {
      confirmButtonText: 'Delete',
      cancelButtonText: 'Cancel',
      type: 'warning'
    }
  )
  
  const ids = selectedPoints.value.map(p => p.id)
  points.value = points.value.filter(p => !ids.includes(p.id))
  ElMessage.success(`${selectedPoints.value.length} points deleted`)
  selectedPoints.value = []
}

function togglePoint(point: any) {
  ElMessage.success(`Point ${point.enabled ? 'enabled' : 'disabled'}`)
}

function importPoints() {
  importPreview.value = []
  showImportDialog.value = true
}

function handleImportFile(file: File) {
  // Simulate CSV parsing
  // In real implementation, parse the CSV file
  importPreview.value = [
    { point_id: 10101, type: 'YC', description: 'Import Voltage A', address: '1:4:30101', scale: 0.1, unit: 'V' },
    { point_id: 10102, type: 'YC', description: 'Import Voltage B', address: '1:4:30102', scale: 0.1, unit: 'V' },
    { point_id: 10103, type: 'YC', description: 'Import Voltage C', address: '1:4:30103', scale: 0.1, unit: 'V' },
    { point_id: 20101, type: 'YX', description: 'Import Status 1', address: '1:2:10101', scale: 1, unit: '' },
    { point_id: 30101, type: 'YK', description: 'Import Control 1', address: '1:5:20101', scale: 1, unit: '' }
  ]
  
  return false // Prevent upload
}

function cancelImport() {
  showImportDialog.value = false
  importPreview.value = []
}

function confirmImport() {
  const baseId = Date.now()
  const newPoints = importPreview.value.map((point, index) => ({
    id: `${selectedChannel.value}-${baseId + index}`,
    ...point,
    min: null,
    max: null,
    enabled: true,
    editing: false,
    _original: null
  }))
  
  points.value.push(...newPoints)
  ElMessage.success(`Imported ${newPoints.length} points`)
  cancelImport()
}

function exportPoints() {
  // Create CSV content
  const headers = ['point_id', 'type', 'description', 'address', 'scale', 'unit', 'min', 'max', 'enabled']
  const rows = points.value.map(p => [
    p.point_id,
    p.type,
    p.description,
    p.address,
    p.scale,
    p.unit || '',
    p.min !== null ? p.min : '',
    p.max !== null ? p.max : '',
    p.enabled ? '1' : '0'
  ])
  
  const csv = [
    headers.join(','),
    ...rows.map(row => row.join(','))
  ].join('\n')
  
  // Download CSV
  const blob = new Blob([csv], { type: 'text/csv' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `channel_${selectedChannel.value}_points.csv`
  a.click()
  URL.revokeObjectURL(url)
  
  ElMessage.success('Points exported to CSV')
}

// Utility
function getTypeColor(type: string) {
  switch (type) {
    case 'YC':
      return 'primary'
    case 'YX':
      return 'success'
    case 'YK':
      return 'warning'
    case 'YT':
      return 'danger'
    default:
      return 'info'
  }
}
</script>

<style lang="scss" scoped>
.point-table {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 20px;
  
  .table-toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  
  .upload-demo {
    width: 100%;
  }
}
</style>