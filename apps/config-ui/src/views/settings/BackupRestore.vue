<script setup lang="ts">
import { ref } from 'vue';
import { ElMessage, ElMessageBox } from 'element-plus';
import { Upload, Download, Clock, Delete } from '@element-plus/icons-vue';

const backups = ref([
  { id: 1, name: 'backup_2024_01_15_10_30.tar.gz', time: '2024-01-15 10:30:00', size: '125 MB', type: 'manual' },
  { id: 2, name: 'backup_2024_01_14_02_00.tar.gz', time: '2024-01-14 02:00:00', size: '123 MB', type: 'auto' },
  { id: 3, name: 'backup_2024_01_13_02_00.tar.gz', time: '2024-01-13 02:00:00', size: '122 MB', type: 'auto' },
  { id: 4, name: 'backup_2024_01_12_15_45.tar.gz', time: '2024-01-12 15:45:00', size: '121 MB', type: 'manual' },
]);

const backupProgress = ref(0);
const backing = ref(false);

function createBackup() {
  backing.value = true;
  backupProgress.value = 0;
  
  const interval = setInterval(() => {
    backupProgress.value += 10;
    if (backupProgress.value >= 100) {
      clearInterval(interval);
      backing.value = false;
      ElMessage.success('备份创建成功');
      
      // 添加新备份到列表
      backups.value.unshift({
        id: backups.value.length + 1,
        name: `backup_${new Date().toISOString().replace(/[:.]/g, '_')}.tar.gz`,
        time: new Date().toLocaleString(),
        size: '126 MB',
        type: 'manual'
      });
    }
  }, 500);
}

function restoreBackup(backup: any) {
  ElMessageBox.confirm(
    `确定要恢复到备份 "${backup.name}" 吗？这将覆盖当前配置。`,
    '恢复确认',
    {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      type: 'warning',
    }
  ).then(() => {
    ElMessage.success('开始恢复备份...');
  });
}

function deleteBackup(backup: any) {
  ElMessageBox.confirm(
    `确定要删除备份 "${backup.name}" 吗？`,
    '删除确认',
    {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      type: 'warning',
    }
  ).then(() => {
    const index = backups.value.findIndex(b => b.id === backup.id);
    if (index > -1) {
      backups.value.splice(index, 1);
    }
    ElMessage.success('备份已删除');
  });
}

function downloadBackup(backup: any) {
  ElMessage.info(`开始下载 ${backup.name}`);
}
</script>

<template>
  <div class="backup-restore">
    <el-row :gutter="20">
      <el-col :span="16">
        <el-card>
          <template #header>
            <div class="card-header">
              <h3>备份列表</h3>
              <el-button type="primary" @click="createBackup" :loading="backing">
                <el-icon><Upload /></el-icon>
                创建备份
              </el-button>
            </div>
          </template>
          
          <el-progress v-if="backing" :percentage="backupProgress" />
          
          <el-table :data="backups" style="width: 100%">
            <el-table-column prop="name" label="备份文件" />
            <el-table-column prop="time" label="创建时间" width="180" />
            <el-table-column prop="size" label="大小" width="100" />
            <el-table-column prop="type" label="类型" width="80">
              <template #default="{ row }">
                <el-tag size="small" :type="row.type === 'auto' ? 'info' : 'success'">
                  {{ row.type === 'auto' ? '自动' : '手动' }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="操作" width="240">
              <template #default="{ row }">
                <el-button size="small" @click="restoreBackup(row)">
                  <el-icon><Clock /></el-icon>
                  恢复
                </el-button>
                <el-button size="small" @click="downloadBackup(row)">
                  <el-icon><Download /></el-icon>
                  下载
                </el-button>
                <el-button size="small" type="danger" @click="deleteBackup(row)">
                  <el-icon><Delete /></el-icon>
                  删除
                </el-button>
              </template>
            </el-table-column>
          </el-table>
        </el-card>
      </el-col>
      
      <el-col :span="8">
        <el-card>
          <template #header>
            <h3>备份设置</h3>
          </template>
          
          <el-form label-width="100px">
            <el-form-item label="自动备份">
              <el-switch />
            </el-form-item>
            
            <el-form-item label="备份时间">
              <el-time-picker
                placeholder="选择时间"
                format="HH:mm"
                value-format="HH:mm"
              />
            </el-form-item>
            
            <el-form-item label="保留天数">
              <el-input-number :min="7" :max="90" />
            </el-form-item>
            
            <el-form-item label="备份内容">
              <el-checkbox-group>
                <el-checkbox label="服务配置" />
                <el-checkbox label="通道配置" />
                <el-checkbox label="点表数据" />
                <el-checkbox label="告警规则" />
                <el-checkbox label="系统设置" />
              </el-checkbox-group>
            </el-form-item>
            
            <el-form-item>
              <el-button type="primary">保存设置</el-button>
            </el-form-item>
          </el-form>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<style lang="scss" scoped>
.backup-restore {
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
  
  .el-progress {
    margin-bottom: 20px;
  }
}
</style>