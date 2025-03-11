<template>
  <div class="config-editor">
    <el-card>
      <template #header>
        <div class="card-header">
          <h2>Comsrv 配置</h2>
          <div class="header-actions">
            <el-button type="primary" @click="saveConfig" :loading="loading">保存配置</el-button>
            <el-button @click="resetConfig">重置</el-button>
            <el-button type="success" @click="applyConfig" :loading="applying">应用配置</el-button>
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
          description="无法加载配置文件，请检查服务状态或网络连接。"
          show-icon
        />
        <el-button class="mt-20" @click="fetchConfig">重试</el-button>
      </div>
      
      <div v-else class="config-container">
        <el-tabs v-model="activeTab" type="card">
          <!-- 通道配置 -->
          <el-tab-pane label="通道配置" name="channels">
            <div class="tab-header">
              <h3>通道配置</h3>
              <el-button type="primary" size="small" @click="addChannel">添加通道</el-button>
            </div>
            
            <el-table :data="config.channels" style="width: 100%" border>
              <el-table-column label="通道名称" prop="name" width="180">
                <template #default="scope">
                  <el-input v-model="scope.row.name" placeholder="通道名称"></el-input>
                </template>
              </el-table-column>
              <el-table-column label="通道类型" prop="type" width="180">
                <template #default="scope">
                  <el-select v-model="scope.row.type" placeholder="选择通道类型" style="width: 100%">
                    <el-option label="Modbus TCP" value="modbus_tcp"></el-option>
                    <el-option label="Modbus RTU" value="modbus_rtu"></el-option>
                    <el-option label="OPC UA" value="opcua"></el-option>
                    <el-option label="IEC104" value="iec104"></el-option>
                  </el-select>
                </template>
              </el-table-column>
              <el-table-column label="启用" prop="enabled" width="100">
                <template #default="scope">
                  <el-switch v-model="scope.row.enabled"></el-switch>
                </template>
              </el-table-column>
              <el-table-column label="操作">
                <template #default="scope">
                  <el-button type="primary" size="small" @click="editChannelComm(scope.row)">通信配置</el-button>
                  <el-button type="success" size="small" @click="openPointsDialog(scope.row)">点表配置</el-button>
                  <el-button type="danger" size="small" @click="removeChannel(scope.$index)">删除</el-button>
                </template>
              </el-table-column>
            </el-table>
          </el-tab-pane>
          
          <!-- 原始配置文件 -->
          <el-tab-pane label="原始配置文件" name="raw">
            <div class="tab-header">
              <h3>原始配置文件</h3>
              <div>
                <el-select v-model="configFormat" style="width: 120px" size="small">
                  <el-option label="TOML" value="toml"></el-option>
                  <el-option label="JSON" value="json"></el-option>
                  <el-option label="YAML" value="yaml"></el-option>
                </el-select>
                <el-button type="primary" size="small" @click="formatConfig" style="margin-left: 10px">格式化</el-button>
              </div>
            </div>
            
            <div class="code-editor-container">
              <el-input
                type="textarea"
                v-model="rawConfig"
                :rows="20"
                resize="none"
                spellcheck="false"
                font-family="monospace"
              ></el-input>
            </div>
          </el-tab-pane>
        </el-tabs>
      </div>
    </el-card>
    
    <!-- 通信配置对话框 -->
    <el-dialog
      v-model="commDialogVisible"
      :title="`${currentChannel.name || '新通道'} - 通信配置`"
      width="50%"
    >
      <div v-if="currentChannel.type === 'modbus_tcp'">
        <el-form label-width="120px">
          <el-form-item label="IP地址">
            <el-input v-model="currentChannel.config.ip" placeholder="例如: 192.168.1.100"></el-input>
          </el-form-item>
          <el-form-item label="端口">
            <el-input-number v-model="currentChannel.config.port" :min="1" :max="65535" placeholder="例如: 502"></el-input-number>
          </el-form-item>
          <el-form-item label="设备ID">
            <el-input-number v-model="currentChannel.config.device_id" :min="1" :max="255" placeholder="例如: 1"></el-input-number>
          </el-form-item>
          <el-form-item label="超时时间(ms)">
            <el-input-number v-model="currentChannel.config.timeout" :min="100" :max="10000" placeholder="例如: 1000"></el-input-number>
          </el-form-item>
        </el-form>
      </div>
      
      <div v-else-if="currentChannel.type === 'modbus_rtu'">
        <el-form label-width="120px">
          <el-form-item label="串口">
            <el-input v-model="currentChannel.config.port" placeholder="例如: /dev/ttyS0"></el-input>
          </el-form-item>
          <el-form-item label="波特率">
            <el-select v-model="currentChannel.config.baudrate" placeholder="选择波特率">
              <el-option label="9600" :value="9600"></el-option>
              <el-option label="19200" :value="19200"></el-option>
              <el-option label="38400" :value="38400"></el-option>
              <el-option label="57600" :value="57600"></el-option>
              <el-option label="115200" :value="115200"></el-option>
            </el-select>
          </el-form-item>
          <el-form-item label="数据位">
            <el-select v-model="currentChannel.config.databits" placeholder="选择数据位">
              <el-option label="7" :value="7"></el-option>
              <el-option label="8" :value="8"></el-option>
            </el-select>
          </el-form-item>
          <el-form-item label="停止位">
            <el-select v-model="currentChannel.config.stopbits" placeholder="选择停止位">
              <el-option label="1" :value="1"></el-option>
              <el-option label="2" :value="2"></el-option>
            </el-select>
          </el-form-item>
          <el-form-item label="校验位">
            <el-select v-model="currentChannel.config.parity" placeholder="选择校验位">
              <el-option label="无" value="N"></el-option>
              <el-option label="奇校验" value="O"></el-option>
              <el-option label="偶校验" value="E"></el-option>
            </el-select>
          </el-form-item>
          <el-form-item label="设备ID">
            <el-input-number v-model="currentChannel.config.device_id" :min="1" :max="255" placeholder="例如: 1"></el-input-number>
          </el-form-item>
        </el-form>
      </div>
      
      <div v-else-if="currentChannel.type === 'opcua'">
        <el-form label-width="120px">
          <el-form-item label="服务器URL">
            <el-input v-model="currentChannel.config.server_url" placeholder="例如: opc.tcp://server:4840"></el-input>
          </el-form-item>
          <el-form-item label="用户名">
            <el-input v-model="currentChannel.config.username" placeholder="用户名 (可选)"></el-input>
          </el-form-item>
          <el-form-item label="密码">
            <el-input v-model="currentChannel.config.password" type="password" placeholder="密码 (可选)"></el-input>
          </el-form-item>
          <el-form-item label="安全策略">
            <el-select v-model="currentChannel.config.security_policy" placeholder="选择安全策略">
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
          <el-form-item label="IP地址">
            <el-input v-model="currentChannel.config.ip" placeholder="例如: 192.168.1.100"></el-input>
          </el-form-item>
          <el-form-item label="端口">
            <el-input-number v-model="currentChannel.config.port" :min="1" :max="65535" placeholder="例如: 2404"></el-input-number>
          </el-form-item>
          <el-form-item label="ASDU地址">
            <el-input-number v-model="currentChannel.config.asdu_addr" :min="1" :max="65535" placeholder="例如: 1"></el-input-number>
          </el-form-item>
          <el-form-item label="T1超时(s)">
            <el-input-number v-model="currentChannel.config.t1" :min="1" :max="255" placeholder="例如: 15"></el-input-number>
          </el-form-item>
        </el-form>
      </div>
      
      <template #footer>
        <span class="dialog-footer">
          <el-button @click="commDialogVisible = false">取消</el-button>
          <el-button type="primary" @click="saveChannelComm">确认</el-button>
        </span>
      </template>
    </el-dialog>
    
    <!-- 点表配置对话框 -->
    <el-dialog
      v-model="pointsDialogVisible"
      :title="`${currentChannel.name || '新通道'} - 点表配置`"
      width="80%"
      :destroy-on-close="false"
    >
      <div class="points-dialog-header">
        <el-button type="primary" size="small" @click="addPoint">添加点位</el-button>
        <el-button type="success" size="small" @click="importPoints">导入点表</el-button>
        <el-button type="warning" size="small" @click="exportPoints">导出点表</el-button>
      </div>
      
      <el-table :data="currentChannel.points || []" style="width: 100%" border max-height="500">
        <el-table-column label="点位名称" prop="name" width="180">
          <template #default="scope">
            <el-input v-model="scope.row.name" placeholder="点位名称"></el-input>
          </template>
        </el-table-column>
        <el-table-column label="点位地址" prop="address" width="180">
          <template #default="scope">
            <el-input v-model="scope.row.address" placeholder="点位地址"></el-input>
          </template>
        </el-table-column>
        <el-table-column label="数据类型" prop="dataType" width="150">
          <template #default="scope">
            <el-select v-model="scope.row.dataType" placeholder="选择数据类型" style="width: 100%">
              <el-option label="整数" value="int"></el-option>
              <el-option label="浮点数" value="float"></el-option>
              <el-option label="布尔值" value="bool"></el-option>
              <el-option label="字符串" value="string"></el-option>
            </el-select>
          </template>
        </el-table-column>
        <el-table-column label="读写类型" prop="access" width="150">
          <template #default="scope">
            <el-select v-model="scope.row.access" placeholder="选择读写类型" style="width: 100%">
              <el-option label="只读" value="read"></el-option>
              <el-option label="读写" value="readwrite"></el-option>
              <el-option label="只写" value="write"></el-option>
            </el-select>
          </template>
        </el-table-column>
        <el-table-column label="描述" prop="description">
          <template #default="scope">
            <el-input v-model="scope.row.description" placeholder="点位描述"></el-input>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="120">
          <template #default="scope">
            <el-button type="danger" size="small" @click="removePoint(scope.$index)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>
      
      <template #footer>
        <span class="dialog-footer">
          <el-button @click="pointsDialogVisible = false">取消</el-button>
          <el-button type="primary" @click="savePoints">确认</el-button>
        </span>
      </template>
    </el-dialog>
    
    <!-- 导入点表对话框 -->
    <el-dialog
      v-model="importDialogVisible"
      title="导入点表"
      width="50%"
    >
      <el-upload
        class="upload-demo"
        drag
        action="#"
        :auto-upload="false"
        :on-change="handleFileChange"
      >
        <el-icon class="el-icon--upload"><el-icon-upload-filled /></el-icon>
        <div class="el-upload__text">拖拽文件到此处或 <em>点击上传</em></div>
        <template #tip>
          <div class="el-upload__tip">支持 .csv, .xlsx, .json 格式文件</div>
        </template>
      </el-upload>
      
      <template #footer>
        <span class="dialog-footer">
          <el-button @click="importDialogVisible = false">取消</el-button>
          <el-button type="primary" @click="confirmImport">确认导入</el-button>
        </span>
      </template>
    </el-dialog>
  </div>
</template>

<script>
import { mapState, mapActions } from 'vuex'

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
      // 模拟加载配置
      this.config = {
        channels: [
          {
            name: 'PCS通道',
            type: 'modbus_tcp',
            enabled: true,
            config: {
              ip: '192.168.1.100',
              port: 502,
              device_id: 1,
              timeout: 1000
            },
            points: [
              { name: 'PCS_Power', address: '40001', dataType: 'float', access: 'read', description: 'PCS功率' },
              { name: 'PCS_Status', address: '40003', dataType: 'int', access: 'read', description: 'PCS状态' }
            ]
          },
          {
            name: 'BMS通道',
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
              { name: 'Battery_SOC', address: '30001', dataType: 'float', access: 'read', description: '电池SOC' },
              { name: 'Battery_Voltage', address: '30003', dataType: 'float', access: 'read', description: '电池电压' }
            ]
          }
        ]
      }
      
      // 生成原始配置文件
      this.generateRawConfig()
    },
    
    generateRawConfig() {
      // 简单模拟TOML格式
      if (this.configFormat === 'toml') {
        let toml = '# Comsrv Configuration\n\n'
        
        this.config.channels.forEach((channel) => {
          toml += `[[channels]]\n`
          toml += `name = "${channel.name}"\n`
          toml += `type = "${channel.type}"\n`
          toml += `enabled = ${channel.enabled}\n\n`
          
          toml += `[channels.config]\n`
          Object.keys(channel.config).forEach(key => {
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
        // 简单模拟YAML格式
        let yaml = 'channels:\n'
        
        this.config.channels.forEach((channel) => {
          yaml += `  - name: ${channel.name}\n`
          yaml += `    type: ${channel.type}\n`
          yaml += `    enabled: ${channel.enabled}\n`
          
          yaml += `    config:\n`
          Object.keys(channel.config).forEach(key => {
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
        name: `通道${this.config.channels.length + 1}`,
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
      this.$confirm('确认删除该通道？删除后无法恢复。', '提示', {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }).then(() => {
        this.config.channels.splice(index, 1)
        this.generateRawConfig()
        this.$message({
          type: 'success',
          message: '删除成功!'
        })
      }).catch(() => {
        this.$message({
          type: 'info',
          message: '已取消删除'
        })
      })
    },
    
    editChannelComm(channel) {
      this.currentChannel = JSON.parse(JSON.stringify(channel))
      this.currentChannelIndex = this.config.channels.findIndex(c => c.name === channel.name)
      
      // 确保配置对象存在
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
      
      // 确保点表数组存在
      if (!this.currentChannel.points) {
        this.currentChannel.points = []
      }
      
      this.pointsDialogVisible = true
    },
    
    addPoint() {
      if (!this.currentChannel.points) {
        this.currentChannel.points = []
      }
      
      const newPoint = {
        name: `Point_${this.currentChannel.points.length + 1}`,
        address: '',
        dataType: 'float',
        access: 'read',
        description: ''
      }
      
      this.currentChannel.points.push(newPoint)
    },
    
    removePoint(index) {
      this.currentChannel.points.splice(index, 1)
    },
    
    savePoints() {
      if (this.currentChannelIndex >= 0) {
        this.config.channels[this.currentChannelIndex].points = this.currentChannel.points
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
        this.$message.warning('请先选择文件')
        return
      }
      
      // 模拟导入成功
      this.$message.success('点表导入成功')
      this.importDialogVisible = false
      
      // 模拟导入的点表数据
      const importedPoints = [
        { name: 'Imported_Point_1', address: '40101', dataType: 'float', access: 'read', description: '导入点位1' },
        { name: 'Imported_Point_2', address: '40103', dataType: 'int', access: 'read', description: '导入点位2' },
        { name: 'Imported_Point_3', address: '40105', dataType: 'bool', access: 'readwrite', description: '导入点位3' }
      ]
      
      // 添加到当前通道的点表中
      if (!this.currentChannel.points) {
        this.currentChannel.points = []
      }
      
      this.currentChannel.points = [...this.currentChannel.points, ...importedPoints]
    },
    
    exportPoints() {
      // 模拟导出功能
      this.$message.success('点表已导出')
    },
    
    async saveConfig() {
      // 模拟保存配置
      setTimeout(() => {
        this.$message.success('配置保存成功')
      }, 1000)
    },
    
    resetConfig() {
      this.$confirm('确认重置配置？所有未保存的更改将丢失。', '提示', {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning'
      }).then(() => {
        this.loadConfig()
        this.$message({
          type: 'success',
          message: '配置已重置!'
        })
      }).catch(() => {
        this.$message({
          type: 'info',
          message: '已取消重置'
        })
      })
    },
    
    applyConfig() {
      this.applying = true
      
      // 模拟应用配置
      setTimeout(() => {
        this.applying = false
        this.$message.success('配置已成功应用到系统')
      }, 2000)
    }
  },
  watch: {
    configFormat() {
      this.generateRawConfig()
    }
  },
  created() {
    this.loadConfig()
  }
}
</script>

<style scoped>
.config-editor {
  padding: 20px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.header-actions {
  display: flex;
  gap: 10px;
}

.loading-container, .error-container {
  padding: 20px;
  text-align: center;
}

.mt-20 {
  margin-top: 20px;
}

.config-container {
  margin-top: 20px;
}

.tab-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.tab-header h3 {
  margin: 0;
}

.code-editor-container {
  border: 1px solid #dcdfe6;
  border-radius: 4px;
  overflow: hidden;
}

.code-editor-container :deep(.el-textarea__inner) {
  font-family: 'Courier New', Courier, monospace;
  padding: 15px;
  line-height: 1.5;
}

.points-dialog-header {
  margin-bottom: 20px;
  display: flex;
  gap: 10px;
}
</style> 