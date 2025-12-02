"""
单点读取服务模块
处理MQTT单点读取请求
"""

import json
import time
from typing import Dict, Any, Optional
from loguru import logger
from app.core.mqtt_client import mqtt_client
from app.core.database import redis_manager
from app.core.device_identity import device_identity
from app.core.config_loader import config_loader

class PointReader:
    """单点读取服务"""
    
    def __init__(self):
        self.read_topic = None
        self.reply_topic = None
        
    def setup_topics(self):
        """设置MQTT主题"""
        try:
            # 获取主题配置
            self.read_topic = config_loader.get_config('mqtt_topics.read')
            self.reply_topic = config_loader.get_config('mqtt_topics.read_reply')
            
            if not self.read_topic or not self.reply_topic:
                logger.error("单点读取主题配置缺失")
                return False
            
            # 格式化主题，替换占位符
            formatted_read_topic = device_identity.format_topic(self.read_topic)
            formatted_reply_topic = device_identity.format_topic(self.reply_topic)
            
            # 保存格式化后的主题
            self.read_topic = formatted_read_topic
            self.reply_topic = formatted_reply_topic
            
            # 订阅读取主题
            mqtt_client.subscribe(self.read_topic)
            
            # 添加消息处理器
            mqtt_client.add_message_handler(self.read_topic, self._handle_read_request)
            
            logger.info(f"单点读取服务已启动，监听主题: {self.read_topic}")
            logger.info(f"回复主题: {self.reply_topic}")
            return True
            
        except Exception as e:
            logger.error(f"设置单点读取主题失败: {e}")
            return False
    
    def _handle_read_request(self, topic: str, payload: str):
        """处理单点读取请求"""
        try:
            # 解析请求数据
            request_data = json.loads(payload)
            logger.debug(f"收到单点读取请求: {request_data}")
            
            # 验证请求格式
            if not self._validate_read_request(request_data):
                logger.warning(f"单点读取请求格式错误: {request_data}")
                # 即使验证失败，也要发送失败回复
                self._send_validation_failure_reply(request_data)
                return
            
            # 处理读取请求（使用同步方式调用异步方法）
            import asyncio
            try:
                # 获取当前事件循环
                loop = asyncio.get_event_loop()
                if loop.is_running():
                    # 如果事件循环正在运行，创建任务
                    loop.create_task(self._process_read_request(request_data))
                else:
                    # 如果事件循环没有运行，直接运行
                    asyncio.run(self._process_read_request(request_data))
            except RuntimeError:
                # 如果没有事件循环，创建一个新的
                asyncio.run(self._process_read_request(request_data))
            
        except json.JSONDecodeError:
            logger.error(f"单点读取请求JSON解析失败: {payload}")
            # JSON解析失败时也要发送回复
            self._send_json_error_reply(payload)
        except Exception as e:
            logger.error(f"处理单点读取请求异常: {e}")
            # 其他异常时也要发送回复
            self._send_general_error_reply(str(e))
    
    def _validate_read_request(self, request_data: Dict[str, Any]) -> bool:
        """验证读取请求格式"""
        required_fields = ['source', 'device', 'data_type', 'key', 'msgId']
        
        for field in required_fields:
            if field not in request_data:
                logger.warning(f"单点读取请求缺少字段: {field}")
                return False
        
        return True
    
    async def _process_read_request(self, request_data: Dict[str, Any]):
        """处理读取请求"""
        try:
            source = request_data['source']
            device = request_data['device']
            data_type = request_data['data_type']
            key = request_data['key']
            msg_id = request_data['msgId']
            
            logger.info(f"处理单点读取: source={source}, device={device}, data_type={data_type}, key={key}")
            
            # 从Redis获取数据
            redis_data = await self._get_redis_data(source, device, data_type, key)
            
            if redis_data is not None:
                # 构建回复消息
                reply_message = self._build_reply_message(source, device, data_type, key, redis_data, msg_id)
                
                # 发送回复
                if mqtt_client.publish(self.reply_topic, reply_message, qos=1):
                    logger.info(f"单点读取回复发送成功: {source}:{device}:{key}")
                else:
                    logger.error(f"单点读取回复发送失败: {source}:{device}:{key}")
            else:
                logger.warning(f"Redis中未找到数据: {source}:{device}:{key}")
                
        except Exception as e:
            logger.error(f"处理单点读取请求异常: {e}")
    
    async def _get_redis_data(self, source: str, device: str, data_type: str, key: str) -> Optional[Any]:
        """从Redis获取数据"""
        try:
            redis_client = redis_manager.get_client()
            if not redis_client:
                logger.warning("Redis连接不可用")
                return None
            
            # 将device字段的下划线转换为空格，构建Redis键
            # 例如: Diesel_Generator1 -> Diesel Generator1
            device_with_spaces = device.replace('_', ' ')
            
            # 构建Redis键: source:device:data_type
            # 例如: modsrv:Diesel Generator1:T
            redis_key = f"{source}:{device_with_spaces}:{data_type}"
            
            logger.debug(f"查找Redis键: {redis_key}")
            
            # 检查键是否存在
            if redis_client.exists(redis_key):
                # 获取键的类型
                key_type = redis_client.type(redis_key)
                
                if key_type == 'hash':
                    # 如果是hash类型，获取指定key的值
                    value = redis_client.hget(redis_key, key)
                    if value is not None:
                        # 转换数字字符串
                        try:
                            float_value = float(value)
                            if float_value.is_integer():
                                return {key: int(float_value)}
                            else:
                                return {key: float_value}
                        except (ValueError, TypeError):
                            return {key: value}
                
                elif key_type == 'string':
                    # 如果是string类型，直接返回
                    value = redis_client.get(redis_key)
                    if value is not None:
                        return {key: value}
                
                elif key_type == 'list':
                    # 如果是list类型，获取指定索引的值
                    try:
                        index = int(key)
                        value = redis_client.lindex(redis_key, index)
                        if value is not None:
                            return {key: value}
                    except (ValueError, IndexError):
                        pass
                
                elif key_type == 'set':
                    # 如果是set类型，检查成员是否存在
                    if redis_client.sismember(redis_key, key):
                        return {key: "exists"}
            else:
                logger.debug(f"Redis键不存在: {redis_key}")
            
            return None
            
        except Exception as e:
            logger.error(f"从Redis获取数据失败: {e}")
            return None
    
    def _build_reply_message(self, source: str, device: str, data_type: str, key: str, 
                           value: Any, msg_id: str) -> Dict[str, Any]:
        """构建回复消息"""
        current_timestamp = int(time.time())
        
        return {
            "timestamp": current_timestamp,
            "property": [
                {
                    "source": source,
                    "device": device,
                    "data_type": data_type,
                    "value": value
                }
            ],
            "msgId": msg_id
        }

    def _send_validation_failure_reply(self, request_data: Dict[str, Any]):
        """发送验证失败回复"""
        try:
            msg_id = request_data.get('msgId', 'unknown')
            reply_message = {
                "result": "fail",
                "error": "validation_failed",
                "message": "请求格式验证失败，缺少必要字段",
                "msgId": msg_id,
                "timestamp": int(time.time())
            }
            
            if mqtt_client.publish(self.reply_topic, reply_message, qos=1):
                logger.info(f"验证失败回复发送成功: {msg_id}")
            else:
                logger.error(f"验证失败回复发送失败: {msg_id}")
                
        except Exception as e:
            logger.error(f"发送验证失败回复异常: {e}")
    
    def _send_json_error_reply(self, payload: str):
        """发送JSON解析错误回复"""
        try:
            reply_message = {
                "result": "fail",
                "error": "json_parse_error",
                "message": "JSON格式解析失败",
                "msgId": "unknown",
                "timestamp": int(time.time())
            }
            
            if mqtt_client.publish(self.reply_topic, reply_message, qos=1):
                logger.info("JSON错误回复发送成功")
            else:
                logger.error("JSON错误回复发送失败")
                
        except Exception as e:
            logger.error(f"发送JSON错误回复异常: {e}")
    
    def _send_general_error_reply(self, error_message: str):
        """发送通用错误回复"""
        try:
            reply_message = {
                "result": "fail",
                "error": "general_error",
                "message": f"处理请求时发生错误: {error_message}",
                "msgId": "unknown",
                "timestamp": int(time.time())
            }
            
            if mqtt_client.publish(self.reply_topic, reply_message, qos=1):
                logger.info("通用错误回复发送成功")
            else:
                logger.error("通用错误回复发送失败")
                
        except Exception as e:
            logger.error(f"发送通用错误回复异常: {e}")

# 全局单点读取服务实例
point_reader = PointReader()
