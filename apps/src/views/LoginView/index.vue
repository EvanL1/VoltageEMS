<template>
  <div class="voltage-class loginPage">
    <header class="loginPage__header">
      <div class="loginPage__head-title">
        <div class="loginpage__head-icon"></div>
        <div class="loginPage__head-text">Log in page</div>
      </div>
    </header>
    <div ref="loginFormContainer" class="loginPage__form">
      <ModuleCard title="Monarch">
        <div class="loginPage__form-content">
          <el-form :model="form" label-position="top" ref="formRef" :rules="formRules">
            <el-form-item label="Role" prop="role">
              <el-select
                v-model="form.role"
                class="loginPage__form-select"
                :append-to="loginFormContainer"
              >
                <el-option
                  v-for="item in roleList"
                  :key="item.value"
                  :label="item.label"
                  :value="item.value"
                />
              </el-select>
            </el-form-item>
            <el-form-item label="Username" prop="username">
              <el-input v-model="form.username" />
            </el-form-item>
            <el-form-item label="Password" prop="password">
              <el-input v-model="form.password" type="password" />
            </el-form-item>
            <el-button type="primary" @click="handleLogin(formRef)">Log in</el-button>
          </el-form>
        </div>
      </ModuleCard>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { FormInstance, FormRules } from 'element-plus'
import { useUserStore } from '@/stores/user'
import type { LoginParams } from '@/types/user'
import { useRouter } from 'vue-router'

const router = useRouter()
const formRef = ref<FormInstance>()
const loginFormContainer = ref()
const form = reactive<LoginParams>({
  role: 'admin',
  username: '',
  password: '',
})
const roleList = ref<{ label: string; value: string }[]>([
  { label: 'Admin', value: 'admin' },
  { label: 'Operator', value: 'operator' },
  { label: 'Engineer', value: 'engineer' },
])
const formRules = reactive<FormRules<LoginParams>>({
  role: [{ required: true, message: '请选择角色', trigger: 'change' }],
  username: [{ required: true, message: '请输入用户名', trigger: 'blur' }],
  password: [
    { required: true, message: '请输入密码', trigger: 'blur' },
    {
      pattern: /^(?=.*[A-Za-z])(?=.*\d)[A-Za-z\d]{4,12}$/,
      message: '密码长度4-12位，必须包含字母和数字',
      trigger: 'blur',
    },
  ],
})
const userStore = useUserStore()

const handleLogin = async (formEl: FormInstance | undefined) => {
  router.push({ name: 'home' })
  // 使用Element Plus表单校验
  // if (!formEl) return
  // formEl.validate((valid: boolean) => {
  //   if (valid) {
  //     // userStore.login(form)
  //     // 这里可以添加登录请求逻辑
  //   } else {
  //     console.log('表单校验未通过')
  //   }
  // })
}
</script>

<style lang="scss" scoped>
.voltage-class.loginPage {
  width: 100%;
  height: 100%;
  background: #02081a;
  position: relative;
  overflow: hidden;
  background-image: url('../../assets/images/login-bg.png');
  background-size: cover;
  background-position: center;
  background-repeat: no-repeat;
  border: 1px solid;

  border-image-source: linear-gradient(
    117.31deg,
    rgba(148, 166, 197, 0.3) 2.77%,
    rgba(148, 166, 197, 0) 32.18%,
    rgba(148, 166, 197, 0.103266) 72.63%,
    rgba(148, 166, 197, 0.3) 99.05%
  );

  // 顶部标题栏
  .loginPage__header {
    width: 100%;
    height: 84px;
    display: flex;
    align-items: center;
    background: rgba(84, 98, 140, 0.3);
    border-bottom: 1px solid rgba(148, 166, 197, 0.3);

    .loginPage__head-title {
      height: 100%;
      display: flex;
      align-items: center;
      margin-left: 30px;
      .loginpage__head-icon {
        width: 24px;
        height: 24px;
        background-image: url('../../assets/images/login-logo.png');
        background-size: cover;
        background-position: center;
        background-repeat: no-repeat;
      }
      .loginPage__head-text {
        margin-left: 20px;
        font-family: 'Montserrat', sans-serif;
        font-weight: 600;
        font-size: 30px;
        line-height: 1.5em;
        color: #ffffff;
      }
    }
    .loginPage__form-button {
      margin-top: 20px;
    }
  }

  // 登录表单区域
  .loginPage__form {
    position: absolute;
    top: 328px;
    left: 1462px;
    width: 324px;
    .loginPage__form-content {
      padding: 40px 21px;
    }
  }
}
:deep(.el-button.el-button--primary) {
  height: 32px;
  width: 240px;
  margin-top: 20px;
}
:deep(.el-form-item__label::before) {
  display: none !important;
}
:deep(.el-select__popper.el-popper) {
  top: 166px !important;
}
</style>
