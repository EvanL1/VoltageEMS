"""
告警服务主入口文件
实现FastAPI应用程序和数据库初始化
"""

import logging
import sys
import os
import asyncio
from pathlib import Path

from fastapi import FastAPI, HTTPException, Query
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import StreamingResponse
from typing import Optional
from datetime import datetime
import uvicorn
import io

from app.core.config import settings
from app.core.database import init_database
from app.services.alert_rule_service import alert_rule_service
from app.services.alert_service import alert_service
from app.services.alarm_monitor import alarm_monitor
from app.models.alert_rule import AlertRule

# 配置日志
logging.basicConfig(
    level=getattr(logging, settings.LOG_LEVEL),
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)

logger = logging.getLogger(__name__)

# 创建FastAPI应用
app = FastAPI(
    title=settings.APP_NAME,
    version=settings.APP_VERSION,
    description="工业物联网告警服务 - 监控和管理告警规则",
    docs_url="/docs",
    redoc_url="/redoc"
)

# 配置CORS
app.add_middleware(
    CORSMiddleware,
    allow_origins=settings.CORS_ORIGINS,
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


@app.on_event("startup")
async def startup_event():
    """应用启动时的初始化操作"""
    logger.info("启动告警服务...")
    
    # 初始化数据库
    logger.info("初始化数据库...")
    success = init_database()
    if not success:
        logger.error("数据库初始化失败！")
        sys.exit(1)
    
    logger.info("数据库初始化成功")
    
    # 启动告警监控引擎
    logger.info("启动告警监控引擎...")
    try:
        alarm_monitor.start()
        logger.info("告警监控引擎启动成功")
    except Exception as e:
        logger.error(f"告警监控启动失败: {e}")
        # 监控启动失败不影响主服务
    
    # 记录服务启动信息
    logger.info(f"告警服务启动成功")
    logger.info(f"数据库路径: {settings.VOLTAGE_DB_PATH}")
    logger.info(f"Redis连接: {settings.REDIS_HOST}:{settings.REDIS_PORT}")
    logger.info(f"数据监控间隔: {settings.DATA_FETCH_INTERVAL}秒")


@app.on_event("shutdown")
async def shutdown_event():
    """应用关闭时的清理操作"""
    logger.info("关闭告警服务...")
    
    # 停止告警监控引擎
    try:
        await alarm_monitor.stop()
        logger.info("告警监控引擎已停止")
    except Exception as e:
        logger.error(f"停止监控引擎失败: {e}")
    
    logger.info("告警服务已关闭")


@app.get("/")
async def root():
    """根路径"""
    return {
        "service": settings.APP_NAME,
        "version": settings.APP_VERSION,
        "status": "running",
        "database": settings.VOLTAGE_DB_PATH,
    }


@app.get("/health")
async def health_check():
    """健康检查接口"""
    try:
        # 检查数据库连接
        rule_count = alert_rule_service.get_rule_count()
        enabled_count = alert_rule_service.get_enabled_rule_count()
        
        # 检查告警统计
        alert_stats = alert_service.get_alert_statistics()
        
        # 检查监控状态
        monitor_status = alarm_monitor.get_monitor_status()
        
        return {
            "status": "healthy",
            "database": "connected",
            "rules": {
                "total": rule_count,
                "enabled": enabled_count
            },
            "alerts": alert_stats.get("data", {}),
            "monitor": monitor_status
        }
    except Exception as e:
        logger.error(f"健康检查失败: {e}")
        raise HTTPException(status_code=500, detail="Service unhealthy")


# 告警规则API端点
@app.post("/alarmApi/rules", response_model=dict)
async def create_alert_rule(rule_data: dict):
    """创建告警规则"""
    try:
        # 验证必需字段
        required_fields = {
            "channel_id": "通道ID", 
            "data_type": "数据类型", 
            "point_id": "点位ID", 
            "rule_name": "规则名称", 
            "warning_level": "告警级别", 
            "operator": "比较操作符", 
            "value": "阈值"
        }
        
        missing_fields = []
        for field, field_name in required_fields.items():
            if field not in rule_data or rule_data[field] is None:
                missing_fields.append(f"{field_name}({field})")
        
        if missing_fields:
            return {
                "success": False,
                "message": f"缺少必需字段: {', '.join(missing_fields)}",
                "data": {
                    "missing_fields": list(required_fields.keys()),
                    "example": {
                        "service_type": "comsrv",
                        "channel_id": 1,
                        "data_type": "T",
                        "point_id": 100,
                        "rule_name": "温度告警",
                        "warning_level": 2,
                        "operator": ">",
                        "value": 50.0,
                        "description": "温度超过50度时告警",
                        "enabled": True
                    }
                }
            }
        
        # 验证数据类型
        try:
            # 确保数值字段为正确类型
            rule_data["channel_id"] = int(rule_data["channel_id"])
            rule_data["point_id"] = int(rule_data["point_id"])
            rule_data["warning_level"] = int(rule_data["warning_level"])
            rule_data["value"] = float(rule_data["value"])
        except (ValueError, TypeError) as e:
            return {
                "success": False,
                "message": f"数据类型错误: channel_id、point_id、warning_level必须为整数，value必须为数值",
                "data": {"type_error": str(e)}
            }
        
        # 创建AlertRule对象
        rule = AlertRule.from_dict(rule_data)
        
        # 详细验证规则
        is_valid, error_message = rule.validate_detailed()
        if not is_valid:
            return {
                "success": False,
                "message": f"规则验证失败: {error_message}",
                "data": {
                    "validation_error": error_message,
                    "current_data": rule_data
                }
            }
        
        # 检查是否存在相同的规则配置
        existing_rules = alert_rule_service.get_rules_by_service_channel_point(
            rule.service_type, rule.channel_id, rule.data_type, rule.point_id
        )
        
        if existing_rules:
            existing_rule = existing_rules[0]
            return {
                "success": False,
                "message": f"已存在相同的规则配置 (服务类型:{rule.service_type}, 通道:{rule.channel_id}, 数据类型:{rule.data_type}, 点位:{rule.point_id})",
                "data": {
                    "conflict": "规则重复",
                    "existing_rule": {
                        "id": existing_rule.id,
                        "rule_name": existing_rule.rule_name,
                        "created_at": existing_rule.created_at
                    },
                    "suggestion": f"请更换其他点位或修改现有规则 ID:{existing_rule.id}"
                }
            }
        
        # 创建规则
        rule_id = alert_rule_service.create_rule(rule)
        if rule_id:
            return {
                "success": True,
                "message": f"告警规则'{rule.rule_name}'创建成功",
                "data": {
                    "rule_id": rule_id,
                    "rule_name": rule.rule_name,
                    "redis_key": rule.redis_key(),
                    "monitoring": rule.enabled
                }
            }
        else:
            return {
                "success": False,
                "message": "数据库创建失败，请检查数据库连接和权限",
                "data": {"database_error": "insert operation failed"}
            }
            
    except KeyError as e:
        return {
            "success": False,
            "message": f"请求数据格式错误，缺少字段: {str(e)}",
            "data": {"format_error": str(e)}
        }
    except ValueError as e:
        return {
            "success": False,
            "message": f"数据值错误: {str(e)}",
            "data": {"value_error": str(e)}
        }
    except Exception as e:
        logger.error(f"创建告警规则失败: {e}")
        return {
            "success": False,
            "message": f"系统内部错误: {str(e)}",
            "data": {"system_error": str(e)}
        }


@app.get("/alarmApi/rules/{rule_id}")
async def get_alert_rule(rule_id: int):
    """获取指定ID的告警规则"""
    try:
        rule = alert_rule_service.get_rule_by_id(rule_id)
        if rule:
            return {
                "success": True,
                "message": "获取规则成功",
                "data": {
                    "total": 1,
                    "list": [rule.to_dict()]
                }
            }
        else:
            return {
                "success": False,
                "message": "规则不存在",
                "data": {
                    "total": 0,
                    "list": []
                }
            }
            
    except Exception as e:
        logger.error(f"获取告警规则失败: {e}")
        return {
            "success": False,
            "message": f"获取失败: {str(e)}",
            "data": {
                "total": 0,
                "list": []
            }
        }


@app.get("/alarmApi/rules")
async def list_alert_rules(
    keyword: str = Query("", description="关键词搜索，支持模糊匹配：规则名称、描述、通道ID、点位ID"),
    service_type: str = Query("", description="服务类型过滤：comsrv, rulesrv, modsrv等"),
    warning_level: Optional[int] = Query(None, description="告警级别过滤：1=低级, 2=中级, 3=高级"),
    enabled: Optional[bool] = Query(None, description="启用状态过滤"),
    start_time: Optional[str] = Query(None, description="开始时间，支持多种格式：2025-08-21、2025-08-21 00:00:00、2025-08-21T00:00:00等"),
    end_time: Optional[str] = Query(None, description="结束时间，支持多种格式：2025-08-21、2025-08-21 23:59:59、2025-08-21T23:59:59等"),
    page: int = Query(1, ge=1, description="页码，从1开始"),
    page_size: int = Query(10, ge=1, le=100, description="每页大小，最大100")
):
    """高级搜索告警规则列表"""
    try:
        # 使用增强的时间解析器
        from app.utils.time_parser import parse_time_range
        
        start_datetime, end_datetime = parse_time_range(start_time, end_time)
        
        # 如果时间解析失败，返回错误信息
        if start_time and not start_datetime:
            return {
                "success": False,
                "message": f"开始时间格式错误: {start_time}，支持格式：2025-08-21、2025-08-21 00:00:00、2025-08-21T00:00:00等",
                "data": {"total": 0, "list": []}
            }
        
        if end_time and not end_datetime:
            return {
                "success": False,
                "message": f"结束时间格式错误: {end_time}，支持格式：2025-08-21、2025-08-21 23:59:59、2025-08-21T23:59:59等",
                "data": {"total": 0, "list": []}
            }
        
        # 执行搜索
        result = alert_rule_service.search_rules(
            keyword=keyword,
            service_type=service_type,
            warning_level=warning_level,
            enabled=enabled,
            start_time=start_datetime,
            end_time=end_datetime,
            page=page,
            page_size=page_size
        )
        
        return result
        
    except Exception as e:
        logger.error(f"获取告警规则列表失败: {e}")
        return {
            "success": False,
            "message": f"查询失败: {str(e)}",
            "data": {"total": 0, "list": []}
        }


@app.get("/alarmApi/rules/channel/{channel_id}")
async def get_channel_rules(
    channel_id: int,
    service_type: str = Query("", description="服务类型，如：comsrv")
):
    """获取指定通道的告警规则"""
    try:
        if service_type:
            # 如果指定了服务类型，使用更精确的搜索
            result = alert_rule_service.search_rules(
                keyword=str(channel_id),
                service_type=service_type,
                page=1,
                page_size=100
            )
        else:
            rules = alert_rule_service.get_rules_by_channel(channel_id)
            result = {
                "success": True,
                "message": f"查询成功，共找到 {len(rules)} 条记录",
                "data": {
                    "total": len(rules),
                    "list": [rule.to_dict() for rule in rules]
                }
            }
        
        return result
        
    except Exception as e:
        logger.error(f"获取通道告警规则失败: {e}")
        return {
            "success": False,
            "message": f"查询失败: {str(e)}",
            "data": {"total": 0, "list": []}
        }


@app.put("/alarmApi/rules/{rule_id}")
async def update_alert_rule(rule_id: int, rule_data: dict):
    """更新告警规则"""
    try:
        # 确保规则ID正确
        rule_data["id"] = rule_id
        
        # 创建AlertRule对象
        rule = AlertRule.from_dict(rule_data)
        
        # 更新规则
        success = alert_rule_service.update_rule(rule)
        if success:
            # 通知监控引擎规则已更新
            asyncio.create_task(alarm_monitor.on_rule_updated(rule_id))
            return {
                "success": True,
                "message": "告警规则更新成功",
                "data": {"rule_id": rule_id}
            }
        else:
            return {
                "success": False,
                "message": "规则不存在或更新失败",
                "data": {}
            }
            
    except Exception as e:
        logger.error(f"更新告警规则失败: {e}")
        return {
            "success": False,
            "message": f"更新失败: {str(e)}",
            "data": {}
        }


@app.delete("/alarmApi/rules/{rule_id}")
async def delete_alert_rule(rule_id: int):
    """删除告警规则"""
    try:
        # 先通知监控引擎处理相关告警
        await alarm_monitor.on_rule_deleted(rule_id)
        
        success = alert_rule_service.delete_rule(rule_id)
        if success:
            return {
                "success": True,
                "message": "告警规则删除成功",
                "data": {"rule_id": rule_id}
            }
        else:
            return {
                "success": False,
                "message": "规则不存在",
                "data": {}
            }
            
    except Exception as e:
        logger.error(f"删除告警规则失败: {e}")
        return {
            "success": False,
            "message": f"删除失败: {str(e)}",
            "data": {}
        }


@app.patch("/alarmApi/rules/{rule_id}/enable")
async def enable_alert_rule(rule_id: int):
    """启用告警规则"""
    try:
        success = alert_rule_service.enable_rule(rule_id)
        if success:
            # 通知监控引擎规则已启用
            asyncio.create_task(alarm_monitor.on_rule_updated(rule_id))
            return {
                "success": True,
                "message": "告警规则启用成功",
                "data": {"rule_id": rule_id}
            }
        else:
            return {
                "success": False,
                "message": "规则不存在",
                "data": {}
            }
            
    except Exception as e:
        logger.error(f"启用告警规则失败: {e}")
        return {
            "success": False,
            "message": f"启用失败: {str(e)}",
            "data": {}
        }


@app.patch("/alarmApi/rules/{rule_id}/disable")
async def disable_alert_rule(rule_id: int):
    """禁用告警规则"""
    try:
        success = alert_rule_service.disable_rule(rule_id)
        if success:
            # 通知监控引擎规则已禁用（将解除相关告警）
            asyncio.create_task(alarm_monitor.on_rule_updated(rule_id))
            return {
                "success": True,
                "message": "告警规则禁用成功",
                "data": {"rule_id": rule_id}
            }
        else:
            return {
                "success": False,
                "message": "规则不存在",
                "data": {}
            }
            
    except Exception as e:
        logger.error(f"禁用告警规则失败: {e}")
        return {
            "success": False,
            "message": f"禁用失败: {str(e)}",
            "data": {}
        }


# ==================== 告警管理API ====================

@app.get("/alarmApi/alerts")
async def list_alerts(
    keyword: str = Query("", description="关键词搜索，支持规则名称、通道ID、点位ID"),
    service_type: str = Query("", description="服务类型过滤"),
    warning_level: Optional[int] = Query(None, description="告警级别过滤"),
    start_time: Optional[str] = Query(None, description="开始时间，支持多种格式：2025-08-21、2025-08-21 00:00:00、2025-08-21T00:00:00等"),
    end_time: Optional[str] = Query(None, description="结束时间，支持多种格式：2025-08-21、2025-08-21 23:59:59、2025-08-21T23:59:59等"),
    page: int = Query(1, ge=1, description="页码"),
    page_size: int = Query(10, ge=1, le=100, description="每页大小")
):
    """获取当前告警列表"""
    try:
        # 使用增强的时间解析器
        from app.utils.time_parser import parse_time_range
        
        start_datetime, end_datetime = parse_time_range(start_time, end_time)
        
        # 如果时间解析失败，返回错误信息
        if start_time and not start_datetime:
            return {
                "success": False,
                "message": f"开始时间格式错误: {start_time}，支持格式：2025-08-21、2025-08-21 00:00:00、2025-08-21T00:00:00等",
                "data": {"total": 0, "list": []}
            }
        
        if end_time and not end_datetime:
            return {
                "success": False,
                "message": f"结束时间格式错误: {end_time}，支持格式：2025-08-21、2025-08-21 23:59:59、2025-08-21T23:59:59等",
                "data": {"total": 0, "list": []}
            }
        
        # 执行搜索
        result = alert_service.search_alerts(
            keyword=keyword,
            warning_level=warning_level,
            service_type=service_type,
            start_time=start_datetime,
            end_time=end_datetime,
            page=page,
            page_size=page_size
        )
        
        return result
        
    except Exception as e:
        logger.error(f"获取告警列表失败: {e}")
        return {
            "success": False,
            "message": f"查询失败: {str(e)}",
            "data": {"total": 0, "list": []}
        }


@app.get("/alarmApi/alerts/{alert_id}")
async def get_alert(alert_id: int):
    """获取指定告警详情"""
    try:
        alert = alert_service.get_alert_by_id(alert_id)
        if alert:
            return {
                "success": True,
                "message": "获取告警成功",
                "data": {
                    "total": 1,
                    "list": [alert.to_dict()]
                }
            }
        else:
            return {
                "success": False,
                "message": "告警不存在",
                "data": {"total": 0, "list": []}
            }
            
    except Exception as e:
        logger.error(f"获取告警失败: {e}")
        return {
            "success": False,
            "message": f"获取失败: {str(e)}",
            "data": {"total": 0, "list": []}
        }


@app.patch("/alarmApi/alerts/{alert_id}/resolve")
async def resolve_alert(alert_id: int, resolve_data: dict = None):
    """手动解除告警"""
    try:
        recovery_value = resolve_data.get("recovery_value") if resolve_data else None
        
        success = alert_service.resolve_alert(alert_id, recovery_value)
        if success:
            return {
                "success": True,
                "message": "告警已解除",
                "data": {"alert_id": alert_id}
            }
        else:
            return {
                "success": False,
                "message": "告警不存在或解除失败",
                "data": {}
            }
            
    except Exception as e:
        logger.error(f"解除告警失败: {e}")
        return {
            "success": False,
            "message": f"解除失败: {str(e)}",
            "data": {}
        }


@app.get("/alarmApi/alert-events")
async def list_alert_events(
    keyword: str = Query("", description="关键词搜索，支持模糊匹配：规则名称、通道ID、点位ID"),
    service_type: str = Query("", description="服务类型过滤"),
    warning_level: Optional[int] = Query(None, description="告警级别过滤"),
    event_type: str = Query("", description="事件类型：trigger/recovery"),
    start_time: Optional[str] = Query(None, description="开始时间，支持多种格式：2025-08-21、2025-08-21 00:00:00、2025-08-21T00:00:00等"),
    end_time: Optional[str] = Query(None, description="结束时间，支持多种格式：2025-08-21、2025-08-21 23:59:59、2025-08-21T23:59:59等"),
    page: int = Query(1, ge=1, description="页码"),
    page_size: int = Query(10, ge=1, le=100, description="每页大小")
):
    """获取告警事件历史"""
    try:
        # 使用增强的时间解析器
        from app.utils.time_parser import parse_time_range
        
        start_datetime, end_datetime = parse_time_range(start_time, end_time)
        
        # 如果时间解析失败，返回错误信息
        if start_time and not start_datetime:
            return {
                "success": False,
                "message": f"开始时间格式错误: {start_time}，支持格式：2025-08-21、2025-08-21 00:00:00、2025-08-21T00:00:00等",
                "data": {"total": 0, "list": []}
            }
        
        if end_time and not end_datetime:
            return {
                "success": False,
                "message": f"结束时间格式错误: {end_time}，支持格式：2025-08-21、2025-08-21 23:59:59、2025-08-21T23:59:59等",
                "data": {"total": 0, "list": []}
            }
        
        # 执行查询
        result = alert_service.get_alert_events(
            keyword=keyword,
            warning_level=warning_level,
            service_type=service_type,
            event_type=event_type,
            start_time=start_datetime,
            end_time=end_datetime,
            page=page,
            page_size=page_size
        )
        
        return result
        
    except Exception as e:
        logger.error(f"获取告警事件失败: {e}")
        return {
            "success": False,
            "message": f"查询失败: {str(e)}",
            "data": {"total": 0, "list": []}
        }


@app.get("/alarmApi/alert-events/export")
async def export_alert_events(
    keyword: str = Query("", description="关键词搜索，支持模糊匹配：规则名称、通道ID、点位ID"),
    service_type: str = Query("", description="服务类型过滤：comsrv, rulesrv, modsrv等"),
    warning_level: Optional[int] = Query(None, description="告警级别过滤：1=一般, 2=重要, 3=紧急"),
    event_type: str = Query("", description="事件类型过滤：trigger=触发, recovery=恢复"),
    start_time: Optional[str] = Query(None, description="开始时间，支持多种格式：2025-08-21、2025-08-21 00:00:00、2025-08-21T00:00:00等"),
    end_time: Optional[str] = Query(None, description="结束时间，支持多种格式：2025-08-21、2025-08-21 23:59:59、2025-08-21T23:59:59等")
):
    """导出告警事件历史为CSV文件"""
    try:
        # 使用增强的时间解析器
        from app.utils.time_parser import parse_time_range
        
        start_datetime, end_datetime = parse_time_range(start_time, end_time)
        
        # 如果时间解析失败，返回错误信息
        if start_time and not start_datetime:
            raise HTTPException(status_code=400, detail=f"开始时间格式错误: {start_time}，支持格式：2025-08-21、2025-08-21 00:00:00、2025-08-21T00:00:00等")
        
        if end_time and not end_datetime:
            raise HTTPException(status_code=400, detail=f"结束时间格式错误: {end_time}，支持格式：2025-08-21、2025-08-21 23:59:59、2025-08-21T23:59:59等")
        
        # 调用服务层导出方法
        csv_content = alert_service.export_alert_events_csv(
            keyword=keyword,
            service_type=service_type,
            warning_level=warning_level,
            event_type=event_type,
            start_time=start_datetime,
            end_time=end_datetime
        )
        
        # 生成文件名（使用英文避免编码问题）
        current_time = datetime.now().strftime("%Y%m%d_%H%M%S")
        filename = f"alarm_events_export_{current_time}.csv"
        
        # 将CSV内容转换为字节流
        csv_bytes = csv_content.encode('utf-8-sig')  # 使用UTF-8 BOM以支持中文Excel
        
        # 返回文件下载响应
        return StreamingResponse(
            io.BytesIO(csv_bytes),
            media_type="text/csv",
            headers={
                "Content-Disposition": f"attachment; filename={filename}",
                "Cache-Control": "no-cache",
                "Content-Length": str(len(csv_bytes))
            }
        )
        
    except Exception as e:
        logger.error(f"导出告警事件失败: {e}")
        raise HTTPException(status_code=500, detail=f"导出失败: {str(e)}")


@app.get("/alarmApi/alert-statistics")
async def get_alert_statistics():
    """获取告警统计信息"""
    try:
        return alert_service.get_alert_statistics()
    except Exception as e:
        logger.error(f"获取告警统计失败: {e}")
        return {
            "success": False,
            "message": f"获取失败: {str(e)}",
            "data": {}
        }


# ==================== 监控管理API ====================

@app.get("/alarmApi/monitor/status")
async def get_monitor_status():
    """获取监控状态"""
    try:
        status = alarm_monitor.get_monitor_status()
        return {
            "success": True,
            "message": "获取监控状态成功",
            "data": status
        }
    except Exception as e:
        logger.error(f"获取监控状态失败: {e}")
        return {
            "success": False,
            "message": f"获取失败: {str(e)}",
            "data": {}
        }


@app.post("/alarmApi/monitor/check-rule/{rule_id}")
async def manual_check_rule(rule_id: int):
    """手动检查指定规则"""
    try:
        result = await alarm_monitor.manual_check_rule(rule_id)
        return result
    except Exception as e:
        logger.error(f"手动检查规则失败: {e}")
        return {
            "success": False,
            "message": f"检查失败: {str(e)}",
            "data": {}
        }


@app.post("/alarmApi/call-data")
async def call_data_broadcast():
    """手动触发当前告警广播到6006端口"""
    try:
        result = await alarm_monitor.broadcast_current_alarms()
        return result
    except Exception as e:
        logger.error(f"告警广播失败: {e}")
        return {
            "success": False,
            "message": f"广播失败: {str(e)}",
            "data": {}
        }


if __name__ == "__main__":
    # 确保日志目录存在
    log_dir = Path(settings.LOG_FILE).parent
    if not log_dir.exists():
        log_dir.mkdir(parents=True, exist_ok=True)
    
    # 启动服务
    uvicorn.run(
        "main:app",
        host=settings.HOST,
        port=settings.PORT,
        reload=settings.DEBUG,
        log_level=settings.LOG_LEVEL.lower()
    )
