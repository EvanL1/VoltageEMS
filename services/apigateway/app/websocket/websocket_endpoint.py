"""
WebSocket端点
提供WebSocket连接接口
"""

import uuid
import logging
from fastapi import WebSocket, WebSocketDisconnect, Query
from typing import Optional

from app.websocket.websocket_manager import WebSocketManager

logger = logging.getLogger(__name__)

# 全局WebSocket管理器（将在main.py中初始化）
websocket_manager: WebSocketManager = None

def get_websocket_manager() -> WebSocketManager:
    """获取WebSocket管理器实例"""
    if websocket_manager is None:
        raise RuntimeError("WebSocket管理器未初始化")
    return websocket_manager

async def websocket_endpoint(
    websocket: WebSocket,
    client_id: Optional[str] = Query(None, description="客户端ID"),
    data_type: str = Query("general", description="数据类型")
):
    """
    WebSocket连接端点
    
    Args:
        websocket: WebSocket连接
        client_id: 客户端ID，如果不提供则自动生成
        data_type: 数据类型，用于过滤消息
    """
    if client_id is None:
        client_id = str(uuid.uuid4())
    
    manager = get_websocket_manager()
    
    try:
        # 建立连接
        await manager.connect_client(websocket, client_id, data_type)
        logger.info(f"WebSocket连接建立: {client_id}")
        
        # 消息处理循环
        while True:
            try:
                # 接收消息
                message = await websocket.receive_text()
                logger.debug(f"收到消息来自 {client_id}: {message}")
                
                # 处理消息
                await manager.handle_client_message(client_id, message)
                
            except WebSocketDisconnect:
                logger.info(f"WebSocket连接断开: {client_id}")
                break
            except Exception as e:
                logger.error(f"处理WebSocket消息失败: {e}")
                # 发送错误消息
                error_message = {
                    "type": "error",
                    "data": {
                        "message": "消息处理失败",
                        "error": str(e)
                    }
                }
                await websocket.send_text(str(error_message))
                
    except Exception as e:
        logger.error(f"WebSocket连接处理失败: {e}")
    finally:
        # 清理连接
        manager.disconnect_client(client_id)
        logger.info(f"WebSocket连接清理完成: {client_id}")

async def websocket_status():
    """获取WebSocket状态"""
    manager = get_websocket_manager()
    return manager.get_status()
