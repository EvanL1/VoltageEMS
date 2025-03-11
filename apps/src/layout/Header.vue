<template>
  <div class="voltage-class header">
    <div class="header__left">
      <div class="header__left-title">Norton creek solar energy center</div>
      <div class="header__left-status">
        <div class="header__left-statusIcon">
          <div class="header__left-statusIconCircle"></div>
        </div>
        <div class="header__left-statusText">Online</div>
      </div>
    </div>

    <div class="header__right">
      <div class="header__right-weather">
        <img :src="sunIcon" alt="sunIcon" class="header__right-weatherIcon" />
        <div class="header__right-weatherStatus">wind</div>
        <div class="header__right-weatherValue">67F~79F</div>
      </div>

      <div class="header__right-avatar">
        <el-dropdown @command="handleUserCommand" trigger="click">
          <div class="header__user">
            <!-- 头像图片无法显示的常见原因有：路径写法不对、图片未被正确引入、打包后路径丢失等。推荐用import方式引入图片资源。 -->
            <!-- <el-avatar  :src="userStore.userInfo?.avatar" class="header__user-avatar" /> -->
            <el-avatar :src="headerAvatar" class="header__user-avatar" />
            <!-- <span class="header__user-name">{{
              userStore.userInfo?.nickname || userStore.userInfo?.username
            }}</span> -->
            <span class="header__user-name">Esthera Jackson</span>
            <img :src="arrowDownIcon" class="header__user-arrow" />
          </div>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="logout" class="header__user-Item">
                <img :src="logoutIcon" class="header__user-logoutIcon" />
                退出登录
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
      </div>
      <div class="header__right-notice">
        <el-button link class="header__right-noticeBtn" @click="toggleNotifications">
          <el-badge :value="notificationCount" :hidden="notificationCount === 0">
            <img :src="noticeIcon" class="header__right-noticeIcon" />
          </el-badge>
        </el-button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useUserStore } from '@/stores/user'
import { Bell, ArrowDown, SwitchButton } from '@element-plus/icons-vue'

// 正确引入头像图片，确保路径无误，推荐用import方式
import headerAvatar from '@/assets/images/header-avatar.png'
import logoutIcon from '@/assets/icons/user-logout.svg'
import noticeIcon from '@/assets/icons/notice.svg'
import arrowDownIcon from '@/assets/icons/arrowDownIcon.svg'
import sunIcon from '@/assets/icons/sunny.svg'

const route = useRoute()
const router = useRouter()
const userStore = useUserStore()

const notificationCount = ref(3)

// 面包屑导航
const breadcrumbItems = computed(() => {
  const items = [{ title: '首页', path: '/dashboard' }]

  if (route.meta?.title) {
    items.push({ title: route.meta.title as string, path: route.path })
  }

  return items
})

// 切换通知
const toggleNotifications = () => {
  ElMessage.info('通知功能开发中...')
}

// 用户操作
const handleUserCommand = async (command: string) => {
  switch (command) {
    case 'logout':
      await userStore.logout()
      router.push('/login')
      ElMessage.success('已退出登录')
      break
  }
}
</script>

<style lang="scss" scoped>
.voltage-class.header {
  position: relative;
  height: 85px;
  background: rgba(84, 98, 140, 0.3);
  border-bottom: 1px solid rgba(148, 166, 197, 0.3);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px;
  z-index: 99;
  .header__left {
    display: flex;
    align-items: center;
    gap: 10px;
    .header__left-title {
      font-family: Montserrat;
      font-weight: 600;
      font-style: Semi Bold;
      font-size: 30px;
      line-height: 150%;
      letter-spacing: 0%;
      color: #ffffff;
      margin-right: 10px;
    }
    .header__left-status {
      border: 1px solid transparent;
      width: 100px;
      height: 30px;
      padding: 7px 0 7px 10px;
      display: flex;
      align-items: center;
      background: rgba(84, 98, 140, 0.5);
      border-radius: 15px;
      backdrop-filter: blur(10px);
      .header__left-statusIcon {
        width: 16px;
        height: 16px;
        display: flex;
        justify-content: center;
        align-items: center;
        border-radius: 50%;
        background-color: rgba(106, 193, 97, 0.2);
        margin-right: 6px;
        .header__left-statusIconCircle {
          width: 10px;
          height: 10px;
          border-radius: 50%;
          background-color: rgba(106, 193, 97, 1);
        }
      }
      .header__left-statusText {
        font-family: Arimo;
        font-weight: 700;
        font-style: Bold;
        font-size: 16px;
        line-height: 100%;
        letter-spacing: 0%;
        vertical-align: middle;
        color: #ffffff;
      }
    }
  }
  .header__right {
    flex: 1;
    display: flex;
    justify-content: flex-end;
    align-items: center;

    .header__right-weather {
      display: flex;
      align-items: center;
      gap: 10px;
      font-size: 20px;
      letter-spacing: 0%;
      margin-right: 126px;
      .header__right-weatherIcon {
        width: 40px;
        height: 36px;
        object-fit: contain;
      }

      .header__right-weatherStatus {
        font-weight: 500;
        font-style: Medium;
      }
      .header__right-weatherValue {
        font-weight: 700;
        font-style: Bold;
      }
    }
    .header__right-avatar {
      cursor: pointer;
      margin-right: 30px;
    }
    .header__right-notice {
      .header__right-noticeBtn {
        padding: 20px 10px 13px 0;
        transition: all 0.3s ease;
        :deep(.el-badge__content) {
          width: 16px;
          height: 16px;
          border: none;
          border-radius: 50%;
          background-color: rgba(218, 45, 44, 1);
          font-family: Arimo;
          font-weight: 400;
          font-size: 14px;
        }
        .header__right-noticeIcon {
          width: 20px;
          height: 20px;
          object-fit: contain;
        }
      }
    }
  }
}

.header__user {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 8px;
  border-radius: 8px;
  transition: all 0.3s ease;
}

.header__user-avatar {
  width: 40px;
  height: 40px;
  border-radius: 50%;
}

.header__user-name {
  font-family: Arimo;
  font-weight: 500;
  font-style: Medium;
  font-size: 18px;
  line-height: 140%;
  letter-spacing: 0%;
  color: #ffffff;
}

.header__user-arrow {
  height: 8px;
  width: 10px;
  font-size: 12px;
  color: #909399;
}
:deep(.el-badge__content.is-fixed) {
  right: 10px;
  padding: 0;
}

.header__user-Item {
  width: 153px;
  display: flex;
  align-items: center;
  color: #ffffff;
  font-weight: 500;
  font-size: 14px;
  line-height: 100%;
  letter-spacing: 0%;
  .header__user-logoutIcon {
    width: 20px;
    height: 20px;
    object-fit: contain;
    margin-right: 10px;
  }
}
</style>
