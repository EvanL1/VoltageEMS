"""
告警服务
提供alert表和alert_event表的CRUD操作
"""

import logging
import json
import csv
import io
from datetime import datetime
from typing import List, Optional, Dict, Any

from app.models.alert import Alert, AlertEvent
from app.models.alert_rule import AlertRule
from app.core.database import get_db_manager

logger = logging.getLogger(__name__)


class AlertService:
    """告警服务类"""
    
    def __init__(self):
        self.db_manager = get_db_manager()
    
    # ==================== Alert CRUD ====================
    
    def create_alert(self, rule: AlertRule, current_value: float) -> Optional[int]:
        """创建告警记录"""
        try:
            # 检查是否已存在相同规则的告警
            existing = self.get_alert_by_rule_id(rule.id)
            if existing:
                logger.warning(f"规则ID {rule.id} 的告警已存在，不重复创建")
                return existing.id
            
            now = int(datetime.now().timestamp())
            
            # 创建规则快照
            rule_snapshot = json.dumps({
                "rule_name": rule.rule_name,
                "warning_level": rule.warning_level,
                "operator": rule.operator,
                "value": rule.value,
                "description": rule.description
            })
            
            sql = """
            INSERT INTO alert (
                rule_id, rule_snapshot, service_type, channel_id, data_type, point_id,
                rule_name, warning_level, operator, threshold_value, current_value,
                status, triggered_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """
            
            params = (
                rule.id, rule_snapshot, rule.service_type, rule.channel_id,
                rule.data_type, rule.point_id, rule.rule_name, rule.warning_level,
                rule.operator, rule.value, current_value, "active", now
            )
            
            alert_id = self.db_manager.execute_insert(sql, params)
            logger.info(f"创建告警成功，ID: {alert_id}, 规则: {rule.rule_name}")
            return alert_id
            
        except Exception as e:
            logger.error(f"创建告警失败: {e}")
            return None
    
    def get_alert_by_id(self, alert_id: int) -> Optional[Alert]:
        """根据ID获取告警"""
        try:
            sql = "SELECT * FROM alert WHERE id = ?"
            results = self.db_manager.execute_query(sql, (alert_id,))
            
            if results:
                return self._row_to_alert(results[0])
            return None
            
        except Exception as e:
            logger.error(f"获取告警失败: {e}")
            return None
    
    def get_alert_by_rule_id(self, rule_id: int) -> Optional[Alert]:
        """根据规则ID获取告警"""
        try:
            sql = "SELECT * FROM alert WHERE rule_id = ?"
            results = self.db_manager.execute_query(sql, (rule_id,))
            
            if results:
                return self._row_to_alert(results[0])
            return None
            
        except Exception as e:
            logger.error(f"获取规则告警失败: {e}")
            return None
    
    def get_active_alerts(self, page: int = 1, page_size: int = 10) -> Dict[str, Any]:
        """获取活跃告警列表（分页）"""
        try:
            offset = (page - 1) * page_size
            
            # 查询总数
            count_sql = "SELECT COUNT(*) FROM alert WHERE status = 'active'"
            count_result = self.db_manager.execute_query(count_sql)
            total = count_result[0][0] if count_result else 0
            
            # 查询数据
            data_sql = """
            SELECT * FROM alert WHERE status = 'active' 
            ORDER BY warning_level DESC, triggered_at DESC 
            LIMIT ? OFFSET ?
            """
            results = self.db_manager.execute_query(data_sql, (page_size, offset))
            
            alerts = [self._row_to_alert(row) for row in results]
            
            return {
                "success": True,
                "message": f"查询成功，共找到 {total} 条活跃告警",
                "data": {
                    "total": total,
                    "list": [alert.to_dict() for alert in alerts]
                }
            }
            
        except Exception as e:
            logger.error(f"获取活跃告警失败: {e}")
            return {
                "success": False,
                "message": f"查询失败: {str(e)}",
                "data": {"total": 0, "list": []}
            }
    
    def search_alerts(self, keyword: str = "", warning_level: Optional[int] = None,
                     service_type: str = "", start_time: Optional[datetime] = None,
                     end_time: Optional[datetime] = None, page: int = 1, page_size: int = 10) -> Dict[str, Any]:
        """搜索告警"""
        try:
            conditions = ["status = 'active'"]
            params = []
            
            if keyword:
                conditions.append("(rule_name LIKE ? OR CAST(channel_id AS TEXT) LIKE ? OR CAST(point_id AS TEXT) LIKE ?)")
                keyword_pattern = f"%{keyword}%"
                params.extend([keyword_pattern, keyword_pattern, keyword_pattern])
            
            if service_type:
                conditions.append("service_type = ?")
                params.append(service_type)
            
            if warning_level is not None and warning_level in [1, 2, 3]:
                conditions.append("warning_level = ?")
                params.append(warning_level)
            
            if start_time:
                conditions.append("triggered_at >= ?")
                params.append(int(start_time.timestamp()))
            
            if end_time:
                conditions.append("triggered_at <= ?")
                params.append(int(end_time.timestamp()))
            
            where_clause = " AND ".join(conditions)
            offset = (page - 1) * page_size
            
            # 查询总数
            count_sql = f"SELECT COUNT(*) FROM alert WHERE {where_clause}"
            count_result = self.db_manager.execute_query(count_sql, tuple(params))
            total = count_result[0][0] if count_result else 0
            
            # 查询数据
            data_sql = f"""
            SELECT * FROM alert WHERE {where_clause} 
            ORDER BY warning_level ASC, triggered_at DESC 
            LIMIT ? OFFSET ?
            """
            data_params = list(params) + [page_size, offset]
            results = self.db_manager.execute_query(data_sql, tuple(data_params))
            
            alerts = [self._row_to_alert(row) for row in results]
            
            return {
                "success": True,
                "message": f"查询成功，共找到 {total} 条记录",
                "data": {
                    "total": total,
                    "list": [alert.to_dict() for alert in alerts]
                }
            }
            
        except Exception as e:
            logger.error(f"搜索告警失败: {e}")
            return {
                "success": False,
                "message": f"查询失败: {str(e)}",
                "data": {"total": 0, "list": []}
            }
    
    def get_all_active_alerts(self) -> List[Alert]:
        """获取所有活跃告警（不分页）"""
        try:
            sql = """
            SELECT * FROM alert WHERE status = 'active' 
            ORDER BY warning_level DESC, triggered_at DESC
            """
            results = self.db_manager.execute_query(sql)
            return [self._row_to_alert(row) for row in results]
        except Exception as e:
            logger.error(f"获取所有活跃告警失败: {e}")
            return []
    
    def get_active_alert_count(self) -> int:
        """获取活跃告警数量"""
        try:
            sql = "SELECT COUNT(*) FROM alert WHERE status = 'active'"
            result = self.db_manager.execute_query(sql)
            return result[0][0] if result else 0
        except Exception as e:
            logger.error(f"获取活跃告警数量失败: {e}")
            return 0
    
    def get_active_alert_count_by_level(self) -> Dict[str, int]:
        """获取按等级分类的活跃告警数量"""
        try:
            # 获取总数
            total_count = self.get_active_alert_count()
            
            # 按等级统计
            level_sql = """
            SELECT warning_level, COUNT(*) FROM alert 
            WHERE status = 'active' 
            GROUP BY warning_level
            """
            level_results = self.db_manager.execute_query(level_sql)
            
            # 初始化等级统计，确保所有等级都有值
            level_counts = {"1": 0, "2": 0, "3": 0}
            
            # 填充实际数据
            for row in level_results:
                level_counts[str(row[0])] = row[1]
            
            # 返回包含总数和等级统计的字典
            return {
                "current_alarms": total_count,
                "1": level_counts["1"],
                "2": level_counts["2"], 
                "3": level_counts["3"]
            }
            
        except Exception as e:
            logger.error(f"获取按等级分类告警数量失败: {e}")
            return {
                "current_alarms": 0,
                "1": 0,
                "2": 0,
                "3": 0
            }
    
    def update_alert_value(self, alert_id: int, current_value: float) -> bool:
        """更新告警的当前值"""
        try:
            sql = """
            UPDATE alert SET current_value = ? 
            WHERE id = ?
            """
            affected_rows = self.db_manager.execute_update(sql, (current_value, alert_id))
            return affected_rows > 0
            
        except Exception as e:
            logger.error(f"更新告警值失败: {e}")
            return False
    
    def resolve_alert(self, alert_id: int, recovery_value: float) -> bool:
        """解除告警（移动到历史表）"""
        try:
            # 使用事务确保数据一致性
            with self.db_manager.get_connection() as conn:
                # 获取告警信息
                cursor = conn.execute("SELECT * FROM alert WHERE id = ?", (alert_id,))
                row = cursor.fetchone()
                
                if not row:
                    logger.warning(f"告警ID {alert_id} 不存在")
                    return False
                
                alert = self._row_to_alert(row)
                
                # 创建告警事件记录
                event = AlertEvent.from_alert(alert, "recovery", recovery_value)
                
                # 插入AlertEvent记录
                event_sql = """
                INSERT INTO alert_event (
                    rule_id, rule_snapshot, service_type, channel_id, data_type, point_id,
                    rule_name, warning_level, operator, threshold_value, trigger_value,
                    recovery_value, event_type, triggered_at, recovered_at, duration
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """
                
                event_params = (
                    event.rule_id, event.rule_snapshot, event.service_type, event.channel_id,
                    event.data_type, event.point_id, event.rule_name, event.warning_level,
                    event.operator, event.threshold_value, event.trigger_value, event.recovery_value,
                    event.event_type, event.triggered_at, event.recovered_at, event.duration
                )
                
                event_cursor = conn.execute(event_sql, event_params)
                event_id = event_cursor.lastrowid
                
                if event_id:
                    # 删除告警记录
                    delete_cursor = conn.execute("DELETE FROM alert WHERE id = ?", (alert_id,))
                    if delete_cursor.rowcount > 0:
                        conn.commit()
                        logger.info(f"告警已解除，ID: {alert_id}, 移动到事件表: {event_id}")
                        return True
                    else:
                        logger.error(f"删除告警记录失败: {alert_id}")
                        raise Exception(f"删除告警记录失败: {alert_id}")
                else:
                    logger.error(f"创建告警事件记录失败: {alert_id}")
                    raise Exception(f"创建告警事件记录失败: {alert_id}")
            
        except Exception as e:
            logger.error(f"解除告警失败: {e}")
            return False
    
    def resolve_alerts_by_rule_id(self, rule_id: int) -> List[Alert]:
        """根据规则ID解除所有相关告警（规则被禁用/删除时使用）"""
        resolved_alerts = []
        try:
            # 使用事务确保数据一致性
            with self.db_manager.get_connection() as conn:
                # 获取相关的告警
                cursor = conn.execute("SELECT * FROM alert WHERE rule_id = ?", (rule_id,))
                results = cursor.fetchall()
                
                if not results:
                    logger.info(f"规则ID {rule_id} 没有相关告警需要解除")
                    return resolved_alerts
                
                # 在事务中处理所有告警
                for row in results:
                    alert = self._row_to_alert(row)
                    
                    # 创建事件记录（规则变更触发的解除）
                    event = AlertEvent.from_alert(alert, "recovery", None)
                    
                    # 插入AlertEvent记录
                    event_sql = """
                    INSERT INTO alert_event (
                        rule_id, rule_snapshot, service_type, channel_id, data_type, point_id,
                        rule_name, warning_level, operator, threshold_value, trigger_value,
                        recovery_value, event_type, triggered_at, recovered_at, duration
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """
                    
                    event_params = (
                        event.rule_id, event.rule_snapshot, event.service_type, event.channel_id,
                        event.data_type, event.point_id, event.rule_name, event.warning_level,
                        event.operator, event.threshold_value, event.trigger_value, event.recovery_value,
                        event.event_type, event.triggered_at, event.recovered_at, event.duration
                    )
                    
                    cursor = conn.execute(event_sql, event_params)
                    event_id = cursor.lastrowid
                    
                    if event_id:
                        # 删除告警记录
                        delete_cursor = conn.execute("DELETE FROM alert WHERE id = ?", (alert.id,))
                        if delete_cursor.rowcount > 0:
                            resolved_alerts.append(alert)
                            logger.debug(f"告警已解除: ID={alert.id}, 规则={alert.rule_name}, 事件ID={event_id}")
                        else:
                            logger.warning(f"删除告警记录失败: ID={alert.id}")
                    else:
                        logger.error(f"创建告警事件记录失败: ID={alert.id}")
                        raise Exception(f"创建告警事件记录失败: ID={alert.id}")
                
                # 提交事务
                conn.commit()
                logger.info(f"规则ID {rule_id} 相关的 {len(resolved_alerts)} 条告警已解除")
                return resolved_alerts
            
        except Exception as e:
            logger.error(f"批量解除告警失败: {e}")
            return []
    
    # ==================== AlertEvent CRUD ====================
    
    def create_alert_event(self, event: AlertEvent) -> Optional[int]:
        """创建告警事件记录"""
        try:
            sql = """
            INSERT INTO alert_event (
                rule_id, rule_snapshot, service_type, channel_id, data_type, point_id,
                rule_name, warning_level, operator, threshold_value, trigger_value,
                recovery_value, event_type, triggered_at, recovered_at, duration
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """
            
            params = (
                event.rule_id, event.rule_snapshot, event.service_type, event.channel_id,
                event.data_type, event.point_id, event.rule_name, event.warning_level,
                event.operator, event.threshold_value, event.trigger_value, event.recovery_value,
                event.event_type, event.triggered_at, event.recovered_at, event.duration
            )
            
            event_id = self.db_manager.execute_insert(sql, params)
            logger.info(f"创建告警事件成功，ID: {event_id}")
            return event_id
            
        except Exception as e:
            logger.error(f"创建告警事件失败: {e}")
            return None
    
    def get_alert_events(self, keyword: str = "", warning_level: Optional[int] = None,
                        service_type: str = "", event_type: str = "",
                        start_time: Optional[datetime] = None, end_time: Optional[datetime] = None,
                        page: int = 1, page_size: int = 10) -> Dict[str, Any]:
        """查询告警事件历史"""
        try:
            conditions = []
            params = []
            
            if keyword:
                conditions.append("(rule_name LIKE ? OR CAST(channel_id AS TEXT) LIKE ? OR CAST(point_id AS TEXT) LIKE ?)")
                keyword_pattern = f"%{keyword}%"
                params.extend([keyword_pattern, keyword_pattern, keyword_pattern])
            
            if service_type:
                conditions.append("service_type = ?")
                params.append(service_type)
            
            if warning_level is not None and warning_level in [1, 2, 3]:
                conditions.append("warning_level = ?")
                params.append(warning_level)
            
            if event_type and event_type in ["trigger", "recovery"]:
                conditions.append("event_type = ?")
                params.append(event_type)
            
            if start_time:
                conditions.append("triggered_at >= ?")
                params.append(int(start_time.timestamp()))
            
            if end_time:
                conditions.append("triggered_at <= ?")
                params.append(int(end_time.timestamp()))
            
            where_clause = " AND ".join(conditions) if conditions else "1=1"
            offset = (page - 1) * page_size
            
            # 查询总数
            count_sql = f"SELECT COUNT(*) FROM alert_event WHERE {where_clause}"
            count_result = self.db_manager.execute_query(count_sql, tuple(params))
            total = count_result[0][0] if count_result else 0
            
            # 查询数据
            data_sql = f"""
            SELECT * FROM alert_event WHERE {where_clause} 
            ORDER BY triggered_at DESC 
            LIMIT ? OFFSET ?
            """
            data_params = list(params) + [page_size, offset]
            results = self.db_manager.execute_query(data_sql, tuple(data_params))
            
            events = [self._row_to_alert_event(row) for row in results]
            
            return {
                "success": True,
                "message": f"查询成功，共找到 {total} 条记录",
                "data": {
                    "total": total,
                    "list": [event.to_dict() for event in events]
                }
            }
            
        except Exception as e:
            logger.error(f"查询告警事件失败: {e}")
            return {
                "success": False,
                "message": f"查询失败: {str(e)}",
                "data": {"total": 0, "list": []}
            }
    
    def export_alert_events_csv(self, keyword: str = "", warning_level: Optional[int] = None,
                               service_type: str = "", event_type: str = "",
                               start_time: Optional[datetime] = None, end_time: Optional[datetime] = None) -> str:
        """导出告警事件历史为CSV格式"""
        try:
            conditions = []
            params = []
            
            # 构建查询条件（与get_alert_events相同）
            if keyword:
                conditions.append("(rule_name LIKE ? OR CAST(channel_id AS TEXT) LIKE ? OR CAST(point_id AS TEXT) LIKE ?)")
                keyword_pattern = f"%{keyword}%"
                params.extend([keyword_pattern, keyword_pattern, keyword_pattern])
            
            if service_type:
                conditions.append("service_type = ?")
                params.append(service_type)
            
            if warning_level is not None and warning_level in [1, 2, 3]:
                conditions.append("warning_level = ?")
                params.append(warning_level)
            
            if event_type and event_type in ["trigger", "recovery"]:
                conditions.append("event_type = ?")
                params.append(event_type)
            
            if start_time:
                conditions.append("triggered_at >= ?")
                params.append(int(start_time.timestamp()))
            
            if end_time:
                conditions.append("triggered_at <= ?")
                params.append(int(end_time.timestamp()))
            
            where_clause = " AND ".join(conditions) if conditions else "1=1"
            
            # 查询所有符合条件的数据（无分页限制）
            data_sql = f"""
            SELECT * FROM alert_event WHERE {where_clause} 
            ORDER BY triggered_at DESC
            """
            results = self.db_manager.execute_query(data_sql, tuple(params))
            
            # 创建CSV内容
            output = io.StringIO()
            writer = csv.writer(output)
            
            # 写入表头（英文，除规则名称外）
            headers = [
                'Event ID', 'Rule ID', 'Rule Name', 'Service Type', 'Channel ID', 
                'Data Type', 'Point ID', 'Warning Level', 'Operator', 'Threshold',
                'Trigger Value', 'Recovery Value', 'Event Type', 'Triggered At', 
                'Recovered At', 'Duration (Seconds)'
            ]
            writer.writerow(headers)
            
            # 写入数据行
            for row in results:
                event = self._row_to_alert_event(row)
                
                # 格式化数据（保持原始格式）
                warning_level_value = event.warning_level  # 保持数字 1/2/3
                event_type_value = event.event_type  # 保持英文 trigger/recovery  
                data_type_value = event.data_type  # 保持单字母 T/S/C/A
                
                # 时间格式化
                triggered_at_str = datetime.fromtimestamp(event.triggered_at).strftime("%Y-%m-%d %H:%M:%S") if event.triggered_at else ""
                recovered_at_str = datetime.fromtimestamp(event.recovered_at).strftime("%Y-%m-%d %H:%M:%S") if event.recovered_at else ""
                
                # 持续时间保持秒数
                duration_value = event.duration if event.duration is not None else ""
                
                writer.writerow([
                    event.id,
                    event.rule_id,
                    event.rule_name,  # 保持中文规则名称
                    event.service_type,
                    event.channel_id,
                    data_type_value,  # T/S/C/A
                    event.point_id,
                    warning_level_value,  # 1/2/3
                    event.operator,
                    event.threshold_value,
                    event.trigger_value,
                    event.recovery_value if event.recovery_value is not None else "",
                    event_type_value,  # trigger/recovery
                    triggered_at_str,
                    recovered_at_str,
                    duration_value  # 移除备注列
                ])
            
            csv_content = output.getvalue()
            output.close()
            
            logger.info(f"成功导出 {len(results)} 条告警事件记录")
            return csv_content
            
        except Exception as e:
            logger.error(f"导出告警事件CSV失败: {e}")
            raise e

    def get_alert_statistics(self) -> Dict[str, Any]:
        """获取告警统计信息"""
        try:
            stats = {}
            
            # 当前活跃告警数
            active_sql = "SELECT COUNT(*) FROM alert WHERE status = 'active'"
            active_result = self.db_manager.execute_query(active_sql)
            stats["active_count"] = active_result[0][0] if active_result else 0
            
            # 按级别统计活跃告警
            level_sql = """
            SELECT warning_level, COUNT(*) FROM alert 
            WHERE status = 'active' 
            GROUP BY warning_level
            """
            level_results = self.db_manager.execute_query(level_sql)
            level_stats = {1: 0, 2: 0, 3: 0}
            for row in level_results:
                level_stats[row[0]] = row[1]
            stats["by_level"] = level_stats
            
            # 今日告警事件数
            today_sql = """
            SELECT COUNT(*) FROM alert_event 
            WHERE DATE(created_at) = DATE('now')
            """
            today_result = self.db_manager.execute_query(today_sql)
            stats["today_events"] = today_result[0][0] if today_result else 0
            
            return {
                "success": True,
                "message": "统计数据获取成功",
                "data": stats
            }
            
        except Exception as e:
            logger.error(f"获取统计信息失败: {e}")
            return {
                "success": False,
                "message": f"获取失败: {str(e)}",
                "data": {}
            }
    
    # ==================== Helper Methods ====================
    
    def _row_to_alert(self, row) -> Alert:
        """将数据库行转换为Alert对象"""
        return Alert(
            id=row["id"],
            rule_id=row["rule_id"],
            rule_snapshot=row["rule_snapshot"] or "{}",
            service_type=row["service_type"],
            channel_id=row["channel_id"],
            data_type=row["data_type"],
            point_id=row["point_id"],
            rule_name=row["rule_name"],
            warning_level=row["warning_level"],
            operator=row["operator"],
            threshold_value=row["threshold_value"],
            current_value=row["current_value"],
            status=row["status"],
            triggered_at=row["triggered_at"],  # 直接使用时间戳
        )
    
    def _row_to_alert_event(self, row) -> AlertEvent:
        """将数据库行转换为AlertEvent对象"""
        return AlertEvent(
            id=row["id"],
            rule_id=row["rule_id"],
            rule_snapshot=row["rule_snapshot"] or "{}",
            service_type=row["service_type"],
            channel_id=row["channel_id"],
            data_type=row["data_type"],
            point_id=row["point_id"],
            rule_name=row["rule_name"],
            warning_level=row["warning_level"],
            operator=row["operator"],
            threshold_value=row["threshold_value"],
            trigger_value=row["trigger_value"],
            recovery_value=row["recovery_value"],
            event_type=row["event_type"],
            triggered_at=row["triggered_at"],  # 直接使用时间戳
            recovered_at=row["recovered_at"],  # 直接使用时间戳
            duration=row["duration"],
        )


# 创建全局服务实例
alert_service = AlertService()
