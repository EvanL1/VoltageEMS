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
        <div class="header__right-weatherValue">67℉~79℉</div>
      </div>

      <div class="header__right-avatar">
        <el-dropdown @command="handleUserCommand" trigger="click" :teleported="false">
          <div class="header__user">
            <!-- 头像图片无法显示的常见原因有：路径写法不对、图片未被正确引入、打包后路径丢失等。推荐用import方式引入图片资源。 -->
            <!-- <el-avatar  :src="userStore.userInfo?.avatar" class="header__user-avatar" /> -->
            <!-- <el-avatar :src="headerAvatar" class="header__user-avatar" /> -->
            <div class="header__user-avatar">
              <div class="header__user-avatar-initials">
                {{ getAvatarName(userStore.userInfo?.username || 'Admin') }}
              </div>
            </div>
            <span class="header__user-name">{{ userStore.userInfo?.username || '' }}</span>
            <!-- <span class="header__user-name">Esthera Jackson</span> -->
            <img :src="arrowDownIcon" class="header__user-arrow" />
          </div>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="logout">
                <div class="header__user-Item">
                  <img :src="logoutIcon" class="header__user-logoutIcon" />
                  Logout
                </div>
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
      </div>
      <div class="header__right-notice">
        <el-button link class="header__right-noticeBtn" @click="toggleNotifications">
          <el-badge :value="globalStore.alarmNum" :hidden="globalStore.alarmNum === 0">
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
import { useGlobalStore } from '@/stores/global'

// 正确引入头像图片，确保路径无误，推荐用import方式
import headerAvatar from '@/assets/images/header-avatar.png'
import logoutIcon from '@/assets/icons/user-logout.svg'
import noticeIcon from '@/assets/icons/notice.svg'
import arrowDownIcon from '@/assets/icons/arrowDownIcon.svg'
import sunIcon from '@/assets/icons/sunny.svg'

const route = useRoute()
const router = useRouter()
const userStore = useUserStore()

const globalStore = useGlobalStore()

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
  router.push({ name: 'alarmCurrentRecords' })
}

// 用户操作
const handleUserCommand = async (command: string) => {
  switch (command) {
    case 'logout':
      await userStore.logout()
      router.push('/login')
      break
  }
}
const getAvatarName = (name: string): string => {
  const nameStr = name.split(' ')
  if (nameStr.length === 1) {
    return name.charAt(0).toUpperCase()
  } else {
    return nameStr[0].charAt(0).toUpperCase() + nameStr[1].charAt(0).toUpperCase()
  }
}
</script>

<style lang="scss" scoped>
.voltage-class.header {
  position: relative;
  height: 0.85rem;
  background: rgba(84, 98, 140, 0.3);
  border-bottom: 0.01rem solid rgba(148, 166, 197, 0.3);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.2rem;
  z-index: 99;

  .header__left {
    display: flex;
    align-items: center;
    gap: 0.1rem;

    .header__left-title {
      font-family: Montserrat;
      font-weight: 600;
      font-style: Semi Bold;
      font-size: 0.3rem;
      line-height: 150%;
      letter-spacing: 0%;
      color: #ffffff;
      margin-right: 0.1rem;
    }

    .header__left-status {
      border: 0.01rem solid transparent;
      width: 1rem;
      height: 0.3rem;
      padding: 0.07rem 0 0.07rem 0.1rem;
      display: flex;
      align-items: center;
      background: rgba(84, 98, 140, 0.5);
      border-radius: 0.15rem;
      backdrop-filter: blur(0.1rem);

      .header__left-statusIcon {
        width: 0.16rem;
        height: 0.16rem;
        display: flex;
        justify-content: center;
        align-items: center;
        border-radius: 50%;
        background-color: rgba(106, 193, 97, 0.2);
        margin-right: 0.06rem;

        .header__left-statusIconCircle {
          width: 0.1rem;
          height: 0.1rem;
          border-radius: 50%;
          background-color: rgba(106, 193, 97, 1);
        }
      }

      .header__left-statusText {
        font-family: Arimo;
        font-weight: 700;
        font-style: Bold;
        font-size: 0.16rem;
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
      gap: 0.1rem;
      font-size: 0.2rem;
      letter-spacing: 0%;
      margin-right: 1.26rem;

      .header__right-weatherIcon {
        width: 0.4rem;
        height: 0.36rem;
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
      margin-right: 0.3rem;
    }

    .header__right-notice {
      .header__right-noticeBtn {
        padding: 0.2rem 0.1rem 0.13rem 0;
        transition: all 0.3s ease;

        :deep(.el-badge__content) {
          width: 0.16rem;
          height: 0.16rem;
          border: none;
          border-radius: 50%;
          background-color: rgba(218, 45, 44, 1);
          font-family: Arimo;
          font-weight: 400;
          font-size: 0.14rem;
        }

        .header__right-noticeIcon {
          width: 0.2rem;
          height: 0.2rem;
          object-fit: contain;
        }
      }
    }
  }
}

.header__user {
  display: flex;
  align-items: center;
  gap: 0.08rem;
  padding: 0.04rem 0.08rem;
  border-radius: 0.08rem;
  transition: all 0.3s ease;
}

.header__user-avatar {
  width: 0.4rem;
  height: 0.4rem;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: rgba(29, 134, 255, 0.2);

  .header__user-avatar-initials {
    color: rgba(29, 134, 255, 1);
    font-weight: 700;
    font-size: 0.19rem;
    line-height: 100%;
    letter-spacing: 0%;
  }
}

.header__user-avatar {
  width: 0.4rem;
  height: 0.4rem;
  border-radius: 50%;
}

.header__user-name {
  font-family: Arimo;
  font-weight: 500;
  font-style: Medium;
  font-size: 0.18rem;
  line-height: 140%;
  letter-spacing: 0%;
  color: #ffffff;
}

.header__user-arrow {
  height: 0.08rem;
  width: 0.1rem;
  font-size: 0.12rem;
  color: #909399;
}

:deep(.el-badge__content.is-fixed) {
  right: 0.1rem;
  top: 0.04rem;
  padding: 0;
}

.header__user-Item {
  width: 100%;
  padding: 0.1rem;
  display: flex;
  align-items: center;
  color: #ffffff;
  font-weight: 500;
  font-size: 0.14rem;
  line-height: 100%;
  letter-spacing: 0%;

  .header__user-logoutIcon {
    width: 0.2rem;
    height: 0.2rem;
    object-fit: contain;
    margin-right: 0.1rem;
  }
}
</style>
