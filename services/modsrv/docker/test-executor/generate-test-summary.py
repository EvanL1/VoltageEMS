#!/usr/bin/env python3
"""生成测试结果摘要报告"""

import json
from pathlib import Path
from datetime import datetime


class TestSummaryGenerator:
    """测试摘要生成器"""

    def __init__(self, results_dir: str = "test-results"):
        self.results_dir = Path(results_dir)
        self.summary_data = {
            "generation_time": datetime.now().isoformat(),
            "results_directory": str(self.results_dir),
            "api_tests": {},
            "template_tests": {},
            "integration_tests": {},
            "performance_data": {},
            "total_statistics": {},
        }

    def collect_api_test_results(self):
        """收集API测试结果"""
        api_messages_dir = self.results_dir / "api-messages"

        if not api_messages_dir.exists():
            return

        api_stats = {
            "health_check": {"count": 0, "avg_response_time": 0},
            "model_list": {"count": 0, "avg_response_time": 0},
            "model_detail": {"count": 0, "avg_response_time": 0},
            "control_commands": {"count": 0, "avg_response_time": 0},
            "performance": {"count": 0, "avg_response_time": 0},
        }

        for category in api_stats.keys():
            category_dir = api_messages_dir / category
            if category_dir.exists():
                message_files = list(category_dir.glob("*.json"))
                api_stats[category]["count"] = len(message_files)

                # 计算平均响应时间
                response_times = []
                for msg_file in message_files:
                    try:
                        with open(msg_file, "r", encoding="utf-8") as f:
                            msg_data = json.load(f)

                        if (
                            "metadata" in msg_data
                            and "response_time_ms" in msg_data["metadata"]
                        ):
                            response_times.append(
                                msg_data["metadata"]["response_time_ms"]
                            )
                    except:
                        continue

                if response_times:
                    api_stats[category]["avg_response_time"] = round(
                        sum(response_times) / len(response_times), 2
                    )

        self.summary_data["api_tests"] = api_stats

    def collect_template_test_results(self):
        """收集模板测试结果"""
        template_tests_dir = self.results_dir / "template-tests"

        if not template_tests_dir.exists():
            return

        template_stats = {
            "templates_discovered": 0,
            "models_generated": 0,
            "categories_tested": [],
            "success_rate": 0,
        }

        # 读取测试结果文件
        results_file = template_tests_dir / "template_test_results.json"
        if results_file.exists():
            try:
                with open(results_file, "r", encoding="utf-8") as f:
                    results_data = json.load(f)

                template_stats["models_generated"] = len(
                    results_data.get("generated_models", [])
                )

                # 统计测试类别
                categories = set()
                for model in results_data.get("generated_models", []):
                    if "category" in model:
                        categories.add(model["category"])

                template_stats["categories_tested"] = list(categories)

                # 计算成功率
                subtests = [
                    "template_discovery",
                    "template_loading",
                    "variable_extraction",
                    "model_building",
                ]
                passed = sum(1 for test in subtests if results_data.get(test, False))
                template_stats["success_rate"] = round(
                    (passed / len(subtests)) * 100, 1
                )

            except Exception as e:
                print(f"读取模板测试结果失败: {e}")

        # 统计生成的模型文件
        model_files = list(template_tests_dir.glob("built_*.yml"))
        template_stats["templates_discovered"] = len(model_files)

        self.summary_data["template_tests"] = template_stats

    def collect_integration_test_results(self):
        """收集集成测试结果"""
        # 查找主要的测试报告文件
        test_report_files = list(self.results_dir.glob("test-report*.json"))
        api_report_files = list(self.results_dir.glob("api_test_report*.json"))

        integration_stats = {
            "total_test_runs": len(test_report_files),
            "api_test_runs": len(api_report_files),
            "last_test_results": {},
            "overall_success_rate": 0,
        }

        # 分析最新的测试报告
        if test_report_files:
            latest_report = max(test_report_files, key=lambda f: f.stat().st_mtime)
            try:
                with open(latest_report, "r", encoding="utf-8") as f:
                    report_data = json.load(f)

                if "test_execution" in report_data:
                    test_exec = report_data["test_execution"]
                    integration_stats["last_test_results"] = {
                        "timestamp": test_exec.get("timestamp", ""),
                        "total_tests": test_exec.get("total_tests", 0),
                        "passed_tests": test_exec.get("passed_tests", 0),
                        "failed_tests": test_exec.get("failed_tests", 0),
                        "success_rate": test_exec.get("success_rate", 0),
                    }
                    integration_stats["overall_success_rate"] = test_exec.get(
                        "success_rate", 0
                    )
            except Exception as e:
                print(f"读取集成测试报告失败: {e}")

        self.summary_data["integration_tests"] = integration_stats

    def collect_performance_data(self):
        """收集性能数据"""
        performance_dir = self.results_dir / "api-messages" / "performance"

        perf_stats = {
            "performance_test_runs": 0,
            "latest_performance": {},
            "response_time_trends": {},
        }

        if performance_dir.exists():
            perf_files = list(performance_dir.glob("performance_test_*.json"))
            perf_stats["performance_test_runs"] = len(perf_files)

            # 分析最新的性能测试
            if perf_files:
                latest_perf = max(perf_files, key=lambda f: f.stat().st_mtime)
                try:
                    with open(latest_perf, "r", encoding="utf-8") as f:
                        perf_data = json.load(f)

                    if "statistics" in perf_data:
                        perf_stats["latest_performance"] = perf_data["statistics"]
                except Exception as e:
                    print(f"读取性能测试数据失败: {e}")

        self.summary_data["performance_data"] = perf_stats

    def calculate_total_statistics(self):
        """计算总体统计"""
        total_stats = {
            "total_api_messages": 0,
            "total_models_generated": 0,
            "total_test_files": 0,
            "avg_api_response_time": 0,
            "overall_health_score": 0,
        }

        # API消息统计
        api_count = 0
        total_response_time = 0
        response_count = 0

        for category, stats in self.summary_data["api_tests"].items():
            api_count += stats.get("count", 0)
            if stats.get("avg_response_time", 0) > 0:
                total_response_time += stats["avg_response_time"]
                response_count += 1

        total_stats["total_api_messages"] = api_count
        if response_count > 0:
            total_stats["avg_api_response_time"] = round(
                total_response_time / response_count, 2
            )

        # 模板生成统计
        total_stats["total_models_generated"] = self.summary_data["template_tests"].get(
            "models_generated", 0
        )

        # 测试文件统计
        if self.results_dir.exists():
            all_files = list(self.results_dir.rglob("*"))
            total_stats["total_test_files"] = len([f for f in all_files if f.is_file()])

        # 整体健康度评分 (0-100)
        health_factors = []

        # API测试健康度
        if api_count > 0:
            health_factors.append(min(100, api_count * 10))  # 每个API消息+10分，最高100

        # 响应时间健康度
        avg_response = total_stats["avg_api_response_time"]
        if avg_response > 0:
            if avg_response < 100:  # <100ms = 优秀
                health_factors.append(100)
            elif avg_response < 500:  # <500ms = 良好
                health_factors.append(80)
            elif avg_response < 1000:  # <1s = 一般
                health_factors.append(60)
            else:  # >1s = 较差
                health_factors.append(40)

        # 模板系统健康度
        template_success = self.summary_data["template_tests"].get("success_rate", 0)
        if template_success > 0:
            health_factors.append(template_success)

        # 集成测试健康度
        integration_success = self.summary_data["integration_tests"].get(
            "overall_success_rate", 0
        )
        if integration_success > 0:
            health_factors.append(integration_success)

        if health_factors:
            total_stats["overall_health_score"] = round(
                sum(health_factors) / len(health_factors), 1
            )

        self.summary_data["total_statistics"] = total_stats

    def generate_summary_report(self, output_file: str = None):
        """生成摘要报告"""
        print("📊 收集测试结果数据...")

        self.collect_api_test_results()
        self.collect_template_test_results()
        self.collect_integration_test_results()
        self.collect_performance_data()
        self.calculate_total_statistics()

        if not output_file:
            output_file = (
                self.results_dir
                / f"test_summary_{int(datetime.now().timestamp())}.json"
            )
        else:
            output_file = Path(output_file)

        # 确保输出目录存在
        output_file.parent.mkdir(parents=True, exist_ok=True)

        # 保存JSON格式
        with open(output_file, "w", encoding="utf-8") as f:
            json.dump(self.summary_data, f, ensure_ascii=False, indent=2)

        # 生成可读格式的摘要
        readable_file = output_file.with_suffix(".md")
        self.generate_readable_summary(readable_file)

        print("✅ 测试摘要报告已生成:")
        print(f"  📄 JSON格式: {output_file}")
        print(f"  📖 可读格式: {readable_file}")

        return output_file

    def generate_readable_summary(self, output_file: Path):
        """生成可读的摘要报告"""
        content = f"""# ModSrv 测试摘要报告

生成时间: {self.summary_data["generation_time"]}
结果目录: {self.summary_data["results_directory"]}

## 📊 总体统计

- **API消息总数**: {self.summary_data["total_statistics"].get("total_api_messages", 0)}
- **生成模型数**: {self.summary_data["total_statistics"].get("total_models_generated", 0)}
- **测试文件数**: {self.summary_data["total_statistics"].get("total_test_files", 0)}
- **平均响应时间**: {self.summary_data["total_statistics"].get("avg_api_response_time", 0)}ms
- **整体健康度**: {self.summary_data["total_statistics"].get("overall_health_score", 0)}/100

## 🔌 API测试结果

"""

        for category, stats in self.summary_data["api_tests"].items():
            content += f"### {category}\n"
            content += f"- 消息数: {stats.get('count', 0)}\n"
            content += f"- 平均响应时间: {stats.get('avg_response_time', 0)}ms\n\n"

        template_stats = self.summary_data["template_tests"]
        content += f"""## 🔧 模板系统测试

- **发现模板数**: {template_stats.get("templates_discovered", 0)}
- **生成模型数**: {template_stats.get("models_generated", 0)}
- **测试类别**: {", ".join(template_stats.get("categories_tested", []))}
- **成功率**: {template_stats.get("success_rate", 0)}%

"""

        integration_stats = self.summary_data["integration_tests"]
        last_test = integration_stats.get("last_test_results", {})
        content += f"""## 🧪 集成测试结果

- **测试运行次数**: {integration_stats.get("total_test_runs", 0)}
- **API测试运行次数**: {integration_stats.get("api_test_runs", 0)}
- **整体成功率**: {integration_stats.get("overall_success_rate", 0)}%

### 最近测试结果
- 测试时间: {last_test.get("timestamp", "N/A")}
- 总测试数: {last_test.get("total_tests", 0)}
- 通过测试: {last_test.get("passed_tests", 0)}
- 失败测试: {last_test.get("failed_tests", 0)}
- 成功率: {last_test.get("success_rate", 0)}%

"""

        perf_stats = self.summary_data["performance_data"]
        latest_perf = perf_stats.get("latest_performance", {})
        content += f"""## ⚡ 性能测试数据

- **性能测试运行次数**: {perf_stats.get("performance_test_runs", 0)}

"""

        if latest_perf:
            content += "### 最新性能数据\n"
            for test_type, metrics in latest_perf.items():
                if isinstance(metrics, dict):
                    content += f"#### {test_type}\n"
                    content += (
                        f"- 平均响应时间: {metrics.get('avg_response_time', 0)}ms\n"
                    )
                    content += (
                        f"- 最大响应时间: {metrics.get('max_response_time', 0)}ms\n"
                    )
                    content += (
                        f"- 最小响应时间: {metrics.get('min_response_time', 0)}ms\n"
                    )
                    content += f"- 成功率: {metrics.get('success_rate', 0)}%\n\n"

        content += """
---
*报告由ModSrv测试系统自动生成*
"""

        with open(output_file, "w", encoding="utf-8") as f:
            f.write(content)


def main():
    """主函数"""
    import argparse

    parser = argparse.ArgumentParser(description="生成ModSrv测试摘要报告")
    parser.add_argument(
        "--results-dir", default="test-results", help="测试结果目录路径"
    )
    parser.add_argument("--output", "-o", help="输出文件路径")

    args = parser.parse_args()

    try:
        generator = TestSummaryGenerator(args.results_dir)
        output_file = generator.generate_summary_report(args.output)

        print(f"\n🎉 测试摘要生成完成: {output_file}")

    except Exception as e:
        print(f"❌ 生成测试摘要失败: {e}")
        return 1

    return 0


if __name__ == "__main__":
    exit(main())
