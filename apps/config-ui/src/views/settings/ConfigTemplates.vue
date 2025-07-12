<script setup lang="ts">
import { ref } from 'vue';
import { ElMessage, ElMessageBox } from 'element-plus';
import { Document, Download, Upload, Edit, Delete } from '@element-plus/icons-vue';

const templates = ref([
  { id: 1, name: 'Modbus设备模板', type: 'channel', description: '标准Modbus TCP设备配置模板', tags: ['modbus', '通用'], downloads: 156 },
  { id: 2, name: '电力监测点表', type: 'point-table', description: '电力系统常用监测点配置', tags: ['电力', '三相'], downloads: 243 },
  { id: 3, name: 'IEC104标准配置', type: 'channel', description: 'IEC 60870-5-104协议标准配置', tags: ['iec104', '电网'], downloads: 89 },
  { id: 4, name: '温度告警规则集', type: 'alarm', description: '温度监测告警规则模板', tags: ['告警', '温度'], downloads: 178 },
]);

const showUploadDialog = ref(false);
const templateForm = ref({
  name: '',
  type: 'channel',
  description: '',
  tags: '',
  file: null
});

function applyTemplate(template: any) {
  ElMessageBox.confirm(
    `确定要应用模板 "${template.name}" 吗？`,
    '应用确认',
    {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      type: 'info',
    }
  ).then(() => {
    ElMessage.success('模板应用成功');
    template.downloads++;
  });
}

function downloadTemplate(template: any) {
  ElMessage.info(`开始下载模板 ${template.name}`);
  template.downloads++;
}

function deleteTemplate(template: any) {
  ElMessageBox.confirm(
    `确定要删除模板 "${template.name}" 吗？`,
    '删除确认',
    {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      type: 'warning',
    }
  ).then(() => {
    const index = templates.value.findIndex(t => t.id === template.id);
    if (index > -1) {
      templates.value.splice(index, 1);
    }
    ElMessage.success('模板已删除');
  });
}

function uploadTemplate() {
  // 处理模板上传
  showUploadDialog.value = false;
  ElMessage.success('模板上传成功');
}

function getTypeColor(type: string) {
  const colors: Record<string, string> = {
    'channel': '',
    'point-table': 'success',
    'alarm': 'warning',
    'system': 'info'
  };
  return colors[type] || '';
}
</script>

<template>
  <div class="config-templates">
    <el-card>
      <template #header>
        <div class="card-header">
          <h3>配置模板库</h3>
          <el-button type="primary" @click="showUploadDialog = true">
            <el-icon><Upload /></el-icon>
            上传模板
          </el-button>
        </div>
      </template>
      
      <el-row :gutter="20">
        <el-col v-for="template in templates" :key="template.id" :span="12">
          <el-card shadow="hover" class="template-card">
            <div class="template-header">
              <div class="template-info">
                <h4>{{ template.name }}</h4>
                <el-tag :type="getTypeColor(template.type)" size="small">
                  {{ template.type === 'channel' ? '通道' : template.type === 'point-table' ? '点表' : template.type === 'alarm' ? '告警' : '系统' }}
                </el-tag>
              </div>
              <span class="download-count">
                <el-icon><Download /></el-icon>
                {{ template.downloads }}
              </span>
            </div>
            
            <p class="template-description">{{ template.description }}</p>
            
            <div class="template-tags">
              <el-tag v-for="tag in template.tags" :key="tag" size="small" type="info">
                {{ tag }}
              </el-tag>
            </div>
            
            <div class="template-actions">
              <el-button size="small" @click="applyTemplate(template)">应用</el-button>
              <el-button size="small" @click="downloadTemplate(template)">
                <el-icon><Download /></el-icon>
                下载
              </el-button>
              <el-button size="small" type="danger" @click="deleteTemplate(template)">
                <el-icon><Delete /></el-icon>
                删除
              </el-button>
            </div>
          </el-card>
        </el-col>
      </el-row>
    </el-card>
    
    <!-- 上传模板对话框 -->
    <el-dialog v-model="showUploadDialog" title="上传配置模板" width="600px">
      <el-form :model="templateForm" label-width="100px">
        <el-form-item label="模板名称" required>
          <el-input v-model="templateForm.name" placeholder="请输入模板名称" />
        </el-form-item>
        
        <el-form-item label="模板类型" required>
          <el-select v-model="templateForm.type">
            <el-option label="通道配置" value="channel" />
            <el-option label="点表配置" value="point-table" />
            <el-option label="告警规则" value="alarm" />
            <el-option label="系统配置" value="system" />
          </el-select>
        </el-form-item>
        
        <el-form-item label="描述">
          <el-input v-model="templateForm.description" type="textarea" :rows="3" placeholder="请输入模板描述" />
        </el-form-item>
        
        <el-form-item label="标签">
          <el-input v-model="templateForm.tags" placeholder="多个标签用逗号分隔" />
        </el-form-item>
        
        <el-form-item label="模板文件" required>
          <el-upload
            class="upload-demo"
            drag
            :auto-upload="false"
            :limit="1"
          >
            <el-icon class="el-icon--upload"><upload-filled /></el-icon>
            <div class="el-upload__text">
              将文件拖到此处，或<em>点击上传</em>
            </div>
            <template #tip>
              <div class="el-upload__tip">
                支持 JSON、YAML 格式的配置文件
              </div>
            </template>
          </el-upload>
        </el-form-item>
      </el-form>
      
      <template #footer>
        <el-button @click="showUploadDialog = false">取消</el-button>
        <el-button type="primary" @click="uploadTemplate">上传</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style lang="scss" scoped>
.config-templates {
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    
    h3 {
      margin: 0;
      font-size: 18px;
      font-weight: 600;
    }
  }
  
  .template-card {
    margin-bottom: 20px;
    
    .template-header {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      margin-bottom: 12px;
      
      .template-info {
        display: flex;
        align-items: center;
        gap: 8px;
        
        h4 {
          margin: 0;
          font-size: 16px;
          font-weight: 600;
        }
      }
      
      .download-count {
        display: flex;
        align-items: center;
        gap: 4px;
        color: #909399;
        font-size: 14px;
      }
    }
    
    .template-description {
      margin: 0 0 12px;
      color: #606266;
      font-size: 14px;
      line-height: 1.5;
    }
    
    .template-tags {
      display: flex;
      gap: 8px;
      margin-bottom: 16px;
    }
    
    .template-actions {
      display: flex;
      gap: 8px;
    }
  }
}
</style>