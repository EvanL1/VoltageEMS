"""
WebSocket管理器
处理WebSocket连接管理和消息转发，集成Edge数据功能
"""

import asyncio
import json
import logging
from typing import Dict, Set, Any, Optional, List
from fastapi import WebSocket, WebSocketDisconnect
from datetime import datetime
import time

from app.core.redis_client import RedisClient
from app.core.edge_data_client import EdgeDataClient
from app.models.edge_data import (
    WebSocketMessageType, create_data_update_message, 
    create_alarm_message, create_subscribe_ack_message, create_unsubscribe_ack_message,
    create_control_ack_message, create_error_message, create_pong_message
)
from app.models.response import WebSocketMessage, SafeJSONEncoder

logger = logging.getLogger(__name__)

class ConnectionManager:
    """连接管理器"""
    
    def __init__(self):
        self.active_connections: Dict[str, WebSocket] = {}
        self.connection_info: Dict[str, Dict[str, Any]] = {}
        self.subscriptions: Dict[str, Dict[str, Any]] = {}  # 客户端订阅信息
    
    async def connect(self, websocket: WebSocket, client_id: str, data_type: str = "general"):
        """建立连接"""
        await websocket.accept()
        
        # 如果客户端ID已存在，断开旧连接
        if client_id in self.active_connections:
            try:
                old_websocket = self.active_connections[client_id]
                await old_websocket.close(code=1000, reason="新连接替换")
                logger.warning(f"客户端 {client_id} 的旧连接已被新连接替换")
            except Exception as e:
                logger.debug(f"关闭旧连接时出错: {e}")
        
        self.active_connections[client_id] = websocket
        self.connection_info[client_id] = {
            "websocket": websocket,
            "data_type": data_type,
            "connected_at": datetime.now(),
            "last_activity": datetime.now()
        }
        self.subscriptions[client_id] = {
            "source": "inst",  # 默认数据源
            "channels": [],
            "data_types": [],
            "interval": 1000
        }
        logger.info(f"WebSocket连接建立: {client_id}")
    
    def disconnect(self, client_id: str):
        """断开连接"""
        if client_id in self.active_connections:
            del self.active_connections[client_id]
        if client_id in self.connection_info:
            del self.connection_info[client_id]
        if client_id in self.subscriptions:
            del self.subscriptions[client_id]
        logger.info(f"WebSocket连接断开: {client_id}")
    
    async def send_personal_message(self, message: str, client_id: str):
        """发送个人消息"""
        if client_id in self.active_connections:
            try:
                await self.active_connections[client_id].send_text(message)
                self.connection_info[client_id]["last_activity"] = datetime.now()
            except Exception as e:
                logger.error(f"发送消息到 {client_id} 失败: {e}")
                self.disconnect(client_id)
    
    async def broadcast(self, message: str, data_type: str = "general"):
        """广播消息"""
        disconnected_clients = []
        
        for client_id, info in self.connection_info.items():
            if info["data_type"] == data_type or data_type == "general":
                try:
                    await info["websocket"].send_text(message)
                    info["last_activity"] = datetime.now()
                except Exception as e:
                    logger.error(f"广播消息到 {client_id} 失败: {e}")
                    disconnected_clients.append(client_id)
        
        # 清理断开的连接
        for client_id in disconnected_clients:
            self.disconnect(client_id)
    
    def get_connection_count(self) -> int:
        """获取连接数量"""
        return len(self.active_connections)
    
    def get_connections_info(self) -> Dict[str, Dict[str, Any]]:
        """获取连接信息"""
        return self.connection_info.copy()
    
    def get_subscriptions(self) -> Dict[str, Dict[str, Any]]:
        """获取订阅信息"""
        return self.subscriptions.copy()

class WebSocketManager:
    """WebSocket管理器"""
    
    def __init__(self, redis_client: RedisClient):
        self.redis_client = redis_client
        self.edge_data_client = EdgeDataClient(redis_client.redis_client)
        self.connection_manager = ConnectionManager()
        self.running = False
        self.heartbeat_task = None
    
    async def start(self):
        """启动WebSocket管理器"""
        self.running = True
        # 启动心跳任务
        self.heartbeat_task = asyncio.create_task(self._heartbeat_loop())
        logger.info("WebSocket管理器已启动")
    
    async def stop(self):
        """停止WebSocket管理器"""
        self.running = False
        if self.heartbeat_task:
            self.heartbeat_task.cancel()
            try:
                await self.heartbeat_task
            except asyncio.CancelledError:
                pass
        logger.info("WebSocket管理器已停止")
    
    async def connect_client(self, websocket: WebSocket, client_id: str, data_type: str = "general"):
        """连接客户端"""
        await self.connection_manager.connect(websocket, client_id, data_type)
        
        # 发送欢迎消息
        welcome_message = {
            "type": "connection_established",
            "id": f"welcome_{client_id}",
            "timestamp": int(time.time()),
            "data": {
                "client_id": client_id,
                "data_type": data_type,
                "message": "连接成功，请订阅数据通道以接收实时数据"
            }
        }
        await self.send_message(client_id, welcome_message)
        
        logger.info(f"WebSocket客户端 {client_id} 连接成功，等待订阅")
    
    async def disconnect_client(self, client_id: str):
        """断开客户端"""
        self.connection_manager.disconnect(client_id)
    
    async def send_message(self, client_id: str, message: Any):
        """发送消息到指定客户端"""
        try:
            if isinstance(message, dict):
                message_json = json.dumps(message, ensure_ascii=False, cls=SafeJSONEncoder)
            else:
                # 对于Pydantic模型，使用dict()转换后再序列化
                if hasattr(message, 'dict'):
                    message_dict = message.dict()
                    message_json = json.dumps(message_dict, ensure_ascii=False, cls=SafeJSONEncoder)
                else:
                    message_json = json.dumps(str(message), ensure_ascii=False, cls=SafeJSONEncoder)
            
            await self.connection_manager.send_personal_message(message_json, client_id)
        except Exception as e:
            logger.error(f"序列化消息失败: {e}, message: {type(message)}")
            # 发送简化的错误消息
            fallback_message = json.dumps({
                "type": "error",
                "message": "消息序列化失败",
                "timestamp": int(time.time())
            }, cls=SafeJSONEncoder)
            await self.connection_manager.send_personal_message(fallback_message, client_id)
    
    async def broadcast_message(self, message: Any, data_type: str = "general"):
        """广播消息"""
        try:
            if isinstance(message, dict):
                message_json = json.dumps(message, ensure_ascii=False, cls=SafeJSONEncoder)
            else:
                # 对于Pydantic模型，使用dict()转换后再序列化
                if hasattr(message, 'dict'):
                    message_dict = message.dict()
                    message_json = json.dumps(message_dict, ensure_ascii=False, cls=SafeJSONEncoder)
                else:
                    message_json = json.dumps(str(message), ensure_ascii=False, cls=SafeJSONEncoder)
            
            await self.connection_manager.broadcast(message_json, data_type)
        except Exception as e:
            logger.error(f"广播消息序列化失败: {e}, message: {type(message)}")
            # 发送简化的错误消息
            fallback_message = json.dumps({
                "type": "error",
                "message": "广播消息序列化失败",
                "timestamp": int(time.time())
            }, cls=SafeJSONEncoder)
            await self.connection_manager.broadcast(fallback_message, data_type)
    
    async def handle_client_message(self, client_id: str, message: str):
        """处理客户端消息"""
        try:
            data = json.loads(message)
            message_type = data.get("type", "unknown")
            
            if message_type == "ping":
                # 心跳响应
                start_time = time.time()
                pong_message = create_pong_message(
                    data.get("id", "ping"),
                    int((time.time() - start_time) * 1000)
                )
                await self.send_message(client_id, pong_message)
            
            elif message_type == "subscribe":
                # 订阅数据
                await self._handle_subscribe(client_id, data)
            
            elif message_type == "unsubscribe":
                # 取消订阅
                await self._handle_unsubscribe(client_id, data)
            
            elif message_type == "control":
                # 控制命令
                await self._handle_control(client_id, data)
            
            else:
                logger.info(f"收到未知消息类型: {message_type} 来自 {client_id}")
                
        except json.JSONDecodeError:
            logger.error(f"无效的JSON消息来自 {client_id}: {message}")
            error_msg = create_error_message(
                "INVALID_JSON",
                "无效的JSON格式",
                "消息格式错误",
                None
            )
            await self.send_message(client_id, error_msg)
        except Exception as e:
            logger.error(f"处理客户端消息失败: {e}")
            error_msg = create_error_message(
                "PROCESSING_ERROR",
                "消息处理失败",
                str(e),
                None
            )
            await self.send_message(client_id, error_msg)
    
    async def _handle_subscribe(self, client_id: str, data: Dict[str, Any]):
        """处理订阅请求"""
        try:
            source = data.get("data", {}).get("source", "inst")  # 获取数据源，默认inst

            # 规则监控订阅
            if source == "rule":
                await self._handle_rule_subscribe(client_id, data)
                return

            # 常规数据订阅
            channels = data.get("data", {}).get("channels", [])
            data_types = data.get("data", {}).get("data_types", ["T"])
            interval = data.get("data", {}).get("interval", 1000)

            # 前端完全控制订阅的source/channels/data_types，不做额外验证
            # 数据源可以是任意字符串（如 inst/comsrv/aaasrv 等）

            # 更新订阅信息
            self.connection_manager.subscriptions[client_id] = {
                "source": source,
                "channels": channels,
                "data_types": data_types,
                "interval": interval
            }

            # 发送订阅确认
            ack_message = create_subscribe_ack_message(
                data.get("id", "sub"),
                channels,
                []
            )
            await self.send_message(client_id, ack_message)

            # 立即推送一次数据
            await self._push_initial_data_to_client(client_id, source, channels, data_types)

            # 重置数据调度器的推送时间，避免立即再次推送
            if hasattr(self, 'data_scheduler') and self.data_scheduler:
                self.data_scheduler.reset_client_push_time(client_id)

            logger.info(f"客户端 {client_id} 订阅了数据源 {source}, 通道 {channels}, 数据类型 {data_types}")

        except Exception as e:
            logger.error(f"处理订阅请求失败: {e}")
            error_msg = create_error_message(
                "SUBSCRIPTION_ERROR",
                "订阅失败",
                str(e),
                data.get("id")
            )
            await self.send_message(client_id, error_msg)

    async def _handle_rule_subscribe(self, client_id: str, data: Dict[str, Any]):
        """处理规则监控订阅请求"""
        try:
            rule_id = data.get("data", {}).get("rule_id")
            interval = data.get("data", {}).get("interval", 1000)

            if not rule_id:
                raise ValueError("缺少必要的 rule_id 参数")

            # 存储订阅信息
            self.connection_manager.subscriptions[client_id] = {
                "source": "rule",
                "rule_id": rule_id,
                "interval": interval,
                "variables_cache": None  # 首次获取时填充
            }

            # 发送订阅确认
            ack_message = {
                "type": "subscribe_ack",
                "id": data.get("id", "sub"),
                "timestamp": int(time.time()),
                "data": {
                    "source": "rule",
                    "rule_id": rule_id,
                    "status": "subscribed"
                }
            }
            await self.send_message(client_id, ack_message)

            # 立即推送一次规则监控数据
            await self._push_rule_data_to_client(client_id, rule_id)

            # 重置数据调度器的推送时间
            if hasattr(self, 'data_scheduler') and self.data_scheduler:
                self.data_scheduler.reset_client_push_time(client_id)

            logger.info(f"客户端 {client_id} 订阅了规则监控: {rule_id}, 间隔: {interval}ms")

        except Exception as e:
            logger.error(f"处理规则订阅请求失败: {e}")
            error_msg = create_error_message(
                "SUBSCRIPTION_ERROR",
                "规则订阅失败",
                str(e),
                data.get("id")
            )
            await self.send_message(client_id, error_msg)

    async def _push_rule_data_to_client(self, client_id: str, rule_id: str):
        """推送规则监控数据到客户端"""
        try:
            redis = self.redis_client.redis_client

            # 读取变量当前值: rule:{rule_id}:vars
            vars_key = f"rule:{rule_id}:vars"
            vars_data = await redis.hgetall(vars_key)

            # 转换变量值
            variables = {}
            vars_ts = None
            for key, value in vars_data.items():
                if key == "_ts":
                    vars_ts = value
                else:
                    try:
                        variables[key] = float(value)
                    except (ValueError, TypeError):
                        variables[key] = value

            # 读取执行结果: rule:{rule_id}:exec
            exec_key = f"rule:{rule_id}:exec"
            exec_data = await redis.hgetall(exec_key)

            # 解析执行结果
            last_execution = None
            if exec_data:
                last_execution = {
                    "success": exec_data.get("success", "false") == "true",
                    "timestamp": int(exec_data.get("timestamp", 0)),
                    "error": exec_data.get("error") or None
                }

                # 解析 JSON 字段
                for field in ["execution_path", "variable_values", "node_details"]:
                    if field in exec_data and exec_data[field]:
                        try:
                            last_execution[field] = json.loads(exec_data[field])
                        except json.JSONDecodeError:
                            last_execution[field] = None

            # 构建 rule_monitor 消息
            monitor_message = {
                "type": "rule_monitor",
                "id": f"rule_{rule_id}_{int(time.time())}",
                "timestamp": int(time.time()),
                "data": {
                    "rule_id": rule_id,
                    "variables": variables,
                    "last_execution": last_execution
                }
            }

            await self.send_message(client_id, monitor_message)
            logger.debug(f"已向客户端 {client_id} 推送规则 {rule_id} 的监控数据")

        except Exception as e:
            logger.error(f"推送规则监控数据失败: {e}")
    
    async def _push_initial_data_to_client(self, client_id: str, source: str, channels: List[int], data_types: List[str]):
        """订阅成功后立即推送一次数据"""
        try:
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
                    # 创建初始数据推送消息
                    initial_message = {
                        "type": "data_batch",
                        "id": f"initial_{channel_id}_{int(time.time())}",
                        "timestamp": int(time.time()),
                        "data": {
                            "updates": updates
                        }
                    }
                    
                    # 向客户端推送初始数据
                    await self.send_message(client_id, initial_message)
                    logger.info(f"已向客户端 {client_id} 推送数据源 {source} 通道 {channel_id} 的初始数据，更新数量: {len(updates)}")
                    
        except Exception as e:
            logger.error(f"向客户端 {client_id} 推送初始数据失败: {e}")
    
    async def _handle_unsubscribe(self, client_id: str, data: Dict[str, Any]):
        """处理取消订阅请求"""
        try:
            channels = data.get("data", {}).get("channels", [])
            
            # 更新订阅信息
            unsubscribed_channels = []
            if client_id in self.connection_manager.subscriptions:
                current_channels = self.connection_manager.subscriptions[client_id]["channels"]
                # 记录实际取消订阅的通道
                unsubscribed_channels = [ch for ch in channels if ch in current_channels]
                # 更新订阅列表
                self.connection_manager.subscriptions[client_id]["channels"] = [
                    ch for ch in current_channels if ch not in channels
                ]
            
            # 发送取消订阅确认
            ack_message = create_unsubscribe_ack_message(
                data.get("id", "unsub"),
                unsubscribed_channels,
                []
            )
            await self.send_message(client_id, ack_message)
            
            logger.info(f"客户端 {client_id} 取消订阅了通道 {unsubscribed_channels}")
            
        except Exception as e:
            logger.error(f"处理取消订阅请求失败: {e}")
            error_msg = create_error_message(
                "UNSUBSCRIPTION_ERROR",
                "取消订阅失败",
                str(e),
                data.get("id")
            )
            await self.send_message(client_id, error_msg)
    
    async def _handle_control(self, client_id: str, data: Dict[str, Any]):
        """处理控制命令"""
        try:
            control_data = data.get("data", {})
            source = control_data.get("source", "inst")  # 获取数据源，默认inst
            channel_id = control_data.get("channel_id")
            point_id = control_data.get("point_id")
            command_type = control_data.get("command_type")
            value = control_data.get("value")
            
            if not all([channel_id, point_id, command_type, value]):
                raise ValueError("缺少必要的控制参数")
            
            # 发布控制命令到Redis
            command_data = {
                "point_id": point_id,
                "value": value,
                "source": client_id,
                "command_id": data.get("id", f"cmd_{int(time.time())}"),
                "timestamp": int(time.time())
            }
            
            success = await self.edge_data_client.publish_command(
                channel_id, 
                "C",  # 控制命令类型
                command_data,
                source  # 传递数据源
            )
            
            if success:
                # 发送控制确认
                ack_message = create_control_ack_message(
                    data.get("id", "ctrl"),
                    command_data["command_id"],
                    "executed",
                    True,
                    value
                )
                await self.send_message(client_id, ack_message)
                logger.info(f"控制命令执行成功: {command_data}")
            else:
                raise Exception("发布控制命令失败")
            
        except Exception as e:
            logger.error(f"处理控制命令失败: {e}")
            error_msg = create_error_message(
                "CONTROL_ERROR",
                "控制命令执行失败",
                str(e),
                data.get("id")
            )
            await self.send_message(client_id, error_msg)
    
    async def _heartbeat_loop(self):
        """心跳循环"""
        while self.running:
            try:
                # 发送心跳消息
                heartbeat_message = {
                    "type": "heartbeat",
                    "id": f"heartbeat_{int(time.time())}",
                    "timestamp": int(time.time()),
                    "data": {
                        "server_time": int(time.time())
                    }
                }
                await self.broadcast_message(heartbeat_message)
                
                # 检查连接状态
                await self._check_connections()
                
                # 等待下次心跳
                await asyncio.sleep(30)  # 30秒发送一次心跳
                
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"心跳循环出错: {e}")
                await asyncio.sleep(5)
    
    async def _check_connections(self):
        """检查连接状态"""
        current_time = datetime.now()
        disconnected_clients = []
        
        for client_id, info in self.connection_manager.connection_info.items():
            # 检查最后活动时间，如果超过5分钟没有活动则断开
            last_activity = info["last_activity"]
            if (current_time - last_activity).total_seconds() > 300:  # 5分钟
                disconnected_clients.append(client_id)
        
        # 断开超时的连接
        for client_id in disconnected_clients:
            logger.info(f"断开超时连接: {client_id}")
            self.disconnect_client(client_id)
    
    async def close_all(self):
        """关闭所有连接"""
        for client_id in list(self.connection_manager.active_connections.keys()):
            self.disconnect_client(client_id)
        await self.stop()
    
    def get_status(self) -> Dict[str, Any]:
        """获取状态信息"""
        return {
            "running": self.running,
            "connection_count": self.connection_manager.get_connection_count(),
            "connections_info": self.connection_manager.get_connections_info(),
            "subscriptions": self.connection_manager.get_subscriptions()
        }
    
    async def push_alarm(self, alarm_data: Dict[str, Any]):
        """推送告警消息"""
        try:
            alarm_message = create_alarm_message(
                alarm_data.get("alarm_id", "unknown"),
                alarm_data.get("channel_id", 0),
                alarm_data.get("point_id", 0),
                alarm_data.get("status", 1),
                alarm_data.get("level", 2),
                alarm_data.get("value", 0.0),
                alarm_data.get("message", "未知告警")
            )
            
            await self.broadcast_message(alarm_message)
            logger.info(f"告警消息推送成功: {alarm_data.get('alarm_id')}")
            
        except Exception as e:
            logger.error(f"推送告警消息失败: {e}")
    
    async def broadcast_custom_message(self, custom_data: Dict[str, Any]):
        """广播自定义消息到所有连接的WebSocket客户端（原样转发）"""
        try:
            # 获取所有连接的客户端
            connected_clients = list(self.connection_manager.active_connections.keys())
            
            if connected_clients:
                # 向每个连接的客户端发送原始消息（不包装）
                for client_id in connected_clients:
                    await self.send_message(client_id, custom_data)
                
                logger.info(f"广播消息发送成功，接收客户端数量: {len(connected_clients)}")
                return {
                    "success": True,
                    "message": f"消息已广播到 {len(connected_clients)} 个客户端",
                    "client_count": len(connected_clients),
                    "clients": connected_clients
                }
            else:
                logger.info("没有连接的客户端，跳过广播")
                return {
                    "success": True,
                    "message": "没有连接的客户端",
                    "client_count": 0,
                    "clients": []
                }
                
        except Exception as e:
            logger.error(f"广播消息失败: {e}")
            return {
                "success": False,
                "message": f"广播失败: {str(e)}",
                "client_count": 0,
                "clients": []
            }
