<template>
  <div class="voltage-class sidebar" :class="{ collapse: globalStore.isCollapse }">
    <div class="sidebar__header" :class="{ collapse: globalStore.isCollapse }">
      <div class="sidebar__header-img" :class="{ collapse: globalStore.isCollapse }"></div>
    </div>

    <nav class="sidebar__nav">
      <el-menu
        :collapse="globalStore.isCollapse"
        class="sidebar__menu"
        :default-active="activeMenuPath"
        router
        background-color="transparent"
        text-color="#fff"
        active-text-color="#fff"
        :unique-opened="true"
      >
        <template v-for="item in globalStore.filterRoutesList" :key="item.path">
          <el-sub-menu
            v-if="item.meta.isSubMenu"
            :index="item.meta.activeNav"
            class="sidebar__subMenu"
          >
            <template #title>
              <img
                v-if="item.meta.iconName"
                :src="getSidebarIconUrl(item.meta.iconName)"
                class="sidebar__subMenu-img"
                :class="{ collapse: globalStore.isCollapse }"
                alt=""
              />
              <span class="sidebar__subMenu-title">{{ item.meta.title }}</span>
            </template>
            <el-menu-item
              v-for="child in item.children"
              :key="child.path"
              :index="child.meta.activeNav"
            >
              <span>{{ child.meta.title }}</span>
            </el-menu-item>
          </el-sub-menu>
          <el-menu-item v-else :index="item.meta.activeNav" class="sidebar__menu-item">
            <img
              v-if="item.meta.iconName"
              :src="getSidebarIconUrl(item.meta.iconName)"
              class="sidebar__subMenu-img"
              :class="{ collapse: globalStore.isCollapse }"
              alt=""
            />
            <span class="sidebar__menu-text">{{ item.meta.title }}</span>
          </el-menu-item>
        </template>
      </el-menu>
    </nav>

    <div class="sidebar__footer" :class="{ collapse: globalStore.isCollapse }">
      <div
        class="sidebar__footer-shrink"
        :class="{ collapse: globalStore.isCollapse }"
        @click="handleShrink"
      ></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useGlobalStore } from '@/stores/global'
import { useRoute } from 'vue-router'

const globalStore = useGlobalStore()
const route = useRoute()
const activeMenuPath = ref<string>(globalStore.filterRoutesList[0].meta.activeNav)
// 1. 使用import.meta.globEager批量导入侧边栏icon图片
const sidebarIcons = import.meta.glob('@/assets/images/sidebar-*-icon.png', {
  eager: true,
  import: 'default',
})

/**
 * 获取侧边栏icon图片的真实路径
 * @param iconName 图标名
 * @returns 图片路径
 */
function getSidebarIconUrl(iconName: string): string {
  // 兼容@和/src两种写法
  const key1 = `/src/assets/images/sidebar-${iconName}-icon.png`
  const key2 = `@/assets/images/sidebar-${iconName}-icon.png`
  const result = sidebarIcons[key1] || sidebarIcons[key2] || ''
  return typeof result === 'string' ? result : ''
}

// 监听路由变化，更新激活的菜单
watch(
  route,
  (newPath) => {
    // console.log(newPath)
    activeMenuPath.value = newPath.meta.activeNav as string
  },
  { immediate: true },
)

const handleShrink = () => {
  globalStore.isCollapse = !globalStore.isCollapse
}
</script>

<style lang="scss" scoped>
.voltage-class.sidebar {
  position: relative;
  height: 100vh;
  padding: 20px 0;
  background: rgba(84, 98, 140, 0.4);
  border-right: 1px solid;
  border-image-source: linear-gradient(
    147.24deg,
    rgba(148, 166, 197, 0.72) 39.16%,
    rgba(148, 166, 197, 0.36) 66.27%,
    rgba(148, 166, 197, 0.72) 98.58%
  );
  backdrop-filter: blur(10px);
  display: flex;
  flex-direction: column;

  transition: width 0.3s ease-in-out;
  width: 2.2rem; // 默认展开
  &.collapse {
    width: 85px;
  }

  .sidebar__header {
    width: 100%;
    display: flex;
    justify-content: center;
    align-items: center;
    margin: 20px auto;
    // 默认展开
    // &.collapse {
    //   margin: 20px 0;
    // }
    // &.no-collapse {
    //   margin: 20px 40px;
    // }
    .sidebar__header-img {
      background-repeat: no-repeat;
      background-image: url('@/assets/images/sidebar-logo.png');
      width: 100px;
      height: 68.6px;
      background-size: 100% 100%;
      &.collapse {
        background-image: url('@/assets/images/sidebar-logo-collapse.png');
        width: 50px;
        height: 68.6px;
        background-size: 100% auto;
      }
    }
  }

  .sidebar__logo {
    display: flex;
    align-items: center;
    gap: 12px;

    .sidebar__logo-text {
      font-size: 18px;
      font-weight: 600;
      color: #ffffff;
      font-family: 'Montserrat', sans-serif;
    }
  }

  .sidebar__nav {
    flex: 1;
    padding: 20px;
    .sidebar__menu-item {
    }
  }
  .sidebar__subMenu-img {
    width: 24px;
    height: 24px;
    margin-right: 10px; // 默认展开
    &.collapse {
      margin-right: 0;
    }
  }
  .sidebar__menu-text {
    font-weight: 700;
    font-size: 14px;
  }
  .sidebar__subMenu-title {
    font-family: Arimo;
    font-weight: 700;
    font-style: Bold;
    font-size: 14px;
    letter-spacing: 0%;
    color: #fff;
  }
  .sidebar__footer {
    position: relative;
    right: 0;
    bottom: 0;
    padding: 0 20px;
    display: flex;
    flex-direction: row-reverse;
    transition: all 0.3s ease;
    justify-content: flex-start;
    align-items: center;
    &.collapse {
      justify-content: center;
      align-items: center;
    }
    .sidebar__footer-shrink {
      background-image: url('@/assets/icons/sidebar-shrink.svg');
      background-size: 100% 100%;
      background-repeat: no-repeat;
      background-position: center;
      width: 24px;
      height: 24px;
      cursor: pointer;
      &.collapse {
        background-image: url('@/assets/icons/sidebar-grow.svg');
      }
    }
  }
}
</style>
