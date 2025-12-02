"""
API路由模块
"""

from fastapi import APIRouter, HTTPException, Request, UploadFile, File, Form
from typing import Dict, Any
from pathlib import Path
import asyncio
from loguru import logger
from app.services.data_forwarder import data_forwarder
from app.services.alarm_broadcaster import alarm_broadcaster
from app.services.certificate_manager import certificate_manager
from app.core.database import redis_manager
from app.core.mqtt_client import mqtt_client
from app.core.config import settings
from app.core.config_loader import config_loader
# 由于模块导入问题，直接在此处定义模型
from pydantic import BaseModel, Field
from typing import Optional

class SSLConfig(BaseModel):
    """SSL配置"""
    enabled: bool = Field(default=False, description="是否启用SSL")
    ca_cert: Optional[str] = Field(default=None, description="CA证书路径")
    client_cert: Optional[str] = Field(default=None, description="客户端证书路径")
    client_key: Optional[str] = Field(default=None, description="客户端私钥路径")

class ReconnectConfig(BaseModel):
    """重连配置"""
    enabled: bool = Field(default=True, description="是否启用自动重连")
    max_attempts: int = Field(default=10, ge=1, le=100, description="最大重连次数")
    delay: int = Field(default=5, ge=1, le=300, description="重连间隔（秒）")

class StatusConfig(BaseModel):
    """状态消息配置"""
    will_message_enabled: bool = Field(default=True, description="是否启用遗嘱消息")
    auto_online_message: bool = Field(default=True, description="连接成功后是否自动发送在线消息")

class BrokerConfig(BaseModel):
    """MQTT代理配置"""
    host: str = Field(..., min_length=1, max_length=255, description="MQTT代理地址")
    port: int = Field(default=1883, ge=1, le=65535, description="MQTT代理端口")
    username: Optional[str] = Field(default="", max_length=255, description="用户名")
    password: Optional[str] = Field(default="", max_length=255, description="密码")
    client_id: str = Field(..., min_length=1, max_length=255, description="客户端ID")
    keepalive: int = Field(default=60, ge=10, le=3600, description="保活时间（秒）")
    ssl: Optional[SSLConfig] = Field(default_factory=SSLConfig, description="SSL配置")
    reconnect: Optional[ReconnectConfig] = Field(default_factory=ReconnectConfig, description="重连配置")
    status: Optional[StatusConfig] = Field(default_factory=StatusConfig, description="状态消息配置")

class MQTTConnectionConfig(BaseModel):
    """完整的MQTT连接配置"""
    broker: BrokerConfig = Field(..., description="MQTT代理配置")

class MQTTConfigUpdateRequest(BaseModel):
    """MQTT配置更新请求"""
    mqtt_connection: MQTTConnectionConfig = Field(..., description="MQTT连接配置")

class MQTTConfigResponse(BaseModel):
    """MQTT配置响应"""
    status: str = Field(default="success", description="响应状态")
    message: str = Field(default="操作成功", description="响应消息")
    data: Optional[dict] = Field(default=None, description="当前MQTT配置")

class MQTTDisconnectResponse(BaseModel):
    """MQTT断开连接响应"""
    status: str = Field(default="success", description="响应状态")
    message: str = Field(default="连接已关闭", description="响应消息")
    was_connected: bool = Field(default=False, description="之前是否已连接")

class MQTTReconnectResponse(BaseModel):
    """MQTT重连响应"""
    status: str = Field(default="success", description="响应状态")
    message: str = Field(default="重连操作已启动", description="响应消息")
    connection_status: bool = Field(default=False, description="当前连接状态")

class MQTTStatusResponse(BaseModel):
    """MQTT状态响应"""
    status: str = Field(default="success", description="响应状态")
    connected: bool = Field(default=False, description="是否已连接")
    current_config: Optional[dict] = Field(default=None, description="当前配置信息")
    reconnect_attempts: int = Field(default=0, description="当前重连尝试次数")

class CertificateUploadResponse(BaseModel):
    """证书上传响应"""
    status: str = Field(default="success", description="响应状态")
    message: str = Field(default="证书上传成功", description="响应消息")
    cert_type: str = Field(description="证书类型")
    filename: str = Field(description="保存的文件名")
    path: str = Field(description="证书在配置文件中的路径")
    config_updated: bool = Field(default=True, description="配置文件是否已更新")

class CertificateInfoResponse(BaseModel):
    """证书信息响应"""
    status: str = Field(default="success", description="响应状态")
    ssl_enabled: bool = Field(description="SSL是否启用")
    certificates: Dict[str, Any] = Field(description="证书信息")

class CertificateDeleteResponse(BaseModel):
    """证书删除响应"""
    status: str = Field(default="success", description="响应状态")
    message: str = Field(default="证书删除成功", description="响应消息")
    cert_type: str = Field(description="证书类型")

router = APIRouter(prefix="/netApi", tags=["网络服务"])

# 告警接口已改为直接接收JSON数据，不再需要特定的请求模型

@router.get("/health")
async def health_check():
    """健康检查接口"""
    return {
        "status": "healthy",
        "service": settings.APP_NAME,
        "version": settings.APP_VERSION,
        "redis_connected": redis_manager.is_connected(),
        "mqtt_connected": mqtt_client.is_connected,
        "forwarder_running": data_forwarder.is_running,
        "message": "网络服务运行正常"
    }

@router.post("/alarm/broadcast")
async def broadcast_alarm(request: Request):
    """
    告警广播接口
    
    接收任意JSON格式的告警数据，并原样通过MQTT发布到 alarm/{productSN}/{deviceSN} 主题
    """
    try:
        # 直接获取原始JSON数据
        alarm_data = await request.json()
        
        # 验证告警数据格式
        is_valid, error_msg = alarm_broadcaster.validate_alarm_data(alarm_data)
        if not is_valid:
            raise HTTPException(status_code=400, detail=f"告警数据格式错误: {error_msg}")
        
        # 检查MQTT连接状态，如果未连接则等待一段时间后重试
        if not mqtt_client.is_connected:
            # 等待最多30秒，每5秒检查一次连接状态
            for i in range(6):
                await asyncio.sleep(5)
                if mqtt_client.is_connected:
                    break
                logger.warning(f"MQTT未连接，等待重连... ({i+1}/6)")
            
            # 如果仍然未连接，返回503错误
            if not mqtt_client.is_connected:
                raise HTTPException(status_code=503, detail="MQTT服务未连接，无法发送告警")
        
        # 广播告警消息（原样转发）
        success = alarm_broadcaster.broadcast_alarm(alarm_data)
        
        if success:
            return {
                "status": "success",
                "message": "告警消息发送成功",
                "topic": alarm_broadcaster.get_alarm_topic(),
                "timestamp": alarm_data.get("timestamp"),
                "data_size": len(str(alarm_data))
            }
        else:
            raise HTTPException(status_code=500, detail="告警消息发送失败")
            
    except HTTPException:
        raise
    except ValueError as e:
        raise HTTPException(status_code=400, detail=f"JSON格式错误: {str(e)}")
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"服务内部错误: {str(e)}")

@router.get("/alarm/config")
async def get_alarm_config():
    """
    获取告警配置信息
    """
    try:
        return {
            "status": "success",
            "config": {
                "alarm_topic": alarm_broadcaster.get_alarm_topic(),
                "mqtt_connected": mqtt_client.is_connected,
                "service_status": "running" if alarm_broadcaster.formatted_alarm_topic else "not_configured"
            }
        }
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"获取告警配置失败: {str(e)}")


# MQTT配置管理接口

@router.get("/mqtt/config", response_model=MQTTConfigResponse)
async def get_mqtt_config():
    """
    获取MQTT连接配置
    
    返回当前的MQTT连接配置，包括代理设置、SSL配置、重连配置等
    """
    try:
        # 获取当前配置（使用API专用方法，返回原始相对路径）
        mqtt_config = config_loader.get_mqtt_connection_config_for_api()
        
        # 获取连接状态
        connection_status = mqtt_client.get_connection_status()
        
        # 移除敏感信息（密码）
        if 'broker' in mqtt_config and 'password' in mqtt_config['broker']:
            mqtt_config['broker']['password'] = "***" if mqtt_config['broker']['password'] else ""
        
        return MQTTConfigResponse(
            status="success",
            message="获取MQTT配置成功",
            data={
                "broker": mqtt_config.get('broker', {})
            }
        )
        
    except Exception as e:
        logger.error(f"获取MQTT配置失败: {e}")
        raise HTTPException(status_code=500, detail=f"获取MQTT配置失败: {str(e)}")


@router.post("/mqtt/config", response_model=MQTTConfigResponse)
async def update_mqtt_config(config_request: MQTTConfigUpdateRequest):
    """
    更新MQTT连接配置
    
    更新MQTT连接配置并自动断线重连。配置将保存到YAML文件中。
    """
    try:
        # 转换为字典格式
        new_config = config_request.mqtt_connection.model_dump()
        
        # 验证配置
        is_valid, error_msg = config_loader.validate_mqtt_connection_config(new_config)
        if not is_valid:
            raise HTTPException(status_code=400, detail=f"配置验证失败: {error_msg}")
        
        # 保存配置到文件
        if not config_loader.update_mqtt_connection_config(new_config):
            raise HTTPException(status_code=500, detail="保存MQTT配置失败")
        
        # 重新加载配置并重连
        reconnect_success = mqtt_client.reload_config_and_reconnect()
        
        if reconnect_success:
            logger.info("MQTT配置更新并重连成功")
            response_message = "MQTT配置更新并重连成功"
        else:
            logger.warning("MQTT配置更新成功，但重连失败")
            response_message = "MQTT配置更新成功，但重连失败，请检查配置或网络连接"
        
        # 返回更新后的配置（移除敏感信息）
        updated_config = config_loader.get_mqtt_connection_config_for_api()
        if 'broker' in updated_config and 'password' in updated_config['broker']:
            updated_config['broker']['password'] = "***" if updated_config['broker']['password'] else ""
        
        return MQTTConfigResponse(
            status="success",
            message=response_message,
            data={
                "broker": updated_config.get('broker', {})
            }
        )
        
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"更新MQTT配置失败: {e}")
        raise HTTPException(status_code=500, detail=f"更新MQTT配置失败: {str(e)}")


@router.post("/mqtt/disconnect", response_model=MQTTDisconnectResponse)
async def disconnect_mqtt():
    """
    关闭MQTT连接
    
    主动关闭当前MQTT连接，禁用自动重连。
    """
    try:
        # 获取关闭前的连接状态
        was_connected = mqtt_client.is_connected
        
        # 关闭连接
        success = mqtt_client.close_connection()
        
        if success:
            message = "MQTT连接已成功关闭" if was_connected else "MQTT连接已经处于关闭状态"
            logger.info(message)
        else:
            message = "MQTT连接关闭失败"
            logger.warning(message)
        
        return MQTTDisconnectResponse(
            status="success" if success else "failed",
            message=message,
            was_connected=was_connected
        )
        
    except Exception as e:
        logger.error(f"关闭MQTT连接失败: {e}")
        raise HTTPException(status_code=500, detail=f"关闭MQTT连接失败: {str(e)}")


@router.post("/mqtt/reconnect", response_model=MQTTReconnectResponse)
async def reconnect_mqtt():
    """
    MQTT重新连接
    
    重新加载配置并重新连接MQTT。无论当前是否连接都可以使用。
    """
    try:
        # 重新加载配置并重连
        success = mqtt_client.reload_config_and_reconnect()
        
        # 获取当前连接状态
        connection_status = mqtt_client.get_connection_status()
        
        if success:
            message = "MQTT重连成功"
            logger.info(message)
        else:
            message = "MQTT重连失败，请检查配置或网络连接"
            logger.warning(message)
        
        return MQTTReconnectResponse(
            status="success" if success else "failed",
            message=message,
            connection_status=connection_status.get("connected", False)
        )
        
    except Exception as e:
        logger.error(f"MQTT重连操作失败: {e}")
        raise HTTPException(status_code=500, detail=f"MQTT重连操作失败: {str(e)}")


@router.get("/mqtt/status", response_model=MQTTStatusResponse)
async def get_mqtt_status():
    """
    获取MQTT连接状态
    
    返回当前MQTT连接状态和基本配置信息
    """
    try:
        # 获取连接状态
        connection_status = mqtt_client.get_connection_status()
        
        # 获取当前配置概要
        current_config = {
            "host": connection_status.get("host", "unknown"),
            "port": connection_status.get("port", 1883),
            "client_id": connection_status.get("client_id", "unknown"),
            "reconnect_enabled": connection_status.get("reconnect_enabled", False),
            "config_file": config_loader.get_config_file_path()
        }
        
        return MQTTStatusResponse(
            status="success",
            connected=connection_status.get("connected", False),
            current_config=current_config,
            reconnect_attempts=connection_status.get("reconnect_attempts", 0)
        )
        
    except Exception as e:
        logger.error(f"获取MQTT状态失败: {e}")
        raise HTTPException(status_code=500, detail=f"获取MQTT状态失败: {str(e)}")


# 证书管理接口

@router.post("/certificate/upload", response_model=CertificateUploadResponse)
async def upload_certificate(
    cert_type: str = Form(..., description="证书类型 (ca_cert, client_cert, client_key)"),
    file: UploadFile = File(..., description="证书文件")
):
    """
    上传证书文件
    
    支持上传三种类型的证书文件：
    - ca_cert: CA根证书
    - client_cert: 客户端证书
    - client_key: 客户端私钥
    
    上传成功后会自动更新配置文件中的证书路径，并删除旧证书文件。
    """
    try:
        # 验证证书类型
        if cert_type not in ['ca_cert', 'client_cert', 'client_key']:
            raise HTTPException(
                status_code=400, 
                detail=f"不支持的证书类型: {cert_type}。支持的类型: ca_cert, client_cert, client_key"
            )
        
        # 验证文件类型
        if not file.filename:
            raise HTTPException(status_code=400, detail="文件名不能为空")
        
        # 检查文件扩展名
        allowed_extensions = ['.pem', '.crt', '.key', '.cer', '.p12', '.pfx']
        file_ext = Path(file.filename).suffix.lower()
        if file_ext not in allowed_extensions:
            raise HTTPException(
                status_code=400, 
                detail=f"不支持的文件类型: {file_ext}。支持的类型: {', '.join(allowed_extensions)}"
            )
        
        # 读取文件内容
        file_content = await file.read()
        
        # 验证文件大小（限制为1MB）
        max_size = 1024 * 1024  # 1MB
        if len(file_content) > max_size:
            raise HTTPException(status_code=400, detail=f"文件大小超过限制: {len(file_content)} bytes，最大允许: {max_size} bytes")
        
        # 验证文件内容不为空
        if len(file_content) == 0:
            raise HTTPException(status_code=400, detail="文件内容不能为空")
        
        # 上传证书
        success, message, saved_filename = certificate_manager.upload_certificate(cert_type, file_content, file.filename)
        
        if success:
            # 获取证书在配置文件中的路径
            cert_path = f"cert/{saved_filename}"
            
            return CertificateUploadResponse(
                status="success",
                message=message,
                cert_type=cert_type,
                filename=saved_filename,
                path=cert_path,
                config_updated=True
            )
        else:
            raise HTTPException(status_code=500, detail=message)
            
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"上传证书失败: {e}")
        raise HTTPException(status_code=500, detail=f"上传证书失败: {str(e)}")


@router.get("/certificate/info", response_model=CertificateInfoResponse)
async def get_certificate_info():
    """
    获取当前证书信息
    
    返回当前配置的证书文件信息，包括文件路径、是否存在、文件大小等。
    """
    try:
        cert_info = certificate_manager.get_certificate_info()
        
        return CertificateInfoResponse(
            status="success",
            ssl_enabled=cert_info.get('ssl_enabled', False),
            certificates=cert_info.get('certificates', {})
        )
        
    except Exception as e:
        logger.error(f"获取证书信息失败: {e}")
        raise HTTPException(status_code=500, detail=f"获取证书信息失败: {str(e)}")


@router.delete("/certificate/{cert_type}", response_model=CertificateDeleteResponse)
async def delete_certificate(cert_type: str):
    """
    删除证书文件
    
    删除指定类型的证书文件，并清空配置文件中的对应路径。
    
    支持的证书类型：
    - ca_cert: CA根证书
    - client_cert: 客户端证书  
    - client_key: 客户端私钥
    """
    try:
        # 验证证书类型
        if cert_type not in ['ca_cert', 'client_cert', 'client_key']:
            raise HTTPException(
                status_code=400, 
                detail=f"不支持的证书类型: {cert_type}。支持的类型: ca_cert, client_cert, client_key"
            )
        
        # 删除证书
        success, message = certificate_manager.delete_certificate(cert_type)
        
        if success:
            return CertificateDeleteResponse(
                status="success",
                message=message,
                cert_type=cert_type
            )
        else:
            raise HTTPException(status_code=500, detail=message)
            
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"删除证书失败: {e}")
        raise HTTPException(status_code=500, detail=f"删除证书失败: {str(e)}")


