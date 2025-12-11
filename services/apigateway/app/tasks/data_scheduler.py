"""
数据调度器
定时从Redis获取Edge数据并转发到WebSocket
"""

import asyncio
import json
import logging
from datetime import datetime
from typing import Dict, List, Any, Optional
import schedule
import time

from app.core.redis_client import RedisClient
from app.websocket.websocket_manager import WebSocketManager
from app.core.config import settings
from app.models.response import WebSocketMessage
from app.core.edge_data_client import EdgeDataClient

logger = logging.getLogger(__name__)

class DataScheduler:
    """数据调度器"""
    
    def __init__(self, redis_client: RedisClient, websocket_manager: WebSocketManager):
        self.redis_client = redis_client
        self.websocket_manager = websocket_manager
        self.edge_data_client = EdgeDataClient(redis_client.redis_client)
        self.running = False
        self.scheduler_task = None
        
    async def start(self):
        """启动数据调度器"""
        self.running = True
        self.scheduler_task = asyncio.create_task(self._scheduler_loop())
        logger.info("数据调度器已启动")
    
    async def stop(self):
        """停止数据调度器"""
        self.running = False
        if self.scheduler_task:
            self.scheduler_task.cancel()
            try:
                await self.scheduler_task
            except asyncio.CancelledError:
                pass
        logger.info("数据调度器已停止")
    
    async def _scheduler_loop(self):
        """调度器主循环"""
        while self.running:
            try:
                # 检查是否有订阅的客户端
                if not self.websocket_manager:
                    await asyncio.sleep(1)
                    continue
                
                # 获取所有订阅信息
                subscriptions = self.websocket_manager.connection_manager.get_subscriptions()
                
                if not subscriptions:
                    # 没有订阅的客户端时，等待更长时间
                    await asyncio.sleep(5)
                    continue
                
                # 为每个订阅的客户端处理数据推送
                await self._process_subscriptions(subscriptions)
                
                # 使用较短的检查间隔，以便及时响应新的订阅
                await asyncio.sleep(1)
                
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"数据调度器循环出错: {e}")
                await asyncio.sleep(5)
    
    async def _process_subscriptions(self, subscriptions: dict):
        """处理所有订阅的数据推送"""
        try:
            current_time = time.time()

            for client_id, subscription in subscriptions.items():
                # 检查客户端是否仍然连接
                if client_id not in self.websocket_manager.connection_manager.active_connections:
                    continue

                # 获取数据源类型
                source = subscription.get("source", "inst")

                # 获取客户端的推送间隔设置
                interval = subscription.get("interval", 5000)  # 默认5秒
                interval_seconds = interval / 1000.0  # 转换为秒

                # 检查是否到了推送时间
                last_push_key = f"last_push_{client_id}"
                last_push_time = getattr(self, last_push_key, 0)

                if current_time - last_push_time >= interval_seconds:
                    # 根据数据源类型选择推送方法
                    if source == "rule":
                        await self._push_rule_data_to_client(client_id, subscription)
                    else:
                        await self._push_data_to_client(client_id, subscription)
                    # 更新最后推送时间
                    setattr(self, last_push_key, current_time)

        except Exception as e:
            logger.error(f"处理订阅数据推送失败: {e}")
    
    def reset_client_push_time(self, client_id: str):
        """重置客户端的推送时间（用于初始推送后）"""
        last_push_key = f"last_push_{client_id}"
        setattr(self, last_push_key, time.time())
        logger.debug(f"重置客户端 {client_id} 的推送时间")
    
    async def _push_data_to_client(self, client_id: str, subscription: dict):
        """向特定客户端推送数据"""
        try:
            source = subscription.get("source", "inst")  # 获取数据源，默认inst
            channels = subscription.get("channels", [])
            data_types = subscription.get("data_types", ["T"])
            
            if not channels:
                return
            
            # 为每个通道获取数据
            for channel_id in channels:
                updates = []
                
                # 获取各种类型的数据
                for data_type_str in data_types:
                    try:
                        # 直接使用字符串，不转换为枚举，支持任意数据类型
                        data = await self.edge_data_client.get_data(channel_id, data_type_str, source)
                        
                        if data:
                            updates.append({
                                "source": source,  # 添加source字段
                                "channel_id": channel_id,
                                "data_type": data_type_str,
                                "values": data
                            })
                    except Exception as e:
                        logger.warning(f"获取数据类型 {data_type_str} 失败: {e}")
                        continue
                
                if updates:
                    # 创建批量数据更新消息
                    batch_message = {
                        "type": "data_batch",
                        "id": f"batch_{channel_id}_{int(time.time())}",
                        "timestamp": int(time.time()),
                        "data": {
                            "updates": updates
                        }
                    }
                    
                    # 向客户端推送数据
                    await self.websocket_manager.send_message(client_id, batch_message)
                    
                    logger.debug(f"已向客户端 {client_id} 推送数据源 {source} 通道 {channel_id} 的数据，更新数量: {len(updates)}")

        except Exception as e:
            logger.error(f"向客户端 {client_id} 推送数据失败: {e}")

    async def _push_rule_data_to_client(self, client_id: str, subscription: dict):
        """推送规则监控数据到客户端（简化版）

        从 rule:{rule_id}:exec 的 node_details 中提取每个节点的变量值。
        消息格式：
        {
            "type": "data_batch",
            "data": {
                "rule_id": 1,
                "execution_path": ["start", "node-xxx", "end"],
                "node_variables": {
                    "node-xxx": {"X1": 0.0},
                    "node-yyy": {"X1": 999, "X2": 42290}
                }
            }
        }
        """
        try:
            rule_id = subscription.get("rule_id")
            if not rule_id:
                logger.warning(f"客户端 {client_id} 订阅了规则但未指定 rule_id")
                return

            redis = self.redis_client.redis_client

            # 只读取执行结果: rule:{rule_id}:exec
            exec_key = f"rule:{rule_id}:exec"
            exec_data = await redis.hgetall(exec_key)

            if not exec_data:
                return

            # 提取执行路径和节点变量
            raw_path = []
            if "execution_path" in exec_data:
                try:
                    raw_path = json.loads(exec_data["execution_path"])
                except json.JSONDecodeError:
                    pass

            # 从 node_details 提取每个节点的变量
            node_variables = {}
            if "node_details" in exec_data:
                try:
                    details = json.loads(exec_data["node_details"])
                    for node_id, detail in details.items():
                        if "input_values" in detail:
                            node_variables[node_id] = detail["input_values"]
                except json.JSONDecodeError:
                    pass

            # 构建带变量的执行路径
            execution_path = []
            for node_id in raw_path:
                node = {"id": node_id}
                if node_id in node_variables:
                    node["variables"] = node_variables[node_id]
                execution_path.append(node)

            # 简化后的消息
            monitor_message = {
                "type": "data_batch",
                "timestamp": int(time.time()),
                "data": {
                    "rule_id": rule_id,
                    "execution_path": execution_path
                }
            }

            await self.websocket_manager.send_message(client_id, monitor_message)
            logger.debug(f"已向客户端 {client_id} 推送规则 {rule_id} 的监控数据")

        except Exception as e:
            logger.error(f"推送规则监控数据失败: {e}")
    
    async def _fetch_and_broadcast_edge_data(self):
        """获取并广播Edge数据"""
        try:
            # 获取所有通道
            channels = await self.edge_data_client.get_all_channels()
            
            if not channels:
                logger.debug("没有可用的通道数据")
                return
            
            # 为每个通道获取数据并推送
            for channel_id in channels:
                await self._process_channel_data(channel_id)
                
        except Exception as e:
            logger.error(f"获取并广播Edge数据失败: {e}")
    
    async def _process_channel_data(self, channel_id: int, source: str = "inst"):
        """处理单个通道的数据"""
        try:
            updates = []
            
            # 获取各种类型的数据
            for data_type in DataType:
                data = await self.edge_data_client.get_data(channel_id, data_type, source)
                
                if data:
                    updates.append({
                        "source": source,  # 添加source字段
                        "channel_id": channel_id,
                        "data_type": data_type.value,
                        "values": data
                    })
            
            if updates:
                # 创建批量数据更新消息
                batch_message = {
                    "type": "data_batch",
                    "id": f"batch_{channel_id}_{int(time.time())}",
                    "timestamp": int(time.time()),
                    "data": {
                        "updates": updates
                    }
                }
                
                # 只向订阅了该通道的客户端推送数据
                await self._push_to_subscribed_clients(channel_id, source, batch_message)
                
                logger.debug(f"已处理数据源 {source} 通道 {channel_id} 的数据，更新数量: {len(updates)}")
            else:
                logger.debug(f"数据源 {source} 通道 {channel_id} 没有新数据")
                
        except Exception as e:
            logger.error(f"处理数据源 {source} 通道 {channel_id} 数据失败: {e}")
    
    async def _push_to_subscribed_clients(self, channel_id: int, source: str, message: dict):
        """向订阅了特定通道和数据源的客户端推送数据"""
        try:
            if not self.websocket_manager:
                return
            
            # 获取所有订阅信息
            subscriptions = self.websocket_manager.connection_manager.get_subscriptions()
            
            # 统计推送的客户端数量
            pushed_count = 0
            
            for client_id, subscription in subscriptions.items():
                # 检查客户端是否订阅了该数据源和通道
                client_source = subscription.get("source", "inst")
                if client_source == source and channel_id in subscription.get("channels", []):
                    try:
                        # 向订阅的客户端推送数据
                        await self.websocket_manager.send_message(client_id, message)
                        pushed_count += 1
                    except Exception as e:
                        logger.error(f"向客户端 {client_id} 推送数据失败: {e}")
            
            if pushed_count > 0:
                logger.info(f"已向 {pushed_count} 个订阅客户端推送数据源 {source} 通道 {channel_id} 的数据")
            else:
                logger.debug(f"数据源 {source} 通道 {channel_id} 没有订阅的客户端")
                
        except Exception as e:
            logger.error(f"推送数据到订阅客户端失败: {e}")
    
    async def fetch_specific_data(self, data_type: str) -> Optional[List[Dict[str, Any]]]:
        """获取指定类型的数据（兼容性方法）"""
        try:
            # 从Redis获取数据
            data = await self.redis_client.get_data_for_websocket(data_type)
            return data
        except Exception as e:
            logger.error(f"获取 {data_type} 数据失败: {e}")
            return None
    
    async def get_data_summary(self) -> Dict[str, Any]:
        """获取数据摘要"""
        try:
            summary = {
                "total_channels": 0,
                "data_types": list(DataType.__members__.keys()),
                "last_update": int(time.time())
            }
            
            # 获取通道数量
            channels = await self.edge_data_client.get_all_channels()
            summary["total_channels"] = len(channels)
            
            return summary
            
        except Exception as e:
            logger.error(f"获取数据摘要失败: {e}")
            return {"error": str(e)}
    
    async def broadcast_custom_message(self, message_type: str, data: Any, data_type: str = "general"):
        """广播自定义消息"""
        try:
            message = WebSocketMessage(
                type=message_type,
                data=data
            )
            
            await self.websocket_manager.broadcast_message(message, data_type)
            logger.info(f"已广播自定义消息: {message_type}")
            
        except Exception as e:
            logger.error(f"广播自定义消息失败: {e}")
    
    async def get_scheduler_status(self) -> Dict[str, Any]:
        """获取调度器状态"""
        return {
            "running": self.running,
            "data_types": list(DataType.__members__.keys()),
            "fetch_interval": settings.DATA_FETCH_INTERVAL,
            "last_run": int(time.time())
        }
    
    def add_data_type(self, data_type: str):
        """添加新的数据类型"""
        if data_type not in DataType.__members__:
            logger.warning(f"尝试添加不支持的数据类型: {data_type}")
        else:
            logger.info(f"添加新的数据类型: {data_type}")
    
    def remove_data_type(self, data_type: str):
        """移除数据类型"""
        if data_type in DataType.__members__:
            logger.info(f"移除数据类型: {data_type}")
        else:
            logger.warning(f"尝试移除不支持的数据类型: {data_type}")
    
    async def manual_trigger(self, data_type: str = None):
        """手动触发数据获取"""
        try:
            if data_type:
                # 手动触发时，data_type 是具体的类型，如 "sensor", "metrics" 等
                # 需要转换为 DataType 枚举
                if data_type in DataType.__members__:
                    await self._process_channel_data(0) # 假设手动触发时 channel_id 为 0
                    logger.info(f"手动触发数据获取: {data_type}")
                else:
                    logger.warning(f"手动触发不支持的数据类型: {data_type}")
            else:
                # 手动触发所有数据获取
                await self._fetch_and_broadcast_edge_data()
                logger.info("手动触发所有数据获取")
        except Exception as e:
            logger.error(f"手动触发数据获取失败: {e}")
