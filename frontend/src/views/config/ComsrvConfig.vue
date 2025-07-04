<template>
  <div class="config-editor">
    <el-card>
      <template #header>
        <div class="card-header">
          <h2>Comsrv Configuration</h2>
          <div class="header-actions">
            <el-button type="primary" @click="saveConfig" :loading="loading">Save</el-button>
            <el-button @click="resetConfig">Reset</el-button>
            <el-button type="success" @click="applyConfig" :loading="applying">Apply</el-button>
          </div>
        </div>
      </template>
      
      <div v-if="loading" class="loading-container">
        <el-skeleton :rows="10" animated />
      </div>
      
      <div v-else-if="error" class="error-container">
        <el-alert
          :title="error"
          type="error"
          description="Unable to load configuration file. Please check service status or network connection."
          show-icon
        />
        <el-button class="mt-20" @click="fetchConfig">Retry</el-button>
      </div>
      
      <div v-else class="config-container">
        <el-tabs v-model="activeTab" type="card">
          <!-- Channel Configuration -->
          <el-tab-pane label="Channels" name="channels">
            <div class="tab-header">
              <h3>Channel Configuration</h3>
              <el-button type="primary" size="small" @click="addChannel">Add Channel</el-button>
            </div>
            
            <el-table :data="config.channels" style="width: 100%" border>
              <el-table-column label="Name" prop="name" width="180">
                <template #default="scope">
                  <el-input v-model="scope.row.name" placeholder="Channel name"></el-input>
                </template>
              </el-table-column>
              <el-table-column label="Type" prop="type" width="180">
                <template #default="scope">
                  <el-select v-model="scope.row.type" placeholder="Select type" style="width: 100%">
                    <el-option label="Modbus TCP" value="modbus_tcp"></el-option>
                    <el-option label="Modbus RTU" value="modbus_rtu"></el-option>
                    <el-option label="OPC UA" value="opcua"></el-option>
                    <el-option label="IEC104" value="iec104"></el-option>
                  </el-select>
                </template>
              </el-table-column>
              <el-table-column label="Enabled" prop="enabled" width="100">
                <template #default="scope">
                  <el-switch v-model="scope.row.enabled"></el-switch>
                </template>
              </el-table-column>
              <el-table-column label="Actions">
                <template #default="scope">
                  <el-button type="primary" size="small" @click="editChannelComm(scope.row)">Communication</el-button>
                  <el-button type="success" size="small" @click="openPointsDialog(scope.row)">Data Points</el-button>
                  <el-button type="danger" size="small" @click="removeChannel(scope.$index)">Delete</el-button>
                </template>
              </el-table-column>
            </el-table>
          </el-tab-pane>
          
          <!-- Raw Configuration File -->
          <el-tab-pane label="Raw Config" name="raw">
            <div class="tab-header">
              <h3>Raw Configuration File</h3>
              <div>
                <el-select v-model="configFormat" style="width: 120px" size="small">
                  <el-option label="TOML" value="toml"></el-option>
                  <el-option label="JSON" value="json"></el-option>
                  <el-option label="YAML" value="yaml"></el-option>
                </el-select>
                <el-button type="primary" size="small" @click="formatConfig" style="margin-left: 10px">Format</el-button>
              </div>
            </div>
            
            <div class="code-editor-container">
              <el-input
                type="textarea"
                v-model="rawConfig"
                :rows="20"
                resize="none"
                spellcheck="false"
              ></el-input>
            </div>
          </el-tab-pane>
        </el-tabs>
      </div>
    </el-card>
    
    <!-- Communication Configuration Dialog -->
    <el-dialog
      v-model="commDialogVisible"
      :title="`${currentChannel.name || 'New Channel'} - Communication Settings`"
      width="50%"
      destroy-on-close
    >
      <div v-if="currentChannel.type === 'modbus_tcp'">
        <el-form label-width="120px">
          <el-form-item label="IP Address">
            <el-input v-model="currentChannel.config.ip" placeholder="e.g. 192.168.1.100"></el-input>
          </el-form-item>
          <el-form-item label="Port">
            <el-input-number v-model="currentChannel.config.port" :min="1" :max="65535" placeholder="e.g. 502"></el-input-number>
          </el-form-item>
          <el-form-item label="Device ID">
            <el-input-number v-model="currentChannel.config.device_id" :min="1" :max="255" placeholder="e.g. 1"></el-input-number>
          </el-form-item>
          <el-form-item label="Timeout (ms)">
            <el-input-number v-model="currentChannel.config.timeout" :min="100" :max="10000" placeholder="e.g. 1000"></el-input-number>
          </el-form-item>
        </el-form>
      </div>
      
      <div v-else-if="currentChannel.type === 'modbus_rtu'">
        <el-form label-width="120px">
          <el-form-item label="Serial Port">
            <el-input v-model="currentChannel.config.port" placeholder="e.g. /dev/ttyS0"></el-input>
          </el-form-item>
          <el-form-item label="Baud Rate">
            <el-select v-model="currentChannel.config.baudrate" placeholder="Select baud rate">
              <el-option label="9600" :value="9600"></el-option>
              <el-option label="19200" :value="19200"></el-option>
              <el-option label="38400" :value="38400"></el-option>
              <el-option label="57600" :value="57600"></el-option>
              <el-option label="115200" :value="115200"></el-option>
            </el-select>
          </el-form-item>
          <el-form-item label="Data Bits">
            <el-select v-model="currentChannel.config.databits" placeholder="Select data bits">
              <el-option label="7" :value="7"></el-option>
              <el-option label="8" :value="8"></el-option>
            </el-select>
          </el-form-item>
          <el-form-item label="Stop Bits">
            <el-select v-model="currentChannel.config.stopbits" placeholder="Select stop bits">
              <el-option label="1" :value="1"></el-option>
              <el-option label="2" :value="2"></el-option>
            </el-select>
          </el-form-item>
          <el-form-item label="Parity">
            <el-select v-model="currentChannel.config.parity" placeholder="Select parity">
              <el-option label="None" value="N"></el-option>
              <el-option label="Odd" value="O"></el-option>
              <el-option label="Even" value="E"></el-option>
            </el-select>
          </el-form-item>
          <el-form-item label="Device ID">
            <el-input-number v-model="currentChannel.config.device_id" :min="1" :max="255" placeholder="e.g. 1"></el-input-number>
          </el-form-item>
        </el-form>
      </div>
      
      <div v-else-if="currentChannel.type === 'opcua'">
        <el-form label-width="120px">
          <el-form-item label="Server URL">
            <el-input v-model="currentChannel.config.server_url" placeholder="e.g. opc.tcp://server:4840"></el-input>
          </el-form-item>
          <el-form-item label="Username">
            <el-input v-model="currentChannel.config.username" placeholder="Username (optional)"></el-input>
          </el-form-item>
          <el-form-item label="Password">
            <el-input v-model="currentChannel.config.password" type="password" placeholder="Password (optional)"></el-input>
          </el-form-item>
          <el-form-item label="Security Policy">
            <el-select v-model="currentChannel.config.security_policy" placeholder="Select security policy">
              <el-option label="None" value="None"></el-option>
              <el-option label="Basic128Rsa15" value="Basic128Rsa15"></el-option>
              <el-option label="Basic256" value="Basic256"></el-option>
              <el-option label="Basic256Sha256" value="Basic256Sha256"></el-option>
            </el-select>
          </el-form-item>
        </el-form>
      </div>
      
      <div v-else-if="currentChannel.type === 'iec104'">
        <el-form label-width="120px">
          <el-form-item label="IP Address">
            <el-input v-model="currentChannel.config.ip" placeholder="e.g. 192.168.1.100"></el-input>
          </el-form-item>
          <el-form-item label="Port">
            <el-input-number v-model="currentChannel.config.port" :min="1" :max="65535" placeholder="e.g. 2404"></el-input-number>
          </el-form-item>
          <el-form-item label="ASDU Address">
            <el-input-number v-model="currentChannel.config.asdu_addr" :min="1" :max="65535" placeholder="e.g. 1"></el-input-number>
          </el-form-item>
          <el-form-item label="T1 Timeout(s)">
            <el-input-number v-model="currentChannel.config.t1" :min="1" :max="255" placeholder="e.g. 15"></el-input-number>
          </el-form-item>
        </el-form>
      </div>
      
      <template #footer>
        <span class="dialog-footer">
          <el-button @click="commDialogVisible = false">Cancel</el-button>
          <el-button type="primary" @click="saveChannelComm">Confirm</el-button>
        </span>
      </template>
    </el-dialog>
    
    <!-- Data Points Configuration Dialog -->
    <el-dialog
      v-model="pointsDialogVisible"
      :title="`${currentChannel.name || 'New Channel'} - Data Points`"
      width="80%"
      destroy-on-close
    >
      <div class="points-dialog-header">
        <el-button type="primary" size="small" @click="addPoint">Add Point</el-button>
        <el-button type="success" size="small" @click="importPoints">Import</el-button>
        <el-button type="warning" size="small" @click="exportPoints">Export</el-button>
      </div>
      
      <el-table :data="currentPoints" style="width: 100%" border max-height="500">
        <el-table-column label="Name" prop="name" width="180">
          <template #default="scope">
            <el-input v-model="scope.row.name" placeholder="Point name"></el-input>
          </template>
        </el-table-column>
        <el-table-column label="Address" prop="address" width="180">
          <template #default="scope">
            <el-input v-model="scope.row.address" placeholder="Point address"></el-input>
          </template>
        </el-table-column>
        <el-table-column label="Data Type" prop="dataType" width="150">
          <template #default="scope">
            <el-select v-model="scope.row.dataType" placeholder="Select data type" style="width: 100%">
              <el-option label="Integer" value="int"></el-option>
              <el-option label="Float" value="float"></el-option>
              <el-option label="Boolean" value="bool"></el-option>
              <el-option label="String" value="string"></el-option>
            </el-select>
          </template>
        </el-table-column>
        <el-table-column label="Access" prop="access" width="150">
          <template #default="scope">
            <el-select v-model="scope.row.access" placeholder="Select access type" style="width: 100%">
              <el-option label="Read Only" value="read"></el-option>
              <el-option label="Read/Write" value="readwrite"></el-option>
              <el-option label="Write Only" value="write"></el-option>
            </el-select>
          </template>
        </el-table-column>
        <el-table-column label="Description" prop="description">
          <template #default="scope">
            <el-input v-model="scope.row.description" placeholder="Description"></el-input>
          </template>
        </el-table-column>
        <el-table-column label="Actions" width="120">
          <template #default="scope">
            <el-button type="danger" size="small" @click="removePoint(scope.$index)">Delete</el-button>
          </template>
        </el-table-column>
      </el-table>
      
      <template #footer>
        <span class="dialog-footer">
          <el-button @click="pointsDialogVisible = false">Cancel</el-button>
          <el-button type="primary" @click="savePoints">Confirm</el-button>
        </span>
      </template>
    </el-dialog>
    
    <!-- Import Data Points Dialog -->
    <el-dialog
      v-model="importDialogVisible"
      title="Import Data Points"
      width="50%"
      destroy-on-close
    >
      <el-upload
        class="upload-demo"
        drag
        action="#"
        :auto-upload="false"
        :on-change="handleFileChange"
      >
        <el-icon class="el-icon--upload"><el-icon-upload-filled /></el-icon>
        <div class="el-upload__text">Drop file here or <em>click to upload</em></div>
        <template #tip>
          <div class="el-upload__tip">Supports .csv, .xlsx, .json files</div>
        </template>
      </el-upload>
      
      <template #footer>
        <span class="dialog-footer">
          <el-button @click="importDialogVisible = false">Cancel</el-button>
          <el-button type="primary" @click="confirmImport">Import</el-button>
        </span>
      </template>
    </el-dialog>
  </div>
</template>

<script>
import { mapState, mapActions } from 'vuex'
import { nextTick } from 'vue'

export default {
  name: 'ComsrvConfig',
  data() {
    return {
      activeTab: 'channels',
      config: {
        channels: []
      },
      rawConfig: '',
      configFormat: 'toml',
      commDialogVisible: false,
      pointsDialogVisible: false,
      importDialogVisible: false,
      currentChannel: {},
      currentChannelIndex: -1,
      currentPoints: [],
      importFile: null,
      applying: false
    }
  },
  computed: {
    ...mapState({
      loading: state => state.loading,
      error: state => state.error,
      savedConfig: state => state.configs.comsrv
    })
  },
  methods: {
    ...mapActions(['fetchConfig', 'saveConfig']),
    
    async loadConfig() {
      // Simulate loading configuration
      this.config = {
        channels: [
          {
            name: 'PCS Channel',
            type: 'modbus_tcp',
            enabled: true,
            config: {
              ip: '192.168.1.100',
              port: 502,
              device_id: 1,
              timeout: 1000
            },
            points: [
              { name: 'PCS_Power', address: '40001', dataType: 'float', access: 'read', description: 'PCS Power' },
              { name: 'PCS_Status', address: '40003', dataType: 'int', access: 'read', description: 'PCS Status' }
            ]
          },
          {
            name: 'BMS Channel',
            type: 'modbus_rtu',
            enabled: true,
            config: {
              port: '/dev/ttyS0',
              baudrate: 9600,
              databits: 8,
              stopbits: 1,
              parity: 'N',
              device_id: 1
            },
            points: [
              { name: 'Battery_SOC', address: '30001', dataType: 'float', access: 'read', description: 'Battery SOC' },
              { name: 'Battery_Voltage', address: '30003', dataType: 'float', access: 'read', description: 'Battery Voltage' }
            ]
          }
        ]
      }
      
      // Generate raw configuration file
      await nextTick()
      this.generateRawConfig()
    },
    
    generateRawConfig() {
      // Simple TOML format simulation
      if (this.configFormat === 'toml') {
        let toml = '# Comsrv Configuration\n\n'
        
        this.config.channels.forEach((channel) => {
          toml += `[[channels]]\n`
          toml += `name = "${channel.name}"\n`
          toml += `type = "${channel.type}"\n`
          toml += `enabled = ${channel.enabled}\n\n`
          
          toml += `[channels.config]\n`
          Object.keys(channel.config || {}).forEach(key => {
            const value = channel.config[key]
            if (typeof value === 'string') {
              toml += `${key} = "${value}"\n`
            } else {
              toml += `${key} = ${value}\n`
            }
          })
          
          toml += '\n# Points\n'
          if (channel.points && channel.points.length > 0) {
            channel.points.forEach(point => {
              toml += `[[channels.points]]\n`
              toml += `name = "${point.name}"\n`
              toml += `address = "${point.address}"\n`
              toml += `dataType = "${point.dataType}"\n`
              toml += `access = "${point.access}"\n`
              toml += `description = "${point.description}"\n\n`
            })
          }
          
          toml += '\n'
        })
        
        this.rawConfig = toml
      } else if (this.configFormat === 'json') {
        this.rawConfig = JSON.stringify(this.config, null, 2)
      } else if (this.configFormat === 'yaml') {
        // Simple YAML format simulation
        let yaml = 'channels:\n'
        
        this.config.channels.forEach((channel) => {
          yaml += `  - name: ${channel.name}\n`
          yaml += `    type: ${channel.type}\n`
          yaml += `    enabled: ${channel.enabled}\n`
          
          yaml += `    config:\n`
          Object.keys(channel.config || {}).forEach(key => {
            const value = channel.config[key]
            if (typeof value === 'string') {
              yaml += `      ${key}: "${value}"\n`
            } else {
              yaml += `      ${key}: ${value}\n`
            }
          })
          
          yaml += '    points:\n'
          if (channel.points && channel.points.length > 0) {
            channel.points.forEach(point => {
              yaml += `      - name: ${point.name}\n`
              yaml += `        address: ${point.address}\n`
              yaml += `        dataType: ${point.dataType}\n`
              yaml += `        access: ${point.access}\n`
              yaml += `        description: ${point.description}\n`
            })
          }
          
          yaml += '\n'
        })
        
        this.rawConfig = yaml
      }
    },
    
    formatConfig() {
      this.generateRawConfig()
    },
    
    addChannel() {
      const newChannel = {
        name: `Channel${this.config.channels.length + 1}`,
        type: 'modbus_tcp',
        enabled: true,
        config: {
          ip: '192.168.1.100',
          port: 502,
          device_id: 1,
          timeout: 1000
        },
        points: []
      }
      
      this.config.channels.push(newChannel)
      this.generateRawConfig()
    },
    
    removeChannel(index) {
      this.$confirm('Are you sure you want to delete this channel? This cannot be undone.', 'Warning', {
        confirmButtonText: 'OK',
        cancelButtonText: 'Cancel',
        type: 'warning'
      }).then(() => {
        this.config.channels.splice(index, 1)
        this.generateRawConfig()
        this.$message({
          type: 'success',
          message: 'Channel deleted successfully'
        })
      }).catch(() => {
        this.$message({
          type: 'info',
          message: 'Delete cancelled'
        })
      })
    },
    
    editChannelComm(channel) {
      this.currentChannel = JSON.parse(JSON.stringify(channel))
      this.currentChannelIndex = this.config.channels.findIndex(c => c.name === channel.name)
      
      // Ensure config object exists
      if (!this.currentChannel.config) {
        if (this.currentChannel.type === 'modbus_tcp') {
          this.currentChannel.config = {
            ip: '192.168.1.100',
            port: 502,
            device_id: 1,
            timeout: 1000
          }
        } else if (this.currentChannel.type === 'modbus_rtu') {
          this.currentChannel.config = {
            port: '/dev/ttyS0',
            baudrate: 9600,
            databits: 8,
            stopbits: 1,
            parity: 'N',
            device_id: 1
          }
        } else if (this.currentChannel.type === 'opcua') {
          this.currentChannel.config = {
            server_url: 'opc.tcp://server:4840',
            username: '',
            password: '',
            security_policy: 'None'
          }
        } else if (this.currentChannel.type === 'iec104') {
          this.currentChannel.config = {
            ip: '192.168.1.100',
            port: 2404,
            asdu_addr: 1,
            t1: 15
          }
        }
      }
      
      this.commDialogVisible = true
    },
    
    saveChannelComm() {
      if (this.currentChannelIndex >= 0) {
        this.config.channels[this.currentChannelIndex] = this.currentChannel
      }
      this.commDialogVisible = false
      this.generateRawConfig()
    },
    
    openPointsDialog(channel) {
      this.currentChannel = JSON.parse(JSON.stringify(channel))
      this.currentChannelIndex = this.config.channels.findIndex(c => c.name === channel.name)
      
      // Ensure points array exists
      this.currentPoints = this.currentChannel.points || []
      
      this.pointsDialogVisible = true
    },
    
    addPoint() {
      const newPoint = {
        name: `Point_${this.currentPoints.length + 1}`,
        address: '',
        dataType: 'float',
        access: 'read',
        description: ''
      }
      
      this.currentPoints.push(newPoint)
    },
    
    removePoint(index) {
      this.currentPoints.splice(index, 1)
    },
    
    savePoints() {
      if (this.currentChannelIndex >= 0) {
        this.config.channels[this.currentChannelIndex].points = [...this.currentPoints]
      }
      this.pointsDialogVisible = false
      this.generateRawConfig()
    },
    
    importPoints() {
      this.importDialogVisible = true
    },
    
    handleFileChange(file) {
      this.importFile = file
    },
    
    confirmImport() {
      if (!this.importFile) {
        this.$message.warning('Please select a file first')
        return
      }
      
      // Simulate successful import
      this.$message.success('Data points imported successfully')
      this.importDialogVisible = false
      
      // Simulate imported data points
      const importedPoints = [
        { name: 'Imported_Point_1', address: '40101', dataType: 'float', access: 'read', description: 'Imported point 1' },
        { name: 'Imported_Point_2', address: '40103', dataType: 'int', access: 'read', description: 'Imported point 2' },
        { name: 'Imported_Point_3', address: '40105', dataType: 'bool', access: 'readwrite', description: 'Imported point 3' }
      ]
      
      // Add to current channel's points
      this.currentPoints = [...this.currentPoints, ...importedPoints]
    },
    
    exportPoints() {
      // Simulate export functionality
      this.$message.success('Data points exported')
    },
    
    async saveConfig() {
      // Simulate saving configuration
      setTimeout(() => {
        this.$message.success('Configuration saved successfully')
      }, 1000)
    },
    
    resetConfig() {
      this.$confirm('Are you sure you want to reset the configuration? All unsaved changes will be lost.', 'Warning', {
        confirmButtonText: 'OK',
        cancelButtonText: 'Cancel',
        type: 'warning'
      }).then(() => {
        this.loadConfig()
        this.$message({
          type: 'success',
          message: 'Configuration reset successfully'
        })
      }).catch(() => {
        this.$message({
          type: 'info',
          message: 'Reset cancelled'
        })
      })
    },
    
    applyConfig() {
      this.applying = true
      
      // Simulate applying configuration
      setTimeout(() => {
        this.applying = false
        this.$message.success('Configuration applied successfully')
      }, 2000)
    }
  },
  watch: {
    configFormat() {
      this.$nextTick(() => {
        this.generateRawConfig()
      })
    },
    activeTab() {
      this.$nextTick(() => {
        if (this.activeTab === 'raw') {
          this.generateRawConfig()
        }
      })
    }
  },
  created() {
    this.loadConfig()
  }
}
</script>

<style scoped>
@import '@/styles/design-tokens.scss';

.config-editor {
  padding: var(--page-padding);
  background: var(--color-background);
  min-height: 100vh;
}

/* Apple Style Page Header */
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--spacing-xl);
}

.card-header h2 {
  font-size: var(--font-size-page-title);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  margin: 0;
  letter-spacing: -0.5px;
}

.header-actions {
  display: flex;
  gap: var(--spacing-md);
}

/* Tesla Style Cards */
.el-card {
  background: var(--color-surface);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  border: 1px solid var(--color-border);
  transition: all 0.3s ease;
}

.el-card:hover {
  box-shadow: var(--shadow-md);
  transform: translateY(-2px);
}

:deep(.el-card__header) {
  border-bottom: 1px solid var(--color-border);
  padding: var(--spacing-xl);
  background: var(--color-surface-hover);
}

:deep(.el-card__body) {
  padding: var(--spacing-xl);
}

/* Loading and Error States */
.loading-container, .error-container {
  padding: var(--spacing-xxl);
  text-align: center;
  min-height: 400px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}

.mt-20 {
  margin-top: var(--spacing-lg);
}

/* Configuration Container */
.config-container {
  margin-top: var(--spacing-lg);
}

/* Enhanced Tabs */
:deep(.el-tabs__header) {
  margin-bottom: var(--spacing-xl);
  border-bottom: 2px solid var(--color-border);
}

:deep(.el-tabs__item) {
  font-size: var(--font-size-base);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-secondary);
  padding: var(--spacing-md) var(--spacing-lg);
  transition: all 0.3s ease;
}

:deep(.el-tabs__item:hover) {
  color: var(--color-primary);
}

:deep(.el-tabs__item.is-active) {
  color: var(--color-primary);
  font-weight: var(--font-weight-semibold);
}

:deep(.el-tabs__active-bar) {
  background-color: var(--color-primary);
  height: 3px;
}

/* Tab Header */
.tab-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--spacing-xl);
  padding-bottom: var(--spacing-lg);
  border-bottom: 1px solid var(--color-border);
}

.tab-header h3 {
  font-size: var(--font-size-lg);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  margin: 0;
}

/* Code Editor */
.code-editor-container {
  border: 2px solid var(--color-border);
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--color-surface);
  transition: all 0.3s ease;
}

.code-editor-container:hover {
  border-color: var(--color-primary-light);
}

.code-editor-container :deep(.el-textarea__inner) {
  font-family: 'SF Mono', Monaco, 'Cascadia Code', 'Roboto Mono', Consolas, 'Courier New', monospace;
  font-size: var(--font-size-sm);
  line-height: 1.6;
  padding: var(--spacing-lg);
  background: var(--color-surface);
  color: var(--color-text-primary);
  border: none;
}

/* Enhanced Tables */
:deep(.el-table) {
  background: var(--color-surface);
  border-radius: var(--radius-md);
  overflow: hidden;
  box-shadow: var(--shadow-sm);
}

:deep(.el-table th) {
  background: var(--color-surface-hover);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  border-bottom: 1px solid var(--color-border);
}

:deep(.el-table td) {
  border-bottom: 1px solid var(--color-border-light);
}

:deep(.el-table__row:hover) {
  background: var(--color-surface-hover);
}

/* Enhanced Form Inputs */
:deep(.el-input__inner) {
  border-radius: var(--radius-md);
  border: 1px solid var(--color-border);
  padding: var(--spacing-sm) var(--spacing-md);
  font-size: var(--font-size-base);
  transition: all 0.3s ease;
}

:deep(.el-input__inner:focus) {
  border-color: var(--color-primary);
  box-shadow: 0 0 0 3px var(--color-primary-light);
}

:deep(.el-select .el-input__inner) {
  cursor: pointer;
}

/* Enhanced Buttons */
:deep(.el-button) {
  border-radius: var(--radius-md);
  font-weight: var(--font-weight-medium);
  padding: var(--spacing-sm) var(--spacing-lg);
  transition: all 0.3s ease;
  border: none;
  box-shadow: var(--shadow-sm);
}

:deep(.el-button--primary) {
  background: var(--gradient-primary);
  color: white;
}

:deep(.el-button--primary:hover) {
  transform: translateY(-1px);
  box-shadow: var(--shadow-md);
  opacity: 0.9;
}

:deep(.el-button--success) {
  background: var(--gradient-success);
  color: white;
}

:deep(.el-button--success:hover) {
  transform: translateY(-1px);
  box-shadow: var(--shadow-md);
  opacity: 0.9;
}

:deep(.el-button--warning) {
  background: var(--gradient-warning);
  color: white;
}

:deep(.el-button--danger) {
  background: var(--gradient-danger);
  color: white;
}

:deep(.el-button--default) {
  background: var(--color-surface);
  color: var(--color-text-primary);
  border: 1px solid var(--color-border);
}

:deep(.el-button--default:hover) {
  background: var(--color-surface-hover);
  border-color: var(--color-primary);
}

/* Enhanced Switch */
:deep(.el-switch__core) {
  border-radius: var(--radius-full);
  background: var(--color-border);
}

:deep(.el-switch.is-checked .el-switch__core) {
  background: var(--gradient-primary);
}

/* Dialog Header */
.points-dialog-header {
  margin-bottom: var(--spacing-xl);
  display: flex;
  gap: var(--spacing-md);
  padding-bottom: var(--spacing-lg);
  border-bottom: 1px solid var(--color-border);
}

/* Enhanced Dialogs */
:deep(.el-dialog) {
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-xl);
}

:deep(.el-dialog__header) {
  background: var(--color-surface-hover);
  padding: var(--spacing-xl);
  border-bottom: 1px solid var(--color-border);
}

:deep(.el-dialog__title) {
  font-size: var(--font-size-lg);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
}

:deep(.el-dialog__body) {
  padding: var(--spacing-xl);
  background: var(--color-surface);
}

:deep(.el-dialog__footer) {
  padding: var(--spacing-lg) var(--spacing-xl);
  background: var(--color-surface-hover);
  border-top: 1px solid var(--color-border);
}

/* Form Labels */
:deep(.el-form-item__label) {
  color: var(--color-text-secondary);
  font-weight: var(--font-weight-medium);
}

/* Upload Area */
:deep(.el-upload-dragger) {
  background: var(--color-surface);
  border: 2px dashed var(--color-border);
  border-radius: var(--radius-md);
  transition: all 0.3s ease;
}

:deep(.el-upload-dragger:hover) {
  border-color: var(--color-primary);
  background: var(--color-surface-hover);
}

:deep(.el-icon--upload) {
  font-size: 48px;
  color: var(--color-primary);
  margin-bottom: var(--spacing-md);
}

/* Alert Styling */
:deep(.el-alert) {
  border-radius: var(--radius-md);
  border: none;
  box-shadow: var(--shadow-sm);
}

:deep(.el-alert--error) {
  background: linear-gradient(135deg, #fee, #fdd);
}

/* Input Number */
:deep(.el-input-number) {
  width: 100%;
}

:deep(.el-input-number__increase),
:deep(.el-input-number__decrease) {
  background: var(--color-surface-hover);
  border-left: 1px solid var(--color-border);
}

/* Skeleton Loading */
:deep(.el-skeleton__item) {
  background: linear-gradient(90deg, var(--color-surface) 25%, var(--color-surface-hover) 50%, var(--color-surface) 75%);
  background-size: 200% 100%;
  animation: skeleton-loading 1.4s ease infinite;
}

@keyframes skeleton-loading {
  0% {
    background-position: 200% 0;
  }
  100% {
    background-position: -200% 0;
  }
}

/* Small Buttons in Tables */
:deep(.el-table .el-button--small) {
  padding: var(--spacing-xs) var(--spacing-sm);
  font-size: var(--font-size-sm);
}

/* Responsive Design */
@media (max-width: 768px) {
  .config-editor {
    padding: var(--spacing-md);
  }
  
  .card-header {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--spacing-md);
  }
  
  .header-actions {
    width: 100%;
  }
  
  .header-actions .el-button {
    flex: 1;
  }
  
  :deep(.el-dialog) {
    width: 95% !important;
  }
}
</style> 