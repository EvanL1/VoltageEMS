<template>
  <div class="voltage-class devices-battery">
    <!-- 页面头部 -->
    <div class="devices-battery__header">
      <div class="devices-battery__tabs">
        <el-button
          :type="activeTab === 'overview' ? 'primary' : 'warning'"
          @click="handleTabClick('overview')"
          class="devices-battery__tab-btn"
        >
          <img :src="alarmCurrentIcon" class="devices-battery__tab-icon" />
          Overview
        </el-button>
        <el-button
          :type="activeTab === 'value' ? 'primary' : 'warning'"
          @click="handleTabClick('value')"
          class="devices-battery__tab-btn"
        >
          <img :src="alarmHistoryIcon" class="devices-battery__tab-icon" />
          Value value
        </el-button>
        <el-button
          :type="activeTab === 'management' ? 'primary' : 'warning'"
          @click="handleTabClick('management')"
          class="devices-battery__tab-btn"
        >
          <img :src="alarmCurrentIcon" class="devices-battery__tab-icon" />
          Battery Management
        </el-button>
      </div>
    </div>
    <!-- 路由内容区域 -->
    <div class="devices-battery__content">
      <router-view />
    </div>
  </div>
</template>

<script setup lang="ts">
// 正确引入SVG图标，避免部署后图片加载不出来
import alarmCurrentIcon from '@/assets/icons/alarm-current.svg'
import alarmHistoryIcon from '@/assets/icons/alarm-history.svg'

// 响应式数据
const route = useRoute()
const router = useRouter()

// 根据当前路由计算激活的标签
const activeTab = computed(() => {
  const path = route.path
  if (path.includes('/value')) {
    return 'value'
  } else if (path.includes('/management')) {
    return 'management'
  }
  return 'overview'
})

// 处理标签点击事件
const handleTabClick = (tab: 'overview' | 'value' | 'management') => {
  if (tab === 'overview') {
    router.push('/devices/deviceBattery/overview')
  } else if (tab === 'value') {
    router.push('/devices/deviceBattery/value')
  } else {
    router.push('/devices/deviceBattery/management')
  }
}
</script>

<style scoped lang="scss">
.voltage-class.devices-battery {
  height: 100%;
  display: flex;
  flex-direction: column;
  .devices-battery__header {
    padding-bottom: 20px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    .devices-battery__tabs {
      display: flex;
      align-items: center;
      gap: 16px;
      .devices-battery__tab-btn {
        display: flex;
        align-items: center;
        gap: 10px;
        .devices-battery__tab-icon {
          width: 16px;
          height: 16px;
          margin-right: 8px;
        }
      }
    }
  }
  .devices-battery__content {
    flex: 1;
    padding-top: 20px;
  }
}
</style>
