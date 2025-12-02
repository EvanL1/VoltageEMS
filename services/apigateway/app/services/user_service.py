"""
用户管理服务
处理用户注册、登录、信息管理等功能
"""

import logging
from typing import Optional, Dict, Any
from datetime import datetime

from app.models.auth import UserCreate, UserLogin, UserUpdate, PasswordChange, Token, UserWithRole
from app.services.database import get_database
from app.services.auth_service import get_auth_service

logger = logging.getLogger(__name__)


class UserService:
    """用户管理服务"""
    
    def __init__(self):
        self.db = None
        self.auth = None
    
    def _get_database(self):
        """延迟初始化数据库"""
        if self.db is None:
            self.db = get_database()
        return self.db
    
    def _get_auth_service(self):
        """延迟初始化认证服务"""
        if self.auth is None:
            self.auth = get_auth_service()
        return self.auth
    
    async def register_user(self, user_data: UserCreate) -> Dict[str, Any]:
        """用户注册"""
        try:
            # 检查用户名是否已存在
            existing_user = await self._get_database().get_user_by_username(user_data.username)
            if existing_user:
                raise ValueError("用户名已存在")
            
            # 加密密码
            password_hash = self._get_auth_service().hash_password(user_data.password)
            
            # 创建用户
            user_id = await self._get_database().create_user(
                username=user_data.username,
                password_hash=password_hash,
                role_id=user_data.role_id
            )
            
            # 获取用户完整信息
            user_info = await self._get_database().get_user_with_role(user_id)
            if not user_info:
                raise RuntimeError("创建用户后无法获取用户信息")
            
            logger.info(f"用户注册成功: {user_data.username} (ID: {user_id})")
            
            return {
                "user_id": user_id,
                "username": user_data.username,
                "role": user_info["role"],
                "message": "用户注册成功"
            }
            
        except ValueError as e:
            logger.warning(f"用户注册失败: {e}")
            raise
        except Exception as e:
            logger.error(f"用户注册异常: {e}")
            raise RuntimeError("注册失败，请稍后重试")
    
    async def authenticate_user(self, login_data: UserLogin) -> Token:
        """用户登录认证"""
        try:
            # 仅支持用户名登录
            user = await self._get_database().get_user_by_username(login_data.username)
            
            if not user:
                raise ValueError("用户不存在")
            
            # 检查用户是否激活
            if not user["is_active"]:
                raise ValueError("用户账号已被禁用")
            
            # 验证密码
            if not self._get_auth_service().verify_password(login_data.password, user["password_hash"]):
                raise ValueError("密码错误")
            
            # 获取用户完整信息（包含角色）
            user_info = await self._get_database().get_user_with_role(user["id"])
            if not user_info:
                raise RuntimeError("无法获取用户角色信息")
            
            # 更新最后登录时间
            await self._get_database().update_user_login_time(user["id"])
            
            # 生成令牌
            tokens = self._get_auth_service().create_tokens(user_info)
            
            logger.info(f"用户登录成功: {user['username']} (ID: {user['id']})")
            
            return tokens
            
        except ValueError as e:
            logger.warning(f"用户登录失败: {e}")
            raise
        except Exception as e:
            logger.error(f"用户登录异常: {e}")
            raise RuntimeError("登录失败，请稍后重试")
    
    async def refresh_token(self, refresh_token: str) -> Token:
        """刷新访问令牌"""
        try:
            # 验证刷新令牌
            token_data = self._get_auth_service().verify_refresh_token(refresh_token)
            if not token_data:
                raise ValueError("无效的刷新令牌")
            
            # 获取用户信息
            user_info = await self._get_database().get_user_with_role(token_data.user_id)
            if not user_info:
                raise ValueError("用户不存在")
            
            # 检查用户是否激活
            if not user_info["is_active"]:
                raise ValueError("用户账号已被禁用")
            
            # 撤销旧的刷新令牌
            self._get_auth_service().revoke_refresh_token(refresh_token)
            
            # 生成新的令牌对
            new_tokens = self._get_auth_service().create_tokens(user_info)
            
            logger.info(f"令牌刷新成功: {user_info['username']} (ID: {user_info['id']})")
            
            return new_tokens
            
        except ValueError as e:
            logger.warning(f"令牌刷新失败: {e}")
            raise
        except Exception as e:
            logger.error(f"令牌刷新异常: {e}")
            raise RuntimeError("令牌刷新失败，请重新登录")
    
    async def get_user_info(self, user_id: int) -> Dict[str, Any]:
        """获取用户信息"""
        try:
            user_info = await self._get_database().get_user_with_role(user_id)
            if not user_info:
                raise ValueError("用户不存在")
            
            return user_info
            
        except ValueError as e:
            logger.warning(f"获取用户信息失败: {e}")
            raise
        except Exception as e:
            logger.error(f"获取用户信息异常: {e}")
            raise RuntimeError("获取用户信息失败")
    
    async def update_user(self, user_id: int, update_data: UserUpdate) -> Dict[str, Any]:
        """更新用户信息"""
        try:
            # 检查用户是否存在
            user = await self._get_database().get_user_by_id(user_id)
            if not user:
                raise ValueError("用户不存在")
            
            # 构建更新语句
            update_fields = []
            params = []
            
            if update_data.role_id is not None:
                # 验证角色ID是否有效
                roles = await self._get_database().get_all_roles()
                valid_role_ids = [role["id"] for role in roles]
                if update_data.role_id not in valid_role_ids:
                    raise ValueError("无效的角色ID")
                update_fields.append("role_id = ?")
                params.append(update_data.role_id)
            
            if update_data.is_active is not None:
                update_fields.append("is_active = ?")
                params.append(update_data.is_active)
            
            if not update_fields:
                raise ValueError("没有需要更新的字段")
            
            # 执行更新
            update_fields.append("updated_at = CURRENT_TIMESTAMP")
            params.append(user_id)
            
            query = f"UPDATE users SET {', '.join(update_fields)} WHERE id = ?"
            await self._get_database().execute_update(query, tuple(params))
            
            # 返回更新后的用户信息
            updated_user = await self._get_database().get_user_with_role(user_id)
            
            logger.info(f"用户信息更新成功: {user['username']} (ID: {user_id})")
            
            return updated_user
            
        except ValueError as e:
            logger.warning(f"更新用户信息失败: {e}")
            raise
        except Exception as e:
            logger.error(f"更新用户信息异常: {e}")
            raise RuntimeError("更新用户信息失败")
    
    async def change_password(self, user_id: int, password_data: PasswordChange) -> Dict[str, str]:
        """修改密码"""
        try:
            # 获取用户信息
            user = await self._get_database().get_user_by_id(user_id)
            if not user:
                raise ValueError("用户不存在")
            
            # 验证原密码
            if not self._get_auth_service().verify_password(password_data.old_password, user["password_hash"]):
                raise ValueError("原密码错误")
            
            # 加密新密码
            new_password_hash = self._get_auth_service().hash_password(password_data.new_password)
            
            # 更新密码
            await self._get_database().execute_update("""
                UPDATE users 
                SET password_hash = ?, updated_at = CURRENT_TIMESTAMP
                WHERE id = ?
            """, (new_password_hash, user_id))
            
            logger.info(f"密码修改成功: {user['username']} (ID: {user_id})")
            
            return {"message": "密码修改成功"}
            
        except ValueError as e:
            logger.warning(f"密码修改失败: {e}")
            raise
        except Exception as e:
            logger.error(f"密码修改异常: {e}")
            raise RuntimeError("密码修改失败")
    
    async def logout(self, refresh_token: str) -> Dict[str, str]:
        """用户退出登录"""
        try:
            # 撤销刷新令牌
            success = self._get_auth_service().revoke_refresh_token(refresh_token)
            
            if success:
                logger.info("用户退出登录成功")
                return {"message": "退出登录成功"}
            else:
                return {"message": "令牌已失效"}
                
        except Exception as e:
            logger.error(f"退出登录异常: {e}")
            raise RuntimeError("退出登录失败")
    
    async def get_all_roles(self) -> list:
        """获取所有角色"""
        try:
            roles = await self._get_database().get_all_roles()
            return [
                {
                    "id": role["id"],
                    "name_en": role["name_en"],
                    "name_zh": role["name_zh"],
                    "description": role["description"]
                }
                for role in roles
            ]
        except Exception as e:
            logger.error(f"获取角色列表异常: {e}")
            raise RuntimeError("获取角色列表失败")
    
    async def get_all_users_public(self) -> list:
        """获取所有用户的公开信息（不包含密码等敏感信息）"""
        try:
            users = await self._get_database().get_all_users_with_roles()
            return [
                {
                    "id": user["id"],
                    "username": user["username"],
                    "role": {
                        "id": user["role_id"],
                        "name_en": user["role_name_en"],
                        "name_zh": user["role_name_zh"]
                    },
                    "created_at": user["created_at"] if user["created_at"] else "",
                    "last_login": user["last_login"] if user["last_login"] else "",
                    "is_active": bool(user["is_active"]) if user["is_active"] is not None else True
                }
                for user in users
            ]
        except Exception as e:
            logger.error(f"获取用户列表异常: {e}")
            raise RuntimeError("获取用户列表失败")
    
    async def delete_user(self, user_id: int) -> Dict[str, Any]:
        """删除用户"""
        try:
            # 检查用户是否存在
            user = await self._get_database().get_user_by_id(user_id)
            if not user:
                raise ValueError("用户不存在")
            
            # 不允许删除管理员用户（ID为1的用户通常是默认管理员）
            if user_id == 1:
                raise ValueError("不能删除默认管理员用户")
            
            # 删除用户
            success = await self._get_database().delete_user(user_id)
            if not success:
                raise RuntimeError("删除用户失败")
            
            logger.info(f"用户删除成功: {user['username']} (ID: {user_id})")
            
            return {
                "user_id": user_id,
                "username": user["username"],
                "message": "用户删除成功"
            }
            
        except ValueError as e:
            logger.warning(f"删除用户失败: {e}")
            raise
        except Exception as e:
            logger.error(f"删除用户异常: {e}")
            raise RuntimeError("删除用户失败")


# 全局用户服务实例
user_service: Optional[UserService] = None


def get_user_service() -> UserService:
    """获取用户服务实例"""
    global user_service
    if user_service is None:
        user_service = UserService()
    return user_service
