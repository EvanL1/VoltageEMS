/**
 * 通用表格数据管理 Composable
 * 专为 Element Plus el-table 组件设计，提供数据管理功能
 *
 * 功能特性：
 * - 数据查询与分页管理
 * - 多条件筛选与搜索
 * - 单个/批量删除操作
 * - 数据导出功能
 * - 加载状态管理
 * - 错误处理
 */

import { ref, reactive, computed, readonly, toRaw } from 'vue'
import Request, { type ApiResponse } from '@/utils/request'
import { ElMessage, ElMessageBox } from 'element-plus'

// 分页参数接口
export interface PaginationParams {
  page: number // 当前页码
  pageSize: number // 每页大小
  total: number // 总记录数
}

// 排序参数接口
export interface SortParams {
  prop: string // 排序字段 (与el-table保持一致)
  order: 'ascending' | 'descending' | null // 排序方向 (与el-table保持一致)
}

// 通用查询参数接口
export interface QueryParams extends Partial<PaginationParams> {
  keyword?: string // 关键字搜索
  sortBy?: string // 排序字段
  sortOrder?: 'asc' | 'desc' // 排序方向
  [key: string]: any // 其他筛选条件
}

// 表格操作配置接口
export interface TableConfig {
  // API端点配置
  listUrl: string // 列表查询接口
  deleteUrl?: string // 删除接口（支持占位符 {id}）
  batchDeleteUrl?: string // 批量删除接口
  exportUrl?: string // 导出接口

  // 功能开关
  enableDelete?: boolean // 是否启用删除功能
  enableBatchDelete?: boolean // 是否启用批量删除功能
  enableExport?: boolean // 是否启用导出功能

  // 分页配置
  defaultPageSize?: number // 默认每页大小

  // 删除确认配置
  deleteConfirmMessage?: string // 删除确认消息
  batchDeleteConfirmMessage?: string // 批量删除确认消息
}

// 表格数据响应接口
export interface TableDataResponse<T = any> {
  list: T[] // 数据列表
  total: number // 总记录数
  page: number // 当前页码
  pageSize: number // 每页大小
}

/**
 * 表格数据管理 Composable
 * @param config 表格配置
 * @returns 表格管理的响应式数据和方法
 */
export function useTableData<T = any>(config: TableConfig) {
  // 响应式数据
  const loading = ref(false) // 加载状态
  const tableData = ref<T[]>([]) // 表格数据

  // 分页数据
  const pagination = reactive<PaginationParams>({
    page: 1,
    pageSize: config.defaultPageSize || 20,
    total: 0,
  })

  // 查询参数
  const queryParams = reactive<QueryParams>({
    keyword: '', // 搜索关键字
  })

  // 筛选条件
  const filters = reactive<Record<string, any>>({})

  /**
   * 获取表格数据
   * @param resetPage 是否重置页码
   */
  const fetchTableData = async (resetPage = false) => {
    try {
      loading.value = true

      if (resetPage) {
        pagination.page = 1
      }

      // 构建请求参数
      const params: QueryParams = {
        page: pagination.page,
        pageSize: pagination.pageSize,
        ...queryParams,
        ...filters,
      }

      // 过滤空值参数
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
   * 搜索数据
   * @param keyword 搜索关键字
   */
  const searchData = (keyword: string) => {
    queryParams.keyword = keyword
    fetchTableData(true)
  }

  /**
   * 设置筛选条件
   * @param filterKey 筛选字段
   * @param filterValue 筛选值
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
   * 批量设置筛选条件
   * @param newFilters 新的筛选条件
   */
  const setFilters = (newFilters: Record<string, any>) => {
    Object.keys(filters).forEach((key) => delete filters[key])
    Object.assign(filters, newFilters)
    fetchTableData(true)
  }

  /**
   * 清空筛选条件
   */
  const clearFilters = () => {
    Object.keys(filters).forEach((key) => delete filters[key])
    queryParams.keyword = ''
    fetchTableData(true)
  }

  /**
   * 处理el-table的排序变化
   * @param sortInfo el-table的排序信息 { prop, order }
   */
  const handleSortChange = (sortInfo: SortParams) => {
    if (sortInfo.prop && sortInfo.order) {
      // 转换el-table的排序格式为后端API格式
      queryParams.sortBy = sortInfo.prop
      queryParams.sortOrder = sortInfo.order === 'ascending' ? 'asc' : 'desc'
    } else {
      // 清空排序
      delete queryParams.sortBy
      delete queryParams.sortOrder
    }
    fetchTableData()
  }

  /**
   * 分页改变处理
   * @param page 页码
   * @param pageSize 每页大小
   */
  const handlePageChange = (page: number, pageSize?: number) => {
    pagination.page = page
    if (pageSize && pageSize !== pagination.pageSize) {
      pagination.pageSize = pageSize
      pagination.page = 1 // 改变每页大小时重置到第一页
    }
    fetchTableData()
  }

  /**
   * 删除单条记录
   * @param id 记录ID
   * @param confirmMessage 确认消息
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
   * 批量删除记录
   * @param ids 记录ID数组
   * @param confirmMessage 确认消息
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
   * 刷新数据
   */
  const refreshData = () => {
    fetchTableData()
  }

  /**
   * 重置表格（清空所有筛选条件和排序，回到第一页）
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
   * 导出数据
   * @param filename 文件名
   * @param params 额外参数
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
   * 获取当前查询参数（用于调试或其他用途）
   */
  const getCurrentParams = () => {
    return {
      pagination: toRaw(pagination),
      query: toRaw(queryParams),
      filters: toRaw(filters),
    }
  }

  // 计算属性
  const isEmpty = computed(() => tableData.value.length === 0)
  const hasData = computed(() => tableData.value.length > 0)

  // 返回响应式数据和方法
  return {
    // 响应式数据
    loading: readonly(loading),
    tableData, // 不使用readonly，保持数组的可变性
    pagination: readonly(pagination),
    queryParams: readonly(queryParams),
    filters: readonly(filters),

    // 计算属性
    isEmpty,
    hasData,

    // 数据操作方法
    fetchTableData,
    refreshData,
    resetTable,

    // 查询和筛选方法
    searchData,
    setFilter,
    setFilters,
    clearFilters,

    // el-table 集成方法
    handleSortChange,
    handlePageChange,

    // 删除方法
    deleteRow,
    batchDeleteRows,

    // 导出方法
    exportData,

    // 工具方法
    getCurrentParams,
  }
}
