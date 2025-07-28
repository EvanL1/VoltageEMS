"""带轮询功能的Modbus 协议插件实现"""

import asyncio
import logging
from typing import Dict, Optional, Set
from pymodbus.client import AsyncModbusTcpClient
from pymodbus.constants import Endian
from pymodbus.payload import BinaryPayloadBuilder
import redis.asyncio as redis

# 这些将在运行时从生成的 protobuf 导入
try:
    import protocol_plugin_pb2 as pb2
    import protocol_plugin_pb2_grpc as pb2_grpc
except ImportError:
    # 开发时可能还没有生成
    pb2 = None
    pb2_grpc = None

logger = logging.getLogger(__name__)


class PollingChannel:
    """轮询通道管理"""

    def __init__(self, channel_id: int, params: Dict[str, str]):
        self.channel_id = channel_id
        self.params = params
        self.enabled = params.get("polling_enabled", "false").lower() == "true"
        self.interval_ms = int(params.get("polling_interval_ms", "2000"))
        self.host = params.get("host", "localhost")
        self.port = int(params.get("port", "502"))
        self.slave_id = int(params.get("slave_id", "1"))
        self.redis_url = params.get("redis_url", "redis://localhost:6379")

        # 点位配置
        self.measurement_points: Set[int] = set()
        self.signal_points: Set[int] = set()
        self.control_points: Set[int] = set()
        self.adjustment_points: Set[int] = set()

        # 轮询任务
        self.polling_task: Optional[asyncio.Task] = None
        self.client: Optional[AsyncModbusTcpClient] = None
        self.redis_client: Optional[redis.Redis] = None

    async def start_polling(self, plugin: "ModbusPluginWithPolling"):
        """启动轮询"""
        if not self.enabled:
            logger.info(f"Channel {self.channel_id} polling is disabled")
            return

        if self.polling_task and not self.polling_task.done():
            logger.warning(f"Channel {self.channel_id} polling already running")
            return

        logger.info(
            f"Starting polling for channel {self.channel_id}, interval: {self.interval_ms}ms"
        )
        self.polling_task = asyncio.create_task(self._polling_loop(plugin))

    async def stop_polling(self):
        """停止轮询"""
        if self.polling_task and not self.polling_task.done():
            logger.info(f"Stopping polling for channel {self.channel_id}")
            self.polling_task.cancel()
            try:
                await self.polling_task
            except asyncio.CancelledError:
                pass

        if self.client and self.client.connected:
            await self.client.close()

        if self.redis_client:
            await self.redis_client.close()

    async def _polling_loop(self, plugin: "ModbusPluginWithPolling"):
        """轮询循环"""
        try:
            # 初始化Redis客户端
            self.redis_client = await redis.from_url(self.redis_url)

            # 初始化Modbus客户端
            self.client = AsyncModbusTcpClient(host=self.host, port=self.port)
            await self.client.connect()

            while True:
                try:
                    # 读取各种类型的点位
                    await self._poll_telemetry_type(plugin, "measurement", "m")
                    await self._poll_telemetry_type(plugin, "signal", "s")

                    # 等待下一个轮询周期
                    await asyncio.sleep(self.interval_ms / 1000.0)

                except Exception as e:
                    logger.error(f"Channel {self.channel_id} polling error: {e}")
                    # 重连
                    if not self.client.connected:
                        await self.client.connect()

        except asyncio.CancelledError:
            logger.info(f"Channel {self.channel_id} polling cancelled")
            raise
        except Exception as e:
            logger.error(f"Channel {self.channel_id} fatal polling error: {e}")

    async def _poll_telemetry_type(
        self, plugin: "ModbusPluginWithPolling", telemetry_type: str, type_suffix: str
    ):
        """轮询特定类型的遥测点"""
        # 获取对应类型的点位集合
        point_set = getattr(self, f"{telemetry_type}_points", set())
        if not point_set:
            return

        # 构建Redis key
        redis_key = f"comsrv:{self.channel_id}:{type_suffix}"

        # 读取数据
        data = {}
        for point_id in point_set:
            try:
                # 简化处理：假设都是读保持寄存器
                # 实际应该根据点表配置
                register_address = point_id  # 简化映射
                result = await self.client.read_holding_registers(
                    register_address, 1, slave=self.slave_id
                )

                if not result.isError():
                    value = float(result.registers[0])
                    # 格式化为6位小数
                    data[str(point_id)] = f"{value:.6f}"

            except Exception as e:
                logger.error(f"Failed to read point {point_id}: {e}")

        # 批量写入Redis
        if data:
            await self.redis_client.hset(redis_key, mapping=data)

            # 发布变更通知
            for point_id, value in data.items():
                pub_channel = f"comsrv:{self.channel_id}:{type_suffix}"
                message = f"{point_id}:{value}"
                await self.redis_client.publish(pub_channel, message)

            logger.debug(
                f"Updated {len(data)} {telemetry_type} points for channel {self.channel_id}"
            )


class ModbusPluginWithPolling(pb2_grpc.ProtocolPluginServicer if pb2_grpc else object):
    """带轮询功能的Modbus 协议插件实现"""

    def __init__(self):
        self.clients: Dict[str, AsyncModbusTcpClient] = {}
        self.config_cache: Dict[str, Dict] = {}
        self.polling_channels: Dict[int, PollingChannel] = {}

    async def GetInfo(self, request, context):
        """获取插件信息"""
        return pb2.PluginInfo(
            name="modbus-python-plugin-polling",
            version="2.0.0",
            protocol_type="modbus_tcp",
            supported_features=[
                "batch_read",
                "write_single",
                "write_multiple",
                "active_polling",
            ],
            metadata={
                "author": "VoltageEMS",
                "description": "Modbus TCP/RTU protocol plugin with active polling",
                "supported_functions": "1,2,3,4,5,6,15,16",
            },
        )

    async def HealthCheck(self, request, context):
        """健康检查"""
        healthy = True
        details = {}

        # 检查所有活跃连接
        for conn_id, client in self.clients.items():
            if client and client.connected:
                details[f"connection_{conn_id}"] = "connected"
            else:
                details[f"connection_{conn_id}"] = "disconnected"
                healthy = False

        # 检查轮询通道
        for channel_id, channel in self.polling_channels.items():
            if channel.enabled:
                if channel.polling_task and not channel.polling_task.done():
                    details[f"polling_channel_{channel_id}"] = "running"
                else:
                    details[f"polling_channel_{channel_id}"] = "stopped"
                    healthy = False

        return pb2.HealthStatus(
            healthy=healthy,
            message="Plugin is running" if healthy else "Some connections are down",
            details=details,
        )

    async def BatchRead(self, request, context):
        """批量读取数据（被动模式）"""
        logger.info(
            f"Received BatchRead request for channel {request.channel_id}, points: {list(request.point_ids)}"
        )
        try:
            # 检查是否已经有轮询任务
            channel_id = request.channel_id

            # 如果是第一次调用，初始化轮询
            if channel_id not in self.polling_channels:
                # 从连接参数中提取配置
                channel = PollingChannel(channel_id, request.connection_params)

                # 设置点位信息（简化处理，实际应该从配置文件读取）
                # 这里假设measurement类型的点位ID从1-10
                channel.measurement_points = set(request.point_ids)

                self.polling_channels[channel_id] = channel

                # 启动轮询
                await channel.start_polling(self)

                # 等待一会儿让轮询开始
                await asyncio.sleep(0.5)

            # 返回空响应，因为数据通过轮询直接写入Redis
            return pb2.BatchReadResponse(
                points=[],
                error=""
                if channel_id in self.polling_channels
                else "Failed to initialize polling",
            )

        except Exception as e:
            logger.error(f"BatchRead failed: {e}")
            return pb2.BatchReadResponse(error=str(e))

    async def ParseData(self, request, context):
        """解析原始数据"""
        # Modbus 通常不需要这个功能，因为数据是直接读取的
        return pb2.ParseResponse(error="Not implemented for Modbus")

    async def EncodeCommand(self, request, context):
        """编码控制命令"""
        try:
            point_id = request.point_id
            value = (
                request.value.float_value
                if request.value.HasField("float_value")
                else 0
            )

            # 这里应该根据点位配置确定功能码和地址
            # 简化示例：假设都是写保持寄存器（功能码6）
            slave_id = int(request.context.get("slave_id", "1"))
            register_address = point_id  # 简化：使用点ID作为寄存器地址

            # 构建 Modbus 请求帧
            builder = BinaryPayloadBuilder(byteorder=Endian.BIG)
            builder.add_16bit_uint(int(value))

            # 实际上应该返回完整的 Modbus 帧
            # 这里简化返回
            encoded = bytes(
                [slave_id, 0x06, register_address >> 8, register_address & 0xFF]
            )

            return pb2.EncodeResponse(encoded_data=encoded)

        except Exception as e:
            logger.error(f"EncodeCommand failed: {e}")
            return pb2.EncodeResponse(error=str(e))

    async def _get_or_create_client(self, host: str, port: int) -> AsyncModbusTcpClient:
        """获取或创建 Modbus 客户端"""
        conn_id = f"{host}:{port}"

        if conn_id not in self.clients:
            client = AsyncModbusTcpClient(host=host, port=port)
            self.clients[conn_id] = client

        return self.clients[conn_id]

    async def shutdown(self):
        """关闭插件"""
        # 停止所有轮询任务
        for channel in self.polling_channels.values():
            await channel.stop_polling()

        # 关闭所有客户端
        for client in self.clients.values():
            if client.connected:
                await client.close()
