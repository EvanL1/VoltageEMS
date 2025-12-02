"""
数据转发服务模块
"""

import json
import asyncio
import time
from typing import Dict, List, Any, Optional
from datetime import datetime
from collections import defaultdict
from loguru import logger
from app.core.database import redis_manager
from app.core.mqtt_client import mqtt_client
from app.core.config_loader import config_loader
from app.core.device_identity import device_identity
from app.services.system_monitor import system_monitor

class DataForwarder:
    """数据转发服务"""
    
    def __init__(self):
        self.is_running = False
        self.forward_task: Optional[asyncio.Task] = None
        self.last_forward_time = 0
        self.last_system_monitor_time = 0
        # 添加MQTT发送失败计数器和延迟控制
        self.mqtt_failure_count = 0
        self.last_failure_time = 0
        self.failure_delay_base = 1  # 基础延迟时间（秒）
        
        # AWS IoT Core 限速配置
        self.max_messages_per_second = 10  # 每秒最大消息数
        self.message_queue = []  # 消息队列
        self.last_send_time = 0  # 上次发送时间
        self.send_interval = 1.0 / self.max_messages_per_second  # 发送间隔
        
    async def start(self):
        """启动数据转发服务"""
        if self.is_running:
            logger.warning("数据转发服务已在运行")
            return
        
        logger.info("启动数据转发服务...")
        self.is_running = True
        
        # 确保MQTT连接正常
        if not mqtt_client.is_connected:
            logger.info("MQTT未连接，数据转发服务将等待MQTT连接建立")
            # 不再强制要求MQTT连接，让数据转发器可以独立启动
            # MQTT的在线消息现在由MQTT客户端的连接回调自动处理
        
        # 启动转发任务
        self.forward_task = asyncio.create_task(self._forward_loop())
        logger.info("数据转发服务启动成功")
    
    async def _rate_limited_send(self, topic: str, payload: str, qos: int = 0):
        """限速发送消息，适应AWS IoT Core"""
        try:
            current_time = time.time()
            
            # 检查发送间隔
            if current_time - self.last_send_time < self.send_interval:
                # 添加到队列，稍后发送
                self.message_queue.append((topic, payload, qos))
                logger.debug(f"消息加入队列（限速），队列长度: {len(self.message_queue)}")
                # 不要直接 return，继续处理队列
            else:
                # 直接发送
                await self._send_message(topic, payload, qos)
                self.last_send_time = current_time
            
            # 处理队列中的消息（无论是否直接发送都要处理队列）
            if self.message_queue:
                await self._process_message_queue()
                
        except Exception as e:
            logger.error(f"限速发送失败: {e}")
    
    async def _process_message_queue(self):
        """处理消息队列"""
        try:
            if not self.message_queue:
                return
                
            current_time = time.time()
            
            # 计算可以发送的消息数量
            time_since_last_send = current_time - self.last_send_time
            # 至少发送一条消息，避免队列堆积
            max_send_count = max(1, int(time_since_last_send * self.max_messages_per_second))
            
            # 发送队列中的消息
            sent_count = 0
            while self.message_queue and sent_count < max_send_count:
                topic, payload, qos = self.message_queue.pop(0)
                
                # 控制发送间隔
                if sent_count > 0:
                    await asyncio.sleep(self.send_interval)
                
                await self._send_message(topic, payload, qos)
                sent_count += 1
            
            if sent_count > 0:
                self.last_send_time = time.time()
                logger.debug(f"队列处理完成，发送了 {sent_count} 条消息，剩余队列: {len(self.message_queue)}")
                
        except Exception as e:
            logger.error(f"处理消息队列失败: {e}")
    
    async def _send_message(self, topic: str, payload: str, qos: int = 0):
        """发送单条消息"""
        try:
            if not mqtt_client.is_connected:
                logger.debug(f"MQTT未连接，跳过消息发送: {topic}")
                return False
            
            # 使用MQTT客户端发送消息
            result = mqtt_client.client.publish(topic, payload, qos=qos)
            
            if result.rc == 0:  # MQTT_ERR_SUCCESS
                logger.debug(f"消息发送成功: {topic}")
                return True
            else:
                logger.warning(f"消息发送失败: {topic}, 错误码: {result.rc}")
                return False
                
        except Exception as e:
            logger.error(f"发送消息异常: {e}")
            return False
    
    async def stop(self):
        """停止数据转发服务"""
        if not self.is_running:
            return
        
        logger.info("停止数据转发服务...")
        self.is_running = False
        
        # 发送设备下线消息（优雅停机时发送）
        await self._send_device_offline("graceful_shutdown")
        
        if self.forward_task:
            self.forward_task.cancel()
            try:
                await self.forward_task
            except asyncio.CancelledError:
                pass
        
        logger.info("数据转发服务已停止")
    
    async def _send_device_offline(self, reason: str = "graceful_shutdown"):
        """发送设备下线消息（用于优雅停机）"""
        try:
            # 获取状态主题
            status_topic = device_identity.format_topic(
                config_loader.get_config('mqtt_topics.status', 'status/{productSN}/{deviceSN}')
            )
            
            # 构建下线消息
            offline_message = {
                "type": "offline",
                "gateway": "",
                "reason": reason,
                "timestamp": int(time.time())
            }
            
            # 发送下线消息（确保立即传输）
            await self._rate_limited_send(status_topic, json.dumps(offline_message, ensure_ascii=False), qos=1)
            logger.info(f"设备下线消息发送成功: {status_topic} (原因: {reason})")
            # 额外强制网络处理，确保下线消息立即发送
            try:
                for i in range(20):
                    mqtt_client.client.loop_write()
                    time.sleep(0.01)
                # 额外等待确保传输完成
                time.sleep(0.5)
            except:
                pass
                
        except Exception as e:
            logger.error(f"发送设备下线消息失败: {e}")
    
    async def _forward_loop(self):
        """数据转发主循环"""
        while self.is_running:
            try:
                current_time = time.time()
                
                # 检查是否需要转发数据
                interval = config_loader.get_config('data_report.interval', 5)
                if current_time - self.last_forward_time >= interval:
                    await self._forward_data()
                    self.last_forward_time = current_time
                
                # 等待下次检查
                await asyncio.sleep(1)
                
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"数据转发循环异常: {e}")
                await asyncio.sleep(5)
    
    async def _forward_data(self):
        """执行数据转发"""
        try:
            current_time = time.time()
            
            # 检查MQTT连接状态
            if not mqtt_client.is_connected:
                logger.debug("MQTT未连接，跳过数据转发")
                return
            
            # 从Redis获取数据
            data = await self._fetch_data_from_redis()
            if data:
                # 按数据类型分组并分别上送
                await self._send_grouped_data(data)
            
            # 检查是否需要发送系统监控数据
            await self._check_and_send_system_monitor_data(current_time)
            
        except Exception as e:
            logger.error(f"数据转发异常: {e}")
    
    async def _check_and_send_system_monitor_data(self, current_time: float):
        """检查并发送系统监控数据"""
        try:
            # 获取系统监控配置
            monitor_config = config_loader.get_system_monitor_config()
            
            if not monitor_config.get('enabled', False):
                return
            
            # 检查收集间隔
            collect_interval = monitor_config.get('collect_interval', 10)
            if current_time - self.last_system_monitor_time < collect_interval:
                return
            
            # 收集系统监控数据
            system_data = system_monitor.get_system_data()
            if not system_data:
                logger.warning("系统监控数据收集失败")
                return
            
            # 获取设备序列号
            device_sn = device_identity.get_device_sn()
            
            # 格式化为MQTT格式
            mqtt_message = system_monitor.format_for_mqtt(system_data, device_sn)
            
            if mqtt_message:
                # 发送系统监控数据
                await self._send_system_monitor_data(mqtt_message)
                self.last_system_monitor_time = current_time
                
        except Exception as e:
            logger.error(f"发送系统监控数据异常: {e}")
    
    async def _send_system_monitor_data(self, message: Dict[str, Any]):
        """发送系统监控数据"""
        try:
            # 获取属性主题
            property_topic = device_identity.format_topic(
                config_loader.get_config('mqtt_topics.property', 'property/{productSN}/{deviceSN}')
            )
            
            # 发送数据
            await self._rate_limited_send(property_topic, json.dumps(message, ensure_ascii=False), qos=1)
            # 发送成功，重置失败计数器
            self._reset_mqtt_failure_count()
            logger.debug(f"系统监控数据上报成功: {property_topic}")
            logger.debug(f"系统监控数据内容: {json.dumps(message, indent=2)}")
            # 额外强制网络处理（在publish中已经处理了一次）
            try:
                for i in range(3):
                    mqtt_client.client.loop_write()
                    time.sleep(0.005)
            except:
                pass
                
        except Exception as e:
            logger.error(f"发送系统监控数据失败: {e}")
    
    async def _fetch_data_from_redis(self) -> Optional[List[Dict]]:
        """从Redis获取数据"""
        try:
            redis_client = redis_manager.get_client()
            if not redis_client:
                logger.warning("Redis连接不可用")
                return None
            
            # 获取订阅模式
            patterns = config_loader.get_config('redis_source.subscribe_patterns', [])
            if not patterns:
                logger.warning("未配置Redis订阅模式")
                return None
            
            # 获取所有匹配的键
            all_data = []
            for pattern in patterns:
                try:
                    keys = redis_client.keys(pattern)
                    logger.debug(f"模式 {pattern} 找到 {len(keys)} 个键")
                    
                    for key in keys:
                        try:
                            # 检查键的类型
                            key_type = redis_client.type(key)
                            logger.debug(f"键 {key} 类型: {key_type}")
                            
                            if key_type == 'string':
                                # 字符串类型
                                value = redis_client.get(key)
                                if value:
                                    try:
                                        parsed_value = json.loads(value)
                                        all_data.append({
                                            'key': key,
                                            'value': parsed_value,
                                            'timestamp': datetime.now().isoformat()
                                        })
                                    except json.JSONDecodeError:
                                        all_data.append({
                                            'key': key,
                                            'value': value,
                                            'timestamp': datetime.now().isoformat()
                                        })
                            
                            elif key_type == 'hash':
                                # 哈希表类型
                                hash_data = redis_client.hgetall(key)
                                if hash_data:
                                    all_data.append({
                                        'key': key,
                                        'value': hash_data,
                                        'timestamp': datetime.now().isoformat()
                                    })
                            
                            elif key_type == 'list':
                                # 列表类型
                                list_data = redis_client.lrange(key, 0, -1)
                                if list_data:
                                    all_data.append({
                                        'key': key,
                                        'value': list_data,
                                        'timestamp': datetime.now().isoformat()
                                    })
                            
                            elif key_type == 'set':
                                # 集合类型
                                set_data = list(redis_client.smembers(key))
                                if set_data:
                                    all_data.append({
                                        'key': key,
                                        'value': set_data,
                                        'timestamp': datetime.now().isoformat()
                                    })
                            
                            else:
                                logger.debug(f"跳过不支持的键类型: {key} ({key_type})")
                                
                        except Exception as e:
                            logger.warning(f"处理键 {key} 失败: {e}")
                            
                except Exception as e:
                    logger.warning(f"获取Redis模式数据失败: {pattern}, {e}")
            
            logger.debug(f"总共获取到 {len(all_data)} 条数据")
            
            # 应用过滤规则
            filtered_data = self._apply_filters(all_data)
            logger.debug(f"过滤后剩余 {len(filtered_data)} 条数据")
            
            return filtered_data
            
        except Exception as e:
            logger.error(f"从Redis获取数据失败: {e}")
            return None
    
    def _apply_filters(self, data: List[Dict]) -> List[Dict]:
        """应用数据过滤规则"""
        try:
            filters = config_loader.get_config('redis_source.filters', {})
            if not filters.get('enabled', True):
                return data
            
            filtered_data = data
            
            # 应用排除模式过滤
            exclude_patterns = filters.get('exclude_patterns', [])
            for pattern in exclude_patterns:
                filtered_data = [item for item in filtered_data 
                               if not self._pattern_match(pattern, item['key'])]
            
            return filtered_data
            
        except Exception as e:
            logger.error(f"应用数据过滤失败: {e}")
            return data
    
    def _pattern_match(self, pattern: str, key: str) -> bool:
        """检查键是否匹配模式"""
        try:
            import fnmatch
            return fnmatch.fnmatch(key, pattern)
        except:
            return False
    
    def _parse_key_format(self, key: str) -> Dict[str, str]:
        """解析键格式 A:B:C"""
        try:
            parts = key.split(':')
            if len(parts) >= 3:
                return {
                    'service': parts[0],      # A - 服务名 (comsrv, modsrv)
                    'channel': parts[1],      # B - 通道ID
                    'data_type': parts[2]     # C - 数据类型 (T, S, C, A等)
                }
            elif len(parts) == 2:
                return {
                    'service': parts[0],
                    'channel': parts[1],
                    'data_type': 'unknown'
                }
            else:
                return {
                    'service': key,
                    'channel': 'unknown',
                    'data_type': 'unknown'
                }
        except:
            return {
                'service': 'unknown',
                'channel': 'unknown',
                'data_type': 'unknown'
            }
    
    def _convert_hash_values(self, hash_data: Dict[str, str]) -> Dict[str, Any]:
        """转换hash值中的数字字符串为float"""
        converted_data = {}
        try:
            for key, value in hash_data.items():
                # 尝试转换为float
                try:
                    float_value = float(value)
                    # 检查是否为整数
                    if float_value.is_integer():
                        converted_data[key] = int(float_value)
                    else:
                        converted_data[key] = float_value
                except (ValueError, TypeError):
                    # 如果转换失败，保持原值
                    converted_data[key] = value
        except Exception as e:
            logger.warning(f"转换hash值失败: {e}")
            return hash_data
        
        return converted_data
    
    async def _send_grouped_data(self, data: List[Dict]):
        """按数据类型分组并分别上送数据"""
        try:
            # 按数据类型分组
            grouped_data = defaultdict(list)
            
            for item in data:
                key_info = self._parse_key_format(item['key'])
                group_key = f"{key_info['service']}:{key_info['channel']}:{key_info['data_type']}"
                grouped_data[group_key].append(item)
            
            # 获取批量大小配置
            batch_size = config_loader.get_config('data_report.batch_size', 50)
            
            # 处理每个分组
            for group_key, group_items in grouped_data.items():
                # 如果分组内的点位超过批量大小，需要分割
                if len(group_items) > batch_size:
                    # 分割成多个批次
                    for i in range(0, len(group_items), batch_size):
                        batch_items = group_items[i:i + batch_size]
                        await self._send_property_data(batch_items, group_key)
                        logger.debug(f"分组 {group_key} 分割发送第 {i//batch_size + 1} 批，包含 {len(batch_items)} 个点位")
                else:
                    # 直接发送整个分组
                    await self._send_property_data(group_items, group_key)
                    logger.debug(f"分组 {group_key} 发送完成，包含 {len(group_items)} 个点位")
                    
        except Exception as e:
            logger.error(f"分组发送数据失败: {e}")
    
    async def _send_property_data(self, data: List[Dict], group_key: str):
        """发送点位数据上报"""
        try:
            # 获取属性主题
            property_topic = device_identity.format_topic(
                config_loader.get_config('mqtt_topics.property', 'property/{productSN}/{deviceSN}')
            )
            
            # 构建点位数据
            property_data = []
            
            for item in data:
                key_info = self._parse_key_format(item['key'])
                
                # 构建source和device字段
                source = key_info['service']  # 服务名 (comsrv, modsrv)
                device = key_info['channel'].replace(' ', '_')  # 通道ID，空格转换为下划线
                
                # 处理不同类型的值
                if isinstance(item['value'], dict):
                    # 如果是hash类型，转换数字字符串为float
                    converted_value = self._convert_hash_values(item['value'])
                else:
                    # 其他类型保持原值
                    converted_value = item['value']
                
                point_data = {
                    "source": source,
                    "device": device,
                    "data_type": key_info['data_type'],
                    "value": converted_value
                }
                property_data.append(point_data)
            
            if property_data:
                # 构建消息
                current_timestamp = int(time.time())  # Unix时间戳
                message = {
                    "timestamp": current_timestamp,
                    "property": property_data
                }
                
                # 发送数据前检查MQTT连接状态
                if not mqtt_client.is_connected:
                    logger.debug(f"MQTT未连接，跳过数据发送: {group_key}")
                    await self._handle_mqtt_failure(f"MQTT未连接，跳过数据发送: {group_key}")
                    return
                
                # 发送数据
                await self._rate_limited_send(property_topic, json.dumps(message, ensure_ascii=False), qos=1)
                # 发送成功，重置失败计数器
                self._reset_mqtt_failure_count()
                logger.debug(f"点位数据上报成功: {property_topic}, 组: {group_key}, 数据量: {len(property_data)}")
                # 额外强制网络处理（在publish中已经处理了一次）
                try:
                    for i in range(3):
                        mqtt_client.client.loop_write()
                        time.sleep(0.005)
                except Exception as e:
                    logger.warning(f"强制网络处理异常: {e}")
            
        except Exception as e:
            logger.error(f"发送点位数据失败: {e}")
    
    async def _handle_mqtt_failure(self, message: str):
        """处理MQTT发送失败"""
        current_time = time.time()
        
        # 增加失败计数器
        self.mqtt_failure_count += 1
        self.last_failure_time = current_time
        
        # 第一次失败记录警告
        if self.mqtt_failure_count == 1:
            logger.warning(message)
        # 如果连续失败超过5次，开始降低日志频率并添加延迟
        elif self.mqtt_failure_count >= 5:
            if self.mqtt_failure_count % 10 == 0:  # 每10次失败记录一次
                logger.warning(f"{message} (连续失败 {self.mqtt_failure_count} 次)")
            
            # 计算延迟时间（指数退避，最大30秒）
            delay = min(self.failure_delay_base * (2 ** min(self.mqtt_failure_count // 5, 5)), 30)
            logger.debug(f"MQTT连续失败，等待 {delay} 秒后继续...")
            await asyncio.sleep(delay)
        else:
            # 前5次失败正常记录
            if self.mqtt_failure_count <= 3:
                logger.warning(message)
    
    def _reset_mqtt_failure_count(self):
        """重置MQTT失败计数器（发送成功时调用）"""
        if self.mqtt_failure_count > 0:
            logger.info(f"MQTT发送恢复正常，重置失败计数器（之前失败 {self.mqtt_failure_count} 次）")
            self.mqtt_failure_count = 0
            self.last_failure_time = 0

# 全局数据转发器实例
data_forwarder = DataForwarder()
