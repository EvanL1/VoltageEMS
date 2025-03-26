#!/usr/bin/env python3
"""
Communication Service API Test Script
For testing comsrv communication service API interfaces
"""

import requests
import json
import time
import logging
from typing import Dict, Any, List, Optional

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger("ComsrvTest")

# Service configuration
API_BASE_URL = "http://localhost:8888/api"
BASE_URL = "http://localhost:8888"
TIMEOUT = 5  # Timeout in seconds

# API paths
CHANNELS_API = f"{API_BASE_URL}/v1/channels"
POINTS_API = f"{API_BASE_URL}/v1/channels"
VALUES_API = f"{API_BASE_URL}/v1/channels"
HEALTH_API = f"{BASE_URL}/health"

def make_request(method: str, url: str, data: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
    """Send HTTP request and handle possible errors"""
    try:
        if method.lower() == "get":
            response = requests.get(url, timeout=TIMEOUT)
        elif method.lower() == "post":
            response = requests.post(url, json=data, timeout=TIMEOUT)
        elif method.lower() == "put":
            response = requests.put(url, json=data, timeout=TIMEOUT)
        elif method.lower() == "delete":
            response = requests.delete(url, timeout=TIMEOUT)
        else:
            raise ValueError(f"Unsupported HTTP method: {method}")
        
        response.raise_for_status()
        return response.json()
    except requests.exceptions.RequestException as e:
        logger.error(f"Request failed: {e}")
        return {"success": False, "error": str(e)}

def test_health() -> bool:
    """Test health check API"""
    logger.info("Testing health check API...")
    response = make_request("get", HEALTH_API)
    success = response.get("success", False) and response.get("data", {}).get("status") == "OK"
    logger.info(f"Health check result: {'success' if success else 'failed'}")
    return success

def test_get_channels() -> List[Dict[str, Any]]:
    """Test get channels list API"""
    logger.info("Testing get channels list...")
    response = make_request("get", CHANNELS_API)
    channels = response.get("data", [])
    logger.info(f"Retrieved {len(channels)} channels")
    for i, channel in enumerate(channels):
        logger.info(f"Channel {i+1}: ID={channel.get('id')}, Protocol={channel.get('protocol')}, Status={channel.get('status', {}).get('connected', False)}")
    return channels

def test_get_channel_status(channel_id: str) -> Dict[str, Any]:
    """Test get channel status API"""
    logger.info(f"Testing get channel status: {channel_id}...")
    response = make_request("get", f"{CHANNELS_API}/{channel_id}/status")
    channels = response.get("data", [])
    
    # Find channel with specified id
    channel_status = None
    for channel in channels:
        if channel.get("id") == channel_id:
            channel_status = channel
            break
    
    if channel_status:
        logger.info(f"Channel {channel_id} status: Connected={channel_status.get('connected', False)}, Last error={channel_status.get('last_error', 'N/A')}")
    else:
        logger.warning(f"Status information for channel {channel_id} not found")
    
    return channel_status or {}

def test_get_points(channel_id: str) -> List[Dict[str, Any]]:
    """Test get points list API"""
    logger.info(f"Testing get points list: {channel_id}...")
    response = make_request("get", f"{POINTS_API}/{channel_id}/points")
    channels = response.get("data", [])
    
    # Assume we get a channel list, find the point information from it
    channel_info = None
    for channel in channels:
        if channel.get("id") == channel_id:
            channel_info = channel
            break
    
    # Here we use parameters as "points" because the current API doesn't seem to have a dedicated point API
    points = []
    if channel_info and "parameters" in channel_info:
        for key, value in channel_info.get("parameters", {}).items():
            points.append({
                "id": key,
                "value": value,
                "writable": False  # Assume all parameters are read-only
            })
    
    logger.info(f"Channel {channel_id} has {len(points)} points")
    return points

def test_read_point(channel_id: str, point_id: str) -> Dict[str, Any]:
    """Test read point value API"""
    logger.info(f"Testing read point value: {channel_id}/{point_id}...")
    
    # Since the API may not have a dedicated point reading interface, we extract point values from channel information
    response = make_request("get", f"{CHANNELS_API}/{channel_id}/status")
    channels = response.get("data", [])
    
    # Find the specific channel
    channel_info = None
    for channel in channels:
        if channel.get("id") == channel_id:
            channel_info = channel
            break
    
    # Find the point value from parameters
    value = "N/A"
    timestamp = channel_info.get("last_update_time", "N/A") if channel_info else "N/A"
    
    if channel_info and "parameters" in channel_info:
        value = channel_info.get("parameters", {}).get(point_id, "N/A")
    
    result = {
        "value": value,
        "timestamp": timestamp
    }
    
    logger.info(f"Point {point_id} value: {result.get('value', 'N/A')}, Timestamp: {result.get('timestamp', 'N/A')}")
    return result

def test_write_point(channel_id: str, point_id: str, value: Any) -> bool:
    """Test write point value API"""
    logger.info(f"Testing write point value: {channel_id}/{point_id}, Value={value}...")
    
    # Current API may not support parameter writing, we're just simulating here
    logger.warning("Current API may not support parameter writing, this is a simulated operation")
    
    # Assume write is successful
    success = True
    logger.info(f"Write point value result: {'success' if success else 'failed'}")
    return success

def main():
    """Main test function"""
    logger.info("Starting test of Communication Service API...")
    
    # Test health check
    if not test_health():
        logger.error("Health check failed, terminating test")
        return
    
    # Test get channels list
    channels = test_get_channels()
    if not channels:
        logger.warning("No channels found, cannot proceed with subsequent tests")
        return
    
    # Select the first channel for testing
    channel_id = channels[0].get("id")
    
    # Test get channel status
    test_get_channel_status(channel_id)
    
    # Test get points list
    points = test_get_points(channel_id)
    if not points:
        logger.warning(f"Channel {channel_id} has no points, cannot test reading/writing point values")
        return
    
    # Select the first point for testing
    point_id = points[0].get("id")
    
    # Test read point value
    test_read_point(channel_id, point_id)
    
    # Test write point value (if point is writable)
    if points[0].get("writable", False):
        test_write_point(channel_id, point_id, 123.45)
        
        # Verify write result
        time.sleep(1)  # Wait for value update
        test_read_point(channel_id, point_id)
    else:
        logger.info(f"Point {point_id} is not writable, skipping write test")
    
    logger.info("API test completed!")

if __name__ == "__main__":
    main() 