"""
数据总召服务模块
处理MQTT数据总召请求
"""

import json
import time
from typing import Dict, Any
from loguru import logger
from app.core.mqtt_client import mqtt_client
from app.core.device_identity import device_identity
from app.core.config_loader import config_loader

class DataCaller:
    """数据总召服务"""
    
    def __init__(self):
        self.call_data_topic = None
        self.call_data_reply_topic = None
        self.data_forwarder = None  # 将在setup时注入
        
    def setup_topics(self, data_forwarder):
        """设置MQTT主题"""
        try:
            # 保存data_forwarder引用
            self.data_forwarder = data_forwarder
            
            # 获取主题配置
            self.call_data_topic = config_loader.get_config('mqtt_topics.call_data')
            self.call_data_reply_topic = config_loader.get_config('mqtt_topics.call_data_reply')
            
            if not self.call_data_topic or not self.call_data_reply_topic:
                logger.error("数据总召主题配置缺失")
                return False
            
            # 格式化主题，替换占位符
            formatted_call_data_topic = device_identity.format_topic(self.call_data_topic)
            formatted_call_data_reply_topic = device_identity.format_topic(self.call_data_reply_topic)
            
            # 保存格式化后的主题
            self.call_data_topic = formatted_call_data_topic
            self.call_data_reply_topic = formatted_call_data_reply_topic
            
            # 订阅数据总召主题
            mqtt_client.subscribe(self.call_data_topic)
            
            # 添加消息处理器
            mqtt_client.add_message_handler(self.call_data_topic, self._handle_call_data_request)
            
            logger.info(f"数据总召服务已启动，监听主题: {self.call_data_topic}")
            logger.info(f"回复主题: {self.call_data_reply_topic}")
            return True
            
        except Exception as e:
            logger.error(f"设置数据总召主题失败: {e}")
            return False
    
    def _handle_call_data_request(self, topic: str, payload: str):
        """处理数据总召请求"""
        try:
            logger.info(f"收到数据总召请求: {payload}")
            
            # 解析请求数据，提取 msgId
            msg_id = ""
            try:
                request_data = json.loads(payload)
                msg_id = request_data.get('msgId', '')
                logger.debug(f"总召请求 msgId: {msg_id}")
            except json.JSONDecodeError:
                logger.warning(f"总召请求 JSON 解析失败，使用空 msgId")
                msg_id = ""
            
            # 处理总召请求（使用同步方式调用异步方法）
            import asyncio
            try:
                # 获取当前事件循环
                loop = asyncio.get_event_loop()
                if loop.is_running():
                    # 如果事件循环正在运行，创建任务
                    loop.create_task(self._process_call_data_request(msg_id))
                else:
                    # 如果事件循环没有运行，直接运行
                    asyncio.run(self._process_call_data_request(msg_id))
            except RuntimeError:
                # 如果没有事件循环，创建一个新的
                asyncio.run(self._process_call_data_request(msg_id))
            
        except Exception as e:
            logger.error(f"处理数据总召请求异常: {e}")
            # 发送错误回复
            self._send_error_reply(str(e), "")
    
    async def _process_call_data_request(self, msg_id: str = ""):
        """处理数据总召请求"""
        try:
            logger.info(f"开始处理数据总召请求, msgId: {msg_id}")
            
            # 1. 发送回复消息，确认收到总召请求
            reply_message = {
                "result": "success",
                "message": "数据总召已启动",
                "timestamp": int(time.time()),
                "msgId": msg_id
            }
            
            if mqtt_client.publish(self.call_data_reply_topic, reply_message, qos=1):
                logger.info(f"数据总召回复发送成功, msgId: {msg_id}")
            else:
                logger.error(f"数据总召回复发送失败, msgId: {msg_id}")
            
            # 2. 触发 data_forwarder 的数据上送逻辑
            if self.data_forwarder:
                logger.info("触发数据转发器进行数据总召上送")
                await self.data_forwarder._forward_data()
                logger.info("数据总召处理完成")
            else:
                logger.error("data_forwarder 未初始化，无法执行数据总召")
            
        except Exception as e:
            logger.error(f"处理数据总召请求异常: {e}")
    
    def _send_error_reply(self, error_message: str, msg_id: str = ""):
        """发送错误回复"""
        try:
            reply_message = {
                "result": "fail",
                "error": "general_error",
                "message": f"处理数据总召请求时发生错误: {error_message}",
                "timestamp": int(time.time()),
                "msgId": msg_id
            }
            
            if mqtt_client.publish(self.call_data_reply_topic, reply_message, qos=1):
                logger.info(f"错误回复发送成功, msgId: {msg_id}")
            else:
                logger.error(f"错误回复发送失败, msgId: {msg_id}")
                
        except Exception as e:
            logger.error(f"发送错误回复异常: {e}")

# 全局数据总召服务实例
data_caller = DataCaller()

