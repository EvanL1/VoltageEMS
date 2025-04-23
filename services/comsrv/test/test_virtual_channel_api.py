#!/usr/bin/env python3
"""
虚拟通道API测试脚本
用于测试通过API发送写入命令，触发comsrv向虚拟通道发送报文
"""

import argparse
import json
import requests
import time
import sys

# 默认配置
DEFAULT_API_HOST = "127.0.0.1"
DEFAULT_API_PORT = 8888       # Docker映射后的API端口
DEFAULT_CHANNEL_ID = 1
DEFAULT_POLL_INTERVAL = 1.0  # 秒

def test_write_command(api_host, api_port, channel_id, value=100, interval=1.0):
    """测试向通道发送写命令"""
    base_url = f"http://{api_host}:{api_port}/api/v1"
    
    # 测试可用点位
    print(f"获取通道 {channel_id} 的所有点位...")
    try:
        response = requests.get(f"{base_url}/channels/{channel_id}/points")
        response.raise_for_status()
        points = response.json()
        
        write_points = [p for p in points if "write" in p.get("read_write", "").lower()]
        if not write_points:
            print("错误: 没有找到可写入的点位")
            return False
            
        print(f"找到 {len(write_points)} 个可写入的点位:")
        for i, point in enumerate(write_points):
            print(f"  {i+1}. {point['id']} ({point['name']}): {point['data_type']}")
            
    except requests.exceptions.RequestException as e:
        print(f"获取点位失败: {e}")
        return False
        
    # 选择第一个可写入的点位进行测试
    test_point = write_points[0]
    point_id = test_point["id"]
    data_type = test_point["data_type"]
    
    # 根据数据类型调整写入值
    if data_type == "UINT16":
        write_value = int(value) % 65536
    elif data_type == "INT16":
        write_value = int(value) % 32768
    elif data_type == "UINT32":
        write_value = int(value) % 4294967296
    elif data_type == "INT32":
        write_value = int(value) % 2147483648
    elif data_type == "FLOAT32":
        write_value = float(value)
    elif data_type == "BOOL":
        write_value = bool(int(value))
    elif data_type == "STRING16":
        write_value = str(value)
    else:
        write_value = value
        
    print(f"\n开始测试写入点位 {point_id} ({data_type})...")
    
    # 循环发送写入命令
    count = 1
    try:
        while True:
            print(f"\n写入第 {count} 次...")
            
            # 构建请求数据
            payload = {
                "id": point_id,
                "value": write_value
            }
            
            # 发送写入请求
            print(f"发送: {payload}")
            try:
                response = requests.post(
                    f"{base_url}/channels/{channel_id}/points/write",
                    json=payload
                )
                response.raise_for_status()
                result = response.json()
                print(f"响应: {result}")
                
            except requests.exceptions.RequestException as e:
                print(f"写入失败: {e}")
                
            count += 1
            time.sleep(interval)
            
    except KeyboardInterrupt:
        print("\n用户中断，停止测试")
        return True

def main():
    """主函数"""
    parser = argparse.ArgumentParser(description="虚拟通道API测试脚本")
    parser.add_argument("--host", default=DEFAULT_API_HOST, help=f"API主机地址 (默认: {DEFAULT_API_HOST})")
    parser.add_argument("--port", type=int, default=DEFAULT_API_PORT, help=f"API端口 (默认: {DEFAULT_API_PORT})")
    parser.add_argument("--channel", type=int, default=DEFAULT_CHANNEL_ID, help=f"通道ID (默认: {DEFAULT_CHANNEL_ID})")
    parser.add_argument("--value", type=float, default=100, help="要写入的值 (默认: 100)")
    parser.add_argument("--interval", type=float, default=DEFAULT_POLL_INTERVAL, help=f"写入间隔(秒) (默认: {DEFAULT_POLL_INTERVAL})")
    
    args = parser.parse_args()
    
    success = test_write_command(
        api_host=args.host,
        api_port=args.port,
        channel_id=args.channel,
        value=args.value,
        interval=args.interval
    )
    
    sys.exit(0 if success else 1)

if __name__ == "__main__":
    main() 