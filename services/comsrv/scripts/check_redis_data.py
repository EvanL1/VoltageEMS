#!/usr/bin/env python3
"""
检查Redis中实际存储的点位数据
"""

import subprocess
import time


def run_docker_redis_cmd(cmd):
    """运行Redis命令"""
    try:
        # 通过临时容器连接到Redis，使用密码认证
        full_cmd = [
            "docker",
            "run",
            "--rm",
            "--network",
            "comsrv-test-network",
            "redis:7-alpine",
            "redis-cli",
            "-h",
            "redis",
            "-p",
            "6379",
            "-a",
            "testpass123",
        ] + cmd.split()

        result = subprocess.run(full_cmd, capture_output=True, text=True, timeout=10)
        if result.returncode == 0:
            return result.stdout.strip()
        else:
            print(f"Redis命令执行失败: {result.stderr}")
            return None
    except subprocess.TimeoutExpired:
        print("Redis命令执行超时")
        return None
    except Exception as e:
        print(f"执行Redis命令时出错: {e}")
        return None


def check_signal_data():
    """检查信号数据"""
    print("=" * 60)
    print("检查Redis中的信号数据")
    print("=" * 60)

    # 按照架构文档，使用Hash结构: comsrv:1001:s
    hash_key = "comsrv:1001:s"
    signal_data = run_docker_redis_cmd(f"HGETALL {hash_key}")

    if signal_data:
        lines = signal_data.split("\n")
        points = {}
        for i in range(0, len(lines), 2):
            if i + 1 < len(lines):
                point_id = lines[i]
                value = lines[i + 1]
                if point_id and value:  # 确保不是空字符串
                    points[int(point_id)] = value

        print(f"找到 {len(points)} 个信号点位:")
        print("-" * 40)

        # 按点位ID排序显示
        for point_id in sorted(points.keys()):
            value = points[point_id]
            print(f"点位{point_id:2d}: {value}")

        # 检查缺失的点位
        expected_points = set(range(1, 17))  # 期望1-16
        actual_points = set(points.keys())
        missing_points = expected_points - actual_points

        if missing_points:
            print(f"\n缺失的点位: {sorted(missing_points)}")
        else:
            print("\n✅ 所有期望的点位都存在")

        return points
    else:
        print("❌ 无法获取信号数据")
        return {}


def check_all_keys():
    """检查所有comsrv相关的键"""
    print("\n" + "=" * 60)
    print("检查所有comsrv相关的Redis键")
    print("=" * 60)

    keys = run_docker_redis_cmd("KEYS comsrv:*")
    if keys and keys != "(empty array)":
        key_list = keys.split("\n") if keys else []
        key_list = [k for k in key_list if k.strip()]  # 过滤空行
        print(f"找到 {len(key_list)} 个键:")
        for key in sorted(key_list):
            key_type = run_docker_redis_cmd(f"TYPE {key}")
            if key_type == "hash":
                count = run_docker_redis_cmd(f"HLEN {key}")
                print(f"  {key} (hash, {count} 个字段)")
            else:
                print(f"  {key} ({key_type})")
    else:
        print("❌ 没有找到comsrv相关的键")


def analyze_bit_mapping():
    """分析位映射对应关系"""
    print("\n" + "=" * 60)
    print("分析位映射对应关系")
    print("=" * 60)

    # 模拟器设置的值
    register1_value = 0xA5  # 10100101
    register2_value = 0x5A  # 01011010

    print("模拟器设置:")
    print(f"寄存器1: 0x{register1_value:02X} = {register1_value:08b}")
    print(f"寄存器2: 0x{register2_value:02X} = {register2_value:08b}")

    print("\n期望的点位值:")
    print("寄存器1 (点位1-8):")
    for bit in range(8):
        expected_value = (register1_value >> bit) & 1
        print(f"  点位{bit + 1} (位{bit}): {expected_value}")

    print("寄存器2 (点位9-16):")
    for bit in range(8):
        expected_value = (register2_value >> bit) & 1
        print(f"  点位{bit + 9} (位{bit}): {expected_value}")


def main():
    """主函数"""
    print("🔍 检查Redis中的Modbus位解析数据")
    print("时间:", time.strftime("%Y-%m-%d %H:%M:%S"))

    # 检查Redis连接
    redis_info = run_docker_redis_cmd("INFO server")
    if redis_info:
        print("✅ Redis连接正常")
    else:
        print("❌ 无法连接到Redis")
        return

    # 检查数据
    signal_points = check_signal_data()
    check_all_keys()
    analyze_bit_mapping()

    # 总结
    print("\n" + "=" * 60)
    print("检查总结")
    print("=" * 60)

    if signal_points:
        expected_points = set(range(1, 17))
        actual_points = set(signal_points.keys())
        missing_points = expected_points - actual_points

        if missing_points:
            print(f"❌ 发现问题: {len(missing_points)} 个点位缺失")
            print(f"   缺失点位: {sorted(missing_points)}")

            # 分析缺失模式
            if missing_points == {1, 2, 3}:
                print("   分析: 寄存器1的位0,1,2没有被存储到Redis")
                print("   可能原因: 配置加载或轮询逻辑问题")
        else:
            print("✅ 所有16个点位都正常存储")

    print("\n检查完成!")


if __name__ == "__main__":
    main()
