// 控制管理相关类型定义

// 操作符类型
export type Operator = '>' | '>=' | '<' | '<=' | '=' | 'gt' | 'gte' | 'lt' | 'lte' | 'eq'

// 规则表单模型类型
export interface RuleFormModel {
  rule_name: string
  service_type: string | number | null
  point_id: string | number | null
  data_type: string | number | null
  warning_level: string | number | null
  operator: Operator | null
  value: number | null
  enabled: boolean
}

// 规则信息类型
export interface RuleInfo {
  id: number
  rule_name: string
  service_type: string | number | null
  point_id: string | number | null
  data_type: string | number | null
  warning_level: string | number | null
  operator: Operator | null
  value: number | null
  notification?: string[]
  enabled: boolean
  created_at: string
}

// 对话框暴露类型
export interface DialogExpose {
  dialogVisible: boolean
}
