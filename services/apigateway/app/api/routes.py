"""
主要API路由文件
所有具体的API接口已移至专门的路由模块中
"""

from fastapi import APIRouter

# 创建主路由
api_router = APIRouter()

# ==================== 所有接口已移至专门的路由模块 ====================
# 认证相关接口: app.routers.auth
# 其他接口根据需要在相应模块中实现
