#!/usr/bin/env python3
"""
ComsRv数据模拟器
严格按照VoltageEMS Redis数据结构规范v3.2实现

数据格式：
- Hash键格式：comsrv:{channelID}:{type}
- Hash字段值：{pointID} -> "{value:.6f}"
- 发布消息：{pointID}:{value:.6f}
- 发布通道：comsrv:{channelID}:{type}
"""

import asyncio
import json
import logging
import os
import random
import time
from typing import Dict, Any, Optional
import redis
import sys
from pythonjsonlogger import jsonlogger


class ComsrvSimulator:
    """ComsRv数据模拟器"""

    def __init__(self, config_path: str = "config.json"):
        """初始化模拟器"""
        self.setup_logging()
        self.load_config(config_path)
        self.setup_redis()
        self.current_values = {}
        self.running = False

    def setup_logging(self):
        """设置日志"""
        log_level = os.getenv("LOG_LEVEL", "INFO")

        # 创建logger
        self.logger = logging.getLogger("comsrv_simulator")
        self.logger.setLevel(getattr(logging, log_level))

        # 控制台处理器
        console_handler = logging.StreamHandler(sys.stdout)
        console_handler.setLevel(getattr(logging, log_level))

        # JSON格式化器
        formatter = jsonlogger.JsonFormatter(
            "%(asctime)s %(name)s %(levelname)s %(message)s"
        )
        console_handler.setFormatter(formatter)
        self.logger.addHandler(console_handler)

    def load_config(self, config_path: str):
        """加载配置文件"""
        try:
            with open(config_path, "r", encoding="utf-8") as f:
                self.config = json.load(f)
            self.logger.info("配置文件加载成功", extra={"config_path": config_path})
        except Exception as e:
            self.logger.error("配置文件加载失败", extra={"error": str(e)})
            raise

    def setup_redis(self):
        """设置Redis连接"""
        redis_url = os.getenv("REDIS_URL", "redis://localhost:6379")
        try:
            self.redis_client = redis.from_url(redis_url, decode_responses=True)
            # 测试连接
            self.redis_client.ping()
            self.logger.info("Redis连接成功", extra={"redis_url": redis_url})
        except Exception as e:
            self.logger.error("Redis连接失败", extra={"error": str(e)})
            raise

    def format_value(self, value: float) -> str:
        """按照规范格式化数值 - 6位小数精度"""
        return f"{value:.6f}"

    def generate_measurement_value(
        self, point_config: Dict[str, Any], current_value: Optional[float] = None
    ) -> float:
        """生成测量值"""
        base_value = point_config["base_value"]
        variance = point_config["variance"]
        min_value = point_config["min_value"]
        max_value = point_config["max_value"]

        if current_value is None:
            # 初始值：基础值加随机偏移
            value = base_value + random.uniform(-variance, variance)
        else:
            # 基于当前值的小幅变化（模拟真实设备的连续性）
            change = random.uniform(-variance * 0.3, variance * 0.3)
            value = current_value + change

        # 限制在合理范围内
        value = max(min_value, min(max_value, value))
        return value

    def generate_signal_value(
        self, point_config: Dict[str, Any], current_value: Optional[int] = None
    ) -> int:
        """生成信号值"""
        states = point_config["states"]
        change_probability = point_config.get("change_probability", 0.1)

        if current_value is None:
            return point_config.get("default", states[0])

        # 根据变化概率决定是否改变状态
        if random.random() < change_probability:
            # 切换到另一个状态
            current_index = (
                states.index(current_value) if current_value in states else 0
            )
            new_index = (current_index + 1) % len(states)
            return states[new_index]
        else:
            return current_value

    def generate_control_value(
        self, point_config: Dict[str, Any], current_value: Optional[int] = None
    ) -> int:
        """生成控制值（通常保持稳定，除非有外部命令）"""
        if current_value is None:
            return point_config.get("default", 0)
        return current_value  # 控制值保持不变，除非有外部命令

    def generate_adjustment_value(
        self, point_config: Dict[str, Any], current_value: Optional[float] = None
    ) -> float:
        """生成调节值"""
        # 调节值变化较少，主要是设定值的微调
        return self.generate_measurement_value(point_config, current_value)

    def update_channel_data(self, channel_id: str, channel_config: Dict[str, Any]):
        """更新单个通道的所有数据"""
        channel_name = channel_config["name"]
        points = channel_config["points"]

        # 处理各种类型的点位
        type_mapping = {
            "measurement": "m",
            "signal": "s",
            "control": "c",
            "adjustment": "a",
        }

        for point_type, type_code in type_mapping.items():
            if point_type not in points:
                continue

            # Redis Hash键
            hash_key = f"comsrv:{channel_id}:{type_code}"
            # 发布通道
            pub_channel = hash_key

            point_updates = {}
            pub_messages = []

            for point_id, point_config in points[point_type].items():
                # 获取当前值
                current_key = f"{channel_id}:{type_code}:{point_id}"
                current_value = self.current_values.get(current_key)

                # 生成新值
                if point_type == "measurement":
                    new_value = self.generate_measurement_value(
                        point_config, current_value
                    )
                    formatted_value = self.format_value(new_value)
                elif point_type == "signal":
                    new_value = self.generate_signal_value(point_config, current_value)
                    formatted_value = str(new_value)
                elif point_type == "control":
                    new_value = self.generate_control_value(point_config, current_value)
                    formatted_value = str(new_value)
                elif point_type == "adjustment":
                    new_value = self.generate_adjustment_value(
                        point_config, current_value
                    )
                    formatted_value = self.format_value(new_value)
                else:
                    continue

                # 保存当前值
                self.current_values[current_key] = new_value

                # 准备批量更新
                point_updates[point_id] = formatted_value

                # 准备发布消息（点位级更新通知）
                pub_message = f"{point_id}:{formatted_value}"
                pub_messages.append(pub_message)

            # 批量更新Hash
            if point_updates:
                try:
                    self.redis_client.hmset(hash_key, point_updates)

                    # 发布更新通知
                    for message in pub_messages:
                        self.redis_client.publish(pub_channel, message)

                    self.logger.debug(
                        "通道数据更新成功",
                        extra={
                            "channel_id": channel_id,
                            "channel_name": channel_name,
                            "type": point_type,
                            "hash_key": hash_key,
                            "points_count": len(point_updates),
                        },
                    )

                except Exception as e:
                    self.logger.error(
                        "数据更新失败",
                        extra={
                            "channel_id": channel_id,
                            "type": point_type,
                            "error": str(e),
                        },
                    )

    def update_all_channels(self):
        """更新所有通道数据"""
        channels = self.config["channels"]

        for channel_id, channel_config in channels.items():
            self.update_channel_data(channel_id, channel_config)

    def log_statistics(self):
        """记录统计信息"""
        channels = self.config["channels"]
        total_points = 0

        stats = {"channels": len(channels), "channel_details": {}}

        for channel_id, channel_config in channels.items():
            channel_stats = {
                "name": channel_config["name"],
                "points": {
                    "measurement": len(channel_config["points"].get("measurement", {})),
                    "signal": len(channel_config["points"].get("signal", {})),
                    "control": len(channel_config["points"].get("control", {})),
                    "adjustment": len(channel_config["points"].get("adjustment", {})),
                },
            }
            channel_stats["total_points"] = sum(channel_stats["points"].values())
            total_points += channel_stats["total_points"]
            stats["channel_details"][channel_id] = channel_stats

        stats["total_points"] = total_points

        self.logger.info("模拟器运行统计", extra=stats)

    async def run(self):
        """运行模拟器主循环"""
        self.running = True
        update_interval = (
            self.config["simulator"]["update_interval"] / 1000.0
        )  # 转换为秒

        self.logger.info(
            "ComsRv数据模拟器启动",
            extra={
                "update_interval_ms": self.config["simulator"]["update_interval"],
                "channels": list(self.config["channels"].keys()),
            },
        )

        # 初次统计
        self.log_statistics()

        cycle_count = 0

        try:
            while self.running:
                start_time = time.time()

                # 更新所有通道数据
                self.update_all_channels()

                cycle_count += 1
                update_time = time.time() - start_time

                # 每100个周期记录一次详细统计
                if cycle_count % 100 == 0:
                    self.logger.info(
                        "模拟器周期统计",
                        extra={
                            "cycle_count": cycle_count,
                            "update_time_ms": round(update_time * 1000, 2),
                            "active_points": len(self.current_values),
                        },
                    )

                # 等待下一个更新周期
                await asyncio.sleep(max(0, update_interval - update_time))

        except KeyboardInterrupt:
            self.logger.info("接收到停止信号")
        except Exception as e:
            self.logger.error("模拟器运行异常", extra={"error": str(e)})
            raise
        finally:
            self.running = False
            self.logger.info(
                "ComsRv数据模拟器停止", extra={"total_cycles": cycle_count}
            )

    def stop(self):
        """停止模拟器"""
        self.running = False


def main():
    """主函数"""
    print("🔄 启动ComsRv数据模拟器...")

    # 等待Redis服务可用
    redis_url = os.getenv("REDIS_URL", "redis://localhost:6379")
    max_retries = 30

    for i in range(max_retries):
        try:
            client = redis.from_url(redis_url)
            client.ping()
            print(f"✅ Redis连接成功: {redis_url}")
            break
        except Exception as e:
            if i == max_retries - 1:
                print(f"❌ Redis连接失败: {e}")
                sys.exit(1)
            print(f"⏳ 等待Redis服务... ({i + 1}/{max_retries})")
            time.sleep(2)

    # 启动模拟器
    try:
        simulator = ComsrvSimulator()
        asyncio.run(simulator.run())
    except Exception as e:
        print(f"❌ 模拟器启动失败: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
