"""Modbus 协议插件实现"""

import logging
import time
from typing import Dict, List, Optional
from pymodbus.client import AsyncModbusTcpClient
from pymodbus.constants import Endian
from pymodbus.payload import BinaryPayloadDecoder, BinaryPayloadBuilder

# 这些将在运行时从生成的 protobuf 导入
try:
    import protocol_plugin_pb2 as pb2
    import protocol_plugin_pb2_grpc as pb2_grpc
except ImportError:
    # 开发时可能还没有生成
    pb2 = None
    pb2_grpc = None

logger = logging.getLogger(__name__)


class ModbusPlugin(pb2_grpc.ProtocolPluginServicer if pb2_grpc else object):
    """Modbus 协议插件实现"""

    def __init__(self):
        self.clients: Dict[str, AsyncModbusTcpClient] = {}
        self.config_cache: Dict[str, Dict] = {}

    async def GetInfo(self, request, context):
        """获取插件信息"""
        return pb2.PluginInfo(
            name="modbus-python-plugin",
            version="1.0.0",
            protocol_type="modbus_tcp",
            supported_features=["batch_read", "write_single", "write_multiple"],
            metadata={
                "author": "VoltageEMS",
                "description": "Modbus TCP/RTU protocol plugin",
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

        return pb2.HealthStatus(
            healthy=healthy,
            message="Plugin is running" if healthy else "Some connections are down",
            details=details,
        )

    async def BatchRead(self, request, context):
        """批量读取数据"""
        try:
            # 解析连接参数
            host = request.connection_params.get("host", "localhost")
            port = int(request.connection_params.get("port", "502"))
            slave_id = int(request.connection_params.get("slave_id", "1"))

            # 获取或创建客户端
            client = await self._get_or_create_client(host, port)
            if not client.connected:
                await client.connect()

            # 读取数据
            points = []
            for point_id in request.point_ids:
                try:
                    value = await self._read_point(
                        client, slave_id, point_id, request.read_params
                    )
                    if value is not None:
                        point = pb2.PointData(
                            point_id=point_id,
                            float_value=float(value),
                            timestamp=int(time.time() * 1000),
                            quality=0,  # 0 表示正常
                        )
                        points.append(point)
                except Exception as e:
                    logger.error(f"Failed to read point {point_id}: {e}")
                    # 继续读取其他点

            return pb2.BatchReadResponse(points=points)

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

    async def _read_point(
        self,
        client: AsyncModbusTcpClient,
        slave_id: int,
        point_id: int,
        params: Dict[str, str],
    ) -> Optional[float]:
        """读取单个点位的值"""
        # 从参数中获取读取配置
        # 实际应该从点表配置中获取
        function_code = int(params.get(f"fc_{point_id}", "3"))
        register_address = int(params.get(f"addr_{point_id}", str(point_id)))
        data_type = params.get(f"type_{point_id}", "uint16")

        # 根据功能码读取
        if function_code == 1:  # 读线圈
            result = await client.read_coils(register_address, 1, slave=slave_id)
            if not result.isError():
                return float(result.bits[0])

        elif function_code == 2:  # 读离散输入
            result = await client.read_discrete_inputs(
                register_address, 1, slave=slave_id
            )
            if not result.isError():
                return float(result.bits[0])

        elif function_code == 3:  # 读保持寄存器
            count = self._get_register_count(data_type)
            result = await client.read_holding_registers(
                register_address, count, slave=slave_id
            )
            if not result.isError():
                return self._decode_registers(result.registers, data_type)

        elif function_code == 4:  # 读输入寄存器
            count = self._get_register_count(data_type)
            result = await client.read_input_registers(
                register_address, count, slave=slave_id
            )
            if not result.isError():
                return self._decode_registers(result.registers, data_type)

        return None

    def _get_register_count(self, data_type: str) -> int:
        """根据数据类型获取需要读取的寄存器数量"""
        type_map = {
            "uint16": 1,
            "int16": 1,
            "uint32": 2,
            "int32": 2,
            "float32": 2,
            "uint64": 4,
            "int64": 4,
            "float64": 4,
        }
        return type_map.get(data_type, 1)

    def _decode_registers(self, registers: List[int], data_type: str) -> float:
        """解码寄存器值"""
        if data_type == "uint16":
            return float(registers[0])
        elif data_type == "int16":
            return float(registers[0] if registers[0] < 32768 else registers[0] - 65536)
        elif data_type == "uint32":
            return float((registers[0] << 16) + registers[1])
        elif data_type == "int32":
            value = (registers[0] << 16) + registers[1]
            return float(value if value < 2147483648 else value - 4294967296)
        elif data_type == "float32":
            decoder = BinaryPayloadDecoder.fromRegisters(
                registers, byteorder=Endian.BIG
            )
            return decoder.decode_32bit_float()
        # 添加更多类型支持...
        else:
            return float(registers[0])
