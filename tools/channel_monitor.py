#!/usr/bin/env python3
"""Simple comsrv channel monitoring tool."""

import argparse
import time
from datetime import datetime
from typing import List

import requests


def fetch_channel_status(base_url: str):
    url = f"{base_url.rstrip('/')}/api/v1/channels"
    resp = requests.get(url, timeout=5)
    resp.raise_for_status()
    data = resp.json()
    if isinstance(data, dict) and 'data' in data:
        return data['data']
    return data


def print_channels(channels: List[dict]):
    print(datetime.now().strftime("%Y-%m-%d %H:%M:%S"))
    header = f"{'ID':<5} {'NAME':<15} {'PROTOCOL':<10} {'STATUS':<8} LAST_ERROR"
    print(header)
    print("-" * len(header))
    for ch in channels:
        status = 'ON' if ch.get('connected') else 'OFF'
        print(f"{ch.get('id', ''):<5} {ch.get('name', ''):<15} {ch.get('protocol', ''):<10} {status:<8} {ch.get('last_error', '')}")
    print()


def interactive_loop(base_url: str, interval: float):
    try:
        while True:
            channels = fetch_channel_status(base_url)
            print_channels(channels)
            time.sleep(interval)
    except KeyboardInterrupt:
        print("Exiting...")


def main():
    parser = argparse.ArgumentParser(description="Monitor comsrv channel status")
    parser.add_argument('-u', '--url', default='http://localhost:3001', help='Base URL of comsrv service')
    parser.add_argument('-i', '--interactive', action='store_true', help='Run in interactive mode')
    parser.add_argument('--interval', type=float, default=2.0, help='Refresh interval in seconds')
    args = parser.parse_args()

    if args.interactive:
        interactive_loop(args.url, args.interval)
    else:
        channels = fetch_channel_status(args.url)
        print_channels(channels)


if __name__ == '__main__':
    main()
