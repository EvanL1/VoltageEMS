"""
身份认证相关数据模型
"""

from datetime import datetime
from typing import Optional, Dict, Any
from pydantic import BaseModel, Field
from enum import Enum


class RoleType(str, Enum):
    """角色类型枚举"""
    ADMIN = "Admin"
    ENGINEER = "Engineer" 
    VIEWER = "Viewer"


class Role(BaseModel):
    """角色模型"""
    id: int
    name_en: str = Field(..., description="英文角色名")
    name_zh: str = Field(..., description="中文角色名")
    description: Optional[str] = Field(None, description="角色描述")
    created_at: datetime
    updated_at: datetime

    class Config:
        from_attributes = True


class User(BaseModel):
    """用户模型"""
    id: int
    username: str = Field(..., min_length=3, max_length=50, description="用户名")
    password_hash: str = Field(..., description="加密后的密码")
    role_id: int = Field(..., description="角色ID")
    is_active: bool = Field(default=True, description="是否激活")
    last_login: Optional[datetime] = Field(None, description="最后登录时间")
    created_at: datetime
    updated_at: datetime

    class Config:
        from_attributes = True


class UserWithRole(BaseModel):
    """包含角色信息的用户模型"""
    id: int
    username: str
    is_active: bool
    last_login: Optional[datetime]
    created_at: datetime
    updated_at: datetime
    role: Role

    class Config:
        from_attributes = True


class UserCreate(BaseModel):
    """用户创建模型"""
    username: str = Field(..., min_length=3, max_length=50)
    password: str = Field(..., min_length=32, max_length=32, description="MD5加密后的密码（32位十六进制字符串）")
    role_id: int = Field(default=3, description="默认为查看者角色")


class UserLogin(BaseModel):
    """用户登录模型"""
    username: str = Field(..., description="用户名")
    password: str = Field(..., min_length=32, max_length=32, description="MD5加密后的密码（32位十六进制字符串）")


class UserUpdate(BaseModel):
    """用户更新模型"""
    role_id: Optional[int] = None
    is_active: Optional[bool] = None
    old_password: Optional[str] = Field(None, min_length=32, max_length=32, description="原密码（MD5加密后的32位十六进制字符串）")
    new_password: Optional[str] = Field(None, min_length=32, max_length=32, description="新密码（MD5加密后的32位十六进制字符串）")


class PasswordChange(BaseModel):
    """密码修改模型"""
    old_password: str = Field(..., min_length=32, max_length=32, description="原密码（MD5加密后的32位十六进制字符串）")
    new_password: str = Field(..., min_length=32, max_length=32, description="新密码（MD5加密后的32位十六进制字符串）")


class Token(BaseModel):
    """令牌模型"""
    access_token: str
    refresh_token: str
    token_type: str = "bearer"
    expires_in: int  # 秒数


class TokenData(BaseModel):
    """令牌数据模型"""
    user_id: Optional[int] = None
    username: Optional[str] = None
    role: Optional[str] = None


class RefreshTokenRequest(BaseModel):
    """刷新令牌请求模型"""
    refresh_token: str
