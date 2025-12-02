"""
单点写入服务模块
处理MQTT单点写入请求
"""

import json
import time
from typing import Dict, Any, Optional
from loguru import logger
from app.core.mqtt_client import mqtt_client
from app.core.database import redis_manager
from app.core.device_identity import device_identity
from app.core.config_loader import config_loader

class PointWriter:
    """单点写入服务"""
    
    def __init__(self):
        self.write_topic = None
        self.reply_topic = None
        
    def setup_topics(self):
        """设置MQTT主题"""
        try:
            # 获取主题配置
            self.write_topic = config_loader.get_config('mqtt_topics.write')
            self.reply_topic = config_loader.get_config('mqtt_topics.write_reply')
            
            if not self.write_topic or not self.reply_topic:
                logger.error("单点写入主题配置缺失")
                return False
            
            # 格式化主题，替换占位符
            formatted_write_topic = device_identity.format_topic(self.write_topic)
            formatted_reply_topic = device_identity.format_topic(self.reply_topic)
            
            # 保存格式化后的主题
            self.write_topic = formatted_write_topic
            self.reply_topic = formatted_reply_topic
            
            # 订阅写入主题
            mqtt_client.subscribe(self.write_topic)
            
            # 添加消息处理器
            mqtt_client.add_message_handler(self.write_topic, self._handle_write_request)
            
            logger.info(f"单点写入服务已启动，监听主题: {self.write_topic}")
            logger.info(f"回复主题: {self.reply_topic}")
            return True
            
        except Exception as e:
            logger.error(f"设置单点写入主题失败: {e}")
            return False
    
    def _handle_write_request(self, topic: str, payload: str):
        """处理单点写入请求"""
        try:
            # 解析请求数据
            request_data = json.loads(payload)
            logger.debug(f"收到单点写入请求: {request_data}")
            
            # 验证请求格式
            if not self._validate_write_request(request_data):
                logger.warning(f"单点写入请求格式错误: {request_data}")
                # 即使验证失败，也要发送失败回复
                self._send_validation_failure_reply(request_data)
                return
            
            # 处理写入请求（使用同步方式调用异步方法）
            import asyncio
            try:
                # 获取当前事件循环
                loop = asyncio.get_event_loop()
                if loop.is_running():
                    # 如果事件循环正在运行，创建任务
                    loop.create_task(self._process_write_request(request_data))
                else:
                    # 如果事件循环没有运行，直接运行
                    asyncio.run(self._process_write_request(request_data))
            except RuntimeError:
                # 如果没有事件循环，创建一个新的
                asyncio.run(self._process_write_request(request_data))
            
        except json.JSONDecodeError:
            logger.error(f"单点写入请求JSON解析失败: {payload}")
            # JSON解析失败时也要发送回复
            self._send_json_error_reply(payload)
        except Exception as e:
            logger.error(f"处理单点写入请求异常: {e}")
            # 其他异常时也要发送回复
            self._send_general_error_reply(str(e))
    
    def _validate_write_request(self, request_data: Dict[str, Any]) -> bool:
        """验证写入请求格式"""
        required_fields = ['source', 'device', 'data_type', 'key', 'value', 'msgId']
        
        for field in required_fields:
            if field not in request_data:
                logger.warning(f"单点写入请求缺少字段: {field}")
                return False
        
        return True
    
    async def _process_write_request(self, request_data: Dict[str, Any]):
        """处理写入请求"""
        try:
            source = request_data['source']
            device = request_data['device']
            data_type = request_data['data_type']
            key = request_data['key']
            value = request_data['value']
            msg_id = request_data['msgId']
            
            logger.info(f"处理单点写入: source={source}, device={device}, data_type={data_type}, key={key}, value={value}")
            
            # 写入Redis数据
            write_success = await self._write_redis_data(source, device, data_type, key, value)
            
            if write_success:
                # 构建成功回复消息
                reply_message = self._build_success_reply(msg_id)
                
                # 发送回复
                if mqtt_client.publish(self.reply_topic, reply_message, qos=1):
                    logger.info(f"单点写入回复发送成功: {source}:{device}:{key}")
                else:
                    logger.error(f"单点写入回复发送失败: {source}:{device}:{key}")
            else:
                # 构建失败回复消息
                reply_message = self._build_failure_reply(msg_id)
                
                # 发送回复
                if mqtt_client.publish(self.reply_topic, reply_message, qos=1):
                    logger.info(f"单点写入失败回复发送成功: {source}:{device}:{key}")
                else:
                    logger.error(f"单点写入失败回复发送失败: {source}:{device}:{key}")
                
        except Exception as e:
            logger.error(f"处理单点写入请求异常: {e}")
    
    async def _write_redis_data(self, source: str, device: str, data_type: str, key: str, value: Any) -> bool:
        """写入Redis数据"""
        try:
            redis_client = redis_manager.get_client()
            if not redis_client:
                logger.warning("Redis连接不可用")
                return False
            
            # 将device字段的下划线转换为空格，构建Redis键
            # 例如: Diesel_Generator1 -> Diesel Generator1
            device_with_spaces = device.replace('_', ' ')
            
            # 构建Redis键: source:device:data_type
            # 例如: modsrv:Diesel Generator1:T
            redis_key = f"{source}:{device_with_spaces}:{data_type}"
            
            logger.debug(f"写入Redis键: {redis_key}, key: {key}, value: {value}")
            
            # 检查键是否存在
            if redis_client.exists(redis_key):
                # 获取键的类型
                key_type = redis_client.type(redis_key)
                
                if key_type == 'hash':
                    # 如果是hash类型，写入指定key的值
                    result = redis_client.hset(redis_key, key, value)
                    if result is not None:
                        logger.info(f"Hash写入成功: {redis_key}:{key} = {value}")
                        return True
                    else:
                        logger.error(f"Hash写入失败: {redis_key}:{key}")
                        return False
                
                elif key_type == 'string':
                    # 如果是string类型，直接覆盖整个值
                    result = redis_client.set(redis_key, value)
                    if result:
                        logger.info(f"String写入成功: {redis_key} = {value}")
                        return True
                    else:
                        logger.error(f"String写入失败: {redis_key}")
                        return False
                
                elif key_type == 'list':
                    # 如果是list类型，设置指定索引的值
                    try:
                        index = int(key)
                        result = redis_client.lset(redis_key, index, value)
                        if result:
                            logger.info(f"List写入成功: {redis_key}[{index}] = {value}")
                            return True
                        else:
                            logger.error(f"List写入失败: {redis_key}[{index}]")
                            return False
                    except (ValueError, IndexError) as e:
                        logger.error(f"List索引错误: {e}")
                        return False
                
                elif key_type == 'set':
                    # 如果是set类型，添加成员
                    result = redis_client.sadd(redis_key, value)
                    if result is not None:
                        logger.info(f"Set写入成功: {redis_key} 添加 {value}")
                        return True
                    else:
                        logger.error(f"Set写入失败: {redis_key}")
                        return False
                
                else:
                    logger.warning(f"不支持的Redis数据类型: {key_type}")
                    return False
            else:
                # 如果键不存在，创建为hash类型并写入
                result = redis_client.hset(redis_key, key, value)
                if result is not None:
                    logger.info(f"新建Hash键并写入成功: {redis_key}:{key} = {value}")
                    return True
                else:
                    logger.error(f"新建Hash键写入失败: {redis_key}:{key}")
                    return False
            
        except Exception as e:
            logger.error(f"写入Redis数据失败: {e}")
            return False
    
    def _build_success_reply(self, msg_id: str) -> Dict[str, Any]:
        """构建成功回复消息"""
        return {
            "result": "success",
            "msgId": msg_id
        }
    
    def _build_failure_reply(self, msg_id: str) -> Dict[str, Any]:
        """构建失败回复消息"""
        return {
            "result": "fail",
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

# 全局单点写入服务实例
point_writer = PointWriter()
