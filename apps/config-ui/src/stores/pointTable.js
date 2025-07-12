import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { ElMessage } from 'element-plus'

export const usePointTableStore = defineStore('pointTable', () => {
  const tables = ref([])
  const currentTable = ref(null)
  const loading = ref(false)
  const error = ref(null)

  const tableCount = computed(() => tables.value.length)

  async function fetchTables() {
    loading.value = true
    error.value = null
    try {
      tables.value = await invoke('get_point_tables')
    } catch (e) {
      error.value = e.toString()
      ElMessage.error(`获取点表列表失败: ${e}`)
    } finally {
      loading.value = false
    }
  }

  async function fetchTable(id) {
    loading.value = true
    error.value = null
    try {
      currentTable.value = await invoke('get_point_table', { id })
    } catch (e) {
      error.value = e.toString()
      ElMessage.error(`获取点表失败: ${e}`)
    } finally {
      loading.value = false
    }
  }

  async function createTable(name, protocolType) {
    loading.value = true
    error.value = null
    try {
      const newTable = await invoke('create_point_table', { name, protocolType })
      await fetchTables()
      ElMessage.success('点表创建成功')
      return newTable
    } catch (e) {
      error.value = e.toString()
      ElMessage.error(`创建点表失败: ${e}`)
      throw e
    } finally {
      loading.value = false
    }
  }

  async function deleteTable(id) {
    loading.value = true
    error.value = null
    try {
      await invoke('delete_point_table', { id })
      await fetchTables()
      ElMessage.success('点表删除成功')
    } catch (e) {
      error.value = e.toString()
      ElMessage.error(`删除点表失败: ${e}`)
      throw e
    } finally {
      loading.value = false
    }
  }

  async function uploadCsv(tableId, csvType, content) {
    loading.value = true
    error.value = null
    try {
      const result = await invoke('upload_csv_file', { tableId, csvType, content })
      if (result.is_valid) {
        ElMessage.success(`CSV文件上传成功`)
        if (result.warnings.length > 0) {
          result.warnings.forEach(warning => {
            ElMessage.warning(`警告: ${warning.message}`)
          })
        }
      } else {
        result.errors.forEach(error => {
          ElMessage.error(`错误: ${error.message}`)
        })
      }
      await fetchTable(tableId)
      return result
    } catch (e) {
      error.value = e.toString()
      ElMessage.error(`上传CSV失败: ${e}`)
      throw e
    } finally {
      loading.value = false
    }
  }

  async function exportCsv(tableId, csvType) {
    loading.value = true
    error.value = null
    try {
      const content = await invoke('export_csv_file', { tableId, csvType })
      return content
    } catch (e) {
      error.value = e.toString()
      ElMessage.error(`导出CSV失败: ${e}`)
      throw e
    } finally {
      loading.value = false
    }
  }

  async function validateTable(tableId) {
    loading.value = true
    error.value = null
    try {
      const result = await invoke('validate_point_table', { tableId })
      if (result.is_valid) {
        ElMessage.success('点表验证通过')
      } else {
        ElMessage.error(`点表验证失败，发现 ${result.errors.length} 个错误`)
      }
      return result
    } catch (e) {
      error.value = e.toString()
      ElMessage.error(`验证点表失败: ${e}`)
      throw e
    } finally {
      loading.value = false
    }
  }

  async function updatePoint(tableId, pointType, pointId, pointData) {
    loading.value = true
    error.value = null
    try {
      await invoke('update_point', { tableId, pointType, pointId, pointData })
      await fetchTable(tableId)
      ElMessage.success('点位更新成功')
    } catch (e) {
      error.value = e.toString()
      ElMessage.error(`更新点位失败: ${e}`)
      throw e
    } finally {
      loading.value = false
    }
  }

  async function deletePoint(tableId, pointType, pointId) {
    loading.value = true
    error.value = null
    try {
      await invoke('delete_point', { tableId, pointType, pointId })
      await fetchTable(tableId)
      ElMessage.success('点位删除成功')
    } catch (e) {
      error.value = e.toString()
      ElMessage.error(`删除点位失败: ${e}`)
      throw e
    } finally {
      loading.value = false
    }
  }

  async function exportToComsrv(tableId) {
    loading.value = true
    error.value = null
    try {
      const path = await invoke('export_to_comsrv_format', { tableId })
      ElMessage.success(`导出成功: ${path}`)
      return path
    } catch (e) {
      error.value = e.toString()
      ElMessage.error(`导出失败: ${e}`)
      throw e
    } finally {
      loading.value = false
    }
  }

  return {
    tables,
    currentTable,
    loading,
    error,
    tableCount,
    fetchTables,
    fetchTable,
    createTable,
    deleteTable,
    uploadCsv,
    exportCsv,
    validateTable,
    updatePoint,
    deletePoint,
    exportToComsrv
  }
})