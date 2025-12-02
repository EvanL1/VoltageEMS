"""
身份认证中间件
提供JWT令牌验证装饰器和依赖注入
"""

import logging
from typing import Optional
from fastapi import Depends, HTTPException, status
from fastapi.security import HTTPBearer, HTTPAuthorizationCredentials

from app.models.auth import TokenData, UserWithRole
from app.services.auth_service import get_auth_service
from app.services.user_service import get_user_service

logger = logging.getLogger(__name__)

# HTTP Bearer 认证方案
security = HTTPBearer(auto_error=False)


class AuthMiddleware:
    """认证中间件"""
    
    def __init__(self):
        self.auth_service = None
        self.user_service = None
    
    def _get_auth_service(self):
        """延迟初始化认证服务"""
        if self.auth_service is None:
            self.auth_service = get_auth_service()
        return self.auth_service
    
    def _get_user_service(self):
        """延迟初始化用户服务"""
        if self.user_service is None:
            self.user_service = get_user_service()
        return self.user_service
    
    async def get_current_user(
        self, 
        credentials: Optional[HTTPAuthorizationCredentials] = Depends(security)
    ) -> dict:
        """获取当前用户信息（必须认证）"""
        if not credentials:
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="缺少认证令牌",
                headers={"WWW-Authenticate": "Bearer"},
            )
        
        # 验证访问令牌
        try:
            token_data = self._get_auth_service().verify_access_token(credentials.credentials)
            if not token_data:
                raise HTTPException(
                    status_code=status.HTTP_401_UNAUTHORIZED,
                    detail="无效或已过期的认证令牌",
                    headers={"WWW-Authenticate": "Bearer"},
                )
        except HTTPException:
            # 重新抛出HTTP异常
            raise
        except Exception as e:
            logger.error(f"令牌验证异常: {e}")
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="令牌验证失败",
                headers={"WWW-Authenticate": "Bearer"},
            )
        
        # 获取用户信息
        try:
            user_info = await self._get_user_service().get_user_info(token_data.user_id)
            return user_info
        except ValueError as e:
            # 用户不存在或被禁用等业务逻辑错误
            logger.warning(f"用户验证失败: {e}")
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail="用户不存在或已被禁用",
                headers={"WWW-Authenticate": "Bearer"},
            )
        except Exception as e:
            # 数据库连接错误等系统异常
            logger.error(f"获取用户信息失败: {e}")
            raise HTTPException(
                status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                detail="认证服务暂时不可用"
            )
    
    async def get_current_active_user(
        self, 
        current_user: dict = Depends(lambda: auth_middleware.get_current_user)
    ) -> dict:
        """获取当前激活用户（必须认证且激活）"""
        if not current_user.get("is_active", False):
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail="用户账号已被禁用"
            )
        return current_user
    
    async def get_optional_user(
        self,
        credentials: Optional[HTTPAuthorizationCredentials] = Depends(security)
    ) -> Optional[dict]:
        """获取可选用户信息（可选认证）"""
        if not credentials:
            return None
        
        # 验证访问令牌
        try:
            token_data = self._get_auth_service().verify_access_token(credentials.credentials)
            if not token_data:
                return None
        except Exception as e:
            logger.debug(f"可选认证令牌验证失败: {e}")
            return None
        
        # 获取用户信息
        try:
            user_info = await self._get_user_service().get_user_info(token_data.user_id)
            return user_info if user_info.get("is_active", False) else None
        except Exception as e:
            logger.debug(f"可选认证获取用户信息失败: {e}")
            return None


# 全局认证中间件实例
auth_middleware = AuthMiddleware()


# 依赖注入函数
async def get_current_user(
    credentials: Optional[HTTPAuthorizationCredentials] = Depends(security)
) -> dict:
    """获取当前用户（依赖注入）"""
    return await auth_middleware.get_current_user(credentials)


async def get_current_active_user(
    current_user: dict = Depends(get_current_user)
) -> dict:
    """获取当前激活用户（依赖注入）"""
    return await auth_middleware.get_current_active_user(current_user)


async def get_optional_user(
    credentials: Optional[HTTPAuthorizationCredentials] = Depends(security)
) -> Optional[dict]:
    """获取可选用户（依赖注入）"""
    return await auth_middleware.get_optional_user(credentials)


def require_role(*allowed_roles: str):
    """角色权限装饰器"""
    def decorator(func):
        async def wrapper(*args, **kwargs):
            # 从kwargs中获取current_user
            current_user = kwargs.get('current_user')
            if not current_user:
                raise HTTPException(
                    status_code=status.HTTP_401_UNAUTHORIZED,
                    detail="需要认证"
                )
            
            user_role = current_user.get("role", {}).get("name_en", "")
            if user_role not in allowed_roles:
                raise HTTPException(
                    status_code=status.HTTP_403_FORBIDDEN,
                    detail=f"需要以下角色之一: {', '.join(allowed_roles)}"
                )
            
            return await func(*args, **kwargs)
        return wrapper
    return decorator


def require_admin(
    current_user: dict = Depends(get_current_active_user)
) -> dict:
    """需要管理员权限（依赖注入）"""
    user_role = current_user.get("role", {}).get("name_en", "")
    if user_role != "Admin":
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="需要管理员权限"
        )
    return current_user


def require_engineer_or_admin(
    current_user: dict = Depends(get_current_active_user)
) -> dict:
    """需要工程师或管理员权限（依赖注入）"""
    user_role = current_user.get("role", {}).get("name_en", "")
    if user_role not in ["Admin", "Engineer"]:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="需要工程师或管理员权限"
        )
    return current_user


class RoleChecker:
    """角色检查器"""
    
    def __init__(self, allowed_roles: list):
        self.allowed_roles = allowed_roles
    
    def __call__(self, current_user: dict = Depends(get_current_active_user)) -> dict:
        user_role = current_user.get("role", {}).get("name_en", "")
        if user_role not in self.allowed_roles:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"需要以下角色之一: {', '.join(self.allowed_roles)}"
            )
        return current_user


# 预定义的角色检查器
admin_only = RoleChecker(["Admin"])
engineer_or_admin = RoleChecker(["Admin", "Engineer"])
any_authenticated = RoleChecker(["Admin", "Engineer", "Viewer"])
