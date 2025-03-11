/**
 * Generic table data management composable tailored for the Element Plus el-table component.
 *
 * Features:
 * - Data querying with pagination
 * - Multi-condition filtering and search
 * - Single and bulk deletion
 * - Data export utilities
 * - Loading state management
 * - Centralized error handling
 */

import { ref, reactive, computed, readonly, toRaw } from 'vue'
import Request, { type ApiResponse } from '@/utils/request'
import { ElMessage, ElMessageBox } from 'element-plus'

// Pagination parameters.
export interface PaginationParams {
  page: number // Current page number.
  pageSize: number // Page size.
  total: number // Total record count.
}

// Sorting parameters.
export interface SortParams {
  prop: string // Sort field (kept consistent with el-table).
  order: 'ascending' | 'descending' | null // Sort direction (aligned with el-table).
}

// Generic query parameters.
export interface QueryParams extends Partial<PaginationParams> {
  keyword?: string // Keyword search.
  sortBy?: string // Sort field.
  sortOrder?: 'asc' | 'desc' // Sort direction.
  [key: string]: any // Additional filter criteria.
}

// Table operation configuration.
export interface TableConfig {
  // API endpoint configuration.
  listUrl: string // List query endpoint.
  deleteUrl?: string // Delete endpoint (supports the {id} placeholder).
  batchDeleteUrl?: string // Bulk delete endpoint.
  exportUrl?: string // Export endpoint.

  // Feature toggles.
  enableDelete?: boolean // Enable delete functionality.
  enableBatchDelete?: boolean // Enable bulk delete functionality.
  enableExport?: boolean // Enable export functionality.

  // Pagination configuration.
  defaultPageSize?: number // Default page size.

  // Delete confirmation configuration.
  deleteConfirmMessage?: string // Delete confirmation message.
  batchDeleteConfirmMessage?: string // Bulk delete confirmation message.
}

// Table data response payload.
export interface TableDataResponse<T = any> {
  list: T[] // Data list.
  total: number // Total record count.
  page: number // Current page number.
  pageSize: number // Page size.
}

/**
 * Table data management composable.
 * @param config Table configuration options.
 * @returns Reactive table state and helper methods.
 */
export function useTableData<T = any>(config: TableConfig) {
  // Reactive state.
  const loading = ref(false) // Loading indicator.
  const tableData = ref<T[]>([]) // Table data set.

  // Pagination state.
  const pagination = reactive<PaginationParams>({
    page: 1,
    pageSize: config.defaultPageSize || 20,
    total: 0,
  })

  // Query parameters.
  const queryParams = reactive<QueryParams>({
    keyword: '', // Search keyword.
  })

  // Filter conditions.
  const filters = reactive<Record<string, any>>({})

  /**
   * Fetch table data.
   * @param resetPage Whether to reset the page index.
   */
  const fetchTableData = async (resetPage = false) => {
    try {
      loading.value = true

      if (resetPage) {
        pagination.page = 1
      }

      // Build request parameters.
      const params: QueryParams = {
        page: pagination.page,
        pageSize: pagination.pageSize,
        ...queryParams,
        ...filters,
      }

      // Remove empty parameters.
      const filteredParams: Record<string, any> = {}
      for (const key in params) {
        const value = params[key]
        if (value !== null && value !== undefined && value !== '') {
          filteredParams[key] = value
        }
      }

      const response: ApiResponse<TableDataResponse<T>> = await Request.get(
        config.listUrl,
        filteredParams,
      )

      if (response.success) {
        tableData.value = response.data.list || []
        pagination.total = response.data.total || 0
        pagination.page = response.data.page || pagination.page
        pagination.pageSize = response.data.pageSize || pagination.pageSize
      }
    } catch (error) {
      console.error('获取表格数据失败:', error)
      ElMessage.error('获取数据失败，请重试')
      tableData.value = []
      pagination.total = 0
    } finally {
      loading.value = false
    }
  }

  /**
   * Search data.
   * @param keyword Search keyword.
   */
  const searchData = (keyword: string) => {
    queryParams.keyword = keyword
    fetchTableData(true)
  }

  /**
   * Set a single filter condition.
   * @param filterKey Filter field name.
   * @param filterValue Filter value.
   */
  const setFilter = (filterKey: string, filterValue: any) => {
    if (filterValue === null || filterValue === undefined || filterValue === '') {
      delete filters[filterKey]
    } else {
      filters[filterKey] = filterValue
    }
    fetchTableData(true)
  }

  /**
   * Set multiple filter conditions at once.
   * @param newFilters New filter map.
   */
  const setFilters = (newFilters: Record<string, any>) => {
    Object.keys(filters).forEach((key) => delete filters[key])
    Object.assign(filters, newFilters)
    fetchTableData(true)
  }

  /**
   * Clear all filters.
   */
  const clearFilters = () => {
    Object.keys(filters).forEach((key) => delete filters[key])
    queryParams.keyword = ''
    fetchTableData(true)
  }

  /**
   * Handle el-table sort changes.
   * @param sortInfo Sorting information from el-table ({ prop, order }).
   */
  const handleSortChange = (sortInfo: SortParams) => {
    if (sortInfo.prop && sortInfo.order) {
      // Convert el-table sort format to the API format.
      queryParams.sortBy = sortInfo.prop
      queryParams.sortOrder = sortInfo.order === 'ascending' ? 'asc' : 'desc'
    } else {
      // Clear sorting.
      delete queryParams.sortBy
      delete queryParams.sortOrder
    }
    fetchTableData()
  }

  /**
   * Handle pagination changes.
   * @param page Current page number.
   * @param pageSize Page size.
   */
  const handlePageChange = (page: number, pageSize?: number) => {
    pagination.page = page
    if (pageSize && pageSize !== pagination.pageSize) {
      pagination.pageSize = pageSize
      pagination.page = 1 // Reset to the first page when the page size changes.
    }
    fetchTableData()
  }

  /**
   * Delete a single record.
   * @param id Record identifier.
   * @param confirmMessage Confirmation message override.
   */
  const deleteRow = async (id: string | number, confirmMessage?: string) => {
    if (!config.enableDelete || !config.deleteUrl) {
      ElMessage.warning('删除功能未启用')
      return false
    }

    try {
      await ElMessageBox.confirm(
        confirmMessage || config.deleteConfirmMessage || '确定要删除这条记录吗？',
        '删除确认',
        {
          confirmButtonText: '确定',
          cancelButtonText: '取消',
          type: 'warning',
        },
      )

      loading.value = true
      const deleteUrl = config.deleteUrl.replace('{id}', String(id))
      const response = await Request.delete(deleteUrl)

      if (response.success) {
        ElMessage.success('删除成功')
        await fetchTableData()
        return true
      }
      return false
    } catch (error: any) {
      if (error !== 'cancel') {
        console.error('删除失败:', error)
        ElMessage.error('删除失败，请重试')
      }
      return false
    } finally {
      loading.value = false
    }
  }

  /**
   * Delete multiple records.
   * @param ids Record identifiers.
   * @param confirmMessage Confirmation message override.
   */
  const batchDeleteRows = async (ids: (string | number)[], confirmMessage?: string) => {
    if (!config.enableBatchDelete || !config.batchDeleteUrl) {
      ElMessage.warning('批量删除功能未启用')
      return false
    }

    if (ids.length === 0) {
      ElMessage.warning('请选择要删除的记录')
      return false
    }

    try {
      await ElMessageBox.confirm(
        confirmMessage ||
          config.batchDeleteConfirmMessage ||
          `确定要删除选中的 ${ids.length} 条记录吗？`,
        '批量删除确认',
        {
          confirmButtonText: '确定',
          cancelButtonText: '取消',
          type: 'warning',
        },
      )

      loading.value = true
      const response = await Request.post(config.batchDeleteUrl, {
        ids: ids,
      })

      if (response.success) {
        ElMessage.success(`成功删除 ${ids.length} 条记录`)
        await fetchTableData()
        return true
      }
      return false
    } catch (error: any) {
      if (error !== 'cancel') {
        console.error('批量删除失败:', error)
        ElMessage.error('批量删除失败，请重试')
      }
      return false
    } finally {
      loading.value = false
    }
  }

  /**
   * Refresh table data.
   */
  const refreshData = () => {
    fetchTableData()
  }

  /**
   * Reset the table (clear filters, sorting, and return to the first page).
   */
  const resetTable = () => {
    pagination.page = 1
    pagination.pageSize = config.defaultPageSize || 20
    queryParams.keyword = ''
    delete queryParams.sortBy
    delete queryParams.sortOrder
    Object.keys(filters).forEach((key) => delete filters[key])
    fetchTableData()
  }

  /**
   * Export table data.
   * @param filename Optional file name.
   * @param params Additional request parameters.
   */
  const exportData = async (filename?: string, params?: Record<string, any>) => {
    if (!config.enableExport || !config.exportUrl) {
      ElMessage.warning('导出功能未启用')
      return false
    }

    try {
      loading.value = true

      const exportParams = {
        ...queryParams,
        ...filters,
        ...params,
      }

      await Request.download(
        config.exportUrl,
        exportParams,
        filename || `export_${Date.now()}.xlsx`,
      )
      return true
    } catch (error) {
      console.error('导出失败:', error)
      ElMessage.error('导出失败，请重试')
      return false
    } finally {
      loading.value = false
    }
  }

  /**
   * Retrieve the current query parameters (useful for debugging or reuse).
   */
  const getCurrentParams = () => {
    return {
      pagination: toRaw(pagination),
      query: toRaw(queryParams),
      filters: toRaw(filters),
    }
  }

  // Computed properties.
  const isEmpty = computed(() => tableData.value.length === 0)
  const hasData = computed(() => tableData.value.length > 0)

  // Expose reactive state and helpers.
  return {
    // Reactive state.
    loading: readonly(loading),
    tableData, // Keep mutable to allow array updates without readonly wrapping.
    pagination: readonly(pagination),
    queryParams: readonly(queryParams),
    filters: readonly(filters),

    // Computed helpers.
    isEmpty,
    hasData,

    // Data operations.
    fetchTableData,
    refreshData,
    resetTable,

    // Query and filter helpers.
    searchData,
    setFilter,
    setFilters,
    clearFilters,

    // el-table integration helpers.
    handleSortChange,
    handlePageChange,

    // Deletion helpers.
    deleteRow,
    batchDeleteRows,

    // Export helpers.
    exportData,

    // Utility helpers.
    getCurrentParams,
  }
}
