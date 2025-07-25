#!/usr/bin/env python3
"""ModSrv API功能完整测试套件 - 支持报文保存"""

import os
import json
import time
import requests
from datetime import datetime
from pathlib import Path
from typing import Dict, Any, Optional


class APITestSuite:
    """API测试套件类"""

    def __init__(self, base_url: str, results_dir: str = "/app/results"):
        self.base_url = base_url.rstrip("/")
        self.results_dir = Path(results_dir)
        self.api_messages_dir = self.results_dir / "api-messages"
        self.setup_directories()

    def setup_directories(self):
        """创建结果目录结构"""
        directories = [
            self.api_messages_dir / "health_check",
            self.api_messages_dir / "model_list",
            self.api_messages_dir / "model_detail",
            self.api_messages_dir / "control_commands",
            self.api_messages_dir / "performance",
        ]

        for directory in directories:
            directory.mkdir(parents=True, exist_ok=True)

    def save_api_message(
        self,
        endpoint: str,
        method: str,
        request_data: Optional[Dict] = None,
        response_data: Optional[Dict] = None,
        metadata: Optional[Dict] = None,
    ) -> str:
        """保存API请求响应报文"""
        timestamp = datetime.now().isoformat()

        # 确定保存目录
        if "health" in endpoint:
            save_dir = self.api_messages_dir / "health_check"
        elif endpoint == "/models" and method == "GET":
            save_dir = self.api_messages_dir / "model_list"
        elif "/models/" in endpoint and "/control/" not in endpoint:
            save_dir = self.api_messages_dir / "model_detail"
        elif "/control/" in endpoint:
            save_dir = self.api_messages_dir / "control_commands"
        else:
            save_dir = self.api_messages_dir / "performance"

        # 生成文件名
        clean_endpoint = endpoint.replace("/", "_").replace("{", "").replace("}", "")
        filename = f"{method.lower()}_{clean_endpoint}_{int(time.time() * 1000)}.json"
        filepath = save_dir / filename

        # 构建消息结构
        message = {
            "timestamp": timestamp,
            "endpoint": endpoint,
            "method": method,
            "url": f"{self.base_url}{endpoint}",
            "request": {
                "headers": {"Content-Type": "application/json"} if request_data else {},
                "body": request_data,
            },
            "response": response_data,
            "metadata": metadata or {},
        }

        # 保存到文件
        with open(filepath, "w", encoding="utf-8") as f:
            json.dump(message, f, ensure_ascii=False, indent=2)

        return str(filepath)

    def make_request(
        self,
        endpoint: str,
        method: str = "GET",
        data: Optional[Dict] = None,
        headers: Optional[Dict] = None,
    ) -> Dict[str, Any]:
        """发送HTTP请求并保存报文"""
        url = f"{self.base_url}{endpoint}"
        start_time = time.time()

        try:
            if method.upper() == "GET":
                response = requests.get(url, headers=headers, timeout=30)
            elif method.upper() == "POST":
                response = requests.post(
                    url,
                    json=data,
                    headers=headers or {"Content-Type": "application/json"},
                    timeout=30,
                )
            elif method.upper() == "PUT":
                response = requests.put(
                    url,
                    json=data,
                    headers=headers or {"Content-Type": "application/json"},
                    timeout=30,
                )
            elif method.upper() == "DELETE":
                response = requests.delete(url, headers=headers, timeout=30)
            else:
                raise ValueError(f"不支持的HTTP方法: {method}")

            end_time = time.time()
            response_time = round((end_time - start_time) * 1000, 2)  # ms

            # 解析响应
            try:
                response_data = response.json()
            except:
                response_data = {"raw_content": response.text}

            # 构建元数据
            metadata = {
                "status_code": response.status_code,
                "response_time_ms": response_time,
                "content_length": len(response.content),
                "headers": dict(response.headers),
            }

            # 保存API报文
            message_file = self.save_api_message(
                endpoint, method, data, response_data, metadata
            )

            return {
                "success": response.status_code < 400,
                "status_code": response.status_code,
                "data": response_data,
                "response_time": response_time,
                "message_file": message_file,
                "error": None,
            }

        except Exception as e:
            end_time = time.time()
            response_time = round((end_time - start_time) * 1000, 2)

            error_data = {"error": str(e), "error_type": type(e).__name__}

            metadata = {
                "status_code": 0,
                "response_time_ms": response_time,
                "error": True,
            }

            message_file = self.save_api_message(
                endpoint, method, data, error_data, metadata
            )

            return {
                "success": False,
                "status_code": 0,
                "data": error_data,
                "response_time": response_time,
                "message_file": message_file,
                "error": str(e),
            }

    def test_health_check(self) -> Dict[str, Any]:
        """测试健康检查API"""
        print("🔍 测试健康检查API...")

        result = self.make_request("/health", "GET")

        if result["success"]:
            data = result["data"]
            expected_fields = ["status", "version", "service"]

            missing_fields = [field for field in expected_fields if field not in data]
            if missing_fields:
                result["success"] = False
                result["error"] = f"响应缺少字段: {missing_fields}"
            else:
                print(
                    f"  ✅ 健康检查通过: {data.get('service', 'unknown')} v{data.get('version', 'unknown')}"
                )

        return result

    def test_model_list(self) -> Dict[str, Any]:
        """测试模型列表API"""
        print("🔍 测试模型列表API...")

        result = self.make_request("/models", "GET")

        if result["success"]:
            data = result["data"]
            if "models" in data and "total" in data:
                models_count = len(data["models"])
                total_count = data["total"]
                print(f"  ✅ 模型列表获取成功: {models_count}/{total_count} 个模型")

                # 验证模型数据结构
                if models_count > 0:
                    model = data["models"][0]
                    required_fields = [
                        "id",
                        "name",
                        "description",
                        "monitoring_count",
                        "control_count",
                    ]
                    missing_fields = [
                        field for field in required_fields if field not in model
                    ]
                    if missing_fields:
                        result["success"] = False
                        result["error"] = f"模型数据缺少字段: {missing_fields}"
            else:
                result["success"] = False
                result["error"] = "响应格式错误：缺少models或total字段"

        return result

    def test_model_detail(self, model_id: str = None) -> Dict[str, Any]:
        """测试模型详情API"""
        print("🔍 测试模型详情API...")

        if not model_id:
            # 先获取模型列表找到第一个模型
            list_result = self.make_request("/models", "GET")
            if not list_result["success"] or not list_result["data"].get("models"):
                return {"success": False, "error": "无法获取模型列表", "data": {}}
            model_id = list_result["data"]["models"][0]["id"]

        result = self.make_request(f"/models/{model_id}", "GET")

        if result["success"]:
            data = result["data"]
            required_fields = [
                "id",
                "name",
                "description",
                "monitoring",
                "control",
            ]
            missing_fields = [field for field in required_fields if field not in data]

            if missing_fields:
                result["success"] = False
                result["error"] = f"模型详情缺少字段: {missing_fields}"
            else:
                monitoring_count = len(data.get("monitoring", {}))
                control_count = len(data.get("control", {}))
                print(
                    f"  ✅ 模型详情获取成功: {data['name']} (监视:{monitoring_count}, 控制:{control_count})"
                )

        return result

    def test_control_command(
        self, model_id: str = None, control_name: str = None, value: float = 1.0
    ) -> Dict[str, Any]:
        """测试控制命令API"""
        print("🔍 测试控制命令API...")

        if not model_id or not control_name:
            # 先获取模型详情找到第一个控制点
            detail_result = self.test_model_detail()
            if not detail_result["success"] or not detail_result["data"].get("control"):
                return {"success": False, "error": "无法找到可用的控制点", "data": {}}

            model_id = detail_result["data"]["id"]
            # control是一个字典，获取第一个键作为控制点名称
            control_name = list(detail_result["data"]["control"].keys())[0]

        command_data = {"value": value}
        result = self.make_request(
            f"/models/{model_id}/control/{control_name}", "POST", command_data
        )

        if result["success"]:
            data = result["data"]
            if "success" in data and data["success"]:
                print(f"  ✅ 控制命令执行成功: {model_id}:{control_name} = {value}")
            else:
                result["success"] = False
                result["error"] = f"控制命令执行失败: {data.get('message', '未知错误')}"

        return result

    def test_api_performance(self, iterations: int = 10) -> Dict[str, Any]:
        """测试API性能"""
        print(f"🔍 测试API性能 ({iterations}次请求)...")

        performance_data = {
            "health_check": [],
            "model_list": [],
            "total_requests": iterations * 2,
            "start_time": datetime.now().isoformat(),
        }

        # 健康检查性能测试
        for i in range(iterations):
            result = self.make_request("/health", "GET")
            performance_data["health_check"].append(
                {
                    "iteration": i + 1,
                    "response_time": result["response_time"],
                    "success": result["success"],
                }
            )

        # 模型列表性能测试
        for i in range(iterations):
            result = self.make_request("/models", "GET")
            performance_data["model_list"].append(
                {
                    "iteration": i + 1,
                    "response_time": result["response_time"],
                    "success": result["success"],
                }
            )

        # 计算统计数据
        health_times = [
            r["response_time"] for r in performance_data["health_check"] if r["success"]
        ]
        model_times = [
            r["response_time"] for r in performance_data["model_list"] if r["success"]
        ]

        performance_data["statistics"] = {
            "health_check": {
                "avg_response_time": round(sum(health_times) / len(health_times), 2)
                if health_times
                else 0,
                "max_response_time": max(health_times) if health_times else 0,
                "min_response_time": min(health_times) if health_times else 0,
                "success_rate": len(health_times) / iterations * 100,
            },
            "model_list": {
                "avg_response_time": round(sum(model_times) / len(model_times), 2)
                if model_times
                else 0,
                "max_response_time": max(model_times) if model_times else 0,
                "min_response_time": min(model_times) if model_times else 0,
                "success_rate": len(model_times) / iterations * 100,
            },
        }

        performance_data["end_time"] = datetime.now().isoformat()

        # 保存性能测试报告
        perf_file = (
            self.api_messages_dir
            / "performance"
            / f"performance_test_{int(time.time())}.json"
        )
        with open(perf_file, "w", encoding="utf-8") as f:
            json.dump(performance_data, f, ensure_ascii=False, indent=2)

        print("  ✅ 性能测试完成:")
        print(
            f"    健康检查平均响应时间: {performance_data['statistics']['health_check']['avg_response_time']}ms"
        )
        print(
            f"    模型列表平均响应时间: {performance_data['statistics']['model_list']['avg_response_time']}ms"
        )

        return {
            "success": True,
            "data": performance_data,
            "message_file": str(perf_file),
        }

    def run_comprehensive_test(self) -> Dict[str, Any]:
        """运行完整的API测试"""
        print("🚀 开始ModSrv API功能完整测试")

        test_results = {
            "start_time": datetime.now().isoformat(),
            "base_url": self.base_url,
            "results_dir": str(self.results_dir),
            "tests": {},
        }

        # 1. 健康检查测试
        test_results["tests"]["health_check"] = self.test_health_check()

        # 2. 模型列表测试
        test_results["tests"]["model_list"] = self.test_model_list()

        # 3. 模型详情测试
        test_results["tests"]["model_detail"] = self.test_model_detail()

        # 4. 控制命令测试
        test_results["tests"]["control_command"] = self.test_control_command()

        # 5. 性能测试
        test_results["tests"]["performance"] = self.test_api_performance()

        test_results["end_time"] = datetime.now().isoformat()

        # 计算总体统计
        total_tests = len(test_results["tests"])
        passed_tests = sum(
            1 for test in test_results["tests"].values() if test.get("success", False)
        )

        test_results["summary"] = {
            "total_tests": total_tests,
            "passed_tests": passed_tests,
            "failed_tests": total_tests - passed_tests,
            "success_rate": round(passed_tests / total_tests * 100, 1)
            if total_tests > 0
            else 0,
        }

        # 保存测试报告
        report_file = self.results_dir / f"api_test_report_{int(time.time())}.json"
        with open(report_file, "w", encoding="utf-8") as f:
            json.dump(test_results, f, ensure_ascii=False, indent=2)

        print("\n📊 测试完成统计:")
        print(f"  总测试数: {total_tests}")
        print(f"  通过测试: {passed_tests}")
        print(f"  失败测试: {total_tests - passed_tests}")
        print(f"  成功率: {test_results['summary']['success_rate']}%")
        print(f"  报告文件: {report_file}")
        print(f"  API报文目录: {self.api_messages_dir}")

        return test_results


def main():
    """主函数"""
    modsrv_url = os.getenv("MODSRV_URL", "http://modsrv:8092")
    results_dir = os.getenv("TEST_OUTPUT", "/app/results")

    print(f"ModSrv URL: {modsrv_url}")
    print(f"结果目录: {results_dir}")

    # 创建测试套件
    test_suite = APITestSuite(modsrv_url, results_dir)

    try:
        # 运行完整测试
        results = test_suite.run_comprehensive_test()

        # 根据测试结果设置退出码
        if results["summary"]["success_rate"] >= 80:  # 80%以上通过率视为成功
            print("\n✅ API测试套件执行成功")
            exit(0)
        else:
            print(
                f"\n❌ API测试套件执行失败 (成功率: {results['summary']['success_rate']}%)"
            )
            exit(1)

    except Exception as e:
        print(f"\n💥 API测试套件执行异常: {e}")
        exit(1)


if __name__ == "__main__":
    main()
