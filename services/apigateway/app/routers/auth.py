"""
身份认证API路由
处理用户注册、登录、令牌刷新等认证相关接口
"""

import logging
from typing import Dict, Any, List
from fastapi import APIRouter, HTTPException, status, Depends
from fastapi.responses import JSONResponse

from app.models.auth import (
    UserCreate, UserLogin, UserUpdate, PasswordChange, 
    Token, RefreshTokenRequest
)
from app.services.user_service import get_user_service
from app.services.auth_service import get_auth_service
from app.middleware.auth import (
    get_current_active_user, get_optional_user, 
    admin_only, engineer_or_admin, any_authenticated
)

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/auth", tags=["身份认证"])


@router.post("/register", response_model=Dict[str, Any])
async def register(user_data: UserCreate):
    """
    用户注册
    
    - **username**: 用户名（3-50字符）
    - **password**: MD5加密后的密码（32位十六进制字符串）
    - **role_id**: 角色ID（可选，默认为3-查看者）
    """
    try:
        user_service = get_user_service()
        result = await user_service.register_user(user_data)
        
        return {
            "success": True,
            "message": "用户注册成功",
            "data": result
        }
        
    except ValueError as e:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=str(e)
        )
    except Exception as e:
        logger.error(f"用户注册异常: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="注册失败，请稍后重试"
        )


@router.post("/login", response_model=Dict[str, Any])
async def login(login_data: UserLogin):
    """
    用户登录
    
    - **username**: 用户名
    - **password**: MD5加密后的密码（32位十六进制字符串）
    
    返回访问令牌和刷新令牌
    """
    try:
        from app.services.user_service import UserService
        user_service = UserService()
        tokens = await user_service.authenticate_user(login_data)
        
        return {
            "success": True,
            "message": "登录成功",
            "data": {
                "access_token": tokens.access_token,
                "refresh_token": tokens.refresh_token,
                "token_type": tokens.token_type,
                "expires_in": tokens.expires_in
            }
        }
        
    except ValueError as e:
        # 密码错误等认证失败情况返回200状态码，但success为False
        return {
            "success": False,
            "message": str(e),
            "data": None
        }
    except Exception as e:
        logger.error(f"用户登录异常: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="登录失败，请稍后重试"
        )


@router.post("/refresh", response_model=Dict[str, Any])
async def refresh_token(token_request: RefreshTokenRequest):
    """
    刷新访问令牌
    
    - **refresh_token**: 刷新令牌
    
    返回新的访问令牌和刷新令牌
    """
    try:
        user_service = get_user_service()
        new_tokens = await user_service.refresh_token(token_request.refresh_token)
        
        return {
            "success": True,
            "message": "令牌刷新成功",
            "data": {
                "access_token": new_tokens.access_token,
                "refresh_token": new_tokens.refresh_token,
                "token_type": new_tokens.token_type,
                "expires_in": new_tokens.expires_in
            }
        }
        
    except ValueError as e:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail=str(e)
        )
    except Exception as e:
        logger.error(f"令牌刷新异常: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="令牌刷新失败，请重新登录"
        )


@router.post("/logout", response_model=Dict[str, Any])
async def logout(
    token_request: RefreshTokenRequest,
    current_user: dict = Depends(get_current_active_user)
):
    """
    用户退出登录
    
    - **refresh_token**: 刷新令牌
    
    撤销刷新令牌
    """
    try:
        user_service = get_user_service()
        result = await user_service.logout(token_request.refresh_token)
        
        return {
            "success": True,
            "message": result["message"]
        }
        
    except Exception as e:
        logger.error(f"退出登录异常: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="退出登录失败"
        )


@router.get("/me", response_model=Dict[str, Any])
async def get_current_user_info(current_user: dict = Depends(get_current_active_user)):
    """
    获取当前用户信息
    
    需要有效的访问令牌
    """
    return {
        "success": True,
        "message": "获取用户信息成功",
        "data": current_user
    }


@router.put("/me", response_model=Dict[str, Any])
async def update_current_user(
    update_data: UserUpdate,
    current_user: dict = Depends(get_current_active_user)
):
    """
    更新当前用户信息
    
    - **role_id**: 角色ID（可选，需要管理员权限）
    - **is_active**: 激活状态（可选，需要管理员权限）
    
    普通用户暂无可修改的字段
    """
    try:
        user_service = get_user_service()
        
        # 检查权限
        user_role = current_user.get("role", {}).get("name_en", "")
        if user_role != "Admin":
            # 非管理员不能修改角色和激活状态
            if update_data.role_id is not None or update_data.is_active is not None:
                raise HTTPException(
                    status_code=status.HTTP_403_FORBIDDEN,
                    detail="只有管理员可以修改角色和激活状态"
                )
        
        updated_user = await user_service.update_user(current_user["id"], update_data)
        
        return {
            "success": True,
            "message": "用户信息更新成功",
            "data": updated_user
        }
        
    except ValueError as e:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=str(e)
        )
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"更新用户信息异常: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="更新用户信息失败"
        )


@router.put("/me/password", response_model=Dict[str, Any])
async def change_password(
    password_data: PasswordChange,
    current_user: dict = Depends(get_current_active_user)
):
    """
    修改密码
    
    - **old_password**: 原密码（MD5加密后的32位十六进制字符串）
    - **new_password**: 新密码（MD5加密后的32位十六进制字符串）
    """
    try:
        user_service = get_user_service()
        result = await user_service.change_password(current_user["id"], password_data)
        
        return {
            "success": True,
            "message": result["message"]
        }
        
    except ValueError as e:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=str(e)
        )
    except Exception as e:
        logger.error(f"修改密码异常: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="修改密码失败"
        )


@router.get("/roles", response_model=Dict[str, Any])
async def get_roles():
    """
    获取所有角色列表
    
    公开接口，无需认证
    """
    try:
        user_service = get_user_service()
        roles = await user_service.get_all_roles()
        
        return {
            "success": True,
            "message": "获取角色列表成功",
            "data": roles,
            "total": len(roles)
        }
        
    except Exception as e:
        logger.error(f"获取角色列表异常: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="获取角色列表失败"
        )


@router.get("/users", response_model=Dict[str, Any])
async def get_all_users():
    """
    获取所有用户列表
    
    公开接口，无需认证
    返回用户基本信息（不包含密码等敏感信息）
    包含上次登录时间字段
    """
    try:
        user_service = get_user_service()
        users = await user_service.get_all_users_public()
        
        return {
            "success": True,
            "message": "获取用户列表成功",
            "data": {
                "total": len(users),
                "list": users
            }
        }
        
    except Exception as e:
        logger.error(f"获取用户列表异常: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="获取用户列表失败"
        )


@router.delete("/users/{user_id}", response_model=Dict[str, Any])
async def delete_user(user_id: int, current_user: dict = Depends(admin_only)):
    """
    删除用户
    
    需要管理员权限
    不能删除默认管理员用户
    """
    try:
        user_service = get_user_service()
        result = await user_service.delete_user(user_id)
        
        return {
            "success": True,
            "message": result["message"],
            "data": {
                "user_id": result["user_id"],
                "username": result["username"]
            }
        }
        
    except ValueError as e:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=str(e)
        )
    except Exception as e:
        logger.error(f"删除用户异常: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="删除用户失败"
        )


@router.get("/stats", response_model=Dict[str, Any])
async def get_auth_stats(current_user: dict = Depends(admin_only)):
    """
    获取认证统计信息
    
    需要管理员权限
    """
    try:
        auth_service = get_auth_service()
        stats = auth_service.get_token_stats()
        
        return {
            "success": True,
            "message": "获取认证统计成功",
            "data": stats
        }
        
    except Exception as e:
        logger.error(f"获取认证统计异常: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="获取认证统计失败"
        )


@router.post("/cleanup-tokens", response_model=Dict[str, Any])
async def cleanup_expired_tokens(current_user: dict = Depends(admin_only)):
    """
    清理过期令牌
    
    需要管理员权限
    """
    try:
        auth_service = get_auth_service()
        auth_service.cleanup_expired_tokens()
        
        return {
            "success": True,
            "message": "过期令牌清理完成"
        }
        
    except Exception as e:
        logger.error(f"清理过期令牌异常: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="清理过期令牌失败"
        )


# 管理员专用接口
@router.put("/users/{user_id}", response_model=Dict[str, Any])
async def admin_update_user(
    user_id: int,
    update_data: UserUpdate,
    current_user: dict = Depends(admin_only)
):
    """
    管理员更新用户信息
    
    需要管理员权限
    - **role_id**: 角色ID（可选）
    - **is_active**: 激活状态（可选）
    - **old_password**: 原密码（可选，需与new_password同时提供）
    - **new_password**: 新密码（可选，需与old_password同时提供）
    """
    try:
        user_service = get_user_service()
        messages = []
        
        # 如果提供了密码修改字段，先处理密码修改
        if update_data.old_password and update_data.new_password:
            from app.models.auth import PasswordChange
            password_data = PasswordChange(
                old_password=update_data.old_password,
                new_password=update_data.new_password
            )
            result = await user_service.change_password(user_id, password_data)
            messages.append(result["message"])
        elif update_data.old_password or update_data.new_password:
            # 只提供了其中一个密码字段，报错
            raise ValueError("修改密码需要同时提供old_password和new_password")
        
        # 处理其他字段的更新
        if update_data.role_id is not None or update_data.is_active is not None:
            updated_user = await user_service.update_user(user_id, update_data)
            messages.append("用户信息更新成功")
        else:
            # 如果没有其他字段需要更新，只是修改了密码
            if messages:
                # 获取用户信息返回
                updated_user = await user_service.get_user_info(user_id)
            else:
                raise ValueError("没有需要更新的字段")
        
        return {
            "success": True,
            "message": "；".join(messages) if messages else "更新成功",
            "data": updated_user
        }
        
    except ValueError as e:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail=str(e)
        )
    except Exception as e:
        logger.error(f"管理员更新用户信息异常: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="更新用户信息失败"
        )


@router.get("/users/{user_id}", response_model=Dict[str, Any])
async def admin_get_user(
    user_id: int,
    current_user: dict = Depends(admin_only)
):
    """
    管理员获取指定用户信息
    
    需要管理员权限
    """
    try:
        user_service = get_user_service()
        user_info = await user_service.get_user_info(user_id)
        
        return {
            "success": True,
            "message": "获取用户信息成功",
            "data": user_info
        }
        
    except ValueError as e:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=str(e)
        )
    except Exception as e:
        logger.error(f"管理员获取用户信息异常: {e}")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="获取用户信息失败"
        )
