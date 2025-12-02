import { describe, it, expect, vi, beforeEach } from 'vitest'
import { useTableData } from '../useTableData'

vi.mock('@/utils/request', () => ({
  Request: {
    get: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
    download: vi.fn(),
  },
}))

vi.mock('element-plus', () => ({
  ElMessage: {
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
  },
  ElMessageBox: {
    confirm: vi.fn(),
  },
}))

describe('useTableData', () => {
  const mockConfig = {
    listUrl: '/test/list',
    defaultPageSize: 20,
    enableDelete: true,
    deleteUrl: '/test/delete/{id}',
  }

  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('should initialize with default values', () => {
    const { loading, tableData, pagination } = useTableData(mockConfig)

    expect(loading.value).toBe(false)
    expect(tableData.value).toEqual([])
    expect(pagination.page).toBe(1)
    expect(pagination.pageSize).toBe(20)
    expect(pagination.total).toBe(0)
  })

  it('should fetch table data successfully', async () => {
    const mockResponse = {
      success: true,
      data: {
        list: [{ id: 1, name: 'test' }],
        total: 1,
      },
    }

    const { Request } = await import('@/utils/request')
    vi.mocked(Request.get).mockResolvedValue(mockResponse)

    const { fetchTableData, tableData, pagination } = useTableData(mockConfig)
    await fetchTableData()

    expect(tableData.value).toEqual([{ id: 1, name: 'test' }])
    expect(pagination.total).toBe(1)
    expect(Request.get).toHaveBeenCalledWith('/test/list', expect.any(Object))
  })

  it('should handle search data', async () => {
    const mockResponse = {
      success: true,
      data: { list: [], total: 0 },
    }

    const { Request } = await import('@/utils/request')
    vi.mocked(Request.get).mockResolvedValue(mockResponse)

    const { searchData } = useTableData(mockConfig)
    await searchData('test keyword')

    expect(Request.get).toHaveBeenCalledWith(
      '/test/list',
      expect.objectContaining({
        keyword: 'test keyword',
        page: 1,
      }),
    )
  })

  it('should handle pagination changes', async () => {
    const mockResponse = {
      success: true,
      data: { list: [], total: 0 },
    }

    const { Request } = await import('@/utils/request')
    vi.mocked(Request.get).mockResolvedValue(mockResponse)

    const { handlePageChange, handlePageSizeChange } = useTableData(mockConfig)

    await handlePageChange(2)
    expect(Request.get).toHaveBeenCalledWith('/test/list', expect.objectContaining({ page: 2 }))

    await handlePageSizeChange(50)
    expect(Request.get).toHaveBeenCalledWith(
      '/test/list',
      expect.objectContaining({ page: 1, page_size: 50 }),
    )
  })

  it('should handle delete row', async () => {
    const mockResponse = { success: true }
    const { Request } = await import('@/utils/request')
    vi.mocked(Request.delete).mockResolvedValue(mockResponse)

    const { deleteRow } = useTableData(mockConfig)
    const result = await deleteRow('1')

    expect(result).toBe(true)
    expect(Request.delete).toHaveBeenCalledWith('/test/delete/1')
  })

  it('should handle set filter', async () => {
    const mockResponse = {
      success: true,
      data: { list: [], total: 0 },
    }

    const { Request } = await import('@/utils/request')
    vi.mocked(Request.get).mockResolvedValue(mockResponse)

    const { setFilter } = useTableData(mockConfig)
    await setFilter('status', 'active')

    expect(Request.get).toHaveBeenCalledWith(
      '/test/list',
      expect.objectContaining({
        status: 'active',
        page: 1,
      }),
    )
  })
})
