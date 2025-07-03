# VoltageEMS 边缘端前端实现指南

## 一、项目初始化

### 1. 技术栈确认
```json
{
  "vue": "^3.3.0",
  "typescript": "^5.0.0",
  "vite": "^4.3.0",
  "element-plus": "^2.3.0",
  "pinia": "^2.1.0",
  "vue-router": "^4.2.0",
  "axios": "^1.4.0",
  "echarts": "^5.4.0",
  "dayjs": "^1.11.0"
}
```

### 2. 目录结构
```
frontend/
├── src/
│   ├── api/              # API接口
│   ├── assets/           # 静态资源
│   ├── components/       # 公共组件
│   ├── composables/      # 组合式函数
│   ├── directives/       # 自定义指令
│   ├── layouts/          # 布局组件
│   ├── router/           # 路由配置
│   ├── stores/           # Pinia状态管理
│   ├── styles/           # 全局样式
│   ├── types/            # TypeScript类型
│   ├── utils/            # 工具函数
│   └── views/            # 页面组件
│       ├── monitoring/   # 监控展示类
│       ├── control/      # 控制操作类
│       ├── config/       # 配置管理类
│       └── system/       # 系统管理类
```

## 二、权限系统实现

### 1. 用户Store定义
```typescript
// stores/user.ts
import { defineStore } from 'pinia'
import { login, getUserInfo } from '@/api/auth'

interface UserState {
  token: string | null
  userInfo: UserInfo | null
  role: 'operator' | 'engineer' | 'admin' | null
  permissions: string[]
}

export const useUserStore = defineStore('user', {
  state: (): UserState => ({
    token: localStorage.getItem('token'),
    userInfo: null,
    role: null,
    permissions: []
  }),

  getters: {
    isOperator: (state) => state.role === 'operator',
    isEngineer: (state) => state.role === 'engineer',
    isAdmin: (state) => state.role === 'admin',
    canControl: (state) => ['engineer', 'admin'].includes(state.role!),
    canConfig: (state) => state.role === 'admin'
  },

  actions: {
    async login(credentials: LoginCredentials) {
      const { token } = await login(credentials)
      this.token = token
      localStorage.setItem('token', token)
      await this.fetchUserInfo()
    },

    async fetchUserInfo() {
      const userInfo = await getUserInfo()
      this.userInfo = userInfo
      this.role = userInfo.role
      this.permissions = userInfo.permissions
    },

    logout() {
      this.token = null
      this.userInfo = null
      this.role = null
      this.permissions = []
      localStorage.removeItem('token')
    }
  }
})
```

### 2. 路由权限配置
```typescript
// router/routes.ts
export const routes = [
  {
    path: '/',
    component: () => import('@/layouts/MainLayout.vue'),
    children: [
      // 监控展示类 - 所有用户
      {
        path: 'dashboard',
        name: 'Dashboard',
        component: () => import('@/views/monitoring/Dashboard.vue'),
        meta: { 
          title: '系统总览',
          icon: 'dashboard',
          roles: ['operator', 'engineer', 'admin']
        }
      },
      {
        path: 'monitoring/realtime',
        name: 'RealtimeMonitoring',
        component: () => import('@/views/monitoring/Realtime.vue'),
        meta: { 
          title: '实时监控',
          icon: 'monitor',
          roles: ['operator', 'engineer', 'admin']
        }
      },
      
      // 控制操作类 - 工程师及以上
      {
        path: 'control/devices',
        name: 'DeviceControl',
        component: () => import('@/views/control/DeviceControl.vue'),
        meta: { 
          title: '设备控制',
          icon: 'control',
          roles: ['engineer', 'admin']
        }
      },
      
      // 配置管理类 - 仅管理员
      {
        path: 'config/channels',
        name: 'ChannelConfig',
        component: () => import('@/views/config/ChannelConfig.vue'),
        meta: { 
          title: '通道配置',
          icon: 'setting',
          roles: ['admin']
        }
      }
    ]
  }
]
```

### 3. 路由守卫实现
```typescript
// router/guards.ts
import { useUserStore } from '@/stores/user'

export function setupRouterGuards(router: Router) {
  router.beforeEach(async (to, from, next) => {
    const userStore = useUserStore()
    
    // 白名单页面
    const whiteList = ['/login', '/403', '/404']
    if (whiteList.includes(to.path)) {
      next()
      return
    }
    
    // 检查登录状态
    if (!userStore.token) {
      next(`/login?redirect=${to.path}`)
      return
    }
    
    // 获取用户信息
    if (!userStore.userInfo) {
      try {
        await userStore.fetchUserInfo()
      } catch (error) {
        userStore.logout()
        next('/login')
        return
      }
    }
    
    // 检查页面权限
    const requiredRoles = to.meta.roles as string[] | undefined
    if (requiredRoles && requiredRoles.length > 0) {
      if (!requiredRoles.includes(userStore.role!)) {
        next('/403')
        return
      }
    }
    
    next()
  })
}
```

### 4. 权限指令实现
```typescript
// directives/permission.ts
import { useUserStore } from '@/stores/user'

export const vPermission = {
  mounted(el: HTMLElement, binding: DirectiveBinding) {
    const { value } = binding
    const userStore = useUserStore()
    
    if (value && value instanceof Array && value.length > 0) {
      const hasPermission = value.includes(userStore.role)
      
      if (!hasPermission) {
        el.style.display = 'none'
      }
    }
  },
  
  updated(el: HTMLElement, binding: DirectiveBinding) {
    const { value } = binding
    const userStore = useUserStore()
    
    if (value && value instanceof Array && value.length > 0) {
      const hasPermission = value.includes(userStore.role)
      
      if (!hasPermission) {
        el.style.display = 'none'
      } else {
        el.style.display = ''
      }
    }
  }
}
```

## 三、核心页面实现

### 1. 系统总览 Dashboard
```vue
<!-- views/monitoring/Dashboard.vue -->
<template>
  <div class="dashboard-container">
    <!-- 统计卡片 -->
    <el-row :gutter="20" class="stat-cards">
      <el-col :xs="24" :sm="12" :lg="6">
        <StatCard 
          title="设备总数" 
          :value="stats.totalDevices"
          icon="device"
          color="#409eff"
        />
      </el-col>
      <el-col :xs="24" :sm="12" :lg="6">
        <StatCard 
          title="在线率" 
          :value="`${stats.onlineRate}%`"
          icon="online"
          color="#67c23a"
        />
      </el-col>
      <el-col :xs="24" :sm="12" :lg="6">
        <StatCard 
          title="今日能耗" 
          :value="`${stats.todayEnergy} kWh`"
          icon="energy"
          color="#e6a23c"
        />
      </el-col>
      <el-col :xs="24" :sm="12" :lg="6">
        <StatCard 
          title="活跃告警" 
          :value="stats.activeAlarms"
          icon="alarm"
          color="#f56c6c"
        />
      </el-col>
    </el-row>

    <!-- 实时功率曲线 -->
    <el-card class="power-chart-card">
      <template #header>
        <div class="card-header">
          <span>实时功率曲线</span>
          <el-radio-group v-model="powerRange" size="small">
            <el-radio-button label="1h">1小时</el-radio-button>
            <el-radio-button label="6h">6小时</el-radio-button>
            <el-radio-button label="24h">24小时</el-radio-button>
          </el-radio-group>
        </div>
      </template>
      <PowerChart :range="powerRange" :height="300" />
    </el-card>

    <!-- 告警分布和设备状态 -->
    <el-row :gutter="20" class="charts-row">
      <el-col :xs="24" :lg="12">
        <el-card>
          <template #header>告警级别分布</template>
          <AlarmPieChart :height="250" />
        </el-card>
      </el-col>
      <el-col :xs="24" :lg="12">
        <el-card>
          <template #header>设备类型分布</template>
          <DeviceTypeChart :height="250" />
        </el-card>
      </el-col>
    </el-row>

    <!-- 快捷操作 - 根据权限显示 -->
    <el-card class="quick-actions" v-permission="['engineer', 'admin']">
      <template #header>快捷操作</template>
      <el-row :gutter="20">
        <el-col :span="6">
          <el-button type="primary" @click="goToControl">设备控制</el-button>
        </el-col>
        <el-col :span="6">
          <el-button @click="exportReport">导出报表</el-button>
        </el-col>
        <el-col :span="6" v-permission="['admin']">
          <el-button @click="systemCheck">系统巡检</el-button>
        </el-col>
        <el-col :span="6" v-permission="['admin']">
          <el-button @click="configAlarmRules">告警规则</el-button>
        </el-col>
      </el-row>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { getDashboardStats } from '@/api/dashboard'
import StatCard from '@/components/StatCard.vue'
import PowerChart from '@/components/charts/PowerChart.vue'
import AlarmPieChart from '@/components/charts/AlarmPieChart.vue'
import DeviceTypeChart from '@/components/charts/DeviceTypeChart.vue'

const router = useRouter()
const powerRange = ref('24h')
const stats = ref({
  totalDevices: 0,
  onlineRate: 0,
  todayEnergy: 0,
  activeAlarms: 0
})

let refreshTimer: number

const fetchStats = async () => {
  const data = await getDashboardStats()
  stats.value = data
}

const goToControl = () => {
  router.push('/control/devices')
}

const exportReport = () => {
  // 导出报表逻辑
}

const systemCheck = () => {
  // 系统巡检逻辑
}

const configAlarmRules = () => {
  router.push('/config/alarms')
}

onMounted(() => {
  fetchStats()
  // 每30秒刷新一次数据
  refreshTimer = setInterval(fetchStats, 30000)
})

onUnmounted(() => {
  clearInterval(refreshTimer)
})
</script>
```

### 2. 实时监控页面
```vue
<!-- views/monitoring/Realtime.vue -->
<template>
  <div class="realtime-container">
    <el-container>
      <!-- 左侧设备树 -->
      <el-aside width="250px">
        <el-card class="device-tree-card">
          <template #header>
            <span>设备列表</span>
          </template>
          <el-input 
            v-model="filterText" 
            placeholder="搜索设备"
            prefix-icon="Search"
            clearable
          />
          <el-tree
            ref="treeRef"
            :data="deviceTree"
            :props="treeProps"
            :filter-node-method="filterNode"
            @node-click="handleNodeClick"
            highlight-current
            default-expand-all
          />
        </el-card>
      </el-aside>

      <!-- 右侧数据展示 -->
      <el-main>
        <!-- 数据刷新控制 -->
        <el-card class="control-bar">
          <el-row justify="space-between" align="middle">
            <el-col :span="12">
              <span>当前设备：{{ currentDevice?.name || '请选择设备' }}</span>
            </el-col>
            <el-col :span="12" class="text-right">
              <el-select v-model="refreshInterval" size="small" style="width: 120px">
                <el-option label="1秒" :value="1000" />
                <el-option label="5秒" :value="5000" />
                <el-option label="10秒" :value="10000" />
                <el-option label="30秒" :value="30000" />
              </el-select>
              <el-button 
                size="small" 
                type="primary" 
                @click="toggleAutoRefresh"
                style="margin-left: 10px"
              >
                {{ autoRefresh ? '停止刷新' : '自动刷新' }}
              </el-button>
            </el-col>
          </el-row>
        </el-card>

        <!-- 实时趋势图 -->
        <el-card class="trend-chart-card">
          <template #header>
            <div class="card-header">
              <span>实时趋势</span>
              <el-button-group size="small">
                <el-button 
                  v-for="point in selectedPoints" 
                  :key="point.id"
                  @click="removePoint(point)"
                >
                  {{ point.name }} <el-icon><Close /></el-icon>
                </el-button>
                <el-button @click="showPointSelector">
                  <el-icon><Plus /></el-icon> 添加测点
                </el-button>
              </el-button-group>
            </div>
          </template>
          <RealtimeTrendChart 
            :points="selectedPoints" 
            :height="300"
            :refresh="autoRefresh"
          />
        </el-card>

        <!-- 数据表格 -->
        <el-card>
          <template #header>
            <div class="card-header">
              <span>实时数据</span>
              <div>
                <!-- 控制按钮 - 根据权限显示 -->
                <el-button 
                  v-permission="['engineer', 'admin']"
                  type="primary"
                  size="small"
                  @click="showControlPanel"
                >
                  远程控制
                </el-button>
                <el-button size="small" @click="exportData">导出数据</el-button>
              </div>
            </div>
          </template>
          <el-table :data="realtimeData" style="width: 100%">
            <el-table-column prop="name" label="测点名称" width="200" />
            <el-table-column prop="value" label="当前值" width="120">
              <template #default="{ row }">
                <span :class="getValueClass(row)">{{ row.value }}</span>
              </template>
            </el-table-column>
            <el-table-column prop="unit" label="单位" width="80" />
            <el-table-column prop="quality" label="质量" width="100">
              <template #default="{ row }">
                <el-tag :type="getQualityType(row.quality)" size="small">
                  {{ getQualityText(row.quality) }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="timestamp" label="更新时间" />
            <el-table-column 
              label="操作" 
              width="150"
              v-if="userStore.canControl"
            >
              <template #default="{ row }">
                <el-button 
                  v-if="row.controllable"
                  size="small"
                  @click="controlPoint(row)"
                >
                  控制
                </el-button>
              </template>
            </el-table-column>
          </el-table>
        </el-card>
      </el-main>
    </el-container>

    <!-- 控制对话框 -->
    <ControlDialog 
      v-model="controlDialogVisible"
      :point="controllingPoint"
      @success="handleControlSuccess"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from 'vue'
import { ElTree } from 'element-plus'
import { useUserStore } from '@/stores/user'
import { getDeviceTree, getRealtimeData } from '@/api/realtime'
import RealtimeTrendChart from '@/components/charts/RealtimeTrendChart.vue'
import ControlDialog from '@/components/ControlDialog.vue'

const userStore = useUserStore()
const treeRef = ref<InstanceType<typeof ElTree>>()
const filterText = ref('')
const deviceTree = ref([])
const currentDevice = ref(null)
const refreshInterval = ref(5000)
const autoRefresh = ref(false)
const selectedPoints = ref([])
const realtimeData = ref([])
const controlDialogVisible = ref(false)
const controllingPoint = ref(null)

let refreshTimer: number

// 监听过滤文本变化
watch(filterText, (val) => {
  treeRef.value?.filter(val)
})

// 实现具体方法...
</script>
```

### 3. 设备控制页面（工程师权限）
```vue
<!-- views/control/DeviceControl.vue -->
<template>
  <div class="device-control-container">
    <!-- 设备筛选 -->
    <el-card class="filter-card">
      <el-form :inline="true" :model="filterForm">
        <el-form-item label="设备类型">
          <el-select v-model="filterForm.deviceType" clearable>
            <el-option label="全部" value="" />
            <el-option label="PCS" value="pcs" />
            <el-option label="BMS" value="bms" />
            <el-option label="电表" value="meter" />
          </el-select>
        </el-form-item>
        <el-form-item label="在线状态">
          <el-select v-model="filterForm.onlineStatus" clearable>
            <el-option label="全部" value="" />
            <el-option label="在线" value="online" />
            <el-option label="离线" value="offline" />
          </el-select>
        </el-form-item>
        <el-form-item label="可控状态">
          <el-select v-model="filterForm.controllable" clearable>
            <el-option label="全部" value="" />
            <el-option label="可控" value="true" />
            <el-option label="不可控" value="false" />
          </el-select>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="fetchDevices">查询</el-button>
          <el-button @click="resetFilter">重置</el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <!-- 设备列表 -->
    <el-card>
      <template #header>
        <div class="card-header">
          <span>可控设备列表</span>
          <el-button 
            type="primary" 
            @click="batchControlVisible = true"
            :disabled="selectedDevices.length === 0"
          >
            批量控制 ({{ selectedDevices.length }})
          </el-button>
        </div>
      </template>

      <el-table 
        :data="devices" 
        @selection-change="handleSelectionChange"
        style="width: 100%"
      >
        <el-table-column type="selection" width="55" />
        <el-table-column prop="name" label="设备名称" />
        <el-table-column prop="type" label="设备类型">
          <template #default="{ row }">
            <el-tag>{{ getDeviceTypeText(row.type) }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="status" label="状态">
          <template #default="{ row }">
            <el-tag :type="row.online ? 'success' : 'danger'">
              {{ row.online ? '在线' : '离线' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="currentState" label="当前状态" />
        <el-table-column label="遥控点">
          <template #default="{ row }">
            <el-tag 
              v-for="point in row.controlPoints" 
              :key="point.id"
              size="small"
              style="margin-right: 5px"
            >
              {{ point.name }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="200">
          <template #default="{ row }">
            <el-button 
              size="small" 
              type="primary"
              :disabled="!row.online || !row.controllable"
              @click="showControlPanel(row)"
            >
              控制
            </el-button>
            <el-button 
              size="small"
              @click="showDeviceDetail(row)"
            >
              详情
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 设备控制面板 -->
    <el-drawer
      v-model="controlPanelVisible"
      :title="`设备控制 - ${currentDevice?.name}`"
      :size="600"
    >
      <div v-if="currentDevice">
        <!-- 设备信息 -->
        <el-descriptions :column="2" border style="margin-bottom: 20px">
          <el-descriptions-item label="设备名称">
            {{ currentDevice.name }}
          </el-descriptions-item>
          <el-descriptions-item label="设备类型">
            {{ getDeviceTypeText(currentDevice.type) }}
          </el-descriptions-item>
          <el-descriptions-item label="当前状态">
            {{ currentDevice.currentState }}
          </el-descriptions-item>
          <el-descriptions-item label="通信地址">
            {{ currentDevice.address }}
          </el-descriptions-item>
        </el-descriptions>

        <!-- 遥控操作 -->
        <el-divider>遥控操作 (YK)</el-divider>
        <el-form label-width="120px">
          <el-form-item 
            v-for="point in currentDevice.controlPoints" 
            :key="point.id"
            :label="point.name"
          >
            <el-switch
              v-if="point.type === 'switch'"
              v-model="point.value"
              active-text="开"
              inactive-text="关"
              @change="handleControl(point, $event)"
            />
            <el-select
              v-else-if="point.type === 'enum'"
              v-model="point.value"
              @change="handleControl(point, $event)"
            >
              <el-option 
                v-for="option in point.options" 
                :key="option.value"
                :label="option.label"
                :value="option.value"
              />
            </el-select>
          </el-form-item>
        </el-form>

        <!-- 遥调操作 -->
        <el-divider>遥调操作 (YT)</el-divider>
        <el-form label-width="120px">
          <el-form-item 
            v-for="point in currentDevice.adjustPoints" 
            :key="point.id"
            :label="point.name"
          >
            <el-input-number
              v-model="point.value"
              :min="point.min"
              :max="point.max"
              :step="point.step"
              @change="handleAdjust(point, $event)"
            />
            <span style="margin-left: 10px">{{ point.unit }}</span>
          </el-form-item>
        </el-form>

        <!-- 操作日志 -->
        <el-divider>最近操作记录</el-divider>
        <el-timeline>
          <el-timeline-item
            v-for="log in operationLogs"
            :key="log.id"
            :timestamp="log.timestamp"
          >
            {{ log.operation }} - {{ log.result }}
          </el-timeline-item>
        </el-timeline>
      </div>
    </el-drawer>

    <!-- 批量控制对话框 -->
    <BatchControlDialog
      v-model="batchControlVisible"
      :devices="selectedDevices"
      @success="handleBatchControlSuccess"
    />
  </div>
</template>
```

### 4. 通道配置页面（管理员权限）
```vue
<!-- views/config/ChannelConfig.vue -->
<template>
  <div class="channel-config-container">
    <!-- 通道列表 -->
    <el-card>
      <template #header>
        <div class="card-header">
          <span>通信通道配置</span>
          <el-button type="primary" @click="showAddChannel">
            <el-icon><Plus /></el-icon> 新增通道
          </el-button>
        </div>
      </template>

      <el-table :data="channels" style="width: 100%">
        <el-table-column prop="name" label="通道名称" />
        <el-table-column prop="protocol" label="协议类型">
          <template #default="{ row }">
            <el-tag>{{ row.protocol.toUpperCase() }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="transport" label="传输方式" />
        <el-table-column prop="address" label="通信地址" />
        <el-table-column prop="status" label="状态">
          <template #default="{ row }">
            <el-tag :type="row.enabled ? 'success' : 'info'">
              {{ row.enabled ? '启用' : '禁用' }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="deviceCount" label="设备数" />
        <el-table-column label="操作" width="250">
          <template #default="{ row }">
            <el-button size="small" @click="editChannel(row)">编辑</el-button>
            <el-button size="small" @click="testChannel(row)">测试</el-button>
            <el-switch
              v-model="row.enabled"
              size="small"
              @change="toggleChannel(row)"
              style="margin: 0 10px"
            />
            <el-button 
              size="small" 
              type="danger"
              @click="deleteChannel(row)"
            >
              删除
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 通道配置对话框 -->
    <el-dialog
      v-model="channelDialogVisible"
      :title="isEdit ? '编辑通道' : '新增通道'"
      width="600px"
    >
      <el-form 
        ref="channelFormRef"
        :model="channelForm"
        :rules="channelRules"
        label-width="120px"
      >
        <el-form-item label="通道名称" prop="name">
          <el-input v-model="channelForm.name" />
        </el-form-item>

        <el-form-item label="协议类型" prop="protocol">
          <el-select v-model="channelForm.protocol" @change="onProtocolChange">
            <el-option label="Modbus TCP" value="modbus_tcp" />
            <el-option label="Modbus RTU" value="modbus_rtu" />
            <el-option label="CAN" value="can" />
            <el-option label="IEC60870-5-104" value="iec104" />
          </el-select>
        </el-form-item>

        <!-- Modbus TCP 配置 -->
        <template v-if="channelForm.protocol === 'modbus_tcp'">
          <el-form-item label="IP地址" prop="params.host">
            <el-input v-model="channelForm.params.host" />
          </el-form-item>
          <el-form-item label="端口" prop="params.port">
            <el-input-number v-model="channelForm.params.port" :min="1" :max="65535" />
          </el-form-item>
        </template>

        <!-- Modbus RTU 配置 -->
        <template v-if="channelForm.protocol === 'modbus_rtu'">
          <el-form-item label="串口" prop="params.port">
            <el-select v-model="channelForm.params.port">
              <el-option label="/dev/ttyS0" value="/dev/ttyS0" />
              <el-option label="/dev/ttyS1" value="/dev/ttyS1" />
              <el-option label="/dev/ttyUSB0" value="/dev/ttyUSB0" />
            </el-select>
          </el-form-item>
          <el-form-item label="波特率" prop="params.baudrate">
            <el-select v-model="channelForm.params.baudrate">
              <el-option :label="9600" :value="9600" />
              <el-option :label="19200" :value="19200" />
              <el-option :label="38400" :value="38400" />
              <el-option :label="115200" :value="115200" />
            </el-select>
          </el-form-item>
        </template>

        <!-- 通用配置 -->
        <el-form-item label="轮询间隔(ms)" prop="params.poll_interval">
          <el-input-number 
            v-model="channelForm.params.poll_interval" 
            :min="100" 
            :max="60000"
            :step="100"
          />
        </el-form-item>

        <el-form-item label="超时时间(ms)" prop="params.timeout">
          <el-input-number 
            v-model="channelForm.params.timeout" 
            :min="100" 
            :max="10000"
            :step="100"
          />
        </el-form-item>

        <el-form-item label="启用状态" prop="enabled">
          <el-switch v-model="channelForm.enabled" />
        </el-form-item>
      </el-form>

      <template #footer>
        <el-button @click="channelDialogVisible = false">取消</el-button>
        <el-button type="primary" @click="saveChannel">保存</el-button>
      </template>
    </el-dialog>

    <!-- 连接测试对话框 -->
    <ConnectionTestDialog
      v-model="testDialogVisible"
      :channel="testingChannel"
    />
  </div>
</template>
```

## 四、实时数据通信

### 1. WebSocket管理
```typescript
// utils/websocket.ts
import { useUserStore } from '@/stores/user'

export class RealtimeWebSocket {
  private ws: WebSocket | null = null
  private reconnectTimer: number | null = null
  private subscriptions: Map<string, Function[]> = new Map()
  
  connect() {
    const userStore = useUserStore()
    const wsUrl = `${import.meta.env.VITE_WS_URL}/realtime?token=${userStore.token}`
    
    this.ws = new WebSocket(wsUrl)
    
    this.ws.onopen = () => {
      console.log('WebSocket connected')
      this.resubscribeAll()
    }
    
    this.ws.onmessage = (event) => {
      const data = JSON.parse(event.data)
      this.handleMessage(data)
    }
    
    this.ws.onerror = (error) => {
      console.error('WebSocket error:', error)
    }
    
    this.ws.onclose = () => {
      console.log('WebSocket disconnected')
      this.scheduleReconnect()
    }
  }
  
  subscribe(topic: string, callback: Function) {
    if (!this.subscriptions.has(topic)) {
      this.subscriptions.set(topic, [])
    }
    this.subscriptions.get(topic)!.push(callback)
    
    // 发送订阅消息
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({
        type: 'subscribe',
        topic
      }))
    }
  }
  
  unsubscribe(topic: string, callback: Function) {
    const callbacks = this.subscriptions.get(topic)
    if (callbacks) {
      const index = callbacks.indexOf(callback)
      if (index > -1) {
        callbacks.splice(index, 1)
      }
      
      if (callbacks.length === 0) {
        this.subscriptions.delete(topic)
        
        // 发送取消订阅消息
        if (this.ws?.readyState === WebSocket.OPEN) {
          this.ws.send(JSON.stringify({
            type: 'unsubscribe',
            topic
          }))
        }
      }
    }
  }
  
  private handleMessage(data: any) {
    const { topic, payload } = data
    const callbacks = this.subscriptions.get(topic)
    
    if (callbacks) {
      callbacks.forEach(callback => {
        try {
          callback(payload)
        } catch (error) {
          console.error('Callback error:', error)
        }
      })
    }
  }
  
  private scheduleReconnect() {
    if (this.reconnectTimer) return
    
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null
      this.connect()
    }, 5000)
  }
  
  private resubscribeAll() {
    this.subscriptions.forEach((_, topic) => {
      if (this.ws?.readyState === WebSocket.OPEN) {
        this.ws.send(JSON.stringify({
          type: 'subscribe',
          topic
        }))
      }
    })
  }
  
  disconnect() {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer)
      this.reconnectTimer = null
    }
    
    if (this.ws) {
      this.ws.close()
      this.ws = null
    }
    
    this.subscriptions.clear()
  }
}

export const realtimeWS = new RealtimeWebSocket()
```

### 2. 实时数据Store
```typescript
// stores/realtime.ts
import { defineStore } from 'pinia'
import { realtimeWS } from '@/utils/websocket'

interface RealtimePoint {
  id: string
  name: string
  value: number
  unit: string
  quality: string
  timestamp: string
}

export const useRealtimeStore = defineStore('realtime', {
  state: () => ({
    points: new Map<string, RealtimePoint>(),
    subscribedTopics: new Set<string>()
  }),
  
  actions: {
    subscribeDevice(deviceId: string) {
      const topic = `device/${deviceId}`
      
      if (this.subscribedTopics.has(topic)) return
      
      realtimeWS.subscribe(topic, (data: any) => {
        this.updatePoints(data.points)
      })
      
      this.subscribedTopics.add(topic)
    },
    
    unsubscribeDevice(deviceId: string) {
      const topic = `device/${deviceId}`
      
      if (!this.subscribedTopics.has(topic)) return
      
      realtimeWS.unsubscribe(topic, () => {})
      this.subscribedTopics.delete(topic)
      
      // 清理该设备的数据点
      for (const [key, point] of this.points) {
        if (key.startsWith(`${deviceId}/`)) {
          this.points.delete(key)
        }
      }
    },
    
    updatePoints(points: RealtimePoint[]) {
      points.forEach(point => {
        this.points.set(point.id, {
          ...point,
          timestamp: new Date().toLocaleString()
        })
      })
    },
    
    getPoint(pointId: string): RealtimePoint | undefined {
      return this.points.get(pointId)
    },
    
    getDevicePoints(deviceId: string): RealtimePoint[] {
      const result: RealtimePoint[] = []
      
      for (const [key, point] of this.points) {
        if (key.startsWith(`${deviceId}/`)) {
          result.push(point)
        }
      }
      
      return result
    },
    
    clearAll() {
      this.subscribedTopics.forEach(topic => {
        realtimeWS.unsubscribe(topic, () => {})
      })
      
      this.points.clear()
      this.subscribedTopics.clear()
    }
  }
})
```

## 五、组件开发规范

### 1. 组件目录结构
```
components/
├── common/           # 通用组件
│   ├── StatCard.vue
│   ├── PermissionButton.vue
│   └── DataTable.vue
├── charts/          # 图表组件
│   ├── PowerChart.vue
│   ├── AlarmPieChart.vue
│   └── RealtimeTrendChart.vue
├── dialogs/         # 对话框组件
│   ├── ControlDialog.vue
│   ├── BatchControlDialog.vue
│   └── ConnectionTestDialog.vue
└── layouts/         # 布局组件
    ├── MainLayout.vue
    ├── SideMenu.vue
    └── HeaderBar.vue
```

### 2. 组件开发示例
```vue
<!-- components/common/PermissionButton.vue -->
<template>
  <el-button
    v-if="hasPermission"
    v-bind="$attrs"
  >
    <slot />
  </el-button>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useUserStore } from '@/stores/user'

interface Props {
  roles?: string[]
  permissions?: string[]
}

const props = defineProps<Props>()
const userStore = useUserStore()

const hasPermission = computed(() => {
  if (!props.roles && !props.permissions) return true
  
  if (props.roles) {
    return props.roles.includes(userStore.role!)
  }
  
  if (props.permissions) {
    return props.permissions.some(p => userStore.permissions.includes(p))
  }
  
  return false
})
</script>
```

## 六、部署配置

### 1. 环境变量配置
```env
# .env.production
VITE_API_BASE_URL=http://localhost:8080/api
VITE_WS_URL=ws://localhost:8080/ws
VITE_APP_TITLE=VoltageEMS 边缘管理系统
```

### 2. Nginx配置
```nginx
server {
    listen 80;
    server_name localhost;
    root /usr/share/nginx/html;
    
    # 前端路由
    location / {
        try_files $uri $uri/ /index.html;
    }
    
    # API代理
    location /api {
        proxy_pass http://backend:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
    
    # WebSocket代理
    location /ws {
        proxy_pass http://backend:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

## 七、测试策略

### 1. 单元测试
```typescript
// tests/unit/stores/user.test.ts
import { setActivePinia, createPinia } from 'pinia'
import { useUserStore } from '@/stores/user'

describe('User Store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })
  
  it('should login successfully', async () => {
    const userStore = useUserStore()
    
    await userStore.login({
      username: 'admin',
      password: 'admin123'
    })
    
    expect(userStore.token).toBeTruthy()
    expect(userStore.role).toBe('admin')
  })
  
  it('should check permissions correctly', () => {
    const userStore = useUserStore()
    userStore.role = 'engineer'
    
    expect(userStore.canControl).toBe(true)
    expect(userStore.canConfig).toBe(false)
  })
})
```

### 2. E2E测试
```typescript
// tests/e2e/login.test.ts
import { test, expect } from '@playwright/test'

test('operator login flow', async ({ page }) => {
  await page.goto('/')
  
  // 应该重定向到登录页
  await expect(page).toHaveURL('/login')
  
  // 填写登录表单
  await page.fill('input[name="username"]', 'operator')
  await page.fill('input[name="password"]', 'operator123')
  await page.click('button[type="submit"]')
  
  // 应该跳转到首页
  await expect(page).toHaveURL('/dashboard')
  
  // 验证菜单权限
  await expect(page.locator('text=设备控制')).not.toBeVisible()
  await expect(page.locator('text=通道配置')).not.toBeVisible()
})
```

## 八、性能优化

### 1. 路由懒加载
```typescript
const routes = [
  {
    path: '/dashboard',
    component: () => import(
      /* webpackChunkName: "dashboard" */ 
      '@/views/monitoring/Dashboard.vue'
    )
  }
]
```

### 2. 组件缓存
```vue
<template>
  <router-view v-slot="{ Component }">
    <keep-alive :include="cachedViews">
      <component :is="Component" />
    </keep-alive>
  </router-view>
</template>
```

### 3. 虚拟滚动
```vue
<template>
  <el-table-v2
    :columns="columns"
    :data="largeDataset"
    :height="600"
    :row-height="50"
    fixed
  />
</template>
```

## 九、安全加固

### 1. XSS防护
- 使用Vue的模板语法自动转义
- 避免使用v-html
- 对用户输入进行验证

### 2. CSRF防护
- 所有API请求携带CSRF Token
- 验证Referer头

### 3. 权限验证
- 前端权限检查仅用于UI展示
- 所有权限验证必须在后端进行
- 敏感操作需要二次确认

## 十、开发进度安排

### 第一周
- [ ] 项目初始化和基础架构
- [ ] 权限系统实现
- [ ] 登录页面和主布局
- [ ] 系统总览页面
- [ ] 用户管理页面

### 第二周
- [ ] 实时监控页面
- [ ] 设备状态页面
- [ ] 设备控制页面
- [ ] 告警总览页面
- [ ] WebSocket实时通信

### 第三周
- [ ] 通道配置页面
- [ ] 点表管理页面
- [ ] 告警处理页面
- [ ] 能耗统计页面
- [ ] 系统设置页面

### 第四周
- [ ] 其余配置页面
- [ ] 批量操作功能
- [ ] 日志审计页面
- [ ] 服务监控页面
- [ ] 测试和优化