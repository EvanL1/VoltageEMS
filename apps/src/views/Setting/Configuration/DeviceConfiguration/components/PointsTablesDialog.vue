<template>
  <FormDialog
    ref="dialogRef"
    :title="viewMode === 'points' ? 'Points Table' : 'Routings Table'"
    width="12rem"
    @close="handleClose"
  >
    <template #dialog-body>
      <div class="voltage-class dc-points-dialog">
        <div class="rule-management__config-section">
          <div class="config-section__header">
            <el-tabs v-model="activeTab" type="card">
              <el-tab-pane v-if="viewMode === 'points'" label="property" name="property">
                <DevicePointTablePoints
                  ref="propertyPointsRef"
                  category="property"
                  :points="propertyRows"
                  :original-points="originalPointsData.property"
                  :view-mode="viewMode"
                  :edit-filters="editFilters"
                  :is-editing="isEditing"
                  :publish-mode="false"
                />
              </el-tab-pane>
              <el-tab-pane label="measurement" name="measurement">
                <template v-if="viewMode === 'points'">
                  <DevicePointTablePoints
                    ref="measurementPointsRef"
                    category="measurement"
                    :points="measurementRows"
                    :original-points="originalPointsData.measurement"
                    :view-mode="viewMode"
                    :edit-filters="editFilters"
                    :is-editing="isEditing"
                    :publish-mode="isPublish && activeTab === 'measurement'"
                    @publish-change="
                      (dirty: boolean) => {
                        publishDirty = dirty
                      }
                    "
                    @toggle-publish="togglePublishMode"
                  />
                </template>
                <template v-else>
                  <DevicePointTableRouting
                    ref="measurementRoutingRef"
                    category="measurement"
                    :points="measurementRows"
                    :original-points="originalPointsData.measurement"
                    :view-mode="viewMode"
                    :edit-filters="editFilters"
                    :is-editing="isEditing"
                    :channels="channelsForRouting"
                  />
                </template>
              </el-tab-pane>
              <el-tab-pane label="action" name="action">
                <template v-if="viewMode === 'points'">
                  <DevicePointTablePoints
                    ref="actionPointsRef"
                    category="action"
                    :points="actionRows"
                    :original-points="originalPointsData.action"
                    :view-mode="viewMode"
                    :edit-filters="editFilters"
                    :is-editing="isEditing"
                    :publish-mode="isPublish && activeTab === 'action'"
                    @publish-change="
                      (dirty: boolean) => {
                        publishDirty = dirty
                      }
                    "
                    @toggle-publish="togglePublishMode"
                  />
                </template>
                <template v-else>
                  <DevicePointTableRouting
                    ref="actionRoutingRef"
                    category="action"
                    :points="actionRows"
                    :original-points="originalPointsData.action"
                    :view-mode="viewMode"
                    :edit-filters="editFilters"
                    :is-editing="isEditing"
                    :channels="channelsForRouting"
                  />
                </template>
              </el-tab-pane>
            </el-tabs>

            <div class="config-section__controls">
              <div v-if="!isEditing" class="view-mode-switch">
                <span class="switch-label">Points</span>
                <el-switch v-model="viewModeSwitch" />
                <span class="switch-label">Routing</span>
              </div>
              <div v-if="isEditing" class="edit-filters">
                <el-checkbox-group v-model="editFilters">
                  <el-checkbox label="modified">modified</el-checkbox>
                  <el-checkbox label="added">added</el-checkbox>
                  <el-checkbox label="deleted">deleted</el-checkbox>
                  <el-checkbox label="invalid">invalid</el-checkbox>
                </el-checkbox-group>
              </div>
            </div>
          </div>
        </div>
      </div>
    </template>

    <template #dialog-footer>
      <div class="dc-points-dialog__toolbar">
        <el-button v-if="!isPublish" type="warning" @click="handleCancel">{{
          isEditing ? 'Cancel Edit' : 'Cancel'
        }}</el-button>
        <el-button
          v-if="viewMode === 'routing' && !isEditing && !isPublish"
          type="primary"
          @click="handleEdit"
          >Edit</el-button
        >

        <el-button v-if="isEditing" type="primary" @click="handleSubmit">Submit</el-button>

        <el-button
          v-if="!isEditing && isPublish && (activeTab === 'action' || activeTab === 'measurement')"
          type="primary"
          :disabled="!publishDirty"
          @click="handleSubmitPublish"
        >
          Submit Execute
        </el-button>
      </div>
    </template>
  </FormDialog>
</template>

<script setup lang="ts">
import { ref, computed, readonly, provide, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import FormDialog from '@/components/dialog/FormDialog.vue'
import { getInstancePoints, executeAction, updateInstanceRouting } from '@/api/devicesManagement'
import type {
  InstancePointList,
  InstanceActionItem,
  InstanceMeasurementItem,
  InstancePropertyItem,
} from '@/types/deviceConfiguration'
import DevicePointTablePoints from './DevicePointTablePoints.vue'
import DevicePointTableRouting from './DevicePointTableRouting.vue'
import { InstanceNameKey, InstanceIdKey } from '@/utils/key'
import wsManager from '@/utils/websocket'
import { Request } from '@/utils/request'
import { getAllChannels } from '@/api/channelsManagement'
import type { ChannelListItem } from '@/types/channelConfiguration'
const dialogRef = ref()
const activeTab = ref<'measurement' | 'action' | 'property'>('property')
const isEditing = ref(false)
const isPublish = ref(false)
const instanceName = ref('')
const instanceId = ref<number>(0)
const measurementRows = ref<InstanceMeasurementItem[]>([])
const actionRows = ref<InstanceActionItem[]>([])
const propertyRows = ref<InstancePropertyItem[]>([])
// 页面订阅ID
const pageId = ref<string>('')
provide(InstanceNameKey, readonly(instanceName))
provide(InstanceIdKey, readonly(instanceId))
// 视图模式与筛选
const viewModeSwitch = ref(false) // false=points, true=routing
const viewMode = computed(() => (viewModeSwitch.value ? 'routing' : 'points'))
const editFilters = ref<string[]>([])
const publishDirty = ref(false)
// routing 所需通道列表
const channelsForRouting = ref<Array<{ id: number; name: string }>>([])
// 原始基线
const originalPointsData = ref<{
  measurement: InstanceMeasurementItem[]
  action: InstanceActionItem[]
  property: InstancePropertyItem[]
}>({
  measurement: [],
  action: [],
  property: [],
})
// 子表 refs
const measurementPointsRef = ref<any>()
const actionPointsRef = ref<any>()
const propertyPointsRef = ref<any>()
const measurementRoutingRef = ref<any>()
const actionRoutingRef = ref<any>()
const propertyRoutingRef = ref<any>()
async function open(id: number, name: string) {
  instanceId.value = id
  instanceName.value = name
  const res = await getInstancePoints(id)
  if (res.success) {
    const data = res.data as InstancePointList
    if (data.measurements) measurementRows.value = Object.values(data.measurements)
    if (data.actions) actionRows.value = Object.values(data.actions)
    if (data.properties) propertyRows.value = Object.values(data.properties)
    originalPointsData.value = {
      measurement: JSON.parse(JSON.stringify(measurementRows.value)),
      action: JSON.parse(JSON.stringify(actionRows.value)),
      property: JSON.parse(JSON.stringify(propertyRows.value)),
    }
  }

  isEditing.value = false
  isPublish.value = false
  publishDirty.value = false
  editFilters.value = []
  // 默认 Points 视图和 property Tab
  viewModeSwitch.value = false
  activeTab.value = 'property'
  dialogRef.value.dialogVisible = true
  // 先取消上一个订阅
  if (pageId.value) {
    try {
      wsManager.unsubscribePage(pageId.value)
    } catch {}
    pageId.value = ''
  }
  pageId.value = `inst-${id}-${Date.now()}`
  wsManager.subscribePage(
    pageId.value,
    {
      source: 'inst',
      channels: [instanceId.value] as any,
      dataTypes: ['A', 'M'] as any,
      interval: 1000,
    } as any,
    {
      onBatchDataUpdate: (payload: any) => {
        if (!payload?.updates?.length) return
        payload.updates.forEach((upd: any) => {
          if (Number(upd.channel_id) !== Number(instanceId.value)) return
          const dt = String(upd.data_type || '').toUpperCase()
          const values = upd.values || {}
          const map: Record<string, number> = {}
          Object.keys(values).forEach((k) => (map[k] = Number(values[k])))
          if (dt === 'M') {
            measurementPointsRef.value?.applyRealtimeValues?.(map)
          } else if (dt === 'A') {
            actionPointsRef.value?.applyRealtimeValues?.(map)
          }
        })
      },
    },
  )
}

function close() {
  dialogRef.value.dialogVisible = false
}

function handleClose() {
  isEditing.value = false
  isPublish.value = false
  publishDirty.value = false
  if (pageId.value) {
    try {
      wsManager.unsubscribePage(pageId.value)
    } catch {}
    pageId.value = ''
  }
}

function handleCancel() {
  if (isEditing.value) {
    isEditing.value = false
    editFilters.value = []
  } else {
    close()
  }
}

const handleEdit = () => {
  isEditing.value = true
  editFilters.value = []
}
const handleSubmit = async () => {
  if (viewMode.value !== 'routing') {
    isEditing.value = false
    return
  }
  const invalidTabs: Array<'measurement' | 'action'> = []
  if (measurementRoutingRef.value?.hasInvalid?.()) invalidTabs.push('measurement')
  if (actionRoutingRef.value?.hasInvalid?.()) invalidTabs.push('action')
  if (invalidTabs.length > 0) {
    ElMessage.warning('Routing has invalid data, please correct and submit again')
    activeTab.value = invalidTabs[0]
    return
  }
  const mappings = [
    ...(measurementRoutingRef.value?.getEditedData?.() || []),
    ...(actionRoutingRef.value?.getEditedData?.() || []),
  ]
  if (!mappings.length) {
    ElMessage.info('No routing changes to submit')
    return
  }
  const routingPayload = mappings.map((item: any) => ({
    channel_id: Number(item.routing.channel_id),
    channel_point_id: Number(item.routing.channel_point_id),
    four_remote: String(item.routing.channel_type || '').toUpperCase(),
    point_id: Number(item.point_id),
  }))
  const res = await updateInstanceRouting(instanceId.value, routingPayload)
  if (res.success) {
    ElMessage.success('Routing updated successfully')
    isEditing.value = false
    // 刷新基线
    const resp = await getInstancePoints(instanceId.value)
    if (resp.success) {
      const data = resp.data as InstancePointList
      measurementRows.value = data.measurements ? Object.values(data.measurements) : []
      actionRows.value = data.actions ? Object.values(data.actions) : []
      propertyRows.value = data.properties ? Object.values(data.properties) : []
      originalPointsData.value = {
        measurement: JSON.parse(JSON.stringify(measurementRows.value)),
        action: JSON.parse(JSON.stringify(actionRows.value)),
        property: JSON.parse(JSON.stringify(propertyRows.value)),
      }
    }
  }
}
const togglePublishMode = async () => {
  if (isPublish.value) {
    if (publishDirty.value) {
      try {
        await ElMessageBox.confirm(
          'You have unsaved publish values. Do you want to discard them?',
          'Unsaved Changes',
          { confirmButtonText: 'Discard', cancelButtonText: 'Cancel', type: 'warning' },
        )
        isPublish.value = false
        publishDirty.value = false
        if (activeTab.value === 'action') {
          actionPointsRef.value?.resetPublish?.()
        } else if (activeTab.value === 'measurement') {
          measurementPointsRef.value?.resetPublish?.()
        }
      } catch {
        return
      }
    } else {
      isPublish.value = false
      publishDirty.value = false
      if (activeTab.value === 'action') {
        actionPointsRef.value?.resetPublish?.()
      } else if (activeTab.value === 'measurement') {
        measurementPointsRef.value?.resetPublish?.()
      }
    }
  } else {
    if (activeTab.value !== 'action' && activeTab.value !== 'measurement')
      activeTab.value = 'action'
    isPublish.value = true
    publishDirty.value = false
    if (activeTab.value === 'action') {
      actionPointsRef.value?.resetPublish?.()
    } else if (activeTab.value === 'measurement') {
      measurementPointsRef.value?.resetPublish?.()
    }
  }
}
const handleSubmitPublish = async () => {
  const sourceRef =
    activeTab.value === 'action'
      ? actionPointsRef
      : activeTab.value === 'measurement'
        ? measurementPointsRef
        : null
  const cmds = sourceRef?.value?.getPublishCommands?.() || []
  if (!Array.isArray(cmds) || cmds.length === 0) return
  for (const { id, value } of cmds) {
    await executeAction(instanceId.value, { point_id: String(id), value })
  }
  publishDirty.value = false
  isPublish.value = false
  if (activeTab.value === 'action') {
    actionPointsRef.value?.resetPublish?.()
  } else if (activeTab.value === 'measurement') {
    measurementPointsRef.value?.resetPublish?.()
  }
}

defineExpose({ open, close })

// 当在 Points 模式下位于 property 标签，切换到 Routing 模式时默认跳到 measurement
watch(
  () => viewMode.value,
  (mode) => {
    if (mode === 'routing' && activeTab.value === 'property') {
      activeTab.value = 'measurement'
      loadChannelsForRouting()
    } else if (mode === 'routing') {
      loadChannelsForRouting()
    }
  },
)

async function loadChannelsForRouting() {
  try {
    const res = await getAllChannels()
    const list = Array.isArray(res?.data?.list)
      ? res.data.list
      : Array.isArray(res)
        ? (res as any)
        : []
    channelsForRouting.value = (list as any[])
      .map((it: any) => ({
        id: Number(it.id),
        name: String(it.name || ''),
      }))
      .filter((x) => Number.isFinite(x.id) && x.id > 0 && x.name)
  } catch {}
}
</script>

<style scoped lang="scss">
.voltage-class {
  .dc-points-dialog {
    .dc-points-dialog__toolbar {
      display: flex;
      gap: 0.08rem;
    }
  }
  :deep(.el-tabs__content) {
    position: static;
  }
  .rule-management__config-section {
    .config-section__header {
      .config-section__controls {
        position: absolute;
        top: 1rem;
        transform: translateY(-50%);
        right: 0.41rem;
        display: flex;
        align-items: center;
        gap: 0.15rem;
        .view-mode-switch {
          display: flex;
          align-items: center;
          gap: 0.08rem;
          .switch-label {
            font-size: 0.12rem;
            color: #fff;
          }
        }
        .edit-filters {
          :deep(.el-checkbox-group) {
            display: flex;
            gap: 0.08rem;
          }
          :deep(.el-checkbox) {
            color: #fff;
            .el-checkbox__label {
              font-size: 0.12rem;
            }
          }
        }
      }
    }
  }
}
</style>
