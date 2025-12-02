"""
定时任务服务
定期从Redis收集数据并存储到InfluxDB
"""

import asyncio
import schedule
import time
import threading
from datetime import datetime, timedelta
from typing import Dict, Any, Optional, List
from threading import Thread
from loguru import logger

from ..core.config import settings
from ..core.config_loader import config_loader
from ..services.data_collector import data_collector
from ..services.data_storage import data_storage
from ..services.query_service import query_service

class SchedulerService:
    """定时任务服务"""
    
    def __init__(self):
        self.is_running = False
        self.scheduler_thread: Optional[Thread] = None
        
        # 数据缓冲区，用于暂存收集的数据
        self.data_buffer: List[Any] = []
        self.buffer_lock = threading.Lock()
        
        self.stats = {
            "last_collection_time": None,
            "last_collection_count": 0,
            "last_flush_time": None,
            "last_flush_count": 0,
            "total_collections": 0,
            "total_stored_points": 0,
            "buffer_size": 0,
            "last_cleanup_time": None,
            "errors": []
        }
        
    def start(self):
        """启动定时任务"""
        if self.is_running:
            logger.warning("定时任务已在运行")
            return
        
        logger.info("启动定时任务服务")
        
        # 配置定时任务
        self._setup_schedules()
        
        # 启动调度器线程
        self.is_running = True
        self.scheduler_thread = Thread(target=self._run_scheduler, daemon=True)
        self.scheduler_thread.start()
        
        logger.info("定时任务服务已启动")
    
    def stop(self):
        """停止定时任务"""
        if not self.is_running:
            return
        
        logger.info("停止定时任务服务")
        self.is_running = False
        
        if self.scheduler_thread and self.scheduler_thread.is_alive():
            self.scheduler_thread.join(timeout=5)
        
        # 清除所有任务
        schedule.clear()
        logger.info("定时任务服务已停止")
    
    def _setup_schedules(self):
        """设置定时任务"""
        # 数据收集任务（收集到缓冲区）
        if config_loader.is_scheduler_task_enabled('data_collection'):
            collection_interval = config_loader.get_data_collection_interval()
            schedule.every(collection_interval).seconds.do(self._collect_data_to_buffer)
            logger.info(f"设置数据收集任务，间隔: {collection_interval} 秒")
            
            # 数据刷新任务（从缓冲区写入InfluxDB）
            flush_interval = config_loader.get_data_flush_interval()
            schedule.every(flush_interval).seconds.do(self._flush_buffer_to_storage)
            logger.info(f"设置数据刷新任务，间隔: {flush_interval} 秒")
        
        # 健康检查任务
        if config_loader.is_scheduler_task_enabled('health_check'):
            interval = config_loader.get_config('scheduler.health_check.interval', 60)
            schedule.every(interval).seconds.do(self._health_check)
            logger.info(f"设置健康检查任务，间隔: {interval} 秒")
        
        # 数据清理任务
        if config_loader.is_scheduler_task_enabled('data_cleanup'):
            cron_expr = config_loader.get_config('scheduler.data_cleanup.cron', '0 2 * * *')
            # 简化实现：每天凌晨2点执行
            schedule.every().day.at("02:00").do(self._cleanup_old_data)
            logger.info(f"设置数据清理任务，时间: {cron_expr}")
        
        # 统计任务
        if config_loader.is_scheduler_task_enabled('statistics'):
            cron_expr = config_loader.get_config('scheduler.statistics.cron', '0 1 * * *')
            # 简化实现：每天凌晨1点执行
            schedule.every().day.at("01:00").do(self._generate_statistics)
            logger.info(f"设置统计任务，时间: {cron_expr}")
    
    def _run_scheduler(self):
        """运行调度器"""
        logger.info("定时任务调度器开始运行")
        
        while self.is_running:
            try:
                schedule.run_pending()
                time.sleep(1)  # 每秒检查一次
            except Exception as e:
                logger.error(f"调度器运行异常: {e}")
                self.stats["errors"].append({
                    "time": datetime.utcnow(),
                    "error": str(e),
                    "type": "scheduler"
                })
                # 保持最近10个错误
                self.stats["errors"] = self.stats["errors"][-10:]
                time.sleep(5)  # 出错后等待5秒
        
        logger.info("定时任务调度器已停止")
    
    def _collect_and_store_data(self):
        """收集并存储数据"""
        try:
            logger.debug("开始数据收集和存储任务")
            start_time = time.time()
            
            # 收集数据
            history_data = data_collector.collect_all_data()
            
            if not history_data:
                logger.debug("没有收集到新数据")
                return
            
            # 批量存储数据
            result = data_storage.store_batch_data(history_data)
            
            # 更新统计信息
            elapsed_time = time.time() - start_time
            self.stats["last_collection_time"] = datetime.utcnow()
            self.stats["last_collection_count"] = len(history_data)
            self.stats["total_collections"] += 1
            self.stats["total_stored_points"] += result["success"]
            
            logger.info(f"数据收集任务完成: 收集 {len(history_data)} 条, "
                       f"存储成功 {result['success']} 条, "
                       f"失败 {result['failed']} 条, "
                       f"耗时 {elapsed_time:.2f} 秒")
            
            # 记录错误
            if result["errors"]:
                for error in result["errors"]:
                    self.stats["errors"].append({
                        "time": datetime.utcnow(),
                        "error": error,
                        "type": "storage"
                    })
                    
        except Exception as e:
            logger.error(f"数据收集和存储任务失败: {e}")
            self.stats["errors"].append({
                "time": datetime.utcnow(),
                "error": str(e),
                "type": "collection"
            })
    
    def _collect_data_to_buffer(self):
        """收集数据到缓冲区"""
        try:
            logger.debug("开始数据收集任务")
            start_time = time.time()
            
            # 收集数据
            history_data = data_collector.collect_all_data()
            
            if not history_data:
                logger.debug("没有收集到新数据")
                return
            
            # 添加数据到缓冲区
            with self.buffer_lock:
                self.data_buffer.extend(history_data)
                self.stats["buffer_size"] = len(self.data_buffer)
            
            # 更新统计信息
            elapsed_time = time.time() - start_time
            self.stats["last_collection_time"] = datetime.utcnow()
            self.stats["last_collection_count"] = len(history_data)
            self.stats["total_collections"] += 1
            
            logger.debug(f"数据收集完成: 收集 {len(history_data)} 条, "
                        f"缓冲区总数 {self.stats['buffer_size']} 条, "
                        f"耗时 {elapsed_time:.2f} 秒")
                
        except Exception as e:
            logger.error(f"数据收集任务失败: {e}")
            self.stats["errors"].append({
                "time": datetime.utcnow(),
                "error": str(e),
                "type": "collection"
            })
    
    def _flush_buffer_to_storage(self):
        """将缓冲区数据刷新到存储"""
        try:
            # 获取缓冲区中的所有数据
            with self.buffer_lock:
                if not self.data_buffer:
                    logger.debug("缓冲区为空，跳过刷新")
                    return
                
                data_to_flush = self.data_buffer.copy()
                self.data_buffer.clear()
                self.stats["buffer_size"] = 0
            
            logger.debug(f"开始刷新 {len(data_to_flush)} 条数据到InfluxDB")
            start_time = time.time()
            
            # 批量存储数据
            result = data_storage.store_batch_data(data_to_flush)
            
            # 更新统计信息
            elapsed_time = time.time() - start_time
            self.stats["last_flush_time"] = datetime.utcnow()
            self.stats["last_flush_count"] = len(data_to_flush)
            self.stats["total_stored_points"] += result["success"]
            
            logger.info(f"数据刷新完成: 刷新 {len(data_to_flush)} 条, "
                       f"存储成功 {result['success']} 条, "
                       f"失败 {result['failed']} 条, "
                       f"耗时 {elapsed_time:.2f} 秒")
            
            # 记录错误
            if result["errors"]:
                for error in result["errors"]:
                    self.stats["errors"].append({
                        "time": datetime.utcnow(),
                        "error": str(error),
                        "type": "flush"
                    })
                
        except Exception as e:
            logger.error(f"数据刷新任务失败: {e}")
            self.stats["errors"].append({
                "time": datetime.utcnow(),
                "error": str(e),
                "type": "flush"
            })
    
    def _health_check(self):
        """健康检查"""
        try:
            logger.debug("开始健康检查")
            
            # 检查Redis连接
            redis_ok = data_collector.test_connection()
            
            # 检查InfluxDB连接
            influxdb_ok = data_storage.test_connection()
            
            if not redis_ok:
                logger.warning("Redis连接异常")
                self.stats["errors"].append({
                    "time": datetime.utcnow(),
                    "error": "Redis连接失败",
                    "type": "health_check"
                })
            
            if not influxdb_ok:
                logger.warning("InfluxDB连接异常")
                self.stats["errors"].append({
                    "time": datetime.utcnow(),
                    "error": "InfluxDB连接失败",
                    "type": "health_check"
                })
            
            if redis_ok and influxdb_ok:
                logger.debug("健康检查通过")
            else:
                logger.warning("健康检查发现问题")
                
        except Exception as e:
            logger.error(f"健康检查失败: {e}")
            self.stats["errors"].append({
                "time": datetime.utcnow(),
                "error": str(e),
                "type": "health_check"
            })
    
    def _cleanup_old_data(self):
        """清理旧数据"""
        try:
            logger.info("开始数据清理任务")
            
            # 获取清理配置
            older_than = config_loader.get_config('scheduler.data_cleanup.cleanup_older_than', '30d')
            
            # 解析天数
            if older_than.endswith('d'):
                days = int(older_than[:-1])
            else:
                days = 30  # 默认30天
            
            # 执行清理
            result = data_storage.cleanup_old_data(days)
            
            self.stats["last_cleanup_time"] = datetime.utcnow()
            
            if result["success"]:
                logger.info(f"数据清理完成，删除了 {result['deleted_points']} 个数据点")
            else:
                logger.error(f"数据清理失败: {result['error']}")
                self.stats["errors"].append({
                    "time": datetime.utcnow(),
                    "error": result["error"],
                    "type": "cleanup"
                })
                
        except Exception as e:
            logger.error(f"数据清理任务失败: {e}")
            self.stats["errors"].append({
                "time": datetime.utcnow(),
                "error": str(e),
                "type": "cleanup"
            })
    
    def _generate_statistics(self):
        """生成统计数据"""
        try:
            logger.info("开始生成统计数据")
            
            # 获取存储统计
            storage_stats = data_storage.get_storage_stats()
            
            # 获取数据范围信息
            range_info = query_service.get_data_range_info()
            
            # 记录统计信息到日志
            logger.info(f"统计信息: 总数据点 {storage_stats['total_points']}, "
                       f"存储大小 {storage_stats['storage_size']} 字节, "
                       f"通道数 {len(range_info['channels'])}")
            
        except Exception as e:
            logger.error(f"生成统计数据失败: {e}")
            self.stats["errors"].append({
                "time": datetime.utcnow(),
                "error": str(e),
                "type": "statistics"
            })
    
    def get_status(self) -> Dict[str, Any]:
        """获取调度器状态"""
        return {
            "is_running": self.is_running,
            "stats": self.stats.copy(),
            "next_runs": {
                job.job_func.__name__: job.next_run 
                for job in schedule.jobs
            } if schedule.jobs else {},
            "job_count": len(schedule.jobs)
        }
    


# 全局调度器服务实例
scheduler_service = SchedulerService()
