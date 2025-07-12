<script setup lang="ts">
import { ref } from 'vue';
import { User, Message, Phone } from '@element-plus/icons-vue';

const notifications = ref([
  { id: 1, name: '运维组', type: 'group', members: 5, email: true, sms: true, wechat: true },
  { id: 2, name: '张三', type: 'user', email: 'zhangsan@example.com', phone: '13800138000', email_enabled: true, sms_enabled: false },
  { id: 3, name: '李四', type: 'user', email: 'lisi@example.com', phone: '13900139000', email_enabled: true, sms_enabled: true },
]);
</script>

<template>
  <div class="alarm-notifications">
    <el-row :gutter="20">
      <el-col :span="12">
        <el-card>
          <template #header>
            <div class="card-header">
              <h3>通知接收人</h3>
              <el-button type="primary" size="small">添加接收人</el-button>
            </div>
          </template>
          
          <el-table :data="notifications" style="width: 100%">
            <el-table-column label="类型" width="60">
              <template #default="{ row }">
                <el-icon :size="16">
                  <User v-if="row.type === 'user'" />
                  <Message v-else />
                </el-icon>
              </template>
            </el-table-column>
            <el-table-column prop="name" label="名称" />
            <el-table-column label="联系方式">
              <template #default="{ row }">
                <div v-if="row.type === 'user'">
                  <div>{{ row.email }}</div>
                  <div>{{ row.phone }}</div>
                </div>
                <div v-else>
                  {{ row.members }} 个成员
                </div>
              </template>
            </el-table-column>
            <el-table-column label="操作" width="120">
              <template #default>
                <el-button size="small">编辑</el-button>
                <el-button size="small" type="danger">删除</el-button>
              </template>
            </el-table-column>
          </el-table>
        </el-card>
      </el-col>
      
      <el-col :span="12">
        <el-card>
          <template #header>
            <h3>通知设置</h3>
          </template>
          
          <el-form label-width="120px">
            <el-form-item label="邮件服务器">
              <el-input placeholder="smtp.example.com" />
            </el-form-item>
            <el-form-item label="SMTP端口">
              <el-input placeholder="587" />
            </el-form-item>
            <el-form-item label="发件人邮箱">
              <el-input placeholder="noreply@example.com" />
            </el-form-item>
            <el-form-item label="短信服务">
              <el-select placeholder="请选择短信服务商">
                <el-option label="阿里云短信" value="aliyun" />
                <el-option label="腾讯云短信" value="tencent" />
              </el-select>
            </el-form-item>
            <el-form-item label="企业微信">
              <el-switch />
            </el-form-item>
            <el-form-item>
              <el-button type="primary">保存设置</el-button>
              <el-button>测试通知</el-button>
            </el-form-item>
          </el-form>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<style lang="scss" scoped>
.alarm-notifications {
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
}
</style>