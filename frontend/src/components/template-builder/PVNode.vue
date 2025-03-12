<template>
  <div class="custom-node pv-node">
    <div class="node-header">
      <span class="node-icon">☀️</span>
      <span class="node-label">{{ data.label }}</span>
    </div>
    <div class="node-content">
      <div class="node-property">
        <span>功率: {{ data.properties.power }}W</span>
      </div>
      <div class="node-property">
        <span>电压: {{ data.properties.voltage }}V</span>
      </div>
      <div class="node-property">
        <span>电流: {{ data.properties.current }}A</span>
      </div>
    </div>
    
    <!-- Connection handles -->
    <div class="handle-container">
      <div class="handle output-handle" data-handle-id="output" data-type="source"></div>
      <div class="handle control-handle" data-handle-id="control" data-type="target"></div>
    </div>
  </div>
</template>

<script>
export default {
  name: 'PVNode',
  props: {
    id: {
      type: String,
      required: true
    },
    data: {
      type: Object,
      required: true
    }
  }
};
</script>

<style scoped>
.custom-node {
  padding: 10px;
  border-radius: 5px;
  background: white;
  border: 1px solid #ddd;
  width: 150px;
  position: relative;
}

.pv-node {
  background-color: #fffbe6;
  border-color: #ffe58f;
}

.node-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  font-weight: bold;
}

.node-icon {
  font-size: 18px;
}

.node-content {
  font-size: 12px;
}

.node-property {
  margin-bottom: 4px;
}

.handle-container {
  position: absolute;
  width: 100%;
  height: 100%;
  top: 0;
  left: 0;
  pointer-events: none;
}

.handle {
  width: 10px;
  height: 10px;
  background: #1890ff;
  border: 2px solid white;
  border-radius: 50%;
  position: absolute;
  pointer-events: all;
}

.output-handle {
  bottom: -5px;
  left: 50%;
  transform: translateX(-50%);
}

.control-handle {
  right: -5px;
  top: 50%;
  transform: translateY(-50%);
  background-color: #f759ab;
}
</style> 