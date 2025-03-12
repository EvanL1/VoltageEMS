<template>
  <div class="template-builder">
    <div class="template-header">
      <h2>模型模板构建器</h2>
      <div class="template-meta">
        <el-form :inline="true" :model="templateMeta" class="meta-form">
          <el-form-item label="模板ID">
            <el-input v-model="templateMeta.id" placeholder="template-id"></el-input>
          </el-form-item>
          <el-form-item label="名称">
            <el-input v-model="templateMeta.name" placeholder="模板名称"></el-input>
          </el-form-item>
          <el-form-item label="描述">
            <el-input v-model="templateMeta.description" placeholder="模板描述"></el-input>
          </el-form-item>
        </el-form>
      </div>
      <div class="template-actions">
        <el-button type="primary" @click="saveTemplate">保存模板</el-button>
        <el-button @click="exportTemplate">导出</el-button>
        <el-button @click="importTemplate">导入</el-button>
      </div>
    </div>

    <div class="template-content">
      <div class="component-panel">
        <h3>组件库</h3>
        <div class="component-list">
          <div 
            v-for="component in componentTypes" 
            :key="component.type"
            class="component-item"
            draggable
            @dragstart="onDragStart($event, component.type)"
          >
            <span class="component-icon">{{ component.icon }}</span>
            <span class="component-label">{{ component.label }}</span>
          </div>
        </div>
      </div>

      <div class="canvas-container" ref="canvasContainer">
        <VueFlow
          v-model="elements"
          :default-viewport="{ zoom: 1.5 }"
          :min-zoom="0.2"
          :max-zoom="4"
          class="flow-container"
          @dragover="onDragOver"
          @drop="onDrop"
          @nodeclick="onNodeClick"
          @connect="onConnect"
        >
          <template #node-battery="nodeProps">
            <BatteryNode v-bind="nodeProps" />
          </template>
          <template #node-motor="nodeProps">
            <MotorNode v-bind="nodeProps" />
          </template>
          <template #node-pv="nodeProps">
            <PVNode v-bind="nodeProps" />
          </template>
          <template #node-meter="nodeProps">
            <MeterNode v-bind="nodeProps" />
          </template>
          <template #node-load="nodeProps">
            <LoadNode v-bind="nodeProps" />
          </template>
          <template #node-controller="nodeProps">
            <ControllerNode v-bind="nodeProps" />
          </template>
          
          <Background pattern="dots" :gap="12" :size="1" />
          <Controls />
          <MiniMap />
        </VueFlow>
      </div>

      <div class="properties-panel">
        <h3>属性</h3>
        <template v-if="selectedNode">
          <div class="property-form">
            <el-form label-position="top">
              <el-form-item label="标签">
                <el-input v-model="selectedNode.data.label" @change="updateNodeData"></el-input>
              </el-form-item>
              
              <template v-if="selectedNode.data.properties">
                <el-form-item 
                  v-for="(value, key) in selectedNode.data.properties" 
                  :key="key"
                  :label="formatPropertyName(key)"
                >
                  <el-input 
                    v-if="typeof value === 'string'" 
                    v-model="selectedNode.data.properties[key]"
                    @change="updateNodeData"
                  ></el-input>
                  <el-input-number 
                    v-else-if="typeof value === 'number'" 
                    v-model="selectedNode.data.properties[key]"
                    @change="updateNodeData"
                  ></el-input-number>
                  <el-switch 
                    v-else-if="typeof value === 'boolean'" 
                    v-model="selectedNode.data.properties[key]"
                    @change="updateNodeData"
                  ></el-switch>
                </el-form-item>
              </template>
            </el-form>
          </div>
        </template>
        <template v-else>
          <p>选择一个元素查看其属性</p>
        </template>
      </div>
    </div>
  </div>
</template>

<script>
import { VueFlow, useVueFlow, Background, Controls, MiniMap } from '@vue-flow/core';
import '@vue-flow/core/dist/style.css';
import '@vue-flow/core/dist/theme-default.css';

// Import custom node components
import BatteryNode from '../components/template-builder/BatteryNode.vue';
import MotorNode from '../components/template-builder/MotorNode.vue';
import PVNode from '../components/template-builder/PVNode.vue';
import MeterNode from '../components/template-builder/MeterNode.vue';
import LoadNode from '../components/template-builder/LoadNode.vue';
import ControllerNode from '../components/template-builder/ControllerNode.vue';

export default {
  name: 'TemplateBuilder',
  components: {
    VueFlow,
    Background,
    Controls,
    MiniMap,
    BatteryNode,
    MotorNode,
    PVNode,
    MeterNode,
    LoadNode,
    ControllerNode
  },
  setup() {
    const { onConnect, addEdge, getNodes, getEdges, findNode, setNodes } = useVueFlow();
    
    return {
      onConnect: (params) => {
        addEdge(params);
      },
      getNodes,
      getEdges,
      findNode,
      setNodes
    };
  },
  data() {
    return {
      templateMeta: {
        id: '',
        name: '新模板',
        description: '模板描述'
      },
      componentTypes: [
        { type: 'battery', label: '电池', icon: '🔋' },
        { type: 'motor', label: '电机', icon: '⚙️' },
        { type: 'pv', label: '光伏板', icon: '☀️' },
        { type: 'meter', label: '电表', icon: '🔌' },
        { type: 'load', label: '负载', icon: '💡' },
        { type: 'controller', label: '控制器', icon: '🎮' }
      ],
      elements: [],
      selectedNode: null
    };
  },
  computed: {
    nodes() {
      return this.getNodes();
    },
    edges() {
      return this.getEdges();
    }
  },
  methods: {
    onDragStart(event, nodeType) {
      event.dataTransfer.setData('application/nodeType', nodeType);
      event.dataTransfer.effectAllowed = 'move';
    },
    
    onDragOver(event) {
      event.preventDefault();
      event.dataTransfer.dropEffect = 'move';
    },
    
    onDrop(event) {
      event.preventDefault();
      
      const nodeType = event.dataTransfer.getData('application/nodeType');
      if (!nodeType) return;
      
      // Get the drop position relative to the canvas
      const canvasBounds = this.$refs.canvasContainer.getBoundingClientRect();
      const position = {
        x: event.clientX - canvasBounds.left,
        y: event.clientY - canvasBounds.top
      };
      
      // Create a new node
      const newNode = {
        id: `${nodeType}-${Date.now()}`,
        type: nodeType,
        position,
        data: {
          label: this.getDefaultLabelForType(nodeType),
          properties: this.getDefaultPropertiesForType(nodeType)
        }
      };
      
      // Add the node to the flow
      this.elements = [...this.elements, newNode];
    },
    
    onNodeClick(event, node) {
      this.selectedNode = node;
    },
    
    updateNodeData() {
      if (!this.selectedNode) return;
      
      // Find the node in the elements array and update it
      const updatedElements = this.elements.map(el => {
        if (el.id === this.selectedNode.id) {
          return {
            ...el,
            data: {
              ...this.selectedNode.data
            }
          };
        }
        return el;
      });
      
      this.elements = updatedElements;
    },
    
    formatPropertyName(key) {
      // Convert camelCase to Title Case with spaces
      return key
        .replace(/([A-Z])/g, ' $1')
        .replace(/^./, str => str.toUpperCase());
    },
    
    getDefaultLabelForType(type) {
      const typeMap = {
        'battery': '电池',
        'motor': '电机',
        'pv': '光伏板',
        'meter': '电表',
        'load': '负载',
        'controller': '控制器'
      };
      
      return typeMap[type] || type;
    },
    
    saveTemplate() {
      if (!this.templateMeta.id || !this.templateMeta.name) {
        this.$message.error('模板ID和名称不能为空');
        return;
      }
      
      if (this.nodes.length === 0) {
        this.$message.error('模板必须包含至少一个组件');
        return;
      }
      
      const template = {
        ...this.templateMeta,
        nodes: this.nodes.map(node => ({
          id: node.id,
          type: node.type,
          position: node.position,
          data: node.data
        })),
        connections: this.edges.map(edge => ({
          id: edge.id,
          source: edge.source,
          sourceHandle: edge.sourceHandle,
          target: edge.target,
          targetHandle: edge.targetHandle,
          type: edge.type || 'default'
        }))
      };
      
      // TODO: Send to backend API
      console.log('Saving template:', template);
      this.$message.success('模板保存成功（模拟）');
    },
    
    exportTemplate() {
      const template = {
        ...this.templateMeta,
        nodes: this.nodes,
        edges: this.edges
      };
      
      const dataStr = JSON.stringify(template, null, 2);
      const dataUri = 'data:application/json;charset=utf-8,'+ encodeURIComponent(dataStr);
      
      const exportFileDefaultName = `${this.templateMeta.id || 'template'}.json`;
      
      const linkElement = document.createElement('a');
      linkElement.setAttribute('href', dataUri);
      linkElement.setAttribute('download', exportFileDefaultName);
      linkElement.click();
    },
    
    importTemplate() {
      // This would typically open a file dialog
      this.$message.info('导入功能尚未实现');
    },
    
    getDefaultPropertiesForType(type) {
      switch (type) {
        case 'battery':
          return { 
            capacity: 100,
            voltage: 12,
            soc: 80,
            temperature: 25
          };
        case 'motor':
          return { 
            power: 500,
            speed: 1750,
            temperature: 25,
            position: 0
          };
        case 'pv':
          return { 
            power: 1000,
            voltage: 48,
            current: 20,
            temperature: 30
          };
        case 'meter':
          return {
            voltage: 220,
            current: 10,
            power: 2200,
            frequency: 50
          };
        case 'load':
          return {
            power: 1000,
            status: 'on',
            priority: 1
          };
        case 'controller':
          return {
            mode: 'auto',
            status: 'active'
          };
        default:
          return {};
      }
    }
  }
};
</script>

<style scoped>
.template-builder {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
}

.template-header {
  padding: 15px;
  background-color: #f5f7fa;
  border-bottom: 1px solid #e4e7ed;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.template-meta {
  flex-grow: 1;
  margin: 0 20px;
}

.meta-form {
  display: flex;
  align-items: center;
}

.template-content {
  display: flex;
  flex: 1;
  overflow: hidden;
  height: calc(100vh - 130px);
}

.component-panel {
  width: 200px;
  padding: 15px;
  background-color: #f5f7fa;
  border-right: 1px solid #e4e7ed;
  overflow-y: auto;
}

.component-list {
  margin-top: 15px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.component-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px;
  border: 1px solid #dcdfe6;
  border-radius: 4px;
  background-color: white;
  cursor: grab;
  transition: all 0.3s;
}

.component-item:hover {
  background-color: #ecf5ff;
  border-color: #409eff;
  transform: translateY(-2px);
  box-shadow: 0 2px 12px 0 rgba(0, 0, 0, 0.1);
}

.component-icon {
  font-size: 24px;
}

.canvas-container {
  flex: 1;
  background-color: #fff;
  position: relative;
  height: 100%;
}

.flow-container {
  height: 100%;
}

.properties-panel {
  width: 280px;
  padding: 15px;
  background-color: #f5f7fa;
  border-left: 1px solid #e4e7ed;
  overflow-y: auto;
}

.property-form {
  margin-top: 15px;
}
</style> 