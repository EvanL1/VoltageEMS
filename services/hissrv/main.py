#!/usr/bin/env python3
"""
历史数据服务启动脚本
"""

import os
import sys
import uvicorn
from loguru import logger

# 添加项目根目录到Python路径
project_root = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, project_root)

def main():
    """主函数"""
    try:
        # 导入应用
        from app.main import app
        from app.core.config import settings
        
        logger.info(f"启动 {settings.APP_NAME} v{settings.APP_VERSION}")
        logger.info(f"服务器地址: http://{settings.HOST}:{settings.PORT}")
        logger.info(f"API文档: http://{settings.HOST}:{settings.PORT}/docs")
        logger.info(f"API前缀: {settings.API_PREFIX}")
        
        # 启动服务器
        uvicorn.run(
            "app.main:app",  # 使用导入字符串而不是app对象
            host=settings.HOST,
            port=settings.PORT,
            reload=settings.DEBUG,
            log_level="info" if not settings.DEBUG else "debug"
        )
        
    except KeyboardInterrupt:
        logger.info("用户中断服务")
    except Exception as e:
        logger.error(f"启动失败: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
