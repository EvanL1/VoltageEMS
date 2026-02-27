"""
计算点位管理API路由
处理计算点位的增删改查操作
"""

import logging
from typing import Dict, Any, List, Optional
from fastapi import APIRouter, HTTPException, status, Query, Request
from fastapi.responses import JSONResponse

from app.models.edge_data import (
    CalculatedPoint, 
    CalculatedPointCreate, 
    CalculatedPointUpdate
)
from app.services.database import get_database

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/homepage", tags=["计算点位"])


def invalidate_homepage_cache(request: Request):
    """使首页缓存失效"""
    try:
        if hasattr(request.app.state, 'websocket_manager'):
            ws_manager = request.app.state.websocket_manager
            if hasattr(ws_manager, 'data_scheduler') and ws_manager.data_scheduler:
                ws_manager.data_scheduler.invalidate_homepage_cache()
                logger.info("首页缓存已失效")
    except Exception as e:
        logger.warning(f"使缓存失效失败: {e}")




@router.get("", response_model=Dict[str, Any])
async def get_points(
    page: int = Query(1, ge=1, description="页码，从1开始"),
    limit: int = Query(20, ge=1, le=100, description="每页数量，最大100"),
    name: Optional[str] = Query(None, description="名称筛选（模糊匹配）")
):
    """
    获取计算点位列表
    
    支持分页和名称筛选
    """
    try:
        db = get_database()
        
        # 计算偏移量
        offset = (page - 1) * limit
        
        # 获取点位列表和总数
        items, total = await db.get_all_calculated_points(
            offset=offset,
            limit=limit,
            name_filter=name
        )
        
        # 转换为字典列表
        points_list = []
        for item in items:
            points_list.append({
                "id": item["id"],
                "name": item["name"],
                "formula": item["formula"],
                "unit": item["unit"],
                "imgurl": item["imgurl"],
                "description": item["description"],
                "created_at": item["created_at"],
                "updated_at": item["updated_at"]
            })
        
        return {
            "success": True,
            "message": "获取计算点位列表成功",
            "data": {
                "items": points_list,
                "total": total,
                "page": page,
                "limit": limit,
                "pages": (total + limit - 1) // limit  # 向上取整
            }
        }
        
    except Exception as e:
        logger.error(f"获取计算点位列表异常: {e}", exc_info=True)
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"获取计算点位列表失败: {str(e)}"
        )


@router.get("/{point_id}", response_model=Dict[str, Any])
async def get_point(point_id: int):
    """
    获取单个计算点位详情
    
    - **point_id**: 点位ID
    """
    try:
        db = get_database()
        
        # 获取点位
        point = await db.get_calculated_point_by_id(point_id)
        
        if not point:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"计算点位 ID {point_id} 不存在"
            )
        
        return {
            "success": True,
            "message": "获取计算点位详情成功",
            "data": {
                "id": point["id"],
                "name": point["name"],
                "formula": point["formula"],
                "unit": point["unit"],
                "imgurl": point["imgurl"],
                "description": point["description"],
                "created_at": point["created_at"],
                "updated_at": point["updated_at"]
            }
        }
        
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"获取计算点位详情异常: {e}", exc_info=True)
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"获取计算点位详情失败: {str(e)}"
        )


@router.put("/{point_id}", response_model=Dict[str, Any])
async def update_point(point_id: int, update_data: CalculatedPointUpdate, request: Request):
    """
    更新计算点位
    
    - **point_id**: 点位ID
    - 可更新字段: name, formula, unit, imgurl, description
    - 只需提供要更新的字段
    """
    try:
        db = get_database()
        
        # 检查点位是否存在
        existing_point = await db.get_calculated_point_by_id(point_id)
        if not existing_point:
            raise HTTPException(
                status_code=status.HTTP_404_NOT_FOUND,
                detail=f"计算点位 ID {point_id} 不存在"
            )
        
        # 更新点位
        affected_rows = await db.update_calculated_point(
            point_id=point_id,
            name=update_data.name,
            formula=update_data.formula,
            unit=update_data.unit,
            imgurl=update_data.imgurl,
            description=update_data.description
        )
        
        if affected_rows == 0:
            return {
                "success": True,
                "message": "无需更新（没有字段变化）",
                "data": {
                    "id": existing_point["id"],
                    "name": existing_point["name"],
                    "formula": existing_point["formula"],
                    "unit": existing_point["unit"],
                    "imgurl": existing_point["imgurl"],
                    "description": existing_point["description"],
                    "created_at": existing_point["created_at"],
                    "updated_at": existing_point["updated_at"]
                }
            }
        
        # 获取更新后的点位
        updated_point = await db.get_calculated_point_by_id(point_id)
        
        # 使缓存失效
        invalidate_homepage_cache(request)
        
        return {
            "success": True,
            "message": "计算点位更新成功",
            "data": {
                "id": updated_point["id"],
                "name": updated_point["name"],
                "formula": updated_point["formula"],
                "unit": updated_point["unit"],
                "imgurl": updated_point["imgurl"],
                "description": updated_point["description"],
                "created_at": updated_point["created_at"],
                "updated_at": updated_point["updated_at"]
            }
        }
        
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"更新计算点位异常: {e}", exc_info=True)
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"更新计算点位失败: {str(e)}"
        )


@router.post("/reset", response_model=Dict[str, Any])
async def reset_to_default(request: Request):
    """
    恢复默认设置
    
    清空 calculated_points 表的所有数据，并从 SQL 文件导入初始数据
    
    ⚠️ 警告：此操作将删除所有现有点位数据！
    """
    try:
        db = get_database()
        
        # 恢复默认设置
        count = await db.reset_calculated_points_to_default()
        
        # 使缓存失效
        invalidate_homepage_cache(request)
        
        return {
            "success": True,
            "message": "已恢复默认设置",
            "data": {
                "imported_count": count,
                "note": "所有自定义点位已被删除，已导入默认点位数据"
            }
        }
        
    except FileNotFoundError as e:
        logger.error(f"恢复默认设置失败: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"初始化 SQL 文件不存在: {str(e)}"
        )
    except Exception as e:
        logger.error(f"恢复默认设置异常: {e}", exc_info=True)
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"恢复默认设置失败: {str(e)}"
        )
