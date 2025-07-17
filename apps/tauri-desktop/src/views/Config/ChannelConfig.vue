<template>
  <div class="channel-config">
    <!-- Toolbar -->
    <div class="config-toolbar">
      <el-button type="primary" @click="showAddDialog = true">
        <el-icon><Plus /></el-icon>
        Add Channel
      </el-button>
      
      <el-button @click="importChannels">
        <el-icon><Upload /></el-icon>
        Import CSV
      </el-button>
      
      <el-button @click="exportChannels">
        <el-icon><Download /></el-icon>
        Export CSV
      </el-button>
      
      <el-button type="danger" :disabled="selectedChannels.length === 0" @click="deleteChannels">
        <el-icon><Delete /></el-icon>
        Delete Selected
      </el-button>
      
      <div style="flex: 1"></div>
      
      <el-input
        v-model="searchQuery"
        placeholder="Search channels..."
        :prefix-icon="Search"
        clearable
        style="width: 300px"
      />
    </div>
    
    <!-- Channel Table -->
    <el-card>
      <el-table
        :data="filteredChannels"
        v-loading="loading"
        @selection-change="handleSelectionChange"
        stripe
        style="width: 100%"
      >
        <el-table-column type="selection" width="55" />
        <el-table-column prop="id" label="ID" width="80" sortable />
        <el-table-column prop="name" label="Channel Name" min-width="200">
          <template #default="{ row }">
            <el-input
              v-if="row.editing"
              v-model="row.name"
              size="small"
              @blur="saveChannel(row)"
              @keyup.enter="saveChannel(row)"
            />
            <span v-else>{{ row.name }}</span>
          </template>
        </el-table-column>
        
        <el-table-column prop="protocol" label="Protocol" width="120">
          <template #default="{ row }">
            <el-select
              v-if="row.editing"
              v-model="row.protocol"
              size="small"
            >
              <el-option label="Modbus TCP" value="modbus_tcp" />
              <el-option label="Modbus RTU" value="modbus_rtu" />
              <el-option label="IEC 60870-5-104" value="iec104" />
              <el-option label="CAN Bus" value="can" />
            </el-select>
            <el-tag v-else>{{ row.protocol }}</el-tag>
          </template>
        </el-table-column>
        
        <el-table-column prop="connection" label="Connection" min-width="200">
          <template #default="{ row }">
            <el-input
              v-if="row.editing"
              v-model="row.connection"
              size="small"
              placeholder="IP:Port or Serial Port"
            />
            <span v-else>{{ row.connection }}</span>
          </template>
        </el-table-column>
        
        <el-table-column prop="status" label="Status" width="100">
          <template #default="{ row }">
            <el-switch
              v-model="row.enabled"
              active-text="Enabled"
              inactive-text="Disabled"
              @change="toggleChannel(row)"
            />
          </template>
        </el-table-column>
        
        <el-table-column prop="pointCount" label="Points" width="100">
          <template #default="{ row }">
            <el-button type="text" @click="showPointsDialog(row)">
              {{ row.pointCount }}
            </el-button>
          </template>
        </el-table-column>
        
        <el-table-column label="Actions" width="200" fixed="right">
          <template #default="{ row }">
            <el-button
              v-if="!row.editing"
              type="primary"
              size="small"
              text
              @click="editChannel(row)"
            >
              Edit
            </el-button>
            <el-button
              v-else
              type="success"
              size="small"
              text
              @click="saveChannel(row)"
            >
              Save
            </el-button>
            
            <el-button
              type="info"
              size="small"
              text
              @click="configureChannel(row)"
            >
              Configure
            </el-button>
            
            <el-button
              type="danger"
              size="small"
              text
              @click="deleteChannel(row)"
            >
              Delete
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>
    
    <!-- Add Channel Dialog -->
    <el-dialog
      v-model="showAddDialog"
      title="Add New Channel"
      width="600px"
    >
      <el-form :model="newChannel" :rules="channelRules" ref="channelForm" label-width="120px">
        <el-form-item label="Channel Name" prop="name">
          <el-input v-model="newChannel.name" placeholder="Enter channel name" />
        </el-form-item>
        
        <el-form-item label="Protocol" prop="protocol">
          <el-select v-model="newChannel.protocol" placeholder="Select protocol">
            <el-option label="Modbus TCP" value="modbus_tcp" />
            <el-option label="Modbus RTU" value="modbus_rtu" />
            <el-option label="IEC 60870-5-104" value="iec104" />
            <el-option label="CAN Bus" value="can" />
          </el-select>
        </el-form-item>
        
        <el-form-item label="Connection" prop="connection">
          <el-input v-model="newChannel.connection" placeholder="IP:Port or Serial Port">
            <template #append>
              <el-button @click="testConnection">Test</el-button>
            </template>
          </el-input>
        </el-form-item>
        
        <el-form-item label="Polling Interval">
          <el-input-number v-model="newChannel.pollingInterval" :min="100" :max="60000" :step="100" />
          <span style="margin-left: 10px">ms</span>
        </el-form-item>
        
        <el-form-item label="Timeout">
          <el-input-number v-model="newChannel.timeout" :min="100" :max="10000" :step="100" />
          <span style="margin-left: 10px">ms</span>
        </el-form-item>
        
        <el-form-item label="Retry Count">
          <el-input-number v-model="newChannel.retryCount" :min="0" :max="10" />
        </el-form-item>
        
        <el-form-item label="Auto Start">
          <el-switch v-model="newChannel.autoStart" />
        </el-form-item>
      </el-form>
      
      <template #footer>
        <el-button @click="showAddDialog = false">Cancel</el-button>
        <el-button type="primary" @click="addChannel">Add Channel</el-button>
      </template>
    </el-dialog>
    
    <!-- Channel Configuration Dialog -->
    <el-dialog
      v-model="showConfigDialog"
      :title="`Configure ${configChannel?.name}`"
      width="800px"
    >
      <el-tabs v-model="configTab">
        <el-tab-pane label="Basic Settings" name="basic">
          <el-form label-width="140px">
            <el-form-item label="Protocol Settings">
              <el-button @click="showProtocolSettings = true">Configure Protocol</el-button>
            </el-form-item>
            
            <el-form-item label="Logging Level">
              <el-select v-model="configChannel.loggingLevel">
                <el-option label="Debug" value="debug" />
                <el-option label="Info" value="info" />
                <el-option label="Warning" value="warning" />
                <el-option label="Error" value="error" />
              </el-select>
            </el-form-item>
            
            <el-form-item label="Data Processing">
              <el-checkbox v-model="configChannel.enableScaling">Enable Scaling</el-checkbox>
              <el-checkbox v-model="configChannel.enableDeadband">Enable Deadband</el-checkbox>
              <el-checkbox v-model="configChannel.enableTimestamp">Add Timestamp</el-checkbox>
            </el-form-item>
          </el-form>
        </el-tab-pane>
        
        <el-tab-pane label="Advanced" name="advanced">
          <el-form label-width="140px">
            <el-form-item label="Buffer Size">
              <el-input-number v-model="configChannel.bufferSize" :min="100" :max="10000" />
            </el-form-item>
            
            <el-form-item label="Queue Mode">
              <el-radio-group v-model="configChannel.queueMode">
                <el-radio label="fifo">FIFO</el-radio>
                <el-radio label="lifo">LIFO</el-radio>
                <el-radio label="priority">Priority</el-radio>
              </el-radio-group>
            </el-form-item>
            
            <el-form-item label="Error Handling">
              <el-select v-model="configChannel.errorHandling">
                <el-option label="Retry" value="retry" />
                <el-option label="Skip" value="skip" />
                <el-option label="Stop" value="stop" />
              </el-select>
            </el-form-item>
          </el-form>
        </el-tab-pane>
        
        <el-tab-pane label="Diagnostics" name="diagnostics">
          <div class="diagnostics">
            <el-descriptions :column="2" border>
              <el-descriptions-item label="Last Connected">
                2025-07-14 10:30:00
              </el-descriptions-item>
              <el-descriptions-item label="Uptime">
                2 days 14 hours
              </el-descriptions-item>
              <el-descriptions-item label="Messages Sent">
                125,432
              </el-descriptions-item>
              <el-descriptions-item label="Messages Received">
                125,410
              </el-descriptions-item>
              <el-descriptions-item label="Error Count">
                12
              </el-descriptions-item>
              <el-descriptions-item label="Success Rate">
                99.8%
              </el-descriptions-item>
            </el-descriptions>
            
            <el-button type="primary" style="margin-top: 20px" @click="resetStatistics">
              Reset Statistics
            </el-button>
          </div>
        </el-tab-pane>
      </el-tabs>
      
      <template #footer>
        <el-button @click="showConfigDialog = false">Close</el-button>
        <el-button type="primary" @click="saveConfiguration">Save Configuration</el-button>
      </template>
    </el-dialog>
    
    <!-- Import Dialog -->
    <el-dialog
      v-model="showImportDialog"
      title="Import Channels"
      width="600px"
    >
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
        <template #tip>
          <div class="el-upload__tip">
            CSV files with columns: name, protocol, connection, enabled
          </div>
        </template>
      </el-upload>
      
      <div v-if="importPreview.length > 0" style="margin-top: 20px">
        <h4>Preview (first 5 rows):</h4>
        <el-table :data="importPreview" size="small">
          <el-table-column prop="name" label="Name" />
          <el-table-column prop="protocol" label="Protocol" />
          <el-table-column prop="connection" label="Connection" />
          <el-table-column prop="enabled" label="Enabled" />
        </el-table>
      </div>
      
      <template #footer>
        <el-button @click="showImportDialog = false">Cancel</el-button>
        <el-button type="primary" @click="confirmImport" :disabled="importPreview.length === 0">
          Import {{ importPreview.length }} Channels
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
  UploadFilled
} from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox } from 'element-plus'

// Mock data
const channels = ref([
  {
    id: 1,
    name: 'Main Power Meter',
    protocol: 'modbus_tcp',
    connection: '192.168.1.100:502',
    enabled: true,
    pointCount: 120,
    editing: false
  },
  {
    id: 2,
    name: 'Solar Panel Controller',
    protocol: 'modbus_rtu',
    connection: '/dev/ttyUSB0',
    enabled: true,
    pointCount: 85,
    editing: false
  },
  {
    id: 3,
    name: 'Energy Storage System',
    protocol: 'iec104',
    connection: '192.168.1.101:2404',
    enabled: false,
    pointCount: 95,
    editing: false
  },
  {
    id: 4,
    name: 'Diesel Generator',
    protocol: 'can',
    connection: 'can0',
    enabled: true,
    pointCount: 60,
    editing: false
  }
])

// State
const loading = ref(false)
const searchQuery = ref('')
const selectedChannels = ref<any[]>([])
const showAddDialog = ref(false)
const showConfigDialog = ref(false)
const showImportDialog = ref(false)
const configTab = ref('basic')
const configChannel = ref<any>(null)
const showProtocolSettings = ref(false)

// New channel form
const newChannel = ref({
  name: '',
  protocol: '',
  connection: '',
  pollingInterval: 1000,
  timeout: 3000,
  retryCount: 3,
  autoStart: true
})

// Channel form rules
const channelRules = {
  name: [
    { required: true, message: 'Please enter channel name', trigger: 'blur' }
  ],
  protocol: [
    { required: true, message: 'Please select protocol', trigger: 'change' }
  ],
  connection: [
    { required: true, message: 'Please enter connection string', trigger: 'blur' }
  ]
}

// Import
const importPreview = ref<any[]>([])

// Computed
const filteredChannels = computed(() => {
  if (!searchQuery.value) return channels.value
  
  const query = searchQuery.value.toLowerCase()
  return channels.value.filter(channel => 
    channel.name.toLowerCase().includes(query) ||
    channel.protocol.toLowerCase().includes(query) ||
    channel.connection.toLowerCase().includes(query)
  )
})

// Methods
function handleSelectionChange(selection: any[]) {
  selectedChannels.value = selection
}

function editChannel(channel: any) {
  channel.editing = true
}

function saveChannel(channel: any) {
  channel.editing = false
  ElMessage.success('Channel saved successfully')
}

function deleteChannel(channel: any) {
  ElMessageBox.confirm(
    `Delete channel "${channel.name}"?`,
    'Confirm Delete',
    {
      confirmButtonText: 'Delete',
      cancelButtonText: 'Cancel',
      type: 'warning'
    }
  ).then(() => {
    const index = channels.value.findIndex(c => c.id === channel.id)
    if (index > -1) {
      channels.value.splice(index, 1)
      ElMessage.success('Channel deleted')
    }
  })
}

function deleteChannels() {
  ElMessageBox.confirm(
    `Delete ${selectedChannels.value.length} selected channels?`,
    'Confirm Delete',
    {
      confirmButtonText: 'Delete',
      cancelButtonText: 'Cancel',
      type: 'warning'
    }
  ).then(() => {
    const ids = selectedChannels.value.map(c => c.id)
    channels.value = channels.value.filter(c => !ids.includes(c.id))
    ElMessage.success(`${selectedChannels.value.length} channels deleted`)
    selectedChannels.value = []
  })
}

function toggleChannel(channel: any) {
  ElMessage.success(`Channel ${channel.enabled ? 'enabled' : 'disabled'}`)
}

function addChannel() {
  // TODO: Validate form
  const newId = Math.max(...channels.value.map(c => c.id)) + 1
  channels.value.push({
    id: newId,
    name: newChannel.value.name,
    protocol: newChannel.value.protocol,
    connection: newChannel.value.connection,
    enabled: newChannel.value.autoStart,
    pointCount: 0,
    editing: false
  })
  
  showAddDialog.value = false
  ElMessage.success('Channel added successfully')
  
  // Reset form
  newChannel.value = {
    name: '',
    protocol: '',
    connection: '',
    pollingInterval: 1000,
    timeout: 3000,
    retryCount: 3,
    autoStart: true
  }
}

function testConnection() {
  ElMessage.info('Testing connection...')
  setTimeout(() => {
    ElMessage.success('Connection successful')
  }, 1500)
}

function configureChannel(channel: any) {
  configChannel.value = {
    ...channel,
    loggingLevel: 'info',
    enableScaling: true,
    enableDeadband: false,
    enableTimestamp: true,
    bufferSize: 1000,
    queueMode: 'fifo',
    errorHandling: 'retry'
  }
  showConfigDialog.value = true
}

function saveConfiguration() {
  ElMessage.success('Configuration saved')
  showConfigDialog.value = false
}

function resetStatistics() {
  ElMessage.success('Statistics reset')
}

function showPointsDialog(channel: any) {
  // TODO: Show points configuration dialog
  ElMessage.info(`Configure ${channel.pointCount} points for ${channel.name}`)
}

function importChannels() {
  showImportDialog.value = true
}

function handleImportFile(file: File) {
  // Simulate CSV parsing
  importPreview.value = [
    { name: 'Import Channel 1', protocol: 'modbus_tcp', connection: '192.168.1.200:502', enabled: 'true' },
    { name: 'Import Channel 2', protocol: 'modbus_rtu', connection: '/dev/ttyUSB1', enabled: 'true' },
    { name: 'Import Channel 3', protocol: 'iec104', connection: '192.168.1.201:2404', enabled: 'false' }
  ]
  return false // Prevent upload
}

function confirmImport() {
  const newChannels = importPreview.value.map((item, index) => ({
    id: Math.max(...channels.value.map(c => c.id)) + index + 1,
    name: item.name,
    protocol: item.protocol,
    connection: item.connection,
    enabled: item.enabled === 'true',
    pointCount: 0,
    editing: false
  }))
  
  channels.value.push(...newChannels)
  ElMessage.success(`Imported ${newChannels.length} channels`)
  showImportDialog.value = false
  importPreview.value = []
}

function exportChannels() {
  // TODO: Implement CSV export
  ElMessage.success('Exporting channels to CSV...')
}
</script>

<style lang="scss" scoped>
.channel-config {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 20px;
  
  .config-toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  
  .upload-demo {
    width: 100%;
  }
  
  .diagnostics {
    padding: 20px;
  }
}
</style>