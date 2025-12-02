"""
网络服务主程序
"""

import asyncio
import signal
import sys
from contextlib import asynccontextmanager
from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from loguru import logger

from app.core.config import settings
from app.core.logger import setup_logger
from app.core.database import redis_manager
from app.core.mqtt_client import mqtt_client
from app.core.config_loader import config_loader
from app.core.device_identity import device_identity
from app.services.data_forwarder import data_forwarder
from app.services.point_reader import point_reader
from app.services.point_writer import point_writer
from app.services.data_caller import data_caller
from app.services.alarm_caller import alarm_caller
from app.api.routes import router

# 设置日志
setup_logger()

# 全局变量
shutdown_event = asyncio.Event()

@asynccontextmanager
async def lifespan(app: FastAPI):
    """应用生命周期管理"""
    # 启动时
    startup_success = False
    try:
        logger.info("正在启动网络服务...")
        
        # 配置加载器已在导入时初始化，这里只需要加载配置
        config_loader.load_config()
        logger.info("配置加载完成")
        
        # 设备身份已在导入时自动加载
        logger.info("设备身份加载完成")
        
        # Redis已在导入时自动连接，这里只需要检查连接状态
        if redis_manager.is_connected():
            logger.info("Redis连接正常")
        else:
            logger.error("Redis连接失败")
            raise Exception("Redis连接失败")
        
        # 连接MQTT
        if mqtt_client.connect():
            logger.info("MQTT连接成功")
            
            # 设置单点读取主题
            if point_reader.setup_topics():
                logger.info("单点读取服务启动成功")
            else:
                logger.error("单点读取服务启动失败")
                raise Exception("单点读取服务启动失败")
            
            # 设置单点写入主题
            if point_writer.setup_topics():
                logger.info("单点写入服务启动成功")
            else:
                logger.error("单点写入服务启动失败")
                raise Exception("单点写入服务启动失败")
        else:
            logger.error("MQTT连接失败")
            raise Exception("MQTT连接失败")
        
        # 启动数据转发器
        await data_forwarder.start()
        logger.info("数据转发器启动成功")
        
        # 设置数据总召主题（需要在 data_forwarder 启动后）
        if data_caller.setup_topics(data_forwarder):
            logger.info("数据总召服务启动成功")
        else:
            logger.error("数据总召服务启动失败")
            raise Exception("数据总召服务启动失败")
        
        # 设置告警总召主题
        if alarm_caller.setup_topics():
            logger.info("告警总召服务启动成功")
        else:
            logger.error("告警总召服务启动失败")
            raise Exception("告警总召服务启动失败")
        
        logger.info("网络服务启动完成")
        startup_success = True
        
    except Exception as e:
        logger.error(f"启动失败: {e}")
        startup_success = False
    
    yield
    
    # 关闭时
    if startup_success:
        try:
            logger.info("正在关闭网络服务...")
            
            # 停止数据转发器
            await data_forwarder.stop()
            logger.info("数据转发器已停止")
            
            # 断开MQTT连接
            mqtt_client.disconnect()
            logger.info("MQTT连接已断开")
            
            # 关闭Redis连接
            redis_manager.close()
            logger.info("Redis连接已关闭")
            
            logger.info("网络服务已关闭")
            
        except Exception as e:
            logger.error(f"关闭异常: {e}")
    else:
        logger.info("启动失败，跳过关闭流程")

# 创建FastAPI应用
app = FastAPI(
    title=settings.APP_NAME,
    description="网络服务 - 云数据转发服务",
    version="1.0.0",
    lifespan=lifespan
)

# 添加CORS中间件
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# 全局异常处理器
@app.exception_handler(Exception)
async def global_exception_handler(request: Request, exc: Exception):
    logger.error(f"全局异常: {exc}")
    return JSONResponse(
        status_code=500,
        content={"detail": "内部服务器错误"}
    )

# 包含路由
app.include_router(router)

def signal_handler(signum, frame):
    """信号处理器"""
    logger.info(f"收到信号 {signum}，正在关闭服务...")
    shutdown_event.set()

# 注册信号处理器
signal.signal(signal.SIGINT, signal_handler)
signal.signal(signal.SIGTERM, signal_handler)

if __name__ == "__main__":
    import uvicorn
    
    try:
        uvicorn.run(
            "main:app",
            host="0.0.0.0",
            port=settings.PORT,
            reload=False,
            log_level="info"
        )
    except KeyboardInterrupt:
        logger.info("收到中断信号，正在关闭...")
    except Exception as e:
        logger.error(f"服务运行异常: {e}")
        sys.exit(1)
