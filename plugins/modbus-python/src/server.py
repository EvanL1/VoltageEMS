"""Modbus gRPC 插件服务器"""

import asyncio
import logging
import os
import signal
import sys
from concurrent import futures
import grpc

# 添加src目录到Python路径
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "."))

try:
    import protocol_plugin_pb2_grpc
    from modbus_plugin_polling import ModbusPluginWithPolling
except ImportError as e:
    print(f"Import error: {e}")
    print("Please run the protobuf compilation first")
    sys.exit(1)

# 配置日志
logging.basicConfig(
    level=logging.INFO, format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
)
logger = logging.getLogger(__name__)


async def serve():
    """启动 gRPC 服务器"""
    port = os.environ.get("GRPC_PORT", "50051")

    # 创建 gRPC 服务器
    server = grpc.aio.server(
        futures.ThreadPoolExecutor(max_workers=10),
        options=[
            ("grpc.max_send_message_length", 50 * 1024 * 1024),  # 50MB
            ("grpc.max_receive_message_length", 50 * 1024 * 1024),  # 50MB
        ],
    )

    # 添加服务
    plugin = ModbusPluginWithPolling()
    protocol_plugin_pb2_grpc.add_ProtocolPluginServicer_to_server(plugin, server)

    # 监听端口
    listen_addr = f"[::]:{port}"
    server.add_insecure_port(listen_addr)

    logger.info(f"Starting Modbus gRPC plugin server on {listen_addr}")

    # 启动服务器
    await server.start()

    # 设置信号处理
    def signal_handler(sig, frame):
        logger.info("Received shutdown signal, stopping server...")
        asyncio.create_task(server.stop(5))

    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)

    # 等待服务器终止
    await server.wait_for_termination()
    logger.info("Server stopped")


def main():
    """主函数"""
    logger.info("Modbus gRPC Plugin starting...")

    # 运行异步服务器
    asyncio.run(serve())


if __name__ == "__main__":
    main()
