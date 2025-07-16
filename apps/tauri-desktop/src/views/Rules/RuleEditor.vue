<template>
  <div class="rule-editor">
    <!-- Rule List Sidebar -->
    <div class="rule-sidebar">
      <el-card>
        <template #header>
          <div class="sidebar-header">
            <span>Rules</span>
            <el-button type="primary" size="small" @click="createNewRule">
              <el-icon><Plus /></el-icon>
            </el-button>
          </div>
        </template>
        
        <el-input
          v-model="ruleSearch"
          placeholder="Search rules..."
          :prefix-icon="Search"
          clearable
          style="margin-bottom: 10px"
        />
        
        <el-tree
          :data="ruleTree"
          :props="{ label: 'name', children: 'children' }"
          :filter-node-method="filterRule"
          ref="ruleTreeRef"
          highlight-current
          @node-click="selectRule"
        >
          <template #default="{ node, data }">
            <span class="rule-tree-node">
              <el-icon v-if="data.type === 'group'"><Folder /></el-icon>
              <el-icon v-else-if="data.type === 'condition'"><CircleCheck /></el-icon>
              <el-icon v-else-if="data.type === 'action'"><VideoPlay /></el-icon>
              <el-icon v-else><Document /></el-icon>
              <span>{{ node.label }}</span>
              <el-tag v-if="data.status" :type="getStatusType(data.status)" size="small" style="margin-left: 5px">
                {{ data.status }}
              </el-tag>
            </span>
          </template>
        </el-tree>
      </el-card>
    </div>
    
    <!-- Rule Editor Main Area -->
    <div class="rule-main">
      <el-card v-if="selectedRule">
        <template #header>
          <div class="editor-header">
            <h3>{{ selectedRule.name }}</h3>
            <el-space>
              <el-button type="primary" @click="saveRule">
                <el-icon><Check /></el-icon>
                Save
              </el-button>
              <el-button @click="testRule">
                <el-icon><CaretRight /></el-icon>
                Test
              </el-button>
              <el-button type="danger" @click="deleteRule">
                <el-icon><Delete /></el-icon>
                Delete
              </el-button>
            </el-space>
          </div>
        </template>
        
        <!-- Rule Editor Tabs -->
        <el-tabs v-model="activeTab">
          <!-- Basic Info -->
          <el-tab-pane label="Basic Info" name="basic">
            <el-form :model="selectedRule" label-width="120px">
              <el-form-item label="Rule Name">
                <el-input v-model="selectedRule.name" />
              </el-form-item>
              
              <el-form-item label="Description">
                <el-input
                  v-model="selectedRule.description"
                  type="textarea"
                  :rows="3"
                />
              </el-form-item>
              
              <el-form-item label="Category">
                <el-select v-model="selectedRule.category">
                  <el-option label="Alarm Rules" value="alarm" />
                  <el-option label="Control Rules" value="control" />
                  <el-option label="Calculation Rules" value="calculation" />
                  <el-option label="Data Quality" value="quality" />
                  <el-option label="System Rules" value="system" />
                </el-select>
              </el-form-item>
              
              <el-form-item label="Priority">
                <el-slider
                  v-model="selectedRule.priority"
                  :min="1"
                  :max="10"
                  show-stops
                  :marks="{ 1: 'Low', 5: 'Medium', 10: 'High' }"
                />
              </el-form-item>
              
              <el-form-item label="Status">
                <el-switch
                  v-model="selectedRule.enabled"
                  active-text="Enabled"
                  inactive-text="Disabled"
                />
              </el-form-item>
              
              <el-form-item label="Schedule">
                <el-radio-group v-model="selectedRule.schedule">
                  <el-radio label="always">Always</el-radio>
                  <el-radio label="cron">Cron Expression</el-radio>
                  <el-radio label="event">Event Triggered</el-radio>
                </el-radio-group>
              </el-form-item>
              
              <el-form-item v-if="selectedRule.schedule === 'cron'" label="Cron Expression">
                <el-input v-model="selectedRule.cronExpression" placeholder="0 */5 * * * *">
                  <template #append>
                    <el-button @click="showCronHelper = true">Helper</el-button>
                  </template>
                </el-input>
              </el-form-item>
            </el-form>
          </el-tab-pane>
          
          <!-- Conditions -->
          <el-tab-pane label="Conditions" name="conditions">
            <div class="condition-builder">
              <div class="condition-toolbar">
                <el-button @click="addCondition">
                  <el-icon><Plus /></el-icon>
                  Add Condition
                </el-button>
                <el-button @click="addGroup">
                  <el-icon><Folder /></el-icon>
                  Add Group
                </el-button>
                <el-select v-model="conditionLogic" style="width: 100px; margin-left: 10px">
                  <el-option label="AND" value="and" />
                  <el-option label="OR" value="or" />
                </el-select>
              </div>
              
              <div class="condition-tree">
                <ConditionNode
                  v-for="(condition, index) in selectedRule.conditions"
                  :key="condition.id"
                  :condition="condition"
                  :index="index"
                  @remove="removeCondition(index)"
                  @update="updateCondition(index, $event)"
                />
              </div>
            </div>
          </el-tab-pane>
          
          <!-- Actions -->
          <el-tab-pane label="Actions" name="actions">
            <div class="action-builder">
              <div class="action-toolbar">
                <el-button @click="showActionDialog = true">
                  <el-icon><Plus /></el-icon>
                  Add Action
                </el-button>
              </div>
              
              <el-table :data="selectedRule.actions" style="width: 100%">
                <el-table-column prop="type" label="Action Type" width="150">
                  <template #default="{ row }">
                    <el-tag>{{ row.type }}</el-tag>
                  </template>
                </el-table-column>
                <el-table-column prop="target" label="Target" />
                <el-table-column prop="parameters" label="Parameters">
                  <template #default="{ row }">
                    <el-text truncated>{{ JSON.stringify(row.parameters) }}</el-text>
                  </template>
                </el-table-column>
                <el-table-column label="Actions" width="150">
                  <template #default="{ row, $index }">
                    <el-button type="primary" size="small" text @click="editAction(row, $index)">
                      Edit
                    </el-button>
                    <el-button type="danger" size="small" text @click="removeAction($index)">
                      Delete
                    </el-button>
                  </template>
                </el-table-column>
              </el-table>
            </div>
          </el-tab-pane>
          
          <!-- Visual Editor -->
          <el-tab-pane label="Visual Editor" name="visual">
            <div class="visual-editor">
              <div class="editor-toolbar">
                <el-button-group>
                  <el-button @click="zoomIn">
                    <el-icon><ZoomIn /></el-icon>
                  </el-button>
                  <el-button @click="zoomOut">
                    <el-icon><ZoomOut /></el-icon>
                  </el-button>
                  <el-button @click="fitView">
                    <el-icon><FullScreen /></el-icon>
                  </el-button>
                </el-button-group>
                
                <el-button @click="autoLayout">
                  <el-icon><Grid /></el-icon>
                  Auto Layout
                </el-button>
              </div>
              
              <div ref="graphContainer" class="graph-container"></div>
            </div>
          </el-tab-pane>
          
          <!-- Code Editor -->
          <el-tab-pane label="Code Editor" name="code">
            <div class="code-editor">
              <div class="editor-toolbar">
                <el-select v-model="codeLanguage" style="width: 120px">
                  <el-option label="JavaScript" value="javascript" />
                  <el-option label="Python" value="python" />
                  <el-option label="Expression" value="expression" />
                </el-select>
                
                <el-button @click="formatCode">
                  <el-icon><MagicStick /></el-icon>
                  Format
                </el-button>
                
                <el-button @click="validateCode">
                  <el-icon><CircleCheck /></el-icon>
                  Validate
                </el-button>
              </div>
              
              <div ref="codeEditorContainer" class="monaco-editor-container"></div>
            </div>
          </el-tab-pane>
          
          <!-- Test & Debug -->
          <el-tab-pane label="Test & Debug" name="test">
            <div class="test-panel">
              <el-form label-width="120px">
                <el-form-item label="Test Data">
                  <el-input
                    v-model="testData"
                    type="textarea"
                    :rows="5"
                    placeholder="Enter test data in JSON format"
                  />
                </el-form-item>
                
                <el-form-item label="Test Mode">
                  <el-radio-group v-model="testMode">
                    <el-radio label="simulation">Simulation</el-radio>
                    <el-radio label="live">Live Data</el-radio>
                    <el-radio label="historical">Historical Data</el-radio>
                  </el-radio-group>
                </el-form-item>
                
                <el-form-item>
                  <el-button type="primary" @click="runTest">
                    <el-icon><CaretRight /></el-icon>
                    Run Test
                  </el-button>
                  <el-button @click="clearResults">
                    <el-icon><Delete /></el-icon>
                    Clear
                  </el-button>
                </el-form-item>
              </el-form>
              
              <div v-if="testResults" class="test-results">
                <h4>Test Results</h4>
                <el-timeline>
                  <el-timeline-item
                    v-for="(result, index) in testResults"
                    :key="index"
                    :timestamp="result.timestamp"
                    :type="result.success ? 'success' : 'danger'"
                  >
                    <p>{{ result.message }}</p>
                    <el-text v-if="result.details" type="info" size="small">
                      {{ result.details }}
                    </el-text>
                  </el-timeline-item>
                </el-timeline>
              </div>
            </div>
          </el-tab-pane>
        </el-tabs>
      </el-card>
      
      <!-- Empty State -->
      <el-empty v-else description="Select a rule to edit or create a new one" />
    </div>
    
    <!-- Action Dialog -->
    <el-dialog
      v-model="showActionDialog"
      title="Add Action"
      width="600px"
    >
      <el-form :model="newAction" label-width="120px">
        <el-form-item label="Action Type">
          <el-select v-model="newAction.type" @change="onActionTypeChange">
            <el-option label="Send Alarm" value="alarm" />
            <el-option label="Send Control" value="control" />
            <el-option label="Update Value" value="update" />
            <el-option label="Send Notification" value="notification" />
            <el-option label="Execute Script" value="script" />
            <el-option label="Call API" value="api" />
          </el-select>
        </el-form-item>
        
        <el-form-item v-if="newAction.type === 'alarm'" label="Alarm Level">
          <el-select v-model="newAction.parameters.level">
            <el-option label="Info" value="info" />
            <el-option label="Warning" value="warning" />
            <el-option label="Error" value="error" />
            <el-option label="Critical" value="critical" />
          </el-select>
        </el-form-item>
        
        <el-form-item v-if="newAction.type === 'control'" label="Control Point">
          <el-input v-model="newAction.parameters.pointId" placeholder="Point ID" />
        </el-form-item>
        
        <el-form-item v-if="newAction.type === 'control'" label="Control Value">
          <el-input v-model="newAction.parameters.value" placeholder="Value" />
        </el-form-item>
        
        <el-form-item label="Description">
          <el-input v-model="newAction.description" type="textarea" :rows="2" />
        </el-form-item>
      </el-form>
      
      <template #footer>
        <el-button @click="showActionDialog = false">Cancel</el-button>
        <el-button type="primary" @click="addAction">Add Action</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, nextTick } from 'vue'
import { 
  Plus, 
  Search, 
  Folder, 
  CircleCheck, 
  VideoPlay,
  Document,
  Check,
  CaretRight,
  Delete,
  ZoomIn,
  ZoomOut,
  FullScreen,
  Grid,
  MagicStick
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import ConditionNode from '@/components/Rules/ConditionNode.vue'

// Mock rule data
const ruleTree = ref([
  {
    id: 1,
    name: 'Alarm Rules',
    type: 'group',
    children: [
      {
        id: 11,
        name: 'High Temperature Alarm',
        type: 'condition',
        status: 'active'
      },
      {
        id: 12,
        name: 'Low Voltage Warning',
        type: 'condition',
        status: 'active'
      }
    ]
  },
  {
    id: 2,
    name: 'Control Rules',
    type: 'group',
    children: [
      {
        id: 21,
        name: 'Auto Start Generator',
        type: 'action',
        status: 'inactive'
      },
      {
        id: 22,
        name: 'Load Balancing',
        type: 'action',
        status: 'active'
      }
    ]
  }
])

// State
const ruleSearch = ref('')
const ruleTreeRef = ref()
const selectedRule = ref<any>(null)
const activeTab = ref('basic')
const conditionLogic = ref('and')
const showActionDialog = ref(false)
const showCronHelper = ref(false)
const codeLanguage = ref('javascript')
const testMode = ref('simulation')
const testData = ref('')
const testResults = ref<any[]>([])

// Visual editor
const graphContainer = ref<HTMLElement>()
const codeEditorContainer = ref<HTMLElement>()

// New action form
const newAction = ref({
  type: '',
  target: '',
  parameters: {},
  description: ''
})

// Watch search
watch(ruleSearch, (val) => {
  ruleTreeRef.value?.filter(val)
})

// Methods
function filterRule(value: string, data: any) {
  if (!value) return true
  return data.name.toLowerCase().includes(value.toLowerCase())
}

function selectRule(data: any) {
  if (data.type === 'group') return
  
  // Mock rule data
  selectedRule.value = {
    id: data.id,
    name: data.name,
    description: 'This rule monitors system parameters and triggers actions',
    category: 'alarm',
    priority: 7,
    enabled: data.status === 'active',
    schedule: 'always',
    cronExpression: '',
    conditions: [
      {
        id: 1,
        field: 'temperature',
        operator: '>',
        value: 80,
        unit: '°C'
      },
      {
        id: 2,
        field: 'pressure',
        operator: '<',
        value: 10,
        unit: 'bar'
      }
    ],
    actions: [
      {
        type: 'alarm',
        target: 'system',
        parameters: { level: 'warning', message: 'Temperature too high' }
      }
    ]
  }
}

function createNewRule() {
  selectedRule.value = {
    id: null,
    name: 'New Rule',
    description: '',
    category: 'alarm',
    priority: 5,
    enabled: true,
    schedule: 'always',
    cronExpression: '',
    conditions: [],
    actions: []
  }
  activeTab.value = 'basic'
}

function saveRule() {
  ElMessage.success('Rule saved successfully')
}

function testRule() {
  ElMessage.info('Testing rule...')
  setTimeout(() => {
    ElMessage.success('Rule test passed')
  }, 1500)
}

function deleteRule() {
  ElMessage.warning('Rule deleted')
  selectedRule.value = null
}

function addCondition() {
  if (!selectedRule.value.conditions) {
    selectedRule.value.conditions = []
  }
  selectedRule.value.conditions.push({
    id: Date.now(),
    field: '',
    operator: '=',
    value: '',
    unit: ''
  })
}

function addGroup() {
  ElMessage.info('Group conditions feature coming soon')
}

function removeCondition(index: number) {
  selectedRule.value.conditions.splice(index, 1)
}

function updateCondition(index: number, condition: any) {
  selectedRule.value.conditions[index] = condition
}

function onActionTypeChange() {
  // Reset parameters based on action type
  newAction.value.parameters = {}
}

function addAction() {
  selectedRule.value.actions.push({
    ...newAction.value,
    id: Date.now()
  })
  showActionDialog.value = false
  
  // Reset form
  newAction.value = {
    type: '',
    target: '',
    parameters: {},
    description: ''
  }
}

function editAction(action: any, index: number) {
  newAction.value = { ...action }
  showActionDialog.value = true
}

function removeAction(index: number) {
  selectedRule.value.actions.splice(index, 1)
}

// Visual editor methods
function zoomIn() {
  ElMessage.info('Zoom in')
}

function zoomOut() {
  ElMessage.info('Zoom out')
}

function fitView() {
  ElMessage.info('Fit view')
}

function autoLayout() {
  ElMessage.success('Auto layout applied')
}

// Code editor methods
function formatCode() {
  ElMessage.success('Code formatted')
}

function validateCode() {
  ElMessage.success('Code is valid')
}

// Test methods
function runTest() {
  testResults.value = [
    {
      timestamp: new Date().toLocaleTimeString(),
      success: true,
      message: 'Rule evaluation started',
      details: 'Testing with provided data'
    },
    {
      timestamp: new Date().toLocaleTimeString(),
      success: true,
      message: 'Condition 1 evaluated',
      details: 'temperature > 80: true'
    },
    {
      timestamp: new Date().toLocaleTimeString(),
      success: false,
      message: 'Condition 2 evaluated',
      details: 'pressure < 10: false'
    },
    {
      timestamp: new Date().toLocaleTimeString(),
      success: true,
      message: 'Action executed',
      details: 'Alarm sent successfully'
    }
  ]
}

function clearResults() {
  testResults.value = []
}

function getStatusType(status: string) {
  return status === 'active' ? 'success' : 'info'
}

onMounted(() => {
  // Initialize visual editor and code editor
  // TODO: Integrate with actual graph library and Monaco editor
})
</script>

<style lang="scss" scoped>
.rule-editor {
  height: 100%;
  display: flex;
  gap: 20px;
  
  .rule-sidebar {
    width: 300px;
    
    .sidebar-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
    }
    
    .rule-tree-node {
      display: flex;
      align-items: center;
      gap: 5px;
      
      .el-icon {
        color: #909399;
      }
    }
  }
  
  .rule-main {
    flex: 1;
    overflow: hidden;
    
    .editor-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      
      h3 {
        margin: 0;
      }
    }
  }
  
  .condition-builder,
  .action-builder {
    .condition-toolbar,
    .action-toolbar {
      margin-bottom: 20px;
    }
    
    .condition-tree {
      border: 1px solid #e4e7ed;
      border-radius: 4px;
      padding: 20px;
      min-height: 200px;
    }
  }
  
  .visual-editor {
    height: 500px;
    display: flex;
    flex-direction: column;
    
    .editor-toolbar {
      display: flex;
      gap: 10px;
      margin-bottom: 10px;
    }
    
    .graph-container {
      flex: 1;
      border: 1px solid #e4e7ed;
      border-radius: 4px;
      background: #fafafa;
    }
  }
  
  .code-editor {
    height: 500px;
    display: flex;
    flex-direction: column;
    
    .editor-toolbar {
      display: flex;
      gap: 10px;
      margin-bottom: 10px;
    }
    
    .monaco-editor-container {
      flex: 1;
      border: 1px solid #e4e7ed;
      border-radius: 4px;
    }
  }
  
  .test-panel {
    .test-results {
      margin-top: 20px;
      padding: 20px;
      background: #f5f7fa;
      border-radius: 4px;
      
      h4 {
        margin-bottom: 20px;
      }
    }
  }
}
</style>