"""
历史数据服务主程序
"""

import time
import uvicorn
from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from loguru import logger
import sys
from contextlib import asynccontextmanager

from .core.config import settings
from .core.config_loader import config_loader
from .api.routes import router
from .services.scheduler import scheduler_service

# 配置日志
logger.remove()
logger.add(
    sys.stdout,
    format="<green>{time:YYYY-MM-DD HH:mm:ss}</green> | <level>{level: <8}</level> | <cyan>{name}</cyan>:<cyan>{function}</cyan>:<cyan>{line}</cyan> - <level>{message}</level>",
    level=settings.LOG_LEVEL
)

if settings.LOG_FILE:
    logger.add(
        settings.LOG_FILE,
        format="{time:YYYY-MM-DD HH:mm:ss} | {level: <8} | {name}:{function}:{line} - {message}",
        level=settings.LOG_LEVEL,
        rotation="100 MB",
        retention="30 days"
    )

@asynccontextmanager
async def lifespan(app: FastAPI):
    """应用生命周期管理"""
    # 启动时执行
    logger.info(f"启动 {settings.APP_NAME} v{settings.APP_VERSION}")
    
    # 验证配置
    if not config_loader.validate_config():
        logger.error("配置文件验证失败")
        sys.exit(1)
    
    # 打印配置摘要
    config_summary = config_loader.get_config_summary()
    logger.info(f"配置摘要: {config_summary}")
    
    # 启动定时任务
    if settings.SCHEDULER_ENABLED:
        try:
            scheduler_service.start()
            logger.info("定时任务服务启动成功")
        except Exception as e:
            logger.error(f"启动定时任务服务失败: {e}")
    
    yield
    
    # 关闭时执行
    logger.info("正在关闭服务...")
    
    # 停止定时任务
    try:
        scheduler_service.stop()
        logger.info("定时任务服务已停止")
    except Exception as e:
        logger.error(f"停止定时任务服务失败: {e}")
    
    logger.info("服务已关闭")

# 创建FastAPI应用
app = FastAPI(
    title=settings.APP_NAME,
    version=settings.APP_VERSION,
    description="历史数据存储和管理，将实时数据持久化到时序数据库并提供查询服务",
    lifespan=lifespan
)

# 添加CORS中间件
app.add_middleware(
    CORSMiddleware,
    allow_origins=settings.CORS_ORIGINS,
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# 添加请求日志中间件
@app.middleware("http")
async def log_requests(request: Request, call_next):
    start_time = time.time()
    
    # 记录请求
    logger.debug(f"请求: {request.method} {request.url}")
    
    try:
        response = await call_next(request)
        process_time = time.time() - start_time
        
        # 记录响应
        logger.debug(f"响应: {response.status_code} - 耗时: {process_time:.3f}s")
        
        return response
    
    except Exception as e:
        process_time = time.time() - start_time
        logger.error(f"请求异常: {e} - 耗时: {process_time:.3f}s")
        
        return JSONResponse(
            status_code=500,
            content={"detail": "内部服务器错误"}
        )

# 添加全局异常处理
@app.exception_handler(Exception)
async def global_exception_handler(request: Request, exc: Exception):
    logger.error(f"全局异常处理: {exc}")
    return JSONResponse(
        status_code=500,
        content={"detail": "内部服务器错误"}
    )

# 注册路由
app.include_router(router, tags=["历史数据"])

# 根路径
@app.get("/", tags=["系统"])
async def root():
    """根路径"""
    return {
        "service": settings.APP_NAME,
        "version": settings.APP_VERSION,
        "status": "running",
        "timestamp": time.time(),
        "api_docs": "/docs",
        "api_prefix": settings.API_PREFIX
    }

# 简单健康检查
@app.get("/ping", tags=["系统"])
async def ping():
    """简单健康检查"""
    return {"status": "ok", "timestamp": time.time()}

if __name__ == "__main__":
    # 直接运行
    logger.info(f"启动服务器: {settings.HOST}:{settings.PORT}")
    uvicorn.run(
        "app.main:app",
        host=settings.HOST,
        port=settings.PORT,
        reload=settings.DEBUG,
        log_level="info"
    )
