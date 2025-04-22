#!/usr/bin/env python3
"""
虚拟通道模拟器
用于监听TCP端口并记录接收到的报文，不提供回复功能
"""

import asyncio
import logging
import argparse
import time
import binascii
import os
from datetime import datetime

# 配置日志
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger("VirtualChannel")

# 默认配置
DEFAULT_HOST = "127.0.0.1"
DEFAULT_PORT = 9000

def format_hex_data(data, bytes_per_line=16):
    """将十六进制数据格式化为可读的格式"""
    hex_str = binascii.hexlify(data).decode('utf-8')
    # 添加空格分隔每两个字符（一个字节）
    hex_bytes = [hex_str[i:i+2] for i in range(0, len(hex_str), 2)]
    # 分行显示
    lines = []
    for i in range(0, len(hex_bytes), bytes_per_line):
        line_bytes = hex_bytes[i:i+bytes_per_line]
        # 格式化为 "00 01 02 03 ..."
        hex_part = " ".join(line_bytes)
        # 尝试解析ASCII部分
        ascii_part = ""
        for byte in line_bytes:
            byte_val = int(byte, 16)
            # 如果是可打印ASCII字符
            if 32 <= byte_val <= 126:
                ascii_part += chr(byte_val)
            else:
                ascii_part += "."
        # 组合结果：地址 + 十六进制 + ASCII
        addr = f"{i//bytes_per_line*bytes_per_line:04X}"
        lines.append(f"{addr}: {hex_part.ljust(bytes_per_line*3-1)}  |{ascii_part}|")
    return "\n".join(lines)

class VirtualChannelProtocol(asyncio.Protocol):
    """TCP协议处理类"""
    
    def __init__(self, log_to_file=False, log_file=None, verbose=False):
        self.log_to_file = log_to_file
        self.log_file = log_file
        self.verbose = verbose
        self.transport = None
        self.packet_count = 0

    def connection_made(self, transport):
        """连接建立时调用"""
        peername = transport.get_extra_info('peername')
        logger.info(f'连接来自: {peername}')
        self.transport = transport

    def data_received(self, data):
        """接收到数据时调用"""
        self.packet_count += 1
        timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S.%f")[:-3]
        peername = self.transport.get_extra_info('peername')
        hex_data = binascii.hexlify(data).decode('utf-8')
        
        # 基本日志信息
        log_message = f"[{timestamp}] 从 {peername} 接收数据包 #{self.packet_count} [长度: {len(data)} 字节]"
        logger.info(log_message)
        
        # 详细模式下显示数据内容
        if self.verbose:
            formatted_data = format_hex_data(data)
            logger.info(f"数据内容:\n{formatted_data}")
        
        # 如果需要，将报文保存到文件
        if self.log_to_file and self.log_file:
            try:
                with open(self.log_file, 'a') as f:
                    f.write(f"{log_message}\n")
                    f.write(f"原始数据: {hex_data}\n")
                    if self.verbose:
                        f.write(f"格式化数据:\n{formatted_data}\n")
                    f.write("="*80 + "\n")
            except Exception as e:
                logger.error(f"写入日志文件时出错: {e}")

    def connection_lost(self, exc):
        """连接断开时调用"""
        peername = self.transport.get_extra_info('peername')
        logger.info(f'连接断开: {peername}，共接收 {self.packet_count} 个数据包')
        if exc:
            logger.error(f'连接错误: {exc}')

async def start_server(host, port, log_to_file=False, log_file=None, verbose=False):
    """启动服务器"""
    # 创建协议工厂
    def protocol_factory():
        return VirtualChannelProtocol(log_to_file, log_file, verbose)
    
    # 创建服务器
    server = await asyncio.start_server(
        protocol_factory,
        host, port
    )

    addr = server.sockets[0].getsockname()
    logger.info(f'虚拟通道服务器启动: {addr}')
    
    if log_to_file:
        # 创建日志文件目录
        os.makedirs(os.path.dirname(os.path.abspath(log_file)), exist_ok=True)
        logger.info(f'日志文件: {log_file}')
        
        # 写入日志文件头部
        with open(log_file, 'w') as f:
            f.write(f"Virtual Channel Simulator Log\n")
            f.write(f"启动时间: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n")
            f.write(f"监听地址: {addr}\n")
            f.write("="*80 + "\n\n")

    async with server:
        await server.serve_forever()

def main():
    """主函数"""
    # 解析命令行参数
    parser = argparse.ArgumentParser(description="虚拟通道模拟器 - 监听TCP端口并记录收到的报文")
    parser.add_argument("--host", default=DEFAULT_HOST, help=f"监听地址 (默认: {DEFAULT_HOST})")
    parser.add_argument("--port", type=int, default=DEFAULT_PORT, help=f"监听端口 (默认: {DEFAULT_PORT})")
    parser.add_argument("--log-dir", type=str, default="./logs", help="日志文件目录 (默认: ./logs)")
    parser.add_argument("--log-file", type=str, help="指定日志文件名 (不含路径)")
    parser.add_argument("--no-log", action="store_true", help="不记录到文件")
    parser.add_argument("-v", "--verbose", action="store_true", help="详细模式，显示数据包内容")
    
    args = parser.parse_args()
    
    # 确定是否记录到文件
    log_to_file = not args.no_log
    
    # 创建日志文件名
    log_file = None
    if log_to_file:
        if args.log_file:
            # 使用指定的文件名
            log_file = os.path.join(args.log_dir, args.log_file)
        else:
            # 创建默认文件名 virtual_channel_YYYYMMDD_HHMMSS.log
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            log_file = os.path.join(args.log_dir, f"virtual_channel_{timestamp}.log")
    
    # 运行服务器
    try:
        asyncio.run(start_server(
            host=args.host,
            port=args.port,
            log_to_file=log_to_file,
            log_file=log_file,
            verbose=args.verbose
        ))
    except KeyboardInterrupt:
        logger.info("收到中断信号，停止服务器")
    except Exception as e:
        logger.error(f"服务器异常: {e}")

if __name__ == "__main__":
    main() 