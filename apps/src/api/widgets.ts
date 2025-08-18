/**
 * Widgets 相关API接口
 * 提供组件和小部件相关的数据接口
 */

import Request, { type ApiResponse } from '@/utils/request'

// 定义Widget数据接口
export interface Widget {
  id: number // 组件ID
  name: string // 组件名称
  type: string // 组件类型 (chart, table, card, etc.)
  title: string // 组件标题
  description?: string // 组件描述
  config: Record<string, any> // 组件配置
  position: {
    // 组件位置信息
    x: number
    y: number
    width: number
    height: number
  }
  isActive: boolean // 是否激活
  createTime: string // 创建时间
  updateTime: string // 更新时间
}

// 定义Widget列表查询参数
export interface WidgetListParams {
  page?: number // 页码，默认1
  pageSize?: number // 每页数量，默认10
  type?: string // 组件类型筛选
  keyword?: string // 搜索关键词
  isActive?: boolean // 激活状态筛选
  sortBy?: string // 排序字段
  sortOrder?: 'asc' | 'desc' // 排序方向
}

// 定义Widget列表响应数据
export interface WidgetListResponse {
  list: Widget[] // 组件列表
  total: number // 总数量
  page: number // 当前页码
  pageSize: number // 每页数量
  totalPages: number // 总页数
}

// 定义Widget创建/更新参数
export interface WidgetFormData {
  name: string
  type: string
  title: string
  description?: string
  config: Record<string, any>
  position: {
    x: number
    y: number
    width: number
    height: number
  }
  isActive?: boolean
}

// 定义图表数据接口
export interface ChartData {
  labels: string[] // 图表标签
  datasets: Array<{
    // 图表数据集
    label: string
    data: number[]
    backgroundColor?: string[]
    borderColor?: string[]
    borderWidth?: number
  }>
}

// 定义统计数据接口
export interface StatisticsData {
  totalWidgets: number // 总组件数
  activeWidgets: number // 激活组件数
  chartWidgets: number // 图表组件数
  tableWidgets: number // 表格组件数
  cardWidgets: number // 卡片组件数
  dailyViews: number // 日访问量
  monthlyViews: number // 月访问量
  growthRate: number // 增长率
}

/**
 * Widget API 类
 * 包含所有Widget相关的接口请求方法
 */
class WidgetAPI {
  /**
   * 获取Widget列表
   * @param params 查询参数
   * @returns Promise<ApiResponse<WidgetListResponse>>
   */
  static getWidgetList(params?: WidgetListParams): Promise<ApiResponse<WidgetListResponse>> {
    return Request.get<WidgetListResponse>('/widgets', params, {
      showErrorMessage: true,
      showSuccessMessage: false,
    })
  }

  /**
   * 根据ID获取Widget详情
   * @param id Widget ID
   * @returns Promise<ApiResponse<Widget>>
   */
  static getWidgetById(id: number): Promise<ApiResponse<Widget>> {
    return Request.get<Widget>(`/widgets/${id}`, undefined, {
      showErrorMessage: true,
    })
  }

  /**
   * 创建新的Widget
   * @param data Widget数据
   * @returns Promise<ApiResponse<Widget>>
   */
  static createWidget(data: WidgetFormData): Promise<ApiResponse<Widget>> {
    return Request.post<Widget>('/widgets', data, {
      showErrorMessage: true,
      showSuccessMessage: true,
    })
  }

  /**
   * 更新Widget信息
   * @param id Widget ID
   * @param data 更新数据
   * @returns Promise<ApiResponse<Widget>>
   */
  static updateWidget(id: number, data: Partial<WidgetFormData>): Promise<ApiResponse<Widget>> {
    return Request.put<Widget>(`/widgets/${id}`, data, {
      showErrorMessage: true,
      showSuccessMessage: true,
    })
  }

  /**
   * 删除Widget
   * @param id Widget ID
   * @returns Promise<ApiResponse<null>>
   */
  static deleteWidget(id: number): Promise<ApiResponse<null>> {
    return Request.delete<null>(`/widgets/${id}`, {
      showErrorMessage: true,
      showSuccessMessage: true,
    })
  }

  /**
   * 批量删除Widget
   * @param ids Widget ID数组
   * @returns Promise<ApiResponse<null>>
   */
  static batchDeleteWidgets(ids: number[]): Promise<ApiResponse<null>> {
    return Request.post<null>(
      '/widgets/batch-delete',
      { ids },
      {
        showErrorMessage: true,
        showSuccessMessage: true,
      },
    )
  }

  /**
   * 切换Widget激活状态
   * @param id Widget ID
   * @param isActive 激活状态
   * @returns Promise<ApiResponse<Widget>>
   */
  static toggleWidgetStatus(id: number, isActive: boolean): Promise<ApiResponse<Widget>> {
    return Request.patch<Widget>(
      `/widgets/${id}/status`,
      { isActive },
      {
        showErrorMessage: true,
        showSuccessMessage: true,
      },
    )
  }

  /**
   * 更新Widget位置
   * @param id Widget ID
   * @param position 位置信息
   * @returns Promise<ApiResponse<Widget>>
   */
  static updateWidgetPosition(
    id: number,
    position: Widget['position'],
  ): Promise<ApiResponse<Widget>> {
    return Request.patch<Widget>(
      `/widgets/${id}/position`,
      { position },
      {
        showErrorMessage: true,
        showSuccessMessage: false,
      },
    )
  }

  /**
   * 复制Widget
   * @param id 原Widget ID
   * @param name 新Widget名称
   * @returns Promise<ApiResponse<Widget>>
   */
  static duplicateWidget(id: number, name?: string): Promise<ApiResponse<Widget>> {
    return Request.post<Widget>(
      `/widgets/${id}/duplicate`,
      { name },
      {
        showErrorMessage: true,
        showSuccessMessage: true,
      },
    )
  }

  /**
   * 获取Widget配置模板
   * @param type Widget类型
   * @returns Promise<ApiResponse<Record<string, any>>>
   */
  static getWidgetTemplate(type: string): Promise<ApiResponse<Record<string, any>>> {
    return Request.get<Record<string, any>>(`/widgets/templates/${type}`, undefined, {
      showErrorMessage: true,
    })
  }

  /**
   * 获取图表数据
   * @param widgetId Widget ID
   * @param params 查询参数
   * @returns Promise<ApiResponse<ChartData>>
   */
  static getChartData(
    widgetId: number,
    params?: {
      dateRange?: [string, string]
      granularity?: 'day' | 'week' | 'month'
    },
  ): Promise<ApiResponse<ChartData>> {
    return Request.get<ChartData>(`/widgets/${widgetId}/chart-data`, params, {
      showErrorMessage: true,
    })
  }

  /**
   * 获取表格数据
   * @param widgetId Widget ID
   * @param params 查询参数
   * @returns Promise<ApiResponse<any[]>>
   */
  static getTableData(
    widgetId: number,
    params?: {
      page?: number
      pageSize?: number
      filters?: Record<string, any>
      sorting?: { field: string; direction: 'asc' | 'desc' }
    },
  ): Promise<ApiResponse<any[]>> {
    return Request.get<any[]>(`/widgets/${widgetId}/table-data`, params, {
      showErrorMessage: true,
    })
  }

  /**
   * 导出Widget配置
   * @param ids Widget ID数组
   * @returns Promise<void>
   */
  static exportWidgets(ids: number[]): Promise<void> {
    return Request.download('/widgets/export', { ids }, `widgets_${Date.now()}.json`)
  }

  /**
   * 导入Widget配置
   * @param file 配置文件
   * @returns Promise<ApiResponse<Widget[]>>
   */
  static importWidgets(file: File): Promise<ApiResponse<Widget[]>> {
    return Request.upload<Widget[]>('/widgets/import', file, undefined, {
      showErrorMessage: true,
      showSuccessMessage: true,
    })
  }

  /**
   * 获取Widget统计数据
   * @param dateRange 日期范围
   * @returns Promise<ApiResponse<StatisticsData>>
   */
  static getWidgetStatistics(dateRange?: [string, string]): Promise<ApiResponse<StatisticsData>> {
    return Request.get<StatisticsData>(
      '/widgets/statistics',
      { dateRange },
      {
        showErrorMessage: true,
      },
    )
  }

  /**
   * 搜索Widget
   * @param keyword 搜索关键词
   * @param options 搜索选项
   * @returns Promise<ApiResponse<Widget[]>>
   */
  static searchWidgets(
    keyword: string,
    options?: {
      type?: string
      limit?: number
      includeInactive?: boolean
    },
  ): Promise<ApiResponse<Widget[]>> {
    return Request.get<Widget[]>(
      '/widgets/search',
      {
        keyword,
        ...options,
      },
      {
        showErrorMessage: true,
      },
    )
  }

  /**
   * 获取Widget类型列表
   * @returns Promise<ApiResponse<string[]>>
   */
  static getWidgetTypes(): Promise<ApiResponse<string[]>> {
    return Request.get<string[]>('/widgets/types', undefined, {
      showErrorMessage: true,
    })
  }

  /**
   * 验证Widget配置
   * @param type Widget类型
   * @param config 配置数据
   * @returns Promise<ApiResponse<{ valid: boolean; errors?: string[] }>>
   */
  static validateWidgetConfig(
    type: string,
    config: Record<string, any>,
  ): Promise<ApiResponse<{ valid: boolean; errors?: string[] }>> {
    return Request.post<{ valid: boolean; errors?: string[] }>(
      '/widgets/validate-config',
      { type, config },
      {
        showErrorMessage: false,
      },
    )
  }
}

// 导出API类和相关接口
export default WidgetAPI
