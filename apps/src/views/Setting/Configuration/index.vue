<template>
  <div class="voltage-class configuration">
    <!-- Page Header -->
    <div class="configuration__header">
      <div class="configuration__tabs">
        <el-button
          :type="activeTab === 'channel configuration' ? 'primary' : 'warning'"
          @click="handleTabClick('channel configuration')"
          class="configuration__tab-btn"
        >
          <img :src="channelIcon" class="configuration__tab-icon" />
          channel configuration
        </el-button>
        <el-button
          :type="activeTab === 'Device model configuration' ? 'primary' : 'warning'"
          @click="handleTabClick('Device model configuration')"
          class="configuration__tab-btn"
        >
          <img :src="instanceIcon" class="configuration__tab-icon" />
          Device model configuration
        </el-button>
        <el-button
          :type="activeTab === 'Rules Configuration' ? 'primary' : 'warning'"
          @click="handleTabClick('Rules Configuration')"
          class="configuration__tab-btn"
        >
          <img :src="ruleIcon" class="configuration__tab-icon" />
          Rules configuration
        </el-button>
      </div>
    </div>
    <!-- Router Content Area -->
    <div class="configuration__content">
      <router-view />
    </div>
  </div>
</template>

<script setup lang="ts">
// Correctly import SVG icons to avoid image loading issues after deployment
import alarmCurrentIcon from '@/assets/icons/alarm-current.svg'
import alarmHistoryIcon from '@/assets/icons/alarm-history.svg'
import ruleIcon from '@/assets/icons/button-rule.svg'
import instanceIcon from '@/assets/icons/button-instance.svg'
import channelIcon from '@/assets/icons/button-channel.svg'
// Reactive data
const route = useRoute()
const router = useRouter()

// Calculate active tab based on current route
const activeTab = computed(() => {
  const path = route.path
  if (path.includes('/channelConfiguration')) {
    return 'channel configuration'
  } else if (path.includes('/modelConfiguration')) {
    return 'Device model configuration'
  }
  return 'Rules Configuration'
})

// Handle tab click event
const handleTabClick = (
  tab: 'channel configuration' | 'Device model configuration' | 'Rules Configuration',
) => {
  if (tab === 'channel configuration') {
    router.push('/setting/configuration/channelConfiguration')
  } else if (tab === 'Device model configuration') {
    router.push('/setting/configuration/modelConfiguration')
  } else if (tab === 'Rules Configuration') {
    router.push('/setting/configuration/ruleConfiguration')
  }
}
</script>

<style scoped lang="scss">
.voltage-class.configuration {
  height: 100%;
  display: flex;
  flex-direction: column;
  position: relative;

  .configuration__header {
    padding-bottom: 0.2rem;
    border-bottom: 0.01rem solid rgba(255, 255, 255, 0.1);

    .configuration__tabs {
      display: flex;
      align-items: center;
      gap: 0.16rem;

      .configuration__tab-btn {
        display: flex;
        align-items: center;
        gap: 0.1rem;

        .configuration__tab-icon {
          width: 0.16rem;
          height: 0.16rem;
          margin-right: 0.08rem;
        }
      }
    }
  }

  .configuration__content {
    // position: relative;
    height: calc(100% - 0.52rem);
    display: flex;
    flex-direction: column;

    .configuration__toolbar {
      padding: 0.2rem 0;
      display: flex;
      align-items: center;
      justify-content: space-between;

      .configuration__toolbar-left {
        display: flex;
        align-items: center;
        gap: 0.16rem;
      }

      .configuration__toolbar-right {
        display: flex;
        align-items: center;
        gap: 0.16rem;

        .configuration__export-btn {
          display: flex;
          align-items: center;
          gap: 0.1rem;

          .configuration__export-icon {
            width: 0.16rem;
            height: 0.16rem;
          }
        }
      }
    }

    .configuration__table {
      flex: 1;
      display: flex;
      flex-direction: column;

      .configuration__table-content {
        flex: 1;
        overflow-y: auto;
      }

      .configuration__pagination {
        padding: 0.2rem 0;
        display: flex;
        justify-content: flex-end;
      }
    }
  }
}
</style>
