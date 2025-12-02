"""
广播API路由
处理向WebSocket客户端广播消息的接口
"""

import logging
from typing import Dict, Any
from fastapi import APIRouter, HTTPException, status, Request, Depends
from fastapi.responses import JSONResponse

from app.middleware.auth import get_optional_user

logger = logging.getLogger(__name__)

router = APIRouter(tags=["广播"])

# 全局WebSocket管理器引用
websocket_manager = None

def set_websocket_manager(manager):
    """设置WebSocket管理器实例"""
    global websocket_manager
    websocket_manager = manager

@router.post("/broadcast", summary="广播消息", description="将JSON数据广播到所有连接的WebSocket客户端")
async def broadcast_message(
    request: Request,
    current_user = Depends(get_optional_user)  # 可选认证，允许匿名访问
):
    """
    广播消息到所有连接的WebSocket客户端
    
    **请求体**: 任意JSON数据
    
    **响应**:
    - success: 是否成功
    - message: 结果消息
    - client_count: 接收消息的客户端数量
    - clients: 接收消息的客户端ID列表
    
    **示例**:
    ```json
    {
        "type": "notification",
        "title": "系统通知",
        "content": "这是一条广播消息",
        "level": "info"
    }
    ```
    """
    try:
        # 检查WebSocket管理器是否可用
        if not websocket_manager:
            raise HTTPException(
                status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
                detail="WebSocket服务不可用"
            )
        
        # 获取请求体中的JSON数据
        try:
            request_data = await request.json()
        except Exception as e:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail=f"无效的JSON数据: {str(e)}"
            )
        
        # 执行广播（原样转发JSON数据）
        result = await websocket_manager.broadcast_custom_message(request_data)
        
        # 记录广播日志
        user_info = f"用户 {current_user.username}" if current_user else "匿名用户"
        logger.info(f"{user_info} 执行广播操作，接收客户端: {result['client_count']}")
        
        return JSONResponse(
            status_code=status.HTTP_200_OK,
            content={
                "success": result["success"],
                "message": result["message"],
                "data": {
                    "client_count": result["client_count"],
                    "clients": result["clients"],
                    "broadcast_data": request_data
                }
            }
        )
        
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"广播消息失败: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"广播失败: {str(e)}"
        )

@router.get("/broadcast/status", summary="获取广播状态", description="获取WebSocket连接和订阅状态")
async def get_broadcast_status():
    """
    获取广播服务状态
    
    **响应**:
    - websocket_available: WebSocket服务是否可用
    - connection_count: 当前连接数
    - subscribed_count: 已订阅客户端数
    - connections: 连接详情
    """
    try:
        if not websocket_manager:
            return JSONResponse(
                status_code=status.HTTP_200_OK,
                content={
                    "success": True,
                    "data": {
                        "websocket_available": False,
                        "connection_count": 0,
                        "subscribed_count": 0,
                        "connections": {},
                        "subscriptions": {}
                    }
                }
            )
        
        # 获取WebSocket状态
        ws_status = websocket_manager.get_status()
        
        # 计算已订阅客户端数量（仅用于统计显示）
        subscribed_count = 0
        for client_id, subscription in ws_status.get("subscriptions", {}).items():
            if subscription.get("channels") or subscription.get("data_types"):
                subscribed_count += 1
        
        # 处理连接信息，移除不可序列化的WebSocket对象
        connections_info = {}
        for client_id, info in ws_status.get("connections_info", {}).items():
            connections_info[client_id] = {
                "data_type": info.get("data_type"),
                "connected_at": info.get("connected_at"),
                "last_activity": info.get("last_activity")
            }
        
        return JSONResponse(
            status_code=status.HTTP_200_OK,
            content={
                "success": True,
                "data": {
                    "websocket_available": True,
                    "connection_count": ws_status.get("connection_count", 0),
                    "subscribed_count": subscribed_count,
                    "connections": connections_info,
                    "subscriptions": ws_status.get("subscriptions", {})
                }
            }
        )
        
    except Exception as e:
        logger.error(f"获取广播状态失败: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"获取状态失败: {str(e)}"
        )
