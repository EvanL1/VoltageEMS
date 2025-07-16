#!/usr/bin/env python3
"""
数据发布测试脚本

模拟各种数据源发布数据到Redis，用于触发规则执行。
支持的数据类型：
1. 点位数据 - 直接设置键值
2. modsrv输出 - 发布到通道
3. 告警事件 - 发布到通道
"""

import json
import redis
import time
import random
import argparse
import threading
from datetime import datetime

# Redis连接配置
REDIS_HOST = "localhost"
REDIS_PORT = 6379
REDIS_DB = 0

class DataPublisher:
    def __init__(self, redis_host=REDIS_HOST, redis_port=REDIS_PORT):
        """初始化数据发布器"""
        self.r = redis.Redis(host=redis_host, port=redis_port, db=REDIS_DB, decode_responses=True)
        self.pubsub = self.r.pubsub()
        self.running = False
        
        # 测试连接
        try:
            self.r.ping()
            print(f"✓ 已连接到Redis {redis_host}:{redis_port}")
        except Exception as e:
            print(f"✗ Redis连接失败: {e}")
            raise

    def publish_point_data(self, channel_id, point_type, point_id, value):
        """
        发布点位数据
        格式：{channel_id}:{type}:{point_id} = value:timestamp
        """
        key = f"{channel_id}:{point_type}:{point_id}"
        timestamp = int(time.time() * 1000)
        data = f"{value}:{timestamp}"
        
        self.r.set(key, data)
        print(f"[点位数据] {key} = {value} (时间戳: {timestamp})")
        
        # 同时发布到通道（用于实时订阅）
        channel = f"point:update:{channel_id}:{point_type}"
        update_msg = json.dumps({
            "channel_id": channel_id,
            "point_type": point_type,
            "point_id": point_id,
            "value": value,
            "timestamp": timestamp
        })
        self.r.publish(channel, update_msg)

    def publish_modsrv_output(self, model_id, output_name, value):
        """
        发布modsrv模型输出
        通道格式：modsrv:outputs:{model_id}
        """
        # 存储到哈希表
        key = f"modsrv:output:{model_id}"
        self.r.hset(key, output_name, json.dumps(value))
        
        # 发布到通道
        channel = f"modsrv:outputs:{model_id}"
        message = json.dumps({
            "model_id": model_id,
            "output": output_name,
            "value": value,
            "timestamp": datetime.now().isoformat()
        })
        
        self.r.publish(channel, message)
        print(f"[模型输出] {model_id}.{output_name} = {value}")

    def publish_alarm_event(self, alarm_id, alarm_type, level, message):
        """
        发布告警事件
        通道格式：alarm:event:{alarm_id}
        """
        channel = f"alarm:event:{alarm_id}"
        event = {
            "alarm_id": alarm_id,
            "type": alarm_type,
            "level": level,
            "message": message,
            "timestamp": datetime.now().isoformat(),
            "status": "active"
        }
        
        self.r.publish(channel, json.dumps(event))
        print(f"[告警事件] {alarm_id}: {message} (级别: {level})")

    def simulate_temperature_data(self, base_temp=25, variation=10, interval=1):
        """模拟温度数据变化"""
        print(f"\n开始模拟温度数据 (基准: {base_temp}°C, 变化: ±{variation}°C)")
        
        while self.running:
            # 生成随机温度
            temp = base_temp + random.uniform(-variation, variation)
            
            # 发布温度数据
            self.publish_point_data("1001", "m", "10001", round(temp, 2))
            
            # 如果温度过高，增加告警概率
            if temp > 30:
                if random.random() > 0.7:  # 30%概率触发告警
                    self.publish_alarm_event(
                        f"temp_alarm_{int(time.time())}",
                        "temperature",
                        "warning",
                        f"温度过高: {temp:.2f}°C"
                    )
            
            time.sleep(interval)

    def simulate_power_data(self, base_power=80000, variation=40000, interval=2):
        """模拟功率数据变化"""
        print(f"\n开始模拟功率数据 (基准: {base_power/1000}kW, 变化: ±{variation/1000}kW)")
        
        while self.running:
            # 生成随机功率（单位：W）
            power = base_power + random.uniform(-variation, variation)
            
            # 发布功率数据
            self.publish_point_data("1001", "m", "10002", round(power, 0))
            
            time.sleep(interval)

    def simulate_model_outputs(self, interval=5):
        """模拟modsrv模型输出"""
        print(f"\n开始模拟模型输出 (间隔: {interval}秒)")
        
        while self.running:
            # 模拟效率计算结果
            efficiency = random.uniform(0.75, 0.95)
            self.publish_modsrv_output("model1", "efficiency", round(efficiency, 3))
            
            # 模拟预测结果
            prediction = {
                "load_forecast": round(random.uniform(50, 150), 2),
                "temperature_forecast": round(random.uniform(20, 35), 1),
                "confidence": round(random.uniform(0.8, 0.99), 2)
            }
            self.publish_modsrv_output("model1", "prediction", prediction)
            
            time.sleep(interval)

    def publish_test_scenario(self, scenario):
        """发布测试场景数据"""
        print(f"\n执行测试场景: {scenario}")
        
        if scenario == "high_temp":
            # 高温场景
            print("场景：温度超过30°C，应触发冷却设备")
            self.publish_point_data("1001", "m", "10001", 32.5)
            time.sleep(1)
            self.publish_point_data("1001", "m", "10001", 35.0)
            
        elif scenario == "high_power":
            # 高功率场景
            print("场景：功率超过100kW，应触发负载降低")
            self.publish_point_data("1001", "m", "10002", 110000)  # 110kW
            time.sleep(1)
            self.publish_point_data("1001", "m", "10002", 125000)  # 125kW
            
        elif scenario == "combined_alarm":
            # 组合告警场景
            print("场景：温度和功率同时超限，应触发综合告警")
            self.publish_point_data("1001", "m", "10001", 38.0)   # 高温
            self.publish_point_data("1001", "m", "10002", 130000) # 高功率
            
        elif scenario == "low_efficiency":
            # 低效率场景
            print("场景：模型输出效率低于85%，应触发优化")
            self.publish_modsrv_output("model1", "efficiency", 0.82)
            time.sleep(1)
            self.publish_modsrv_output("model1", "efficiency", 0.78)
            
        else:
            print(f"未知场景: {scenario}")

    def start_continuous_simulation(self):
        """启动连续模拟"""
        self.running = True
        
        # 启动各个模拟线程
        threads = [
            threading.Thread(target=self.simulate_temperature_data, name="温度模拟"),
            threading.Thread(target=self.simulate_power_data, name="功率模拟"),
            threading.Thread(target=self.simulate_model_outputs, name="模型输出模拟")
        ]
        
        for t in threads:
            t.daemon = True
            t.start()
            
        print("\n持续模拟已启动，按Ctrl+C停止...")
        
        try:
            while True:
                time.sleep(1)
        except KeyboardInterrupt:
            print("\n停止模拟...")
            self.running = False
            time.sleep(2)

    def monitor_redis_channels(self, patterns):
        """监控Redis通道"""
        print(f"\n监控Redis通道: {patterns}")
        
        # 订阅通道
        for pattern in patterns:
            self.pubsub.psubscribe(pattern)
        
        print("开始监控，按Ctrl+C停止...")
        
        try:
            for message in self.pubsub.listen():
                if message['type'] in ['pmessage', 'message']:
                    print(f"\n[{datetime.now().strftime('%H:%M:%S')}] 通道: {message['channel']}")
                    print(f"数据: {message['data']}")
        except KeyboardInterrupt:
            print("\n停止监控")
            self.pubsub.close()

def main():
    parser = argparse.ArgumentParser(description='数据发布测试脚本')
    parser.add_argument('--mode', choices=['continuous', 'scenario', 'monitor', 'single'], 
                        default='continuous', help='运行模式')
    parser.add_argument('--scenario', choices=['high_temp', 'high_power', 'combined_alarm', 'low_efficiency'],
                        help='测试场景（用于scenario模式）')
    parser.add_argument('--redis-host', default=REDIS_HOST, help='Redis主机')
    parser.add_argument('--redis-port', type=int, default=REDIS_PORT, help='Redis端口')
    
    # 单次发布参数
    parser.add_argument('--channel-id', help='通道ID')
    parser.add_argument('--point-type', choices=['m', 's', 'c', 'a'], help='点位类型')
    parser.add_argument('--point-id', help='点位ID')
    parser.add_argument('--value', help='数值')
    
    args = parser.parse_args()
    
    # 创建发布器
    publisher = DataPublisher(args.redis_host, args.redis_port)
    
    if args.mode == 'continuous':
        # 持续模拟模式
        publisher.start_continuous_simulation()
        
    elif args.mode == 'scenario':
        # 场景测试模式
        if not args.scenario:
            print("错误：scenario模式需要指定 --scenario")
            return
        publisher.publish_test_scenario(args.scenario)
        
    elif args.mode == 'monitor':
        # 监控模式
        patterns = [
            "point:update:*",
            "modsrv:outputs:*",
            "alarm:event:*",
            "cmd:*:*"  # 监控命令通道
        ]
        publisher.monitor_redis_channels(patterns)
        
    elif args.mode == 'single':
        # 单次发布模式
        if not all([args.channel_id, args.point_type, args.point_id, args.value]):
            print("错误：single模式需要指定 --channel-id, --point-type, --point-id, --value")
            return
            
        try:
            value = float(args.value)
        except ValueError:
            print(f"错误：数值必须是数字，收到: {args.value}")
            return
            
        publisher.publish_point_data(args.channel_id, args.point_type, args.point_id, value)

if __name__ == "__main__":
    main()