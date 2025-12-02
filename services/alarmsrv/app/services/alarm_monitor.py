"""
告警监控引擎
实现定时从Redis获取数据并根据规则进行告警处理
"""

import asyncio
import logging
import redis
import json
import requests
from datetime import datetime
from typing import Dict, Any, List, Optional
from concurrent.futures import ThreadPoolExecutor

from app.core.config import settings
from app.services.alert_rule_service import alert_rule_service
from app.services.alert_service import alert_service
from app.models.alert_rule import AlertRule

logger = logging.getLogger(__name__)


class AlarmMonitor:
    """告警监控引擎"""
    
    def __init__(self):
        self.redis_client = None
        self.is_running = False
        self.monitor_task = None
        self.alarm_count_task = None  # 告警数量广播任务
        self.executor = ThreadPoolExecutor(max_workers=4)
        self.last_check_time = None
        self.last_alarm_count = 0  # 上次广播的告警数量
        
    def start(self):
        """启动监控"""
        if self.is_running:
            logger.warning("告警监控已在运行")
            return
            
        try:
            # 初始化Redis连接
            self.redis_client = redis.Redis(
                host=settings.REDIS_HOST,
                port=settings.REDIS_PORT,
                db=settings.REDIS_DB,
                password=settings.REDIS_PASSWORD,
                decode_responses=True,
                socket_timeout=5,
                socket_connect_timeout=5,
                health_check_interval=30
            )
            
            # 测试连接
            self.redis_client.ping()
            logger.info(f"Redis连接成功: {settings.REDIS_HOST}:{settings.REDIS_PORT}")
            
            self.is_running = True
            # 启动异步监控任务
            self.monitor_task = asyncio.create_task(self._monitor_loop())
            # 启动告警数量广播任务
            self.alarm_count_task = asyncio.create_task(self._alarm_count_broadcast_loop())
            logger.info("告警监控引擎启动成功")
            
        except Exception as e:
            logger.error(f"告警监控启动失败: {e}")
            self.redis_client = None
    
    async def stop(self):
        """停止监控"""
        if not self.is_running:
            return
            
        self.is_running = False
        
        if self.monitor_task:
            self.monitor_task.cancel()
            try:
                await self.monitor_task
            except asyncio.CancelledError:
                pass
        
        if self.alarm_count_task:
            self.alarm_count_task.cancel()
            try:
                await self.alarm_count_task
            except asyncio.CancelledError:
                pass
        
        if self.redis_client:
            self.redis_client.close()
            
        self.executor.shutdown(wait=True)
        logger.info("告警监控引擎已停止")
    
    async def _monitor_loop(self):
        """主监控循环"""
        logger.info("开始告警监控循环")
        
        while self.is_running:
            try:
                start_time = datetime.now()
                
                # 获取所有启用的规则
                enabled_rules = alert_rule_service.get_all_enabled_rules()
                if not enabled_rules:
                    logger.debug("没有启用的告警规则")
                else:
                    logger.debug(f"监控 {len(enabled_rules)} 条告警规则")
                    
                    # 并发处理规则检查
                    await self._process_rules_concurrent(enabled_rules)
                
                # 记录检查时间
                self.last_check_time = start_time
                processing_time = (datetime.now() - start_time).total_seconds()
                logger.debug(f"告警检查完成，耗时: {processing_time:.2f}秒")
                
                # 等待下一次检查
                await asyncio.sleep(settings.DATA_FETCH_INTERVAL)
                
            except asyncio.CancelledError:
                logger.info("监控循环被取消")
                break
            except Exception as e:
                logger.error(f"监控循环异常: {e}")
                await asyncio.sleep(5)  # 异常时短暂等待
    
    async def _process_rules_concurrent(self, rules: List[AlertRule]):
        """并发处理规则检查"""
        try:
            # 将规则分组以减少Redis连接数
            tasks = []
            for rule in rules:
                task = asyncio.create_task(self._check_single_rule(rule))
                tasks.append(task)
            
            # 等待所有任务完成
            await asyncio.gather(*tasks, return_exceptions=True)
            
        except Exception as e:
            logger.error(f"并发处理规则异常: {e}")
    
    async def _check_single_rule(self, rule: AlertRule):
        """检查单个规则"""
        try:
            # 从Redis获取数据
            current_value = await self._get_redis_value(rule)
            if current_value is None:
                logger.debug(f"规则 {rule.rule_name} 无法获取数据: {rule.redis_key()}")
                return
            
            # 评估规则
            is_triggered = rule.evaluate(current_value)
            
            # 检查当前是否已有告警
            existing_alert = alert_service.get_alert_by_rule_id(rule.id)
            
            if is_triggered:
                if existing_alert:
                    # 已有告警，更新当前值
                    alert_service.update_alert_value(existing_alert.id, current_value)
                    logger.debug(f"更新告警值: {rule.rule_name}, 当前值: {current_value}")
                else:
                    # 新触发告警
                    alert_id = alert_service.create_alert(rule, current_value)
                    if alert_id:
                        logger.warning(f"触发告警: {rule.rule_name}, 当前值: {current_value}, 阈值: {rule.operator} {rule.value}")
                        # 发送告警广播
                        await self._send_alarm_broadcast(alert_id, rule, current_value)
                        # 立即发送告警数量广播
                        await self._send_alarm_count_broadcast()
                        self.last_alarm_count = alert_service.get_active_alert_count()
            else:
                if existing_alert:
                    # 告警恢复
                    if alert_service.resolve_alert(existing_alert.id, current_value):
                        logger.info(f"告警恢复: {rule.rule_name}, 当前值: {current_value}")
                        # 发送恢复广播
                        await self._send_alarm_recovery_broadcast(existing_alert.id, rule, current_value)
                        # 立即发送告警数量广播
                        await self._send_alarm_count_broadcast()
                        self.last_alarm_count = alert_service.get_active_alert_count()
                
        except Exception as e:
            logger.error(f"检查规则失败 {rule.rule_name}: {e}")
    
    async def _get_redis_value(self, rule: AlertRule) -> Optional[float]:
        """从Redis获取数据值"""
        try:
            redis_key = rule.redis_key()
            point_field = str(rule.point_id)
            
            # 使用线程池执行Redis操作
            loop = asyncio.get_event_loop()
            value_str = await loop.run_in_executor(
                self.executor,
                self._redis_hget,
                redis_key,
                point_field
            )
            
            if value_str is not None:
                try:
                    return float(value_str)
                except (ValueError, TypeError):
                    logger.warning(f"无效的数值格式: {redis_key}:{point_field} = {value_str}")
                    return None
            else:
                logger.debug(f"Redis键不存在或字段为空: {redis_key}:{point_field}")
                return None
                
        except Exception as e:
            logger.error(f"从Redis获取数据失败: {e}")
            return None
    
    def _redis_hget(self, key: str, field: str) -> Optional[str]:
        """Redis HGET操作（同步）"""
        try:
            if self.redis_client:
                return self.redis_client.hget(key, field)
            return None
        except Exception as e:
            logger.error(f"Redis操作失败: {e}")
            return None
    
    async def manual_check_rule(self, rule_id: int) -> Dict[str, Any]:
        """手动检查指定规则"""
        try:
            rule = alert_rule_service.get_rule_by_id(rule_id)
            if not rule:
                return {
                    "success": False,
                    "message": "规则不存在",
                    "data": {}
                }
            
            if not rule.enabled:
                return {
                    "success": False,
                    "message": "规则已禁用",
                    "data": {}
                }
            
            # 获取当前值
            current_value = await self._get_redis_value(rule)
            if current_value is None:
                return {
                    "success": False,
                    "message": "无法从Redis获取数据",
                    "data": {
                        "redis_key": rule.redis_key(),
                        "point_id": rule.point_id
                    }
                }
            
            # 评估规则
            is_triggered = rule.evaluate(current_value)
            existing_alert = alert_service.get_alert_by_rule_id(rule.id)
            
            return {
                "success": True,
                "message": "手动检查完成",
                "data": {
                    "rule_name": rule.rule_name,
                    "current_value": current_value,
                    "threshold_value": rule.value,
                    "operator": rule.operator,
                    "is_triggered": is_triggered,
                    "has_active_alert": existing_alert is not None,
                    "redis_key": rule.redis_key(),
                    "check_time": datetime.now().isoformat()
                }
            }
            
        except Exception as e:
            logger.error(f"手动检查规则失败: {e}")
            return {
                "success": False,
                "message": f"检查失败: {str(e)}",
                "data": {}
            }
    
    async def on_rule_updated(self, rule_id: int):
        """规则更新时的回调处理"""
        try:
            rule = alert_rule_service.get_rule_by_id(rule_id)
            if not rule:
                return
            
            existing_alert = alert_service.get_alert_by_rule_id(rule_id)
            
            if not rule.enabled and existing_alert:
                # 规则被禁用，解除现有告警并发送恢复广播
                resolved_alerts = alert_service.resolve_alerts_by_rule_id(rule_id)
                logger.info(f"规则被禁用，解除相关告警: {rule.rule_name}")
                
                # 为每个解除的告警发送恢复广播
                for alert in resolved_alerts:
                    await self._send_alarm_recovery_broadcast(
                        alert.id, rule, None, reason="规则被禁用"
                    )
            
            logger.info(f"规则更新处理完成: {rule.rule_name}")
            
        except Exception as e:
            logger.error(f"处理规则更新失败: {e}")
    
    async def on_rule_deleted(self, rule_id: int):
        """规则删除时的回调处理"""
        try:
            # 先获取规则信息（删除前）
            rule = alert_rule_service.get_rule_by_id(rule_id)
            if not rule:
                logger.warning(f"规则ID {rule_id} 不存在")
                return
                
            # 解除该规则的所有告警
            resolved_alerts = alert_service.resolve_alerts_by_rule_id(rule_id)
            logger.info(f"规则删除，解除了 {len(resolved_alerts)} 条相关告警")
            
            # 为每个解除的告警发送恢复广播
            for alert in resolved_alerts:
                await self._send_alarm_recovery_broadcast(
                    alert.id, rule, None, reason="规则被删除"
                )
            
            # 如果有告警被解除，发送告警数量广播
            if resolved_alerts:
                await self._send_alarm_count_broadcast()
                self.last_alarm_count = alert_service.get_active_alert_count()
            
        except Exception as e:
            logger.error(f"处理规则删除失败: {e}")
    
    async def _send_alarm_broadcast(self, alert_id: int, rule: AlertRule, current_value: float):
        """发送告警广播消息到6005端口"""
        try:
            # 构建广播消息
            broadcast_data = {
                "type": "alarm",
                "id": f"alarm_{alert_id:03d}",
                "timestamp": int(datetime.now().timestamp()),
                "data": {
                    "alarm_id": str(alert_id),
                    "service_type": rule.service_type,
                    "source": rule.service_type,  # 服务类型
                    "device": str(rule.channel_id),  # 设备标识（通道ID）
                    "channel_id": rule.channel_id,
                    "data_type": rule.data_type,  # 支持自定义数据类型
                    "point_id": rule.point_id,
                    "status": 1,  # 1表示触发状态
                    "level": rule.warning_level,
                    "value": current_value,
                    "message": f"{rule.rule_name}: {current_value} {rule.operator} {rule.value}"
                }
            }
            
            # 多端点广播 - 同时向6005和6006端口发送
            broadcast_urls = [
                "http://localhost:6005/api/v1/broadcast",
                "http://localhost:6006/netApi/alarm/broadcast"
            ]
            loop = asyncio.get_event_loop()
            
            def send_request(url):
                try:
                    response = requests.post(
                        url,
                        json=broadcast_data,
                        timeout=3  # 3秒超时
                    )
                    return url, response.status_code == 200, response.text
                except Exception as e:
                    return url, False, str(e)
            
            # 并行发送到多个端点
            tasks = []
            for url in broadcast_urls:
                task = loop.run_in_executor(self.executor, send_request, url)
                tasks.append(task)
            
            # 等待所有广播完成
            results = await asyncio.gather(*tasks, return_exceptions=True)
            
            success_count = 0
            for result in results:
                if isinstance(result, Exception):
                    logger.error(f"告警广播异常: 规则={rule.rule_name}, 告警ID={alert_id}, 异常={result}")
                else:
                    url, success, response_text = result
                    if success:
                        success_count += 1
                        logger.info(f"告警广播发送成功: 规则={rule.rule_name}, 告警ID={alert_id}, 端点={url}")
                    else:
                        logger.warning(f"告警广播发送失败: 规则={rule.rule_name}, 告警ID={alert_id}, 端点={url}, 错误={response_text}")
            
            logger.info(f"告警广播完成: 规则={rule.rule_name}, 告警ID={alert_id}, 成功={success_count}/{len(broadcast_urls)}")
                
        except Exception as e:
            logger.error(f"发送告警广播异常: 规则={rule.rule_name}, 告警ID={alert_id}, 异常={e}")
    
    async def _send_alarm_recovery_broadcast(self, alert_id: int, rule: AlertRule, recovery_value: Optional[float], reason: str = "条件恢复"):
        """发送告警恢复广播消息到6005端口"""
        try:
            # 构建恢复消息
            if recovery_value is not None:
                message = f"{rule.rule_name}已恢复: {recovery_value} (不再满足 {rule.operator} {rule.value})"
                value = recovery_value
            else:
                message = f"{rule.rule_name}已恢复: {reason}"
                value = 0.0  # 规则删除/禁用时使用默认值
            
            # 构建恢复广播消息
            broadcast_data = {
                "type": "alarm",
                "id": f"alarm_{alert_id:03d}_recovery",
                "timestamp": int(datetime.now().timestamp()),
                "data": {
                    "alarm_id": str(alert_id),
                    "service_type": rule.service_type,
                    "source": rule.service_type,  # 服务类型
                    "device": str(rule.channel_id),  # 设备标识（通道ID）
                    "channel_id": rule.channel_id,
                    "data_type": rule.data_type,  # 支持自定义数据类型
                    "point_id": rule.point_id,
                    "status": 0,  # 0表示恢复状态
                    "level": rule.warning_level,
                    "value": value,
                    "message": message
                }
            }
            
            # 多端点广播 - 同时向6005和6006端口发送
            broadcast_urls = [
                "http://localhost:6005/api/v1/broadcast",
                "http://localhost:6006/netApi/alarm/broadcast"
            ]
            loop = asyncio.get_event_loop()
            
            def send_request(url):
                try:
                    response = requests.post(
                        url,
                        json=broadcast_data,
                        timeout=3  # 3秒超时
                    )
                    return url, response.status_code == 200, response.text
                except Exception as e:
                    return url, False, str(e)
            
            # 并行发送到多个端点
            tasks = []
            for url in broadcast_urls:
                task = loop.run_in_executor(self.executor, send_request, url)
                tasks.append(task)
            
            # 等待所有广播完成
            results = await asyncio.gather(*tasks, return_exceptions=True)
            
            success_count = 0
            for result in results:
                if isinstance(result, Exception):
                    logger.error(f"告警恢复广播异常: 规则={rule.rule_name}, 告警ID={alert_id}, 异常={result}")
                else:
                    url, success, response_text = result
                    if success:
                        success_count += 1
                        logger.info(f"告警恢复广播发送成功: 规则={rule.rule_name}, 告警ID={alert_id}, 端点={url}")
                    else:
                        logger.warning(f"告警恢复广播发送失败: 规则={rule.rule_name}, 告警ID={alert_id}, 端点={url}, 错误={response_text}")
            
            logger.info(f"告警恢复广播完成: 规则={rule.rule_name}, 告警ID={alert_id}, 成功={success_count}/{len(broadcast_urls)}")
                
        except Exception as e:
            logger.error(f"发送告警恢复广播异常: 规则={rule.rule_name}, 告警ID={alert_id}, 异常={e}")

    def get_monitor_status(self) -> Dict[str, Any]:
        """获取监控状态"""
        try:
            redis_status = "disconnected"
            if self.redis_client:
                try:
                    self.redis_client.ping()
                    redis_status = "connected"
                except:
                    redis_status = "error"
            
            return {
                "running": self.is_running,
                "redis_status": redis_status,
                "last_check_time": self.last_check_time.isoformat() if self.last_check_time else None,
                "check_interval": settings.DATA_FETCH_INTERVAL,
                "redis_config": {
                    "host": settings.REDIS_HOST,
                    "port": settings.REDIS_PORT,
                    "db": settings.REDIS_DB
                }
            }
            
        except Exception as e:
            logger.error(f"获取监控状态失败: {e}")
            return {
                "running": False,
                "redis_status": "error",
                "error": str(e)
            }
    
    async def _alarm_count_broadcast_loop(self):
        """告警数量广播循环"""
        logger.info("开始告警数量广播循环")
        
        while self.is_running:
            try:
                # 定时发送广播（不管数量是否变化）
                await self._send_alarm_count_broadcast()
                
                # 获取当前告警数量用于日志记录
                current_count = alert_service.get_active_alert_count()
                
                # 记录数量变化日志
                if current_count != self.last_alarm_count:
                    logger.info(f"告警数量变化: {self.last_alarm_count} -> {current_count}")
                else:
                    logger.debug(f"定时广播告警数量: {current_count}")
                
                self.last_alarm_count = current_count
                
                # 每30秒发送一次广播
                await asyncio.sleep(30)
                
            except asyncio.CancelledError:
                logger.info("告警数量广播循环被取消")
                break
            except Exception as e:
                logger.error(f"告警数量广播循环异常: {e}")
                await asyncio.sleep(10)  # 异常时短暂等待
    
    async def _send_alarm_count_broadcast(self):
        """发送告警数量广播消息到6005端口"""
        try:
            # 获取按等级分类的告警数量
            level_counts = alert_service.get_active_alert_count_by_level()
            
            # 构建广播消息
            broadcast_data = {
                "type": "alarm_num",
                "id": f"alarm_num_{int(datetime.now().timestamp())}",
                "timestamp": int(datetime.now().timestamp()),
                "data": {
                    "current_alarms": level_counts["current_alarms"],
                    "1": level_counts["1"],
                    "2": level_counts["2"],
                    "3": level_counts["3"],
                    "update_time": int(datetime.now().timestamp()),
                    "server_id": "alarmsrv"
                }
            }
            
            # 多端点广播 - 同时向6005和6006端口发送
            broadcast_urls = [
                "http://localhost:6005/api/v1/broadcast",
                "http://localhost:6006/netApi/alarm/broadcast"
            ]
            loop = asyncio.get_event_loop()
            
            def send_request(url):
                try:
                    response = requests.post(
                        url,
                        json=broadcast_data,
                        timeout=3  # 3秒超时
                    )
                    return url, response.status_code == 200, response.text
                except Exception as e:
                    return url, False, str(e)
            
            # 并行发送到多个端点
            tasks = []
            for url in broadcast_urls:
                task = loop.run_in_executor(self.executor, send_request, url)
                tasks.append(task)
            
            # 等待所有广播完成
            results = await asyncio.gather(*tasks, return_exceptions=True)
            
            success_count = 0
            for result in results:
                if isinstance(result, Exception):
                    logger.error(f"告警数量广播异常: 异常={result}")
                else:
                    url, success, response_text = result
                    if success:
                        success_count += 1
                        logger.debug(f"告警数量广播发送成功: 端点={url}")
                    else:
                        logger.warning(f"告警数量广播发送失败: 端点={url}, 错误={response_text}")
            
            logger.debug(f"告警数量广播完成: 成功={success_count}/{len(broadcast_urls)}")
                
        except Exception as e:
            logger.error(f"发送告警数量广播异常: 异常={e}")
    
    async def broadcast_current_alarms(self) -> Dict[str, Any]:
        """手动广播当前所有告警到6006端口"""
        try:
            # 获取所有当前活动的告警
            active_alerts = alert_service.get_all_active_alerts()
            
            if not active_alerts:
                logger.info("当前没有活动告警，发送告警数量广播")
                await self._send_alarm_count_broadcast()
                return {
                    "success": True,
                    "message": "当前没有活动告警",
                    "data": {
                        "broadcast_count": 0,
                        "alarm_count": 0
                    }
                }
            
            logger.info(f"开始广播 {len(active_alerts)} 条当前告警")
            
            # 为每个告警构建并发送广播
            broadcast_tasks = []
            for alert in active_alerts:
                # 获取对应的规则
                rule = alert_rule_service.get_rule_by_id(alert.rule_id)
                if rule:
                    # 构建广播数据
                    broadcast_data = {
                        "type": "alarm",
                        "id": f"alarm_{alert.id:03d}",
                        "timestamp": int(datetime.now().timestamp()),
                        "data": {
                            "alarm_id": str(alert.id),
                            "service_type": rule.service_type,
                            "source": rule.service_type,
                            "device": str(rule.channel_id),
                            "channel_id": rule.channel_id,
                            "data_type": rule.data_type,
                            "point_id": rule.point_id,
                            "status": 1,  # 1表示触发状态
                            "level": rule.warning_level,
                            "value": alert.current_value,
                            "message": f"{rule.rule_name}: {alert.current_value} {rule.operator} {rule.value}"
                        }
                    }
                    
                    # 创建广播任务
                    task = self._send_single_broadcast(broadcast_data, alert.id, rule.rule_name)
                    broadcast_tasks.append(task)
            
            # 并发发送所有广播
            results = await asyncio.gather(*broadcast_tasks, return_exceptions=True)
            
            # 统计成功数量
            success_count = sum(1 for r in results if r and not isinstance(r, Exception))
            
            # 同时发送告警数量广播
            await self._send_alarm_count_broadcast()
            
            logger.info(f"告警广播完成: 总数={len(active_alerts)}, 成功={success_count}")
            
            return {
                "success": True,
                "message": f"成功广播 {success_count}/{len(active_alerts)} 条告警",
                "data": {
                    "broadcast_count": success_count,
                    "alarm_count": len(active_alerts),
                    "failed_count": len(active_alerts) - success_count
                }
            }
            
        except Exception as e:
            logger.error(f"广播当前告警失败: {e}")
            return {
                "success": False,
                "message": f"广播失败: {str(e)}",
                "data": {}
            }
    
    async def _send_single_broadcast(self, broadcast_data: Dict[str, Any], alert_id: int, rule_name: str) -> bool:
        """发送单个广播消息到6006端口"""
        try:
            # 只发送到6006端口
            broadcast_url = "http://localhost:6006/netApi/alarm/broadcast"
            loop = asyncio.get_event_loop()
            
            def send_request():
                try:
                    response = requests.post(
                        broadcast_url,
                        json=broadcast_data,
                        timeout=3  # 3秒超时
                    )
                    return response.status_code == 200, response.text
                except Exception as e:
                    return False, str(e)
            
            # 发送请求
            success, response_text = await loop.run_in_executor(self.executor, send_request)
            
            if success:
                logger.debug(f"告警广播发送成功: 规则={rule_name}, 告警ID={alert_id}")
            else:
                logger.warning(f"告警广播发送失败: 规则={rule_name}, 告警ID={alert_id}, 错误={response_text}")
            
            return success
            
        except Exception as e:
            logger.error(f"发送单个广播异常: 规则={rule_name}, 告警ID={alert_id}, 异常={e}")
            return False


# 创建全局监控实例
alarm_monitor = AlarmMonitor()
