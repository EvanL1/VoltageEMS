"""
身份认证服务
处理JWT令牌生成、验证和刷新
"""

import os
import jwt
import hashlib
import secrets
from datetime import datetime, timedelta
from typing import Optional, Dict, Any
import logging
from passlib.context import CryptContext

from app.models.auth import TokenData, User, UserWithRole, Token

logger = logging.getLogger(__name__)

# 密码加密上下文
pwd_context = CryptContext(schemes=["bcrypt"], deprecated="auto")


class AuthService:
    """身份认证服务"""
    
    def __init__(self):
        # JWT配置
        self.secret_key = os.getenv("JWT_SECRET_KEY", "your-secret-key-here-change-in-production")
        self.algorithm = "HS256"
        self.access_token_expire_minutes = int(os.getenv("ACCESS_TOKEN_EXPIRE_MINUTES", "30"))
        self.refresh_token_expire_days = int(os.getenv("REFRESH_TOKEN_EXPIRE_DAYS", "7"))
        
        # 刷新令牌存储 (生产环境应使用Redis)
        self.refresh_tokens: Dict[str, Dict[str, Any]] = {}
        
        if self.secret_key == "your-secret-key-here-change-in-production":
            logger.warning("⚠️ 使用默认JWT密钥，生产环境请修改JWT_SECRET_KEY环境变量")
    
    def hash_password(self, md5_password: str) -> str:
        """
        加密MD5密码
        
        Args:
            md5_password: 前端传来的MD5加密密码
            
        Returns:
            bcrypt哈希后的密码（用于数据库存储）
        """
        return pwd_context.hash(md5_password)
    
    def verify_password(self, md5_password: str, hashed_password: str) -> bool:
        """
        验证MD5密码
        
        Args:
            md5_password: 前端传来的MD5加密密码
            hashed_password: 数据库中存储的bcrypt哈希
            
        Returns:
            验证结果
        """
        return pwd_context.verify(md5_password, hashed_password)
    
    def create_access_token(self, user_data: Dict[str, Any]) -> str:
        """创建访问令牌"""
        try:
            # 令牌载荷
            payload = {
                "user_id": user_data["id"],
                "username": user_data["username"],
                "role": user_data["role"]["name_en"],
                "exp": datetime.utcnow() + timedelta(minutes=self.access_token_expire_minutes),
                "iat": datetime.utcnow(),
                "type": "access"
            }
            
            # 生成令牌
            token = jwt.encode(payload, self.secret_key, algorithm=self.algorithm)
            return token
            
        except Exception as e:
            logger.error(f"创建访问令牌失败: {e}")
            raise
    
    def create_refresh_token(self, user_data: Dict[str, Any]) -> str:
        """创建刷新令牌"""
        try:
            # 生成唯一令牌ID
            token_id = secrets.token_urlsafe(32)
            
            # 令牌载荷
            payload = {
                "user_id": user_data["id"],
                "username": user_data["username"],
                "token_id": token_id,
                "exp": datetime.utcnow() + timedelta(days=self.refresh_token_expire_days),
                "iat": datetime.utcnow(),
                "type": "refresh"
            }
            
            # 生成令牌
            token = jwt.encode(payload, self.secret_key, algorithm=self.algorithm)
            
            # 存储刷新令牌信息
            self.refresh_tokens[token_id] = {
                "user_id": user_data["id"],
                "username": user_data["username"],
                "created_at": datetime.utcnow(),
                "expires_at": datetime.utcnow() + timedelta(days=self.refresh_token_expire_days)
            }
            
            return token
            
        except Exception as e:
            logger.error(f"创建刷新令牌失败: {e}")
            raise
    
    def create_tokens(self, user_data: Dict[str, Any]) -> Token:
        """创建令牌对（访问令牌+刷新令牌）"""
        access_token = self.create_access_token(user_data)
        refresh_token = self.create_refresh_token(user_data)
        
        return Token(
            access_token=access_token,
            refresh_token=refresh_token,
            token_type="bearer",
            expires_in=self.access_token_expire_minutes * 60
        )
    
    def verify_access_token(self, token: str) -> Optional[TokenData]:
        """验证访问令牌"""
        try:
            # 解码令牌
            payload = jwt.decode(token, self.secret_key, algorithms=[self.algorithm])
            
            # 检查令牌类型
            if payload.get("type") != "access":
                logger.debug("令牌类型错误：不是访问令牌")
                return None
            
            # 提取用户信息
            user_id = payload.get("user_id")
            username = payload.get("username")
            role = payload.get("role")
            
            if user_id is None or username is None:
                logger.debug("令牌载荷不完整：缺少用户ID或用户名")
                return None
            
            return TokenData(user_id=user_id, username=username, role=role)
            
        except jwt.ExpiredSignatureError:
            logger.debug("访问令牌已过期")
            return None
        except jwt.InvalidTokenError:
            logger.debug("访问令牌格式无效")
            return None
        except jwt.InvalidSignatureError:
            logger.debug("访问令牌签名无效")
            return None
        except jwt.DecodeError:
            logger.debug("访问令牌解码失败")
            return None
        except jwt.JWTError as e:
            logger.debug(f"访问令牌验证失败: {e}")
            return None
        except Exception as e:
            logger.warning(f"访问令牌验证异常: {e}")
            return None
    
    def verify_refresh_token(self, token: str) -> Optional[TokenData]:
        """验证刷新令牌"""
        try:
            # 解码令牌
            payload = jwt.decode(token, self.secret_key, algorithms=[self.algorithm])
            
            # 检查令牌类型
            if payload.get("type") != "refresh":
                return None
            
            # 检查令牌ID是否存在
            token_id = payload.get("token_id")
            if token_id not in self.refresh_tokens:
                logger.debug("刷新令牌ID不存在")
                return None
            
            # 检查令牌是否过期
            token_info = self.refresh_tokens[token_id]
            if datetime.utcnow() > token_info["expires_at"]:
                # 清理过期令牌
                del self.refresh_tokens[token_id]
                logger.debug("刷新令牌已过期")
                return None
            
            # 提取用户信息
            user_id = payload.get("user_id")
            username = payload.get("username")
            
            if user_id is None or username is None:
                return None
            
            return TokenData(user_id=user_id, username=username)
            
        except jwt.ExpiredSignatureError:
            logger.debug("刷新令牌已过期")
            return None
        except jwt.JWTError as e:
            logger.debug(f"刷新令牌验证失败: {e}")
            return None
    
    def revoke_refresh_token(self, token: str) -> bool:
        """撤销刷新令牌"""
        try:
            payload = jwt.decode(token, self.secret_key, algorithms=[self.algorithm])
            token_id = payload.get("token_id")
            
            if token_id in self.refresh_tokens:
                del self.refresh_tokens[token_id]
                logger.info(f"刷新令牌已撤销: {token_id}")
                return True
            
            return False
            
        except jwt.JWTError:
            return False
    
    def cleanup_expired_tokens(self):
        """清理过期的刷新令牌"""
        current_time = datetime.utcnow()
        expired_tokens = [
            token_id for token_id, token_info in self.refresh_tokens.items()
            if current_time > token_info["expires_at"]
        ]
        
        for token_id in expired_tokens:
            del self.refresh_tokens[token_id]
        
        if expired_tokens:
            logger.info(f"清理了 {len(expired_tokens)} 个过期刷新令牌")
    
    def get_token_stats(self) -> Dict[str, Any]:
        """获取令牌统计信息"""
        active_tokens = len(self.refresh_tokens)
        expired_count = 0
        current_time = datetime.utcnow()
        
        for token_info in self.refresh_tokens.values():
            if current_time > token_info["expires_at"]:
                expired_count += 1
        
        return {
            "active_refresh_tokens": active_tokens,
            "expired_tokens": expired_count,
            "access_token_expire_minutes": self.access_token_expire_minutes,
            "refresh_token_expire_days": self.refresh_token_expire_days
        }


# 全局认证服务实例
auth_service: Optional[AuthService] = None


def get_auth_service() -> AuthService:
    """获取认证服务实例"""
    global auth_service
    if auth_service is None:
        auth_service = AuthService()
    return auth_service
