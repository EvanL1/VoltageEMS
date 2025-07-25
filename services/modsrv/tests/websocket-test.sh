#!/bin/bash
# WebSocket测试脚本 - 测试ModSrv的WebSocket接口

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 配置
MODSRV_HOST="${MODSRV_HOST:-modsrv}"
MODSRV_PORT="${MODSRV_PORT:-8092}"
WS_URL="ws://${MODSRV_HOST}:${MODSRV_PORT}/ws"
RESULT_DIR="${TEST_OUTPUT:-/app/results}"
LOG_FILE="${LOG_FILE:-${RESULT_DIR}/websocket_test.log}"

# 日志函数
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1" | tee -a "$LOG_FILE"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" | tee -a "$LOG_FILE"
}

log_test() {
    echo -e "${BLUE}[TEST]${NC} $1" | tee -a "$LOG_FILE"
}

log_result() {
    if [ $1 -eq 0 ]; then
        echo -e "${GREEN}[PASS]${NC} $2" | tee -a "$LOG_FILE"
    else
        echo -e "${RED}[FAIL]${NC} $2" | tee -a "$LOG_FILE"
    fi
}

# 创建WebSocket测试Python脚本
create_ws_test_script() {
    cat > /tmp/ws_test.py << 'EOF'
#!/usr/bin/env python3
import asyncio
import json
import sys
import time
import websockets
from datetime import datetime

class WebSocketTester:
    def __init__(self, url, output_file):
        self.url = url
        self.output_file = output_file
        self.messages_received = []
        self.test_results = {
            "connection": False,
            "subscribe": False,
            "telemetry_updates": False,
            "unsubscribe": False,
            "message_count": 0,
            "errors": []
        }
    
    async def test_connection(self):
        """测试WebSocket连接"""
        try:
            async with websockets.connect(self.url) as websocket:
                print(f"[PASS] 成功连接到 {self.url}")
                self.test_results["connection"] = True
                
                # 测试订阅
                await self.test_subscribe(websocket)
                
                # 接收消息
                await self.receive_messages(websocket, duration=10)
                
                # 测试取消订阅
                await self.test_unsubscribe(websocket)
                
        except Exception as e:
            print(f"[ERROR] 连接失败: {e}")
            self.test_results["errors"].append(str(e))
    
    async def test_subscribe(self, websocket):
        """测试订阅功能"""
        subscribe_msg = {
            "type": "subscribe",
            "model_id": "power_meter_demo",
            "telemetry_ids": ["voltage_a", "voltage_b", "voltage_c", "current_a", "power_total"]
        }
        
        try:
            await websocket.send(json.dumps(subscribe_msg))
            print(f"[INFO] 发送订阅消息: {subscribe_msg}")
            
            # 等待确认消息
            response = await asyncio.wait_for(websocket.recv(), timeout=5.0)
            response_data = json.loads(response)
            
            if response_data.get("type") == "subscribed":
                print(f"[PASS] 订阅成功: {response_data}")
                self.test_results["subscribe"] = True
            else:
                print(f"[FAIL] 订阅响应异常: {response_data}")
                
        except asyncio.TimeoutError:
            print("[ERROR] 等待订阅确认超时")
            self.test_results["errors"].append("Subscribe confirmation timeout")
        except Exception as e:
            print(f"[ERROR] 订阅失败: {e}")
            self.test_results["errors"].append(f"Subscribe error: {e}")
    
    async def receive_messages(self, websocket, duration=10):
        """接收并记录消息"""
        print(f"[INFO] 开始接收消息，持续 {duration} 秒...")
        start_time = time.time()
        
        try:
            while time.time() - start_time < duration:
                try:
                    message = await asyncio.wait_for(websocket.recv(), timeout=1.0)
                    msg_data = json.loads(message)
                    
                    self.messages_received.append({
                        "timestamp": datetime.now().isoformat(),
                        "data": msg_data
                    })
                    
                    if msg_data.get("type") == "telemetry_update":
                        self.test_results["telemetry_updates"] = True
                        print(f"[INFO] 收到遥测更新: {msg_data.get('telemetry_id')} = {msg_data.get('value')}")
                    
                except asyncio.TimeoutError:
                    continue
                except json.JSONDecodeError as e:
                    print(f"[WARN] 无法解析消息: {e}")
                    
        except Exception as e:
            print(f"[ERROR] 接收消息时出错: {e}")
            self.test_results["errors"].append(f"Receive error: {e}")
        
        self.test_results["message_count"] = len(self.messages_received)
        print(f"[INFO] 共收到 {self.test_results['message_count']} 条消息")
    
    async def test_unsubscribe(self, websocket):
        """测试取消订阅"""
        unsubscribe_msg = {
            "type": "unsubscribe",
            "model_id": "power_meter_demo",
            "telemetry_ids": ["voltage_a"]
        }
        
        try:
            await websocket.send(json.dumps(unsubscribe_msg))
            print(f"[INFO] 发送取消订阅消息: {unsubscribe_msg}")
            
            # 等待确认
            response = await asyncio.wait_for(websocket.recv(), timeout=5.0)
            response_data = json.loads(response)
            
            if response_data.get("type") == "unsubscribed":
                print(f"[PASS] 取消订阅成功: {response_data}")
                self.test_results["unsubscribe"] = True
            else:
                print(f"[FAIL] 取消订阅响应异常: {response_data}")
                
        except asyncio.TimeoutError:
            print("[ERROR] 等待取消订阅确认超时")
            self.test_results["errors"].append("Unsubscribe confirmation timeout")
        except Exception as e:
            print(f"[ERROR] 取消订阅失败: {e}")
            self.test_results["errors"].append(f"Unsubscribe error: {e}")
    
    def save_results(self):
        """保存测试结果"""
        results = {
            "test_time": datetime.now().isoformat(),
            "url": self.url,
            "test_results": self.test_results,
            "messages_received": self.messages_received
        }
        
        with open(self.output_file, 'w') as f:
            json.dump(results, f, indent=2)
        
        print(f"[INFO] 测试结果已保存到: {self.output_file}")
    
    def print_summary(self):
        """打印测试摘要"""
        print("\n========== WebSocket测试摘要 ==========")
        print(f"连接测试: {'通过' if self.test_results['connection'] else '失败'}")
        print(f"订阅测试: {'通过' if self.test_results['subscribe'] else '失败'}")
        print(f"遥测更新: {'通过' if self.test_results['telemetry_updates'] else '失败'}")
        print(f"取消订阅: {'通过' if self.test_results['unsubscribe'] else '失败'}")
        print(f"收到消息数: {self.test_results['message_count']}")
        print(f"错误数: {len(self.test_results['errors'])}")
        print("=====================================")
        
        # 返回状态码
        if all([
            self.test_results['connection'],
            self.test_results['subscribe'],
            self.test_results['telemetry_updates']
        ]) and len(self.test_results['errors']) == 0:
            return 0
        else:
            return 1

async def main():
    if len(sys.argv) != 3:
        print("Usage: python ws_test.py <websocket_url> <output_file>")
        sys.exit(1)
    
    url = sys.argv[1]
    output_file = sys.argv[2]
    
    tester = WebSocketTester(url, output_file)
    await tester.test_connection()
    tester.save_results()
    exit_code = tester.print_summary()
    sys.exit(exit_code)

if __name__ == "__main__":
    asyncio.run(main())
EOF
    
    chmod +x /tmp/ws_test.py
}

# 等待服务就绪
wait_for_service() {
    log_info "等待ModSrv WebSocket服务就绪..."
    local max_attempts=30
    local attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if nc -z "$MODSRV_HOST" "$MODSRV_PORT" 2>/dev/null; then
            log_info "ModSrv服务端口已开放"
            return 0
        fi
        attempt=$((attempt + 1))
        log_info "等待服务启动... ($attempt/$max_attempts)"
        sleep 2
    done
    
    log_error "ModSrv服务未能在预期时间内启动"
    return 1
}

# 测试WebSocket连接性
test_ws_connectivity() {
    log_test "测试WebSocket基本连接"
    
    # 使用websocat进行简单连接测试
    if command -v websocat > /dev/null 2>&1; then
        echo '{"type":"ping"}' | timeout 5 websocat -n1 "$WS_URL" > /tmp/ws_ping.txt 2>&1
        if [ $? -eq 0 ]; then
            log_result 0 "WebSocket连接测试通过"
            return 0
        else
            log_result 1 "WebSocket连接测试失败"
            cat /tmp/ws_ping.txt >> "$LOG_FILE"
            return 1
        fi
    else
        log_info "跳过websocat连接测试（未安装websocat）"
        return 0
    fi
}

# 运行Python WebSocket测试
test_ws_python() {
    log_test "运行完整WebSocket功能测试"
    
    create_ws_test_script
    
    # 确保安装了websockets库
    if ! python3 -c "import websockets" 2>/dev/null; then
        log_info "安装websockets库..."
        pip3 install websockets > /dev/null 2>&1
    fi
    
    # 运行测试
    output_file="${RESULT_DIR}/websocket_test_results.json"
    python3 /tmp/ws_test.py "$WS_URL" "$output_file"
    test_result=$?
    
    if [ $test_result -eq 0 ]; then
        log_result 0 "WebSocket功能测试通过"
        return 0
    else
        log_result 1 "WebSocket功能测试失败"
        return 1
    fi
}

# 测试并发WebSocket连接
test_ws_concurrent() {
    log_test "测试并发WebSocket连接"
    
    cat > /tmp/ws_concurrent.py << 'EOF'
#!/usr/bin/env python3
import asyncio
import json
import sys
import websockets
from datetime import datetime

async def client_task(client_id, url, results):
    try:
        async with websockets.connect(url) as websocket:
            # 订阅
            subscribe_msg = {
                "type": "subscribe",
                "model_id": "power_meter_demo",
                "telemetry_ids": ["voltage_a"]
            }
            await websocket.send(json.dumps(subscribe_msg))
            
            # 接收几条消息
            message_count = 0
            start_time = asyncio.get_event_loop().time()
            
            while asyncio.get_event_loop().time() - start_time < 5:
                try:
                    message = await asyncio.wait_for(websocket.recv(), timeout=1.0)
                    message_count += 1
                except asyncio.TimeoutError:
                    continue
            
            results[client_id] = {
                "success": True,
                "messages": message_count
            }
            
    except Exception as e:
        results[client_id] = {
            "success": False,
            "error": str(e)
        }

async def test_concurrent(url, num_clients):
    results = {}
    tasks = []
    
    for i in range(num_clients):
        task = client_task(i, url, results)
        tasks.append(task)
    
    await asyncio.gather(*tasks)
    
    # 统计结果
    successful = sum(1 for r in results.values() if r["success"])
    total_messages = sum(r.get("messages", 0) for r in results.values())
    
    print(f"并发客户端数: {num_clients}")
    print(f"成功连接数: {successful}")
    print(f"总消息数: {total_messages}")
    
    return successful == num_clients

if __name__ == "__main__":
    url = sys.argv[1]
    num_clients = int(sys.argv[2]) if len(sys.argv) > 2 else 10
    
    success = asyncio.run(test_concurrent(url, num_clients))
    sys.exit(0 if success else 1)
EOF
    
    chmod +x /tmp/ws_concurrent.py
    
    # 运行并发测试
    python3 /tmp/ws_concurrent.py "$WS_URL" 10
    test_result=$?
    
    if [ $test_result -eq 0 ]; then
        log_result 0 "并发WebSocket连接测试通过"
        return 0
    else
        log_result 1 "并发WebSocket连接测试失败"
        return 1
    fi
}

# 主测试流程
main() {
    log_info "开始ModSrv WebSocket测试"
    log_info "WebSocket URL: $WS_URL"
    log_info "结果目录: $RESULT_DIR"
    
    TESTS_PASSED=0
    TESTS_FAILED=0
    
    # 创建结果目录
    mkdir -p "$RESULT_DIR"
    
    # 等待服务就绪
    if ! wait_for_service; then
        exit 1
    fi
    
    # 运行测试
    if test_ws_connectivity; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    
    if test_ws_python; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    
    if test_ws_concurrent; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    
    # 输出总结
    log_info "========================================="
    log_info "WebSocket测试完成"
    log_info "通过: $TESTS_PASSED"
    log_info "失败: $TESTS_FAILED"
    log_info "========================================="
    
    # 保存测试摘要
    cat > "${RESULT_DIR}/websocket_test_summary.json" << EOF
{
    "test_time": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
    "websocket_url": "$WS_URL",
    "tests": {
        "total": $((TESTS_PASSED + TESTS_FAILED)),
        "passed": $TESTS_PASSED,
        "failed": $TESTS_FAILED
    }
}
EOF
    
    if [ $TESTS_FAILED -gt 0 ]; then
        return 1
    else
        return 0
    fi
}

# 执行主函数
main
exit $?