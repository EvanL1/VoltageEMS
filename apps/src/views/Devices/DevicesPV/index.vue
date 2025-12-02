<template>
  <div class="voltage-class devices-pv">
    <!-- 页面头部 -->
    <div class="devices-pv__header">
      <div class="devices-pv__tabs">
        <el-button
          :type="activeTab === 'overview' ? 'primary' : 'warning'"
          @click="handleTabClick('overview')"
          class="devices-pv__tab-btn"
        >
          <img :src="alarmCurrentIcon" class="devices-pv__tab-icon" />
          Overview
        </el-button>
        <el-button
          :type="activeTab === 'monitoring' ? 'primary' : 'warning'"
          @click="handleTabClick('monitoring')"
          class="devices-pv__tab-btn"
        >
          <img :src="alarmHistoryIcon" class="devices-pv__tab-icon" />
          Value Monitoring
        </el-button>
      </div>
    </div>
    <!-- 路由内容区域 -->
    <div class="devices-pv__content">
      <router-view />
    </div>
  </div>
</template>

<script setup lang="ts">
// 正确引入SVG图标，避免部署后图片加载不出�?
import alarmCurrentIcon from '@/assets/icons/alarm-current.svg'
import alarmHistoryIcon from '@/assets/icons/alarm-history.svg'

// 响应式数�?
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
    router.push('/devices/devicesPV/overview')
  } else {
    router.push('/devices/devicesPV/monitoring')
  }
}
</script>

<style scoped lang="scss">
.voltage-class.devices-pv {
  height: 100%;
  display: flex;
  flex-direction: column;
  .devices-pv__header {
    padding-bottom: 0.2rem;
    border-bottom: 0.01rem solid rgba(255, 255, 255, 0.1);
    .devices-pv__tabs {
      display: flex;
      align-items: center;
      gap: 0.16rem;
      .devices-pv__tab-btn {
        display: flex;
        align-items: center;
        gap: 0.1rem;
        .devices-pv__tab-icon {
          width: 0.16rem;
          height: 0.16rem;
          margin-right: 0.08rem;
        }
      }
    }
  }
  .devices-pv__content {
    height: calc(100% - 0.53rem);
    display: flex;
    flex-direction: column;

    .devices-pv__toolbar {
      padding: 0.2rem 0;
      display: flex;
      align-items: center;
      justify-content: space-between;

      .devices-pv__toolbar-left {
        display: flex;
        align-items: center;
        gap: 0.16rem;
      }

      .devices-pv__toolbar-right {
        display: flex;
        align-items: center;
        gap: 0.16rem;

        .devices-pv__export-btn {
          display: flex;
          align-items: center;
          gap: 0.1rem;
          .devices-pv__export-icon {
            width: 0.16rem;
            height: 0.16rem;
          }
        }
      }
    }

    .devices-pv__table {
      flex: 1;
      display: flex;
      flex-direction: column;

      .devices-pv__table-content {
        flex: 1;
        overflow-y: auto;
      }

      .devices-pv__pagination {
        padding: 0.2rem 0;
        display: flex;
        justify-content: flex-end;
      }
    }
  }
}
</style>

