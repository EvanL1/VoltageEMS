"""
告警总召服务模块
处理MQTT告警总召请求
"""

import json
import time
import aiohttp
from typing import Dict, Any
from loguru import logger
from app.core.mqtt_client import mqtt_client
from app.core.device_identity import device_identity
from app.core.config_loader import config_loader

class AlarmCaller:
    """告警总召服务"""
    
    def __init__(self):
        self.call_alarm_topic = None
        self.call_alarm_reply_topic = None
        self.alarm_api_url = "http://localhost:6007/alarmApi/call-data"
        
    def setup_topics(self):
        """设置MQTT主题"""
        try:
            # 获取主题配置
            self.call_alarm_topic = config_loader.get_config('mqtt_topics.call_alarm')
            self.call_alarm_reply_topic = config_loader.get_config('mqtt_topics.call_alarm_reply')
            
            if not self.call_alarm_topic or not self.call_alarm_reply_topic:
                logger.error("告警总召主题配置缺失")
                return False
            
            # 格式化主题，替换占位符
            formatted_call_alarm_topic = device_identity.format_topic(self.call_alarm_topic)
            formatted_call_alarm_reply_topic = device_identity.format_topic(self.call_alarm_reply_topic)
            
            # 保存格式化后的主题
            self.call_alarm_topic = formatted_call_alarm_topic
            self.call_alarm_reply_topic = formatted_call_alarm_reply_topic
            
            # 订阅告警总召主题
            mqtt_client.subscribe(self.call_alarm_topic)
            
            # 添加消息处理器
            mqtt_client.add_message_handler(self.call_alarm_topic, self._handle_call_alarm_request)
            
            logger.info(f"告警总召服务已启动，监听主题: {self.call_alarm_topic}")
            logger.info(f"回复主题: {self.call_alarm_reply_topic}")
            return True
            
        except Exception as e:
            logger.error(f"设置告警总召主题失败: {e}")
            return False
    
    def _handle_call_alarm_request(self, topic: str, payload: str):
        """处理告警总召请求"""
        try:
            logger.info(f"收到告警总召请求: {payload}")
            
            # 解析请求数据，提取 msgId
            msg_id = ""
            try:
                request_data = json.loads(payload)
                msg_id = request_data.get('msgId', '')
                logger.debug(f"告警总召请求 msgId: {msg_id}")
            except json.JSONDecodeError:
                logger.warning(f"告警总召请求 JSON 解析失败，使用空 msgId")
                msg_id = ""
            
            # 处理总召请求（使用同步方式调用异步方法）
            import asyncio
            try:
                # 获取当前事件循环
                loop = asyncio.get_event_loop()
                if loop.is_running():
                    # 如果事件循环正在运行，创建任务
                    loop.create_task(self._process_call_alarm_request(msg_id))
                else:
                    # 如果事件循环没有运行，直接运行
                    asyncio.run(self._process_call_alarm_request(msg_id))
            except RuntimeError:
                # 如果没有事件循环，创建一个新的
                asyncio.run(self._process_call_alarm_request(msg_id))
            
        except Exception as e:
            logger.error(f"处理告警总召请求异常: {e}")
            # 发送错误回复
            self._send_error_reply(str(e), "")
    
    async def _process_call_alarm_request(self, msg_id: str = ""):
        """处理告警总召请求"""
        try:
            logger.info(f"开始处理告警总召请求, msgId: {msg_id}")
            
            # 1. 向告警API发送POST请求
            try:
                async with aiohttp.ClientSession() as session:
                    async with session.post(
                        self.alarm_api_url,
                        json={"msgId": msg_id, "timestamp": int(time.time())},
                        timeout=aiohttp.ClientTimeout(total=10)
                    ) as response:
                        if response.status == 200:
                            logger.info(f"告警API调用成功, msgId: {msg_id}, status: {response.status}")
                            api_result = "success"
                            api_message = "告警数据请求成功"
                        else:
                            logger.warning(f"告警API调用返回非200状态, msgId: {msg_id}, status: {response.status}")
                            api_result = "warning"
                            api_message = f"告警API返回状态码: {response.status}"
            except aiohttp.ClientError as e:
                logger.error(f"告警API调用失败, msgId: {msg_id}, error: {e}")
                api_result = "fail"
                api_message = f"告警API调用失败: {str(e)}"
            except asyncio.TimeoutError:
                logger.error(f"告警API调用超时, msgId: {msg_id}")
                api_result = "fail"
                api_message = "告警API调用超时"
            
            # 2. 发送回复消息
            reply_message = {
                "result": api_result,
                "message": api_message,
                "timestamp": int(time.time()),
                "msgId": msg_id
            }
            
            if mqtt_client.publish(self.call_alarm_reply_topic, reply_message, qos=1):
                logger.info(f"告警总召回复发送成功, msgId: {msg_id}")
            else:
                logger.error(f"告警总召回复发送失败, msgId: {msg_id}")
            
            logger.info("告警总召处理完成")
            
        except Exception as e:
            logger.error(f"处理告警总召请求异常: {e}")
            self._send_error_reply(str(e), msg_id)
    
    def _send_error_reply(self, error_message: str, msg_id: str = ""):
        """发送错误回复"""
        try:
            reply_message = {
                "result": "fail",
                "error": "general_error",
                "message": f"处理告警总召请求时发生错误: {error_message}",
                "timestamp": int(time.time()),
                "msgId": msg_id
            }
            
            if mqtt_client.publish(self.call_alarm_reply_topic, reply_message, qos=1):
                logger.info(f"错误回复发送成功, msgId: {msg_id}")
            else:
                logger.error(f"错误回复发送失败, msgId: {msg_id}")
                
        except Exception as e:
            logger.error(f"发送错误回复异常: {e}")

# 全局告警总召服务实例
alarm_caller = AlarmCaller()

