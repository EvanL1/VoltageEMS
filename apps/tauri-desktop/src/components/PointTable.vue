<template>
  <div class="point-table">
    <el-table :data="points" style="width: 100%" max-height="400" stripe>
      <el-table-column prop="point_id" label="Point ID" width="100" />

      <el-table-column prop="value" label="Value" width="150">
        <template #default="{ row }">
          <span :class="getValueClass(row)">
            {{ formatValue(row.value, row.point_type) }}
          </span>
        </template>
      </el-table-column>

      <el-table-column prop="timestamp" label="Timestamp" width="180">
        <template #default="{ row }">
          {{ formatTime(row.timestamp) }}
        </template>
      </el-table-column>

      <el-table-column prop="description" label="Description" />

      <el-table-column label="Actions" width="120" v-if="showActions">
        <template #default="{ row }">
          <el-button
            v-if="pointType === 'YK'"
            type="primary"
            size="small"
            @click="sendControl(row)"
          >
            Control
          </el-button>

          <el-button
            v-if="pointType === 'YT'"
            type="primary"
            size="small"
            @click="adjustValue(row)"
          >
            Adjust
          </el-button>
        </template>
      </el-table-column>
    </el-table>

    <!-- Control Dialog -->
    <el-dialog
      v-model="showControlDialog"
      title="Send Control Command"
      width="400px"
    >
      <el-form :model="controlForm" label-width="100px">
        <el-form-item label="Point ID">
          <el-input :value="controlForm.pointId" disabled />
        </el-form-item>

        <el-form-item label="Command">
          <el-radio-group v-model="controlForm.value">
            <el-radio :label="1">ON</el-radio>
            <el-radio :label="0">OFF</el-radio>
          </el-radio-group>
        </el-form-item>
      </el-form>

      <template #footer>
        <el-button @click="showControlDialog = false">Cancel</el-button>
        <el-button type="primary" @click="confirmControl">Send</el-button>
      </template>
    </el-dialog>

    <!-- Adjustment Dialog -->
    <el-dialog v-model="showAdjustDialog" title="Adjust Value" width="400px">
      <el-form :model="adjustForm" label-width="100px">
        <el-form-item label="Point ID">
          <el-input :value="adjustForm.pointId" disabled />
        </el-form-item>

        <el-form-item label="New Value">
          <el-input-number
            v-model="adjustForm.value"
            :precision="2"
            :step="0.1"
          />
        </el-form-item>
      </el-form>

      <template #footer>
        <el-button @click="showAdjustDialog = false">Cancel</el-button>
        <el-button type="primary" @click="confirmAdjust">Send</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import { ElMessage } from "element-plus";
import type { PointData } from "@/types/realtime";
import dayjs from "dayjs";

const props = defineProps<{
  points: PointData[];
  pointType: "YC" | "YX" | "YK" | "YT";
}>();

// Local state
const showControlDialog = ref(false);
const showAdjustDialog = ref(false);

const controlForm = ref({
  pointId: 0,
  value: 0,
});

const adjustForm = ref({
  pointId: 0,
  value: 0,
});

// Computed
const showActions = computed(
  () => props.pointType === "YK" || props.pointType === "YT",
);

// Methods
function formatValue(value: any, type: string): string {
  if (value === null || value === undefined) return "-";

  switch (type) {
    case "YX":
    case "YK":
      return value === 1 || value === "1" || value === true ? "ON" : "OFF";
    case "YC":
    case "YT":
      return typeof value === "number" ? value.toFixed(2) : String(value);
    default:
      return String(value);
  }
}

function getValueClass(row: PointData): string {
  if (row.point_type === "YX" || row.point_type === "YK") {
    const isOn = row.value === 1 || row.value === "1" || row.value === true;
    return isOn ? "value-on" : "value-off";
  }
  return "";
}

function formatTime(time: Date): string {
  return dayjs(time).format("HH:mm:ss");
}

function sendControl(row: PointData) {
  controlForm.value = {
    pointId: row.point_id,
    value: 0,
  };
  showControlDialog.value = true;
}

function confirmControl() {
  // TODO: Send control command via API
  ElMessage.success(
    `Control command sent to point ${controlForm.value.pointId}`,
  );
  showControlDialog.value = false;
}

function adjustValue(row: PointData) {
  adjustForm.value = {
    pointId: row.point_id,
    value: typeof row.value === "number" ? row.value : 0,
  };
  showAdjustDialog.value = true;
}

function confirmAdjust() {
  // TODO: Send adjustment command via API
  ElMessage.success(`Adjustment sent to point ${adjustForm.value.pointId}`);
  showAdjustDialog.value = false;
}
</script>

<style scoped lang="scss">
.point-table {
  .value-on {
    color: #67c23a;
    font-weight: bold;
  }

  .value-off {
    color: #909399;
  }
}
</style>
