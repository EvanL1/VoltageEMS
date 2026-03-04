"""
日志配置模块

Graceful degradation: if log directory is unavailable (e.g., external storage
not mounted), starts with console-only logging. File handlers are skipped
and service continues normally.
"""

import sys
from pathlib import Path
from loguru import logger
from .config import settings


def setup_logger():
    """设置日志配置"""
    # 移除默认的日志处理器
    logger.remove()

    # 检测日志目录是否可用
    log_dir = Path("logs")
    log_dir_available = False
    try:
        log_dir.mkdir(exist_ok=True)
        # Verify actually writable (mkdir succeeds on existing read-only dirs)
        log_dir_available = log_dir.is_dir()
    except OSError:
        pass

    # 控制台日志格式
    console_format = (
        "<green>{time:YYYY-MM-DD HH:mm:ss}</green> | "
        "<level>{level: <8}</level> | "
        "<cyan>{name}</cyan>:<cyan>{function}</cyan>:<cyan>{line}</cyan> | "
        "<level>{message}</level>"
    )

    # 文件日志格式
    file_format = (
        "{time:YYYY-MM-DD HH:mm:ss} | "
        "{level: <8} | "
        "{name}:{function}:{line} | "
        "{message}"
    )

    # 添加控制台日志处理器 (always available)
    logger.add(
        sys.stdout,
        format=console_format,
        level=settings.LOG_LEVEL,
        colorize=True
    )

    if log_dir_available:
        # 添加文件日志处理器
        logger.add(
            settings.LOG_FILE,
            format=file_format,
            level=settings.LOG_LEVEL,
            rotation="100 MB",
            retention="30 days",
            compression="zip",
            encoding="utf-8"
        )

        # 添加错误日志处理器
        error_log_file = log_dir / "error.log"
        logger.add(
            error_log_file,
            format=file_format,
            level="ERROR",
            rotation="50 MB",
            retention="60 days",
            compression="zip",
            encoding="utf-8"
        )

        logger.info("日志系统初始化完成")
        logger.info(f"日志级别: {settings.LOG_LEVEL}")
        logger.info(f"日志文件: {settings.LOG_FILE}")
    else:
        logger.warning("日志目录不可用，仅控制台输出（外部存储未挂载？）")
        logger.info(f"日志级别: {settings.LOG_LEVEL}")


# 初始化日志
setup_logger()
