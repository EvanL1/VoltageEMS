"""
告警规则服务
提供alert_rule表的CRUD操作
"""

import logging
from datetime import datetime
from typing import List, Optional, Dict, Any, Tuple

from app.models.alert_rule import AlertRule
from app.core.database import get_db_manager

logger = logging.getLogger(__name__)


class AlertRuleService:
    """告警规则服务类"""
    
    def __init__(self):
        self.db_manager = get_db_manager()
    
    def create_rule(self, rule: AlertRule) -> Optional[int]:
        """创建新的告警规则"""
        try:
            if not rule.validate():
                logger.error("规则验证失败")
                return None
            
            # 设置创建时间（使用时间戳）
            now = int(datetime.now().timestamp())
            rule.created_at = now
            rule.updated_at = now
            
            sql = """
            INSERT INTO alert_rule (
                service_type, channel_id, data_type, point_id, rule_name, 
                warning_level, operator, value, enabled, 
                description, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """
            
            params = (
                rule.service_type,
                rule.channel_id,
                rule.data_type,
                rule.point_id,
                rule.rule_name,
                rule.warning_level,
                rule.operator,
                rule.value,
                rule.enabled,
                rule.description,
                rule.created_at,
                rule.updated_at
            )
            
            rule_id = self.db_manager.execute_insert(sql, params)
            logger.info(f"创建告警规则成功，ID: {rule_id}")
            return rule_id
            
        except Exception as e:
            logger.error(f"创建告警规则失败: {e}")
            return None
    
    def get_rule_by_id(self, rule_id: int) -> Optional[AlertRule]:
        """根据ID获取告警规则"""
        try:
            sql = "SELECT * FROM alert_rule WHERE id = ?"
            results = self.db_manager.execute_query(sql, (rule_id,))
            
            if results:
                row = results[0]
                return self._row_to_alert_rule(row)
            
            return None
            
        except Exception as e:
            logger.error(f"获取告警规则失败: {e}")
            return None
    
    def get_rules_by_service_channel_point(self, service_type: str, channel_id: int, data_type: str, point_id: int) -> List[AlertRule]:
        """根据服务类型、通道和点位获取告警规则"""
        try:
            sql = """
            SELECT * FROM alert_rule 
            WHERE service_type = ? AND channel_id = ? AND data_type = ? AND point_id = ? AND enabled = 1
            ORDER BY warning_level DESC
            """
            
            results = self.db_manager.execute_query(sql, (service_type, channel_id, data_type, point_id))
            
            return [self._row_to_alert_rule(row) for row in results]
            
        except Exception as e:
            logger.error(f"获取告警规则失败: {e}")
            return []
    
    def get_all_enabled_rules(self) -> List[AlertRule]:
        """获取所有启用的告警规则"""
        try:
            sql = "SELECT * FROM alert_rule WHERE enabled = 1 ORDER BY created_at"
            results = self.db_manager.execute_query(sql)
            
            return [self._row_to_alert_rule(row) for row in results]
            
        except Exception as e:
            logger.error(f"获取所有告警规则失败: {e}")
            return []
    
    def get_rules_by_channel(self, channel_id: int) -> List[AlertRule]:
        """根据通道ID获取所有告警规则"""
        try:
            sql = "SELECT * FROM alert_rule WHERE channel_id = ? ORDER BY created_at"
            results = self.db_manager.execute_query(sql, (channel_id,))
            
            return [self._row_to_alert_rule(row) for row in results]
            
        except Exception as e:
            logger.error(f"获取通道告警规则失败: {e}")
            return []
    
    def update_rule(self, rule: AlertRule) -> bool:
        """更新告警规则"""
        try:
            if not rule.validate() or not rule.id:
                logger.error("规则验证失败或缺少ID")
                return False
            
            sql = """
            UPDATE alert_rule SET
                service_type = ?, channel_id = ?, data_type = ?, point_id = ?, rule_name = ?,
                warning_level = ?, operator = ?, value = ?, enabled = ?,
                description = ?, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?
            """
            
            params = (
                rule.service_type,
                rule.channel_id,
                rule.data_type,
                rule.point_id,
                rule.rule_name,
                rule.warning_level,
                rule.operator,
                rule.value,
                rule.enabled,
                rule.description,
                rule.id
            )
            
            affected_rows = self.db_manager.execute_update(sql, params)
            
            if affected_rows > 0:
                logger.info(f"更新告警规则成功，ID: {rule.id}")
                return True
            else:
                logger.warning(f"未找到要更新的规则，ID: {rule.id}")
                return False
                
        except Exception as e:
            logger.error(f"更新告警规则失败: {e}")
            return False
    
    def delete_rule(self, rule_id: int) -> bool:
        """删除告警规则"""
        try:
            sql = "DELETE FROM alert_rule WHERE id = ?"
            affected_rows = self.db_manager.execute_delete(sql, (rule_id,))
            
            if affected_rows > 0:
                logger.info(f"删除告警规则成功，ID: {rule_id}")
                return True
            else:
                logger.warning(f"未找到要删除的规则，ID: {rule_id}")
                return False
                
        except Exception as e:
            logger.error(f"删除告警规则失败: {e}")
            return False
    
    def enable_rule(self, rule_id: int) -> bool:
        """启用告警规则"""
        try:
            sql = "UPDATE alert_rule SET enabled = 1, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            affected_rows = self.db_manager.execute_update(sql, (rule_id,))
            
            if affected_rows > 0:
                logger.info(f"启用告警规则成功，ID: {rule_id}")
                return True
            else:
                logger.warning(f"未找到要启用的规则，ID: {rule_id}")
                return False
                
        except Exception as e:
            logger.error(f"启用告警规则失败: {e}")
            return False
    
    def disable_rule(self, rule_id: int) -> bool:
        """禁用告警规则"""
        try:
            sql = "UPDATE alert_rule SET enabled = 0, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            affected_rows = self.db_manager.execute_update(sql, (rule_id,))
            
            if affected_rows > 0:
                logger.info(f"禁用告警规则成功，ID: {rule_id}")
                return True
            else:
                logger.warning(f"未找到要禁用的规则，ID: {rule_id}")
                return False
                
        except Exception as e:
            logger.error(f"禁用告警规则失败: {e}")
            return False
    
    def search_rules(self, 
                    keyword: str = "", 
                    service_type: str = "", 
                    warning_level: Optional[int] = None,
                    enabled: Optional[bool] = None,
                    start_time: Optional[datetime] = None,
                    end_time: Optional[datetime] = None,
                    page: int = 1,
                    page_size: int = 10) -> Dict[str, Any]:
        """高级搜索告警规则"""
        try:
            conditions = []
            params = []
            
            # 多字段模糊查询
            if keyword:
                conditions.append("(rule_name LIKE ? OR description LIKE ? OR CAST(channel_id AS TEXT) LIKE ? OR CAST(point_id AS TEXT) LIKE ?)")
                keyword_pattern = f"%{keyword}%"
                params.extend([keyword_pattern, keyword_pattern, keyword_pattern, keyword_pattern])
            
            # 服务类型过滤
            if service_type:
                conditions.append("service_type = ?")
                params.append(service_type)
            
            # 告警级别过滤
            if warning_level is not None and warning_level in [1, 2, 3]:
                conditions.append("warning_level = ?")
                params.append(warning_level)
            
            # 启用状态过滤
            if enabled is not None:
                conditions.append("enabled = ?")
                params.append(1 if enabled else 0)
            
            # 时间范围过滤
            if start_time:
                conditions.append("created_at >= ?")
                params.append(int(start_time.timestamp()))
            
            if end_time:
                conditions.append("created_at <= ?")
                params.append(int(end_time.timestamp()))
            
            where_clause = " AND ".join(conditions) if conditions else "1=1"
            
            # 分页计算
            offset = (page - 1) * page_size
            
            # 查询总数
            count_sql = f"SELECT COUNT(*) FROM alert_rule WHERE {where_clause}"
            count_result = self.db_manager.execute_query(count_sql, tuple(params))
            total = count_result[0][0] if count_result else 0
            
            # 查询数据
            data_sql = f"""
            SELECT * FROM alert_rule WHERE {where_clause} 
            ORDER BY id ASC 
            LIMIT ? OFFSET ?
            """
            data_params = list(params) + [page_size, offset]
            results = self.db_manager.execute_query(data_sql, tuple(data_params))
            
            rules = [self._row_to_alert_rule(row) for row in results]
            
            return {
                "success": True,
                "message": f"查询成功，共找到 {total} 条记录",
                "data": {
                    "total": total,
                    "list": [rule.to_dict() for rule in rules]
                }
            }
            
        except Exception as e:
            logger.error(f"搜索告警规则失败: {e}")
            return {
                "success": False,
                "message": f"查询失败: {str(e)}",
                "data": {
                    "total": 0,
                    "list": []
                }
            }
    
    def get_rule_count(self) -> int:
        """获取告警规则总数"""
        try:
            sql = "SELECT COUNT(*) FROM alert_rule"
            results = self.db_manager.execute_query(sql)
            return results[0][0] if results else 0
            
        except Exception as e:
            logger.error(f"获取规则数量失败: {e}")
            return 0
    
    def get_enabled_rule_count(self) -> int:
        """获取启用的告警规则数量"""
        try:
            sql = "SELECT COUNT(*) FROM alert_rule WHERE enabled = 1"
            results = self.db_manager.execute_query(sql)
            return results[0][0] if results else 0
            
        except Exception as e:
            logger.error(f"获取启用规则数量失败: {e}")
            return 0
    
    def get_rules_with_pagination(self, page: int = 1, page_size: int = 10) -> Dict[str, Any]:
        """获取分页的告警规则列表"""
        try:
            offset = (page - 1) * page_size
            
            # 查询总数
            count_sql = "SELECT COUNT(*) FROM alert_rule"
            count_result = self.db_manager.execute_query(count_sql)
            total = count_result[0][0] if count_result else 0
            
            # 查询数据
            data_sql = "SELECT * FROM alert_rule ORDER BY id ASC LIMIT ? OFFSET ?"
            results = self.db_manager.execute_query(data_sql, (page_size, offset))
            
            rules = [self._row_to_alert_rule(row) for row in results]
            
            return {
                "success": True,
                "message": f"查询成功，共 {total} 条记录",
                "data": {
                    "total": total,
                    "list": [rule.to_dict() for rule in rules]
                }
            }
            
        except Exception as e:
            logger.error(f"分页查询失败: {e}")
            return {
                "success": False,
                "message": f"查询失败: {str(e)}",
                "data": {
                    "total": 0,
                    "list": []
                }
            }

    def _row_to_alert_rule(self, row) -> AlertRule:
        """将数据库行转换为AlertRule对象"""
        # SQLite Row对象访问方式
        service_type = row["service_type"] if "service_type" in row.keys() else "comsrv"
        
        return AlertRule(
            id=row["id"],
            service_type=service_type,
            channel_id=row["channel_id"],
            data_type=row["data_type"],
            point_id=row["point_id"],
            rule_name=row["rule_name"],
            warning_level=row["warning_level"],
            operator=row["operator"],
            value=row["value"],
            enabled=bool(row["enabled"]),
            description=row["description"] or "",
            created_at=row["created_at"],  # 直接使用时间戳
            updated_at=row["updated_at"],  # 直接使用时间戳
        )


# 创建全局服务实例
alert_rule_service = AlertRuleService()
