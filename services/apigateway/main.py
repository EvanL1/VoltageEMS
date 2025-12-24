#!/usr/bin/env python3
"""
API网关主程序
统一API入口，提供身份认证、请求路由和实时数据WebSocket接口
"""

import asyncio
import uvicorn
from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
import logging
import os
import uuid
from dotenv import load_dotenv
from fastapi import WebSocket, Query
from fastapi.websockets import WebSocketDisconnect

from app.core.config import settings
from app.api.routes import api_router
from app.websocket.websocket_manager import WebSocketManager
from app.core.redis_client import RedisClient
from app.tasks.data_scheduler import DataScheduler
from app.services.database import initialize_database, close_database, get_database
from app.services.auth_service import get_auth_service
from app.routers.auth import router as auth_router
from app.routers.broadcast import router as broadcast_router, set_websocket_manager

# 加载环境变量
load_dotenv()

# 配置日志
def setup_logging():
    """Setup logging with fallback to stdout-only if file logging fails."""
    log_format = '%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    handlers = [logging.StreamHandler()]

    # Try to setup file logging
    log_dir = os.environ.get('LOG_DIR', 'logs')
    log_file = os.path.join(log_dir, 'apigateway.log')
    try:
        os.makedirs(log_dir, exist_ok=True)
        handlers.append(logging.FileHandler(log_file))
    except (OSError, PermissionError):
        # File logging not available, use stdout only
        pass

    logging.basicConfig(
        level=logging.INFO,
        format=log_format,
        handlers=handlers
    )

setup_logging()
logger = logging.getLogger(__name__)

# 创建FastAPI应用
app = FastAPI(
    title="API网关",
    description="统一API入口，提供身份认证、请求路由和实时数据WebSocket接口",
    version="1.0.0",
    docs_url="/docs",
    redoc_url="/redoc"
)

# 添加CORS中间件
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# 包含API路由
app.include_router(api_router, prefix="/api/v1")
app.include_router(auth_router, prefix="/api/v1")
app.include_router(broadcast_router, prefix="/api/v1")

# 全局变量
websocket_manager = None
redis_client = None
data_scheduler = None

async def init_admin_user_if_needed():
    """如果需要则初始化管理员用户"""
    try:
        db = get_database()
        
        # 检查是否已存在管理员用户
        admin_user = await db.get_user_by_username("admin")
        if admin_user:
            logger.info("管理员用户已存在，跳过创建")
            return
        
        logger.info("未找到管理员用户，开始创建...")
        
        # 创建管理员用户（使用MD5密码）
        auth = get_auth_service()
        # admin123 的 MD5 值
        admin_password_md5 = "0192023a7bbd73250516f069df18b500"
        password_hash = auth.hash_password(admin_password_md5)
        user_id = await db.create_user(
            username="admin",
            password_hash=password_hash,
            role_id=1  # 管理员角色
        )
        
        logger.info(f"管理员用户创建成功 (ID: {user_id})")
        logger.info("默认登录信息: admin / admin123 (前端需要MD5加密后传输)")
        logger.info("MD5密码: 0192023a7bbd73250516f069df18b500")
        logger.info("⚠️ 请尽快修改默认密码！")
        
    except Exception as e:
        logger.error(f"初始化管理员用户失败: {e}")
        # 不抛出异常，让应用继续启动

@app.on_event("startup")
async def startup_event():
    """应用启动时的初始化"""
    global websocket_manager, redis_client, data_scheduler
    
    try:
        # 创建日志目录
        os.makedirs("logs", exist_ok=True)
        
        # 初始化数据库
        await initialize_database()
        logger.info("数据库初始化成功")
        
        # 初始化管理员用户（如果需要）
        await init_admin_user_if_needed()
        logger.info("管理员用户检查完成")
        
        # 初始化Redis客户端
        redis_client = RedisClient()
        await redis_client.connect()
        logger.info("Redis连接成功")
        
        # 设置全局Redis客户端给API路由使用
        from app.api import routes
        routes.redis_client = redis_client
        
        # 初始化WebSocket管理器
        websocket_manager = WebSocketManager(redis_client)
        logger.info("WebSocket管理器初始化成功")
        
        # 初始化数据调度器
        data_scheduler = DataScheduler(redis_client, websocket_manager)
        await data_scheduler.start()
        logger.info("数据调度器启动成功")
        
        # 将数据调度器引用传递给WebSocket管理器
        websocket_manager.data_scheduler = data_scheduler
        
        # 设置WebSocket管理器到广播路由
        set_websocket_manager(websocket_manager)
        
        logger.info("API网关启动成功")
        
    except Exception as e:
        logger.error(f"启动失败: {e}")
        raise

@app.on_event("shutdown")
async def shutdown_event():
    """应用关闭时的清理"""
    global websocket_manager, redis_client, data_scheduler
    
    try:
        if data_scheduler:
            await data_scheduler.stop()
            logger.info("数据调度器已停止")
        
        if websocket_manager:
            await websocket_manager.close_all()
            logger.info("WebSocket管理器已关闭")
        
        if redis_client:
            await redis_client.close()
            logger.info("Redis连接已关闭")
        
        # 关闭数据库连接
        await close_database()
        logger.info("数据库连接已关闭")
            
        logger.info("API网关已关闭")
        
    except Exception as e:
        logger.error(f"关闭时出错: {e}")

@app.get("/")
async def root():
    """根路径"""
    return {
        "message": "API网关服务",
        "version": "1.0.0",
        "status": "running"
    }

@app.get("/health")
async def health_check():
    """健康检查"""
    return {
        "status": "healthy",
        "timestamp": asyncio.get_event_loop().time()
    }

@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket, client_id: str = Query(default=None), data_type: str = Query(default="general")):
    """WebSocket连接端点"""
    global websocket_manager
    
    if websocket_manager is None:
        await websocket.close(code=1008, reason="WebSocket管理器未初始化")
        return
    
    # 如果没有提供client_id，自动生成唯一ID
    if client_id is None:
        client_id = f"client_{uuid.uuid4().hex[:8]}"
        logger.info(f"自动生成客户端ID: {client_id}")
    
    try:
        # 连接客户端
        await websocket_manager.connect_client(websocket, client_id, data_type)
        
        # 处理WebSocket消息
        while True:
            try:
                # 接收消息
                message = await websocket.receive_text()
                
                # 处理消息
                await websocket_manager.handle_client_message(client_id, message)
                
            except WebSocketDisconnect:
                logger.info(f"WebSocket客户端断开连接: {client_id}")
                break
            except Exception as e:
                logger.error(f"处理WebSocket消息时出错: {e}")
                break
                
    except Exception as e:
        logger.error(f"WebSocket连接处理失败: {e}")
    finally:
        # 断开客户端
        if websocket_manager:
            await websocket_manager.disconnect_client(client_id)



if __name__ == "__main__":
    # 启动应用
    uvicorn.run(
        "main:app",
        host="0.0.0.0",
        port=6005,
        reload=False,
        log_level="info"
    )
