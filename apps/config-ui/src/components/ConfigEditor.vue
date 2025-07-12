<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from 'vue';
import loader from '@monaco-editor/loader';
import type { editor } from 'monaco-editor';

const props = defineProps<{
  modelValue: string;
  language: 'yaml' | 'json';
}>();

const emit = defineEmits<{
  'update:modelValue': [value: string];
  change: [value: string];
}>();

const editorContainer = ref<HTMLElement>();
let monacoEditor: editor.IStandaloneCodeEditor | null = null;

onMounted(async () => {
  const monaco = await loader.init();
  
  if (editorContainer.value) {
    monacoEditor = monaco.editor.create(editorContainer.value, {
      value: props.modelValue,
      language: props.language,
      theme: 'vs',
      automaticLayout: true,
      minimap: {
        enabled: false,
      },
      fontSize: 14,
      lineNumbers: 'on',
      scrollBeyondLastLine: false,
      renderWhitespace: 'selection',
      folding: true,
      foldingStrategy: 'indentation',
    });
    
    monacoEditor.onDidChangeModelContent(() => {
      const value = monacoEditor?.getValue() || '';
      emit('update:modelValue', value);
      emit('change', value);
    });
  }
});

onUnmounted(() => {
  monacoEditor?.dispose();
});

watch(() => props.language, (newLanguage) => {
  if (monacoEditor) {
    const model = monacoEditor.getModel();
    if (model) {
      loader.init().then((monaco) => {
        monaco.editor.setModelLanguage(model, newLanguage);
      });
    }
  }
});

watch(() => props.modelValue, (newValue) => {
  if (monacoEditor && monacoEditor.getValue() !== newValue) {
    monacoEditor.setValue(newValue);
  }
});
</script>

<template>
  <div class="config-editor">
    <div ref="editorContainer" class="editor-container"></div>
  </div>
</template>

<style lang="scss" scoped>
.config-editor {
  border: 1px solid #dcdfe6;
  border-radius: 4px;
  overflow: hidden;
  
  .editor-container {
    height: 500px;
    width: 100%;
  }
}
</style>