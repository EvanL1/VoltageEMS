#!/usr/bin/env python3
"""模板系统功能测试"""

import sys
import json
import yaml
from pathlib import Path


def test_template_system():
    """测试模板系统功能"""
    print("🔧 开始模板系统测试...")

    templates_dir = Path("/app/templates")
    results_dir = Path("/app/results/template-tests")
    results_dir.mkdir(parents=True, exist_ok=True)

    test_results = {
        "template_discovery": False,
        "template_loading": False,
        "variable_extraction": False,
        "model_building": False,
        "generated_models": [],
    }

    try:
        # 1. 测试模板发现
        print("  🔍 测试模板发现...")
        if not templates_dir.exists():
            raise Exception(f"模板目录不存在: {templates_dir}")

        template_files = list(templates_dir.glob("**/*.yml"))
        if not template_files:
            raise Exception("未找到任何模板文件")

        print(f"    ✅ 发现 {len(template_files)} 个模板文件")
        test_results["template_discovery"] = True

        # 2. 测试模板加载
        print("  📄 测试模板加载...")
        test_template = template_files[0]  # 使用第一个模板进行测试

        with open(test_template, "r", encoding="utf-8") as f:
            template_content = yaml.safe_load(f)

        if not isinstance(template_content, dict):
            raise Exception("模板内容格式错误")

        required_fields = ["id", "name", "description", "enabled"]
        missing_fields = [
            field for field in required_fields if field not in template_content
        ]
        if missing_fields:
            raise Exception(f"模板缺少必需字段: {missing_fields}")

        print(f"    ✅ 模板加载成功: {template_content.get('name', '未命名')}")
        test_results["template_loading"] = True

        # 3. 测试变量提取
        print("  🔧 测试变量提取...")
        template_str = yaml.dump(template_content)
        import re

        variables = set(re.findall(r"\\$\\{([^}]+)\\}", template_str))

        if not variables:
            print("    ⚠️  模板中未发现变量，跳过变量测试")
        else:
            print(
                f"    ✅ 提取到 {len(variables)} 个变量: {', '.join(sorted(variables))}"
            )

        test_results["variable_extraction"] = True

        # 4. 测试模型构建
        print("  🏗️  测试模型构建...")

        # 为测试准备变量值
        test_variables = {}
        for var in variables:
            if "id" in var.lower():
                test_variables[var] = "TEST001"
            elif "name" in var.lower():
                test_variables[var] = "测试设备"
            elif "location" in var.lower():
                test_variables[var] = "测试位置"
            elif "channel" in var.lower():
                test_variables[var] = 9999
            elif "point_id" in var.lower():
                test_variables[var] = 90000
            else:
                test_variables[var] = "test_value"

        # 构建模型
        built_model = substitute_template_variables(template_content, test_variables)

        # 验证构建结果
        if not isinstance(built_model, dict):
            raise Exception("模型构建结果格式错误")

        # 检查变量是否被正确替换
        built_str = yaml.dump(built_model)
        remaining_vars = re.findall(r"\\$\\{([^}]+)\\}", built_str)
        if remaining_vars:
            raise Exception(f"变量替换不完整，剩余变量: {remaining_vars}")

        # 保存构建的模型
        output_file = results_dir / f"built_model_{test_template.stem}.yml"
        with open(output_file, "w", encoding="utf-8") as f:
            yaml.dump(built_model, f, allow_unicode=True, default_flow_style=False)

        test_results["generated_models"].append(
            {
                "template": str(test_template),
                "output": str(output_file),
                "variables_used": test_variables,
                "model_name": built_model.get("name", "未命名"),
            }
        )

        print(f"    ✅ 模型构建成功: {built_model.get('name', '未命名')}")
        print(f"    💾 已保存到: {output_file}")
        test_results["model_building"] = True

        # 5. 测试多个模板类型
        print("  🔄 测试多个模板类型...")
        categories_tested = set()

        for template_file in template_files[:3]:  # 测试前3个模板
            category = template_file.parent.name
            if category in categories_tested:
                continue

            try:
                with open(template_file, "r", encoding="utf-8") as f:
                    template_data = yaml.safe_load(f)

                template_str = yaml.dump(template_data)
                template_vars = set(re.findall(r"\\$\\{([^}]+)\\}", template_str))

                # 为每个模板准备专用变量
                category_variables = prepare_category_variables(category, template_vars)

                if template_vars:
                    built_model = substitute_template_variables(
                        template_data, category_variables
                    )

                    output_file = (
                        results_dir / f"built_{category}_{template_file.stem}.yml"
                    )
                    with open(output_file, "w", encoding="utf-8") as f:
                        yaml.dump(
                            built_model, f, allow_unicode=True, default_flow_style=False
                        )

                    test_results["generated_models"].append(
                        {
                            "template": str(template_file),
                            "output": str(output_file),
                            "category": category,
                            "variables_used": category_variables,
                            "model_name": built_model.get("name", "未命名"),
                        }
                    )

                    categories_tested.add(category)
                    print(f"    ✅ {category} 类型模板测试成功")

            except Exception as e:
                print(f"    ⚠️  {category} 类型模板测试失败: {e}")

        # 保存测试结果
        results_file = results_dir / "template_test_results.json"
        with open(results_file, "w", encoding="utf-8") as f:
            json.dump(test_results, f, ensure_ascii=False, indent=2)

        # 计算成功率
        total_subtests = 4  # 模板发现、加载、变量提取、模型构建
        passed_subtests = sum(
            [
                test_results["template_discovery"],
                test_results["template_loading"],
                test_results["variable_extraction"],
                test_results["model_building"],
            ]
        )

        success_rate = (passed_subtests / total_subtests) * 100

        print("\\n📊 模板系统测试完成:")
        print(f"  子测试通过: {passed_subtests}/{total_subtests}")
        print(f"  成功率: {success_rate:.1f}%")
        print(f"  生成模型数: {len(test_results['generated_models'])}")
        print(f"  结果文件: {results_file}")

        if success_rate >= 75:
            print("✅ 模板系统测试通过")
            return True
        else:
            print("❌ 模板系统测试失败")
            return False

    except Exception as e:
        print(f"❌ 模板系统测试异常: {e}")

        # 保存错误信息
        error_file = results_dir / "template_test_error.json"
        with open(error_file, "w", encoding="utf-8") as f:
            json.dump(
                {
                    "error": str(e),
                    "error_type": type(e).__name__,
                    "test_results": test_results,
                },
                f,
                ensure_ascii=False,
                indent=2,
            )

        return False


def substitute_template_variables(template_data, variables):
    """替换模板中的变量"""
    template_str = yaml.dump(template_data)

    # 替换变量
    for var_name, var_value in variables.items():
        # 支持数学表达式 (简单的加法)
        if isinstance(var_value, (int, float)):
            # 处理类似 ${base_point_id + 1} 的表达式
            import re

            pattern = f"\\$\\{{{var_name}\\s*\\+\\s*(\\d+)\\}}"
            template_str = re.sub(
                pattern, lambda m: str(var_value + int(m.group(1))), template_str
            )

            # 处理简单变量 ${var_name}
            template_str = template_str.replace(f"${{{var_name}}}", str(var_value))
        else:
            template_str = template_str.replace(f"${{{var_name}}}", str(var_value))

    return yaml.safe_load(template_str)


def prepare_category_variables(category, variables):
    """为不同类别的模板准备变量"""
    base_vars = {}

    for var in variables:
        if "transformer" in var:
            base_vars[var] = "T001" if "id" in var else "测试变压器"
        elif "generator" in var:
            base_vars[var] = "G001" if "id" in var else "测试发电机"
        elif "ups" in var:
            base_vars[var] = "UPS001" if "id" in var else "测试UPS"
        elif "motor" in var:
            base_vars[var] = "M001" if "id" in var else "测试电机"
        elif "servo" in var:
            base_vars[var] = "SV001" if "id" in var else "测试伺服"
        elif "sensor" in var:
            base_vars[var] = "SE001" if "id" in var else "测试传感器"
        elif "location" in var:
            base_vars[var] = f"测试{category}位置"
        elif "channel_id" in var:
            base_vars[var] = 9000 + hash(category) % 1000
        elif "base_point_id" in var:
            base_vars[var] = 90000 + (hash(category) % 10) * 1000
        else:
            base_vars[var] = f"test_{var}"

    return base_vars


if __name__ == "__main__":
    try:
        success = test_template_system()
        if success:
            print("模板系统测试: PASS")
            sys.exit(0)
        else:
            print("模板系统测试: FAIL")
            sys.exit(1)
    except Exception as e:
        print(f"模板系统测试: FAIL - {e}")
        sys.exit(1)
