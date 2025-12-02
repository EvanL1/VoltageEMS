"""
MQTT客户端模块
"""

import json
import time
import asyncio
import threading
import ssl
from typing import Optional, Callable, Dict, Any, List
from loguru import logger
import paho.mqtt.client as mqtt
from .config import settings
from .config_loader import config_loader

class MQTTClient:
    """MQTT客户端管理器"""
    
    def __init__(self):
        self.client: Optional[mqtt.Client] = None
        self.is_connected = False
        self.message_handlers: Dict[str, Callable] = {}
        self.subscribed_topics: Dict[str, int] = {}  # 记录已订阅的主题和QoS
        self.mqtt_config = {}
        self.connection_callbacks: List[Callable] = []
        self.disconnection_callbacks: List[Callable] = []
        self.reconnect_enabled = True
        self.reconnect_delay = 5
        self.max_reconnect_attempts = 10
        self.current_reconnect_attempts = 0
        
        # 连接稳定性相关属性
        self.last_disconnect_time = 0
        self.disconnect_count = 0
        self.min_disconnect_interval = 120  # 最小断开间隔（秒）- 增加到120秒，更宽松
        self.max_disconnect_count = 5  # 最大断开计数，超过后延长等待时间
        self._setup_client()
    
    def _setup_client(self):
        """设置MQTT客户端"""
        try:
            # 获取MQTT连接配置
            self.mqtt_config = config_loader.get_mqtt_connection_config()
            broker_config = self.mqtt_config.get('broker', {})
            
            if not broker_config:
                logger.error("未找到MQTT连接配置")
                return
            
            # 应用重连配置
            if 'reconnect_enabled' in self.mqtt_config:
                self.reconnect_enabled = self.mqtt_config['reconnect_enabled']
            if 'reconnect_delay' in self.mqtt_config:
                self.reconnect_delay = self.mqtt_config['reconnect_delay']
            if 'max_reconnect_attempts' in self.mqtt_config:
                self.max_reconnect_attempts = self.mqtt_config['max_reconnect_attempts']
            
            # 处理client_id逻辑
            client_id = broker_config.get('client_id', 'netsrv_client')
            if client_id in ['auto', '', None] or not client_id.strip():
                # 使用设备序列号作为client_id
                from .device_identity import device_identity
                client_id = device_identity.device_sn
                logger.info(f"使用设备序列号作为MQTT客户端ID: {client_id}")
            else:
                logger.info(f"使用配置的MQTT客户端ID: {client_id}")
            
            self.client = mqtt.Client(
                client_id=client_id,
                clean_session=broker_config.get('clean_session', True),
                protocol=mqtt.MQTTv311
            )
            
            # 设置遗嘱消息
            self._setup_will_message()
            
            # 设置回调函数
            self.client.on_connect = self._on_connect
            self.client.on_disconnect = self._on_disconnect
            self.client.on_message = self._on_message
            self.client.on_publish = self._on_publish
            
            # AWS IoT Core 优化配置
            self._setup_aws_iot_optimization()
            
            # 设置认证
            username = broker_config.get('username', '')
            password = broker_config.get('password', '')
            if username and password:
                self.client.username_pw_set(username, password)
            
            # 设置SSL
            ssl_config = broker_config.get('ssl', {})
            self._ssl_enabled = ssl_config.get('enabled', False)
            if self._ssl_enabled:
                ca_cert = ssl_config.get('ca_cert')
                client_cert = ssl_config.get('client_cert')
                client_key = ssl_config.get('client_key')
                
                if ca_cert and client_cert and client_key:
                    # 使用证书文件进行双向认证（AWS IoT推荐）
                    self.client.tls_set(
                        ca_certs=ca_cert,
                        certfile=client_cert,
                        keyfile=client_key,
                        cert_reqs=mqtt.ssl.CERT_REQUIRED,
                        tls_version=mqtt.ssl.PROTOCOL_TLSv1_2,
                        ciphers=None
                    )
                    logger.info(f"SSL证书配置完成: CA={ca_cert}, Cert={client_cert}, Key={client_key}")
                else:
                    # 仅启用SSL，不验证证书（不推荐用于生产环境）
                    self.client.tls_set()
                    logger.warning("SSL已启用但未配置证书，连接可能不安全")
            
            logger.info("MQTT客户端设置完成")
            
        except Exception as e:
            logger.error(f"MQTT客户端设置失败: {e}")
    
    def _setup_will_message(self):
        """设置MQTT遗嘱消息"""
        try:
            # 延迟导入以避免循环导入
            from .device_identity import device_identity
            
            # 获取状态主题模板
            status_topic_template = config_loader.get_config('mqtt_topics.status', 'status/{productSN}/{deviceSN}')
            
            # 格式化状态主题
            will_topic = device_identity.format_topic(status_topic_template)
            
                        # 构建遗嘱消息
            will_message = {
                "type": "offline", 
                "gateway": "",
                "reason": "unexpected_disconnect",
                "timestamp": int(time.time())
            }
            
            # 设置遗嘱消息
            self.client.will_set(
                topic=will_topic,
                payload=json.dumps(will_message, ensure_ascii=False),
                qos=1,
                retain=True
            )
            
            logger.info(f"MQTT遗嘱消息设置完成: {will_topic}")
            
        except Exception as e:
            logger.error(f"设置MQTT遗嘱消息失败: {e}")
    
    def _setup_aws_iot_optimization(self):
        """设置AWS IoT Core优化配置"""
        try:
            # 调整inflight和队列参数，适应AWS IoT Core
            self.client.max_inflight_messages_set(100)  # 增加inflight消息数
            self.client.max_queued_messages_set(10000)  # 增加队列大小
            
            # 设置keepalive（如果配置中有）
            broker_config = self.mqtt_config.get('broker', {})
            keepalive = broker_config.get('keepalive', 60)
            self.client.keepalive = keepalive
            
            logger.info(f"AWS IoT Core优化配置已应用:")
            logger.info(f"  - max_inflight_messages: 100")
            logger.info(f"  - max_queued_messages: 10000")
            logger.info(f"  - keepalive: {keepalive}秒")
            
        except Exception as e:
            logger.error(f"设置AWS IoT Core优化配置失败: {e}")
    
    def connect(self) -> bool:
        """连接到MQTT代理"""
        try:
            if not self.client:
                self._setup_client()
            
            broker_config = self.mqtt_config.get('broker', {})
            if not broker_config:
                logger.error("MQTT配置未加载")
                return False
            
            self.client.connect(
                broker_config.get('host', 'localhost'),
                broker_config.get('port', 1883),
                keepalive=broker_config.get('keepalive', 60)
            )
            
            # 启动网络循环
            self.client.loop_start()
            
            # 等待连接
            timeout = 10
            start_time = time.time()
            while not self.is_connected and (time.time() - start_time) < timeout:
                time.sleep(0.1)
            
            if self.is_connected:
                logger.info(f"MQTT连接成功: {broker_config.get('host')}:{broker_config.get('port')}")
                # 只有在启用SSL时才启动SSL错误监控线程
                if self._ssl_enabled:
                    self._start_ssl_error_monitor()
                return True
            else:
                logger.error("MQTT连接超时")
                return False
                
        except Exception as e:
            logger.error(f"MQTT连接失败: {e}")
            return False
    
    def disconnect(self):
        """断开MQTT连接"""
        try:
            if self.client and self.is_connected:
                # 在断开前，确保所有待发送的消息都发送完成
                logger.info("正在发送待发送的消息...")
                
                # 强制处理所有待发送的消息
                for i in range(50):  # 增加处理次数
                    rc = self.client.loop_write()
                    if rc != mqtt.MQTT_ERR_SUCCESS:
                        break
                    time.sleep(0.02)
                
                # 等待消息队列清空
                time.sleep(1.0)
                
                self.client.loop_stop()
                self.client.disconnect()
                self.is_connected = False
                logger.info("MQTT连接已断开")
        except Exception as e:
            logger.error(f"MQTT断开连接失败: {e}")
    
    def close_connection(self) -> bool:
        """关闭MQTT连接（API专用方法）"""
        try:
            if not self.is_connected:
                logger.info("MQTT连接已经关闭")
                return True
            
            logger.info("正在关闭MQTT连接...")
            
            # 禁用自动重连
            self.reconnect_enabled = False
            
            # 断开连接
            self.disconnect()
            
            # 停止网络循环
            if self.client:
                try:
                    self.client.loop_stop()
                except:
                    pass
            
            logger.info("MQTT连接已成功关闭")
            return True
            
        except Exception as e:
            logger.error(f"关闭MQTT连接失败: {e}")
            return False
    
    def subscribe(self, topic: str, qos: int = 0):
        """订阅主题"""
        try:
            if self.client and self.is_connected:
                result = self.client.subscribe(topic, qos)
                if result[0] == mqtt.MQTT_ERR_SUCCESS:
                    # 记录订阅的主题，用于重连后自动重新订阅
                    self.subscribed_topics[topic] = qos
                    logger.info(f"订阅主题成功: {topic}")
                else:
                    logger.error(f"订阅主题失败: {topic}")
        except Exception as e:
            logger.error(f"订阅主题异常: {topic}, {e}")
    
    def _resubscribe_all_topics(self):
        """重新订阅所有主题（用于重连后）"""
        try:
            if not self.subscribed_topics:
                logger.debug("没有需要重新订阅的主题")
                return
            
            logger.info(f"重新订阅 {len(self.subscribed_topics)} 个主题...")
            success_count = 0
            for topic, qos in self.subscribed_topics.items():
                try:
                    if self.client and self.is_connected:
                        result = self.client.subscribe(topic, qos)
                        if result[0] == mqtt.MQTT_ERR_SUCCESS:
                            logger.info(f"重新订阅成功: {topic}")
                            success_count += 1
                        else:
                            logger.error(f"重新订阅失败: {topic}")
                except Exception as e:
                    logger.error(f"重新订阅异常: {topic}, {e}")
            
            logger.info(f"重新订阅完成: {success_count}/{len(self.subscribed_topics)} 个主题成功")
        except Exception as e:
            logger.error(f"重新订阅所有主题失败: {e}")
    
    def publish(self, topic: str, payload: Any, qos: int = 0, retain: bool = False) -> bool:
        """发布消息"""
        try:
            if self.client and self.is_connected:
                if isinstance(payload, (dict, list)):
                    payload_str = json.dumps(payload, ensure_ascii=False)
                else:
                    payload_str = str(payload)
                
                result = self.client.publish(topic, payload_str, qos, retain)
                if result.rc == mqtt.MQTT_ERR_SUCCESS:
                    logger.debug(f"发布消息成功: {topic}")
                    
                    # 立即强制发送消息（不等待确认）
                    try:
                        # 强制处理网络写入操作
                        for i in range(5):  
                            rc = self.client.loop_write()
                            if rc != mqtt.MQTT_ERR_SUCCESS:
                                break
                            time.sleep(0.01)  # 给网络处理一点时间
                        
                        logger.debug(f"消息强制发送: {topic}")
                    except Exception as e:
                        logger.warning(f"强制发送消息失败: {topic}, {e}")
                    
                    # 对于状态消息，输出详细信息
                    if 'status/' in topic:
                        logger.debug(f"状态消息发布: {topic} -> {payload_str}")
                    
                    return True
                else:
                    logger.error(f"发布消息失败: {topic}, 错误码: {result.rc}")
                    
                    # 检查是否为连接相关错误，如果是则触发重连
                    if self._is_connection_error(result.rc):
                        logger.warning(f"检测到连接错误码 {result.rc}，触发MQTT重连")
                        self._handle_connection_error()
                    
                    return False
            else:
                logger.warning(f"MQTT客户端未连接，无法发布消息: {topic}")
                return False
        except Exception as e:
            logger.error(f"发布消息异常: {topic}, {e}")
            return False
    
    def _is_connection_error(self, error_code: int) -> bool:
        """检查错误码是否为连接相关错误"""
        # paho-mqtt错误码定义
        # MQTT_ERR_NO_CONN = 4      # 没有连接
        # MQTT_ERR_CONN_LOST = 7    # 连接丢失  
        # MQTT_ERR_NOT_FOUND = 15   # 在发布上下文中通常表示连接问题
        connection_error_codes = [4, 7, 15]
        return error_code in connection_error_codes
    
    def _get_disconnect_error_info(self, rc: int) -> Dict[str, str]:
        """获取断开连接错误的详细信息"""
        error_info = {
            0: {"type": "正常断开", "description": "客户端主动断开连接"},
            1: {"type": "协议错误", "description": "MQTT协议版本不匹配"},
            2: {"type": "客户端ID错误", "description": "客户端ID无效"},
            3: {"type": "服务器不可用", "description": "MQTT服务器不可用"},
            4: {"type": "认证失败", "description": "用户名或密码错误"},
            5: {"type": "未授权", "description": "客户端未授权连接"},
            7: {"type": "连接丢失", "description": "网络连接意外中断，可能是SSL错误"},
            16: {"type": "网络错误", "description": "网络连接问题"},
            17: {"type": "超时", "description": "连接超时"}
        }
        return error_info.get(rc, {"type": "未知错误", "description": f"错误码: {rc}"})
    
    def _is_ssl_related_error(self, rc: int) -> bool:
        """检查是否为SSL相关错误"""
        # 只有在启用SSL时才检查SSL相关错误
        if not hasattr(self, '_ssl_enabled') or not self._ssl_enabled:
            return False
        
        # SSL相关错误码（更精确的判断）
        # 7: 连接丢失 - 可能是SSL握手失败或证书问题
        # 16: 网络错误 - 可能是SSL连接问题
        ssl_related_codes = [7, 16]
        return rc in ssl_related_codes
    
    def _get_network_quality_level(self) -> str:
        """评估网络质量等级（更宽松的判断）"""
        if self.disconnect_count == 0:
            return "excellent"
        elif self.disconnect_count <= 3:
            return "good"
        elif self.disconnect_count <= 10:
            return "fair"
        elif self.disconnect_count <= 20:
            return "poor"
        else:
            return "very_poor"
    
    def _check_connection_stability(self) -> bool:
        """检查连接稳定性，避免频繁重连"""
        import time
        current_time = time.time()
        
        # 计算动态等待时间
        network_quality = self._get_network_quality_level()
        
        if network_quality == "very_poor":
            # 网络质量很差时，使用更长的等待时间
            wait_time = 300  # 5分钟
        elif network_quality == "poor":
            wait_time = 120  # 2分钟
        elif network_quality == "fair":
            wait_time = 60   # 1分钟
        else:
            wait_time = self.min_disconnect_interval  # 60秒
        
        # 如果距离上次断开时间太短，认为是网络波动，不立即重连
        if current_time - self.last_disconnect_time < wait_time:
            self.disconnect_count += 1
            logger.warning(f"连接断开过于频繁（{self.disconnect_count}次，网络质量: {network_quality}），等待 {wait_time} 秒后重连...")
            return False
        
        # 重置断开计数
        if self.disconnect_count > 0:
            logger.info(f"连接稳定，重置断开计数（之前 {self.disconnect_count} 次）")
        self.disconnect_count = 0
        return True
    
    def _check_network_health(self) -> bool:
        """检查网络连通性"""
        try:
            import socket
            broker_config = self.mqtt_config.get('broker', {})
            host = broker_config.get('host', 'localhost')
            port = broker_config.get('port', 1883)
            
            # 创建socket连接测试
            sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            sock.settimeout(5)  # 5秒超时
            result = sock.connect_ex((host, port))
            sock.close()
            
            if result == 0:
                logger.debug(f"网络连通性检查通过: {host}:{port}")
                return True
            else:
                logger.warning(f"网络连通性检查失败: {host}:{port}, 错误码: {result}")
                return False
                
        except Exception as e:
            logger.warning(f"网络连通性检查异常: {e}")
            return False
    
    def _start_ssl_error_monitor(self):
        """启动SSL错误监控线程"""
        # 只有在启用SSL时才启动监控
        if not self._ssl_enabled:
            logger.debug("SSL未启用，跳过SSL错误监控")
            return
            
        def ssl_monitor():
            """SSL错误监控线程"""
            while self.is_connected:
                try:
                    # 检查MQTT客户端状态
                    if not self.client or not self.client._sock:
                        logger.warning("检测到MQTT客户端连接异常，触发重连")
                        self._handle_ssl_error()
                        break
                    
                    # 检查socket状态
                    if hasattr(self.client, '_sock') and self.client._sock:
                        try:
                            # 尝试发送一个小的测试数据包
                            self.client._sock.send(b'\x00')
                        except (OSError, ConnectionError, ssl.SSLError) as e:
                            logger.error(f"检测到SSL连接错误: {e}")
                            self._handle_ssl_error()
                            break
                    
                    time.sleep(5)  # 每5秒检查一次
                    
                except Exception as e:
                    logger.error(f"SSL监控线程异常: {e}")
                    time.sleep(10)
        
        # 启动监控线程
        monitor_thread = threading.Thread(target=ssl_monitor, daemon=True)
        monitor_thread.start()
        logger.info("SSL错误监控线程已启动")
    
    def _handle_ssl_error(self):
        """处理SSL错误"""
        try:
            logger.error("处理SSL错误：强制断开并重连")
            
            # 标记为未连接状态
            self.is_connected = False
            
            # 强制断开连接
            try:
                if self.client:
                    self.client.loop_stop()
                    self.client.disconnect()
                    logger.info("MQTT连接已强制断开")
            except Exception as e:
                logger.warning(f"强制断开连接时发生异常: {e}")
            
            # 等待一小段时间让断开完成
            time.sleep(2)
            
            # 如果启用了重连，触发重连
            if self.reconnect_enabled:
                logger.info("开始自动重连...")
                self._start_reconnect()
            else:
                logger.warning("自动重连未启用，请手动重启服务")
                
        except Exception as e:
            logger.error(f"处理SSL错误异常: {e}")
    
    def _handle_connection_error(self):
        """处理连接错误，强制断开并重连"""
        try:
            logger.info("处理连接错误：强制断开MQTT连接")
            
            # 标记为未连接状态
            self.is_connected = False
            
            # 强制断开连接
            try:
                self.client.disconnect()
                logger.info("MQTT连接已强制断开")
            except Exception as e:
                logger.warning(f"强制断开连接时发生异常: {e}")
            
            # 等待一小段时间让断开完成
            time.sleep(1)
            
            # 如果启用了重连，触发重连
            if hasattr(self, 'reconnect_enabled') and self.reconnect_enabled:
                logger.info("开始自动重连...")
                self._start_reconnect()
            else:
                logger.warning("自动重连未启用，请手动重启服务")
                
        except Exception as e:
            logger.error(f"处理连接错误异常: {e}")

    def add_message_handler(self, topic: str, handler: Callable):
        """添加消息处理器"""
        self.message_handlers[topic] = handler
        logger.info(f"添加消息处理器: {topic}")
    
    def _on_connect(self, client, userdata, flags, rc):
        """连接回调"""
        if rc == 0:
            self.is_connected = True
            self.current_reconnect_attempts = 0  # 重置重连计数
            
            # 连接成功后重置断开计数
            if self.disconnect_count > 0:
                logger.info(f"MQTT连接成功，重置断开计数（之前 {self.disconnect_count} 次）")
                self.disconnect_count = 0
            
            if flags.get('session_present', False):
                logger.info("MQTT连接已建立（恢复会话）")
            else:
                logger.info("MQTT连接已建立（新会话）")
                # 新会话需要重新订阅所有主题
                self._resubscribe_all_topics()
            
            # 立即发送在线消息
            self._send_online_message_sync()
            
            # 调用连接回调
            for callback in self.connection_callbacks:
                try:
                    if asyncio.iscoroutinefunction(callback):
                        try:
                            loop = asyncio.get_running_loop()
                            loop.create_task(callback())
                        except RuntimeError:
                            # 如果没有事件循环，跳过异步回调
                            logger.warning("跳过异步连接回调：没有运行的事件循环")
                    else:
                        callback()
                except Exception as e:
                    logger.error(f"连接回调执行失败: {e}")
        else:
            self.is_connected = False
            error_messages = {
                1: "连接被拒绝 - 协议版本不正确",
                2: "连接被拒绝 - 客户端ID无效",
                3: "连接被拒绝 - 服务器不可用",
                4: "连接被拒绝 - 用户名或密码错误",
                5: "连接被拒绝 - 未授权"
            }
            error_msg = error_messages.get(rc, f"未知错误码: {rc}")
            logger.error(f"MQTT连接失败: {error_msg}")
    
    def _on_disconnect(self, client, userdata, rc):
        """断开连接回调"""
        self.is_connected = False
        
        # 调用断开连接回调
        for callback in self.disconnection_callbacks:
            try:
                if asyncio.iscoroutinefunction(callback):
                    try:
                        loop = asyncio.get_running_loop()
                        loop.create_task(callback())
                    except RuntimeError:
                        # 如果没有事件循环，跳过异步回调
                        logger.warning("跳过异步断开连接回调：没有运行的事件循环")
                else:
                    callback()
            except Exception as e:
                logger.error(f"断开连接回调执行失败: {e}")
        
        if rc != 0:
            # 记录断开时间
            import time
            self.last_disconnect_time = time.time()
            
            # 增强错误分类和日志记录
            error_info = self._get_disconnect_error_info(rc)
            logger.warning(f"MQTT意外断开，错误码: {rc}, 类型: {error_info['type']}, 描述: {error_info['description']}")
            
            # 更保守的重连策略：只在真正的断开时才重连
            # 让TLS和SDK的容错机制处理网络波动
            
            # 检查连接稳定性
            if not self._check_connection_stability():
                logger.info("连接不稳定，跳过本次重连，让SDK容错机制处理")
                return
            
            # 只对严重的连接错误进行重连
            serious_errors = [1, 2, 3, 4, 5]  # 协议错误、认证失败等严重错误
            if rc in serious_errors:
                logger.error(f"检测到严重连接错误 {rc}，触发重连")
                if self.reconnect_enabled:
                    self._start_reconnect()
            else:
                # 对于网络错误(7, 16)等，让SDK容错机制处理
                logger.info(f"网络错误 {rc}，让SDK容错机制处理，不主动重连")
                # 不主动重连，让MQTT客户端库自己处理
        else:
            logger.info("MQTT连接已断开")
    
    async def _send_online_message(self):
        """发送在线消息"""
        try:
            # 延迟导入以避免循环导入
            from .device_identity import device_identity
            
            # 获取状态主题
            status_topic_template = config_loader.get_config('mqtt_topics.status', 'status/{productSN}/{deviceSN}')
            status_topic = device_identity.format_topic(status_topic_template)
            
            # 构建在线消息
            online_message = {
                "type": "online",
                "gateway": "",  # 固定为空字符串
                "timestamp": int(time.time())
            }
            
            # 等待确保连接稳定
            await asyncio.sleep(0.5)
            
            # 发送在线消息
            if self.publish(status_topic, online_message, qos=1, retain=True):
                logger.debug(f"设备在线消息发送成功: {status_topic}")
            else:
                logger.error("设备在线消息发送失败")
                
        except Exception as e:
            logger.error(f"发送在线消息失败: {e}")
    
    def _send_online_message_sync(self):
        """同步发送在线消息（连接成功后立即调用）"""
        try:
            # 延迟导入以避免循环导入
            from .device_identity import device_identity
            
            # 获取状态主题
            status_topic_template = config_loader.get_config('mqtt_topics.status', 'status/{productSN}/{deviceSN}')
            status_topic = device_identity.format_topic(status_topic_template)
            
            # 构建在线消息
            online_message = {
                "type": "online",
                "gateway": "",
                "timestamp": int(time.time())
            }
            
            # 立即发送在线消息（使用QoS 1确保传输）
            if self.publish(status_topic, online_message, qos=1, retain=True):
                logger.debug(f"设备在线消息发送成功: {status_topic}")
                # 额外强制网络处理，确保立即发送
                try:
                    for i in range(10):
                        self.client.loop_write()
                        time.sleep(0.01)
                except:
                    pass
            else:
                logger.error("设备在线消息发送失败")
                
        except Exception as e:
            logger.error(f"同步发送在线消息失败: {e}")
    
    def _start_reconnect(self):
        """启动重连机制（增强版）"""
        # 移除最大重连次数限制，确保重连线程持续运行
        
        def reconnect_worker():
            """重连工作线程"""
            while not self.is_connected:
                try:
                    self.current_reconnect_attempts += 1
                    logger.info(f"开始第 {self.current_reconnect_attempts} 次重连尝试...")
                    
                    # 使用更保守的延迟策略
                    if self.current_reconnect_attempts <= self.max_reconnect_attempts:
                        # 前几次重连使用较短延迟，后续逐渐增加
                        if self.current_reconnect_attempts <= 3:
                            delay = self.reconnect_delay
                        else:
                            delay = min(self.reconnect_delay * (1.2 ** (self.current_reconnect_attempts - 3)), 120)
                    else:
                        # 超过最大重连次数后，使用固定长延迟（5分钟）
                        delay = 300
                        logger.warning(f"已超过最大重连次数 {self.max_reconnect_attempts}，使用长延迟重连")
                    
                    logger.info(f"等待 {delay} 秒后重连...")
                    time.sleep(delay)
                    
                    # 检查网络连通性
                    if not self._check_network_health():
                        logger.warning("网络连通性检查失败，跳过本次重连")
                        continue
                    
                    if not self.is_connected:
                        success = self.connect()
                        if success:
                            logger.info(f"第 {self.current_reconnect_attempts} 次重连成功")
                            break
                        else:
                            logger.error(f"第 {self.current_reconnect_attempts} 次重连失败")
                    
                except Exception as e:
                    logger.error(f"重连异常: {e}")
                    # 发生异常时等待一段时间再继续
                    time.sleep(30)
        
        # 在后台线程中执行重连
        thread = threading.Thread(target=reconnect_worker, daemon=True)
        thread.start()
    
    def _on_message(self, client, userdata, msg):
        """消息接收回调"""
        try:
            topic = msg.topic
            payload = msg.payload.decode('utf-8')
            
            # 查找对应的处理器
            for pattern, handler in self.message_handlers.items():
                if self._topic_match(pattern, topic):
                    try:
                        handler(topic, payload)
                    except Exception as e:
                        logger.error(f"消息处理器异常: {topic}, {e}")
                    break
            else:
                logger.debug(f"未找到消息处理器: {topic}")
                
        except Exception as e:
            logger.error(f"消息处理异常: {e}")
    
    def _on_publish(self, client, userdata, mid):
        """消息发布回调"""
        logger.debug(f"消息发布完成，消息ID: {mid}")
    
    def _topic_match(self, pattern: str, topic: str) -> bool:
        """主题匹配检查"""
        if pattern == topic:
            return True
        
        # 支持通配符匹配
        if '+' in pattern or '#' in pattern:
            pattern_parts = pattern.split('/')
            topic_parts = topic.split('/')
            
            if len(pattern_parts) > len(topic_parts) and '#' not in pattern_parts:
                return False
            
            for i, pattern_part in enumerate(pattern_parts):
                if i >= len(topic_parts):
                    return False
                
                if pattern_part == '#':
                    return True
                elif pattern_part == '+':
                    continue
                elif pattern_part != topic_parts[i]:
                    return False
            
            return True
        
        return False
    
    def add_connection_callback(self, callback: Callable):
        """添加连接成功回调"""
        self.connection_callbacks.append(callback)
        logger.info("添加连接成功回调")
    
    def add_disconnection_callback(self, callback: Callable):
        """添加断开连接回调"""
        self.disconnection_callbacks.append(callback)
        logger.info("添加断开连接回调")
    
    def set_reconnect_config(self, enabled: bool = True, delay: int = 5, max_attempts: int = 10):
        """设置重连配置"""
        self.reconnect_enabled = enabled
        self.reconnect_delay = delay
        self.max_reconnect_attempts = max_attempts
        logger.info(f"重连配置: enabled={enabled}, delay={delay}s, max_attempts={max_attempts}")
    
    def reset_reconnect_counter(self):
        """重置重连计数器"""
        self.current_reconnect_attempts = 0
        logger.info("重连计数器已重置")
    
    def reload_config_and_reconnect(self) -> bool:
        """重新加载配置并重连（无论当前是否连接）"""
        try:
            logger.info("开始重新加载MQTT配置并重连...")
            
            # 如果当前已连接，先断开
            if self.is_connected:
                logger.info("断开当前MQTT连接...")
                self.disconnect()
            
            # 停止当前客户端的网络循环
            if self.client:
                try:
                    self.client.loop_stop()
                except:
                    pass
            
            # 重新加载配置
            logger.info("重新加载MQTT配置...")
            config_loader.reload_config()
            
            # 重新设置客户端
            self._setup_client()
            
            # 重新连接
            logger.info("开始重新连接MQTT...")
            success = self.connect()
            
            if success:
                logger.info("MQTT配置重载和重连成功")
                return True
            else:
                logger.error("MQTT重连失败")
                return False
                
        except Exception as e:
            logger.error(f"MQTT配置重载和重连失败: {e}")
            return False
    
    def force_reconnect(self) -> bool:
        """强制重连（不重新加载配置）"""
        try:
            logger.info("开始强制重连MQTT...")
            
            # 断开当前连接
            if self.is_connected:
                logger.info("断开当前MQTT连接...")
                self.disconnect()
            
            # 停止当前客户端的网络循环
            if self.client:
                try:
                    self.client.loop_stop()
                except:
                    pass
                
            # 重置重连计数器
            self.reset_reconnect_counter()
            
            # 重新连接
            logger.info("开始重新连接MQTT...")
            success = self.connect()
            
            if success:
                logger.info("MQTT强制重连成功")
                return True
            else:
                logger.error("MQTT强制重连失败")
                return False
                
        except Exception as e:
            logger.error(f"MQTT强制重连失败: {e}")
            return False
    
    def get_connection_status(self) -> Dict[str, Any]:
        """获取连接状态信息"""
        try:
            broker_config = self.mqtt_config.get('broker', {})
            return {
                "connected": self.is_connected,
                "host": broker_config.get('host', 'unknown'),
                "port": broker_config.get('port', 1883),
                "client_id": broker_config.get('client_id', 'unknown'),
                "reconnect_enabled": self.reconnect_enabled,
                "reconnect_attempts": self.current_reconnect_attempts,
                "max_reconnect_attempts": self.max_reconnect_attempts,
                "reconnect_delay": self.reconnect_delay
            }
        except Exception as e:
            logger.error(f"获取连接状态失败: {e}")
            return {
                "connected": False,
                "error": str(e)
            }
    
    def update_config(self, new_config: Dict[str, Any]) -> bool:
        """更新MQTT配置（不立即重连）"""
        try:
            logger.info("更新MQTT客户端配置...")
            
            # 保存新配置
            self.mqtt_config = new_config
            broker_config = new_config.get('broker', {})
            
            # 更新重连配置
            reconnect_config = broker_config.get('reconnect', {})
            self.reconnect_enabled = reconnect_config.get('enabled', True)
            self.reconnect_delay = reconnect_config.get('delay', 5)
            self.max_reconnect_attempts = reconnect_config.get('max_attempts', 10)
            
            logger.info("MQTT客户端配置更新成功")
            return True
            
        except Exception as e:
            logger.error(f"更新MQTT客户端配置失败: {e}")
            return False

# 全局MQTT客户端实例
mqtt_client = MQTTClient()
