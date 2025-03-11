<template>
  <div class="voltage-class devices-diesel">
    <!-- 页面头部 -->
    <div class="devices-diesel__header">
      <div class="devices-diesel__tabs">
        <el-button
          :type="activeTab === 'overview' ? 'primary' : 'warning'"
          @click="handleTabClick('overview')"
          class="devices-diesel__tab-btn"
        >
          <img :src="alarmCurrentIcon" class="devices-diesel__tab-icon" />
          Overview
        </el-button>
        <el-button
          :type="activeTab === 'monitoring' ? 'primary' : 'warning'"
          @click="handleTabClick('monitoring')"
          class="devices-diesel__tab-btn"
        >
          <img :src="alarmHistoryIcon" class="devices-diesel__tab-icon" />
          Value Monitoring
        </el-button>
      </div>
    </div>
    <!-- 路由内容区域 -->
    <div class="devices-diesel__main">
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
  if (path.includes('/monitoring')) {
    return 'monitoring'
  }
  return 'overview'
})

// 处理标签点击事件
const handleTabClick = (tab: 'overview' | 'monitoring') => {
  if (tab === 'overview') {
    router.push('/devices/dieselGenerator/overview')
  } else {
    router.push('/devices/dieselGenerator/monitoring')
  }
}
</script>

<style scoped lang="scss">
.voltage-class.devices-diesel {
  height: 100%;
  display: flex;
  flex-direction: column;
  .devices-diesel__header {
    padding-bottom: 20px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    .devices-diesel__tabs {
      display: flex;
      align-items: center;
      gap: 16px;
      .devices-diesel__tab-btn {
        display: flex;
        align-items: center;
        gap: 10px;
        .devices-diesel__tab-icon {
          width: 16px;
          height: 16px;
          margin-right: 8px;
        }
      }
    }
  }
  .devices-diesel__main {
    flex: 1;
    width: 100%;
  }
}
</style>
