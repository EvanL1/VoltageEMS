<template>
  <div class="login-container">
    <div class="login-box">
      <div class="login-header">
        <el-icon size="60" color="#409EFF"><Lightning /></el-icon>
        <h1>VoltageEMS</h1>
        <p>Energy Management System</p>
      </div>
      
      <el-form
        ref="loginFormRef"
        :model="loginForm"
        :rules="loginRules"
        class="login-form"
        @keyup.enter="handleLogin"
      >
        <el-form-item prop="username">
          <el-input
            v-model="loginForm.username"
            placeholder="Username"
            :prefix-icon="User"
            size="large"
          />
        </el-form-item>
        
        <el-form-item prop="password">
          <el-input
            v-model="loginForm.password"
            type="password"
            placeholder="Password"
            :prefix-icon="Lock"
            size="large"
            show-password
          />
        </el-form-item>
        
        <el-form-item>
          <el-checkbox v-model="loginForm.remember">Remember me</el-checkbox>
        </el-form-item>
        
        <el-form-item>
          <el-button
            type="primary"
            size="large"
            style="width: 100%"
            :loading="loading"
            @click="handleLogin"
          >
            Sign In
          </el-button>
        </el-form-item>
      </el-form>
      
      <div class="login-footer">
        <el-text type="info">
          Demo account: admin / admin123
        </el-text>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from 'vue'
import { useRouter } from 'vue-router'
import { User, Lock, Lightning } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'

const router = useRouter()
const loginFormRef = ref<FormInstance>()
const loading = ref(false)

const loginForm = reactive({
  username: '',
  password: '',
  remember: false
})

const loginRules = reactive<FormRules>({
  username: [
    { required: true, message: 'Please enter username', trigger: 'blur' },
    { min: 3, max: 20, message: 'Length should be 3 to 20', trigger: 'blur' }
  ],
  password: [
    { required: true, message: 'Please enter password', trigger: 'blur' },
    { min: 6, message: 'Password must be at least 6 characters', trigger: 'blur' }
  ]
})

async function handleLogin() {
  const valid = await loginFormRef.value?.validate()
  if (!valid) return
  
  loading.value = true
  
  try {
    // Simulate login
    await new Promise(resolve => setTimeout(resolve, 1000))
    
    // Demo: Accept admin/admin123
    if (loginForm.username === 'admin' && loginForm.password === 'admin123') {
      // Store token
      localStorage.setItem('token', 'demo-token-' + Date.now())
      localStorage.setItem('username', loginForm.username)
      
      if (loginForm.remember) {
        localStorage.setItem('remember', 'true')
      }
      
      ElMessage.success('Login successful')
      router.push('/dashboard')
    } else {
      ElMessage.error('Invalid username or password')
    }
  } catch (error) {
    ElMessage.error('Login failed')
  } finally {
    loading.value = false
  }
}
</script>

<style lang="scss" scoped>
.login-container {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  
  .login-box {
    width: 400px;
    background: white;
    border-radius: 8px;
    box-shadow: 0 2px 12px 0 rgba(0, 0, 0, 0.1);
    padding: 40px;
    
    .login-header {
      text-align: center;
      margin-bottom: 40px;
      
      h1 {
        margin: 10px 0;
        font-size: 28px;
        color: #303133;
      }
      
      p {
        color: #909399;
        margin: 0;
      }
    }
    
    .login-footer {
      text-align: center;
      margin-top: 20px;
    }
  }
}
</style>