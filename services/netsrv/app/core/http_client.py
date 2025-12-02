"""
HTTP客户端模块
"""

import json
import time
from typing import Optional, Dict, Any, List
from loguru import logger
import httpx
import requests
from .config import settings
import asyncio

class HTTPClient:
    """HTTP客户端管理器"""
    
    def __init__(self):
        self.timeout = settings.HTTP_TIMEOUT
        self.retry_count = settings.HTTP_RETRY_COUNT
        self.retry_delay = settings.HTTP_RETRY_DELAY
        self.session: Optional[httpx.AsyncClient] = None
        self.sync_session: Optional[requests.Session] = None
    
    async def _get_async_session(self) -> httpx.AsyncClient:
        """获取异步HTTP会话"""
        if self.session is None:
            self.session = httpx.AsyncClient(
                timeout=self.timeout,
                limits=httpx.Limits(max_connections=100, max_keepalive_connections=20)
            )
        return self.session
    
    def _get_sync_session(self) -> requests.Session:
        """获取同步HTTP会话"""
        if self.sync_session is None:
            self.sync_session = requests.Session()
            self.sync_session.headers.update({
                'User-Agent': f'Netsrv/{settings.APP_VERSION}',
                'Content-Type': 'application/json'
            })
        return self.sync_session
    
    async def async_get(self, url: str, headers: Optional[Dict] = None, params: Optional[Dict] = None) -> Optional[Dict]:
        """异步GET请求"""
        session = await self._get_async_session()
        
        for attempt in range(self.retry_count + 1):
            try:
                response = await session.get(url, headers=headers, params=params)
                response.raise_for_status()
                
                if response.headers.get('content-type', '').startswith('application/json'):
                    return response.json()
                else:
                    return {'content': response.text}
                    
            except httpx.HTTPStatusError as e:
                logger.error(f"HTTP状态错误: {url}, 状态码: {e.response.status_code}")
                if attempt == self.retry_count:
                    return None
            except Exception as e:
                logger.error(f"HTTP请求异常: {url}, {e}")
                if attempt == self.retry_count:
                    return None
            
            if attempt < self.retry_count:
                await asyncio.sleep(self.retry_delay)
        
        return None
    
    async def async_post(self, url: str, data: Any, headers: Optional[Dict] = None) -> Optional[Dict]:
        """异步POST请求"""
        session = await self._get_async_session()
        
        if isinstance(data, (dict, list)):
            data = json.dumps(data, ensure_ascii=False)
        
        for attempt in range(self.retry_count + 1):
            try:
                response = await session.post(url, content=data, headers=headers)
                response.raise_for_status()
                
                if response.headers.get('content-type', '').startswith('application/json'):
                    return response.json()
                else:
                    return {'content': response.text}
                    
            except httpx.HTTPStatusError as e:
                logger.error(f"HTTP状态错误: {url}, 状态码: {e.response.status_code}")
                if attempt == self.retry_count:
                    return None
            except Exception as e:
                logger.error(f"HTTP请求异常: {url}, {e}")
                if attempt == self.retry_count:
                    return None
            
            if attempt < self.retry_count:
                await asyncio.sleep(self.retry_delay)
        
        return None
    
    def sync_get(self, url: str, headers: Optional[Dict] = None, params: Optional[Dict] = None) -> Optional[Dict]:
        """同步GET请求"""
        session = self._get_sync_session()
        
        for attempt in range(self.retry_count + 1):
            try:
                response = session.get(url, headers=headers, params=params, timeout=self.timeout)
                response.raise_for_status()
                
                if response.headers.get('content-type', '').startswith('application/json'):
                    return response.json()
                else:
                    return {'content': response.text}
                    
            except requests.exceptions.HTTPError as e:
                logger.error(f"HTTP状态错误: {url}, 状态码: {e.response.status_code}")
                if attempt == self.retry_count:
                    return None
            except Exception as e:
                logger.error(f"HTTP请求异常: {url}, {e}")
                if attempt == self.retry_count:
                    return None
            
            if attempt < self.retry_count:
                time.sleep(self.retry_delay)
        
        return None
    
    def sync_post(self, url: str, data: Any, headers: Optional[Dict] = None) -> Optional[Dict]:
        """同步POST请求"""
        session = self._get_sync_session()
        
        if isinstance(data, (dict, list)):
            data = json.dumps(data, ensure_ascii=False)
        
        for attempt in range(self.retry_count + 1):
            try:
                response = session.post(url, data=data, headers=headers, timeout=self.timeout)
                response.raise_for_status()
                
                if response.headers.get('content-type', '').startswith('application/json'):
                    return response.json()
                else:
                    return {'content': response.text}
                    
            except requests.exceptions.HTTPError as e:
                logger.error(f"HTTP状态错误: {url}, 状态码: {e.response.status_code}")
                if attempt == self.retry_count:
                    return None
            except Exception as e:
                logger.error(f"HTTP请求异常: {url}, {e}")
                if attempt == self.retry_count:
                    return None
            
            if attempt < self.retry_count:
                time.sleep(self.retry_delay)
        
        return None
    
    async def close(self):
        """关闭异步会话"""
        if self.session:
            await self.session.aclose()
            self.session = None
    
    def close_sync(self):
        """关闭同步会话"""
        if self.sync_session:
            self.sync_session.close()
            self.sync_session = None

# 全局HTTP客户端实例
http_client = HTTPClient()
