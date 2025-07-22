//! 模型计算引擎
//!
//! 提供设备模型的计算执行和数据流处理

use super::*;
use crate::error::{ModelSrvError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};

/// 计算上下文
#[derive(Debug, Clone)]
pub struct CalculationContext {
    /// 实例ID
    pub instance_id: String,
    /// 输入变量
    pub inputs: HashMap<String, serde_json::Value>,
    /// 输出变量
    pub outputs: HashMap<String, serde_json::Value>,
    /// 实例属性
    pub properties: HashMap<String, serde_json::Value>,
    /// 临时变量
    pub variables: HashMap<String, serde_json::Value>,
}

/// 计算执行器trait
#[async_trait]
pub trait CalculationExecutor: Send + Sync {
    /// 执行计算
    async fn execute(
        &self,
        expression: &CalculationExpression,
        context: &mut CalculationContext,
    ) -> Result<()>;
}

/// 内置计算执行器
pub struct BuiltInExecutor;

#[async_trait]
impl CalculationExecutor for BuiltInExecutor {
    async fn execute(
        &self,
        expression: &CalculationExpression,
        context: &mut CalculationContext,
    ) -> Result<()> {
        match expression {
            CalculationExpression::Math(expr) => self.execute_math(expr, context).await,
            CalculationExpression::BuiltIn { function, args } => {
                self.execute_builtin(function, args, context).await
            }
            _ => Err(ModelSrvError::NotSupported(
                "Unsupported calculation type".to_string(),
            )),
        }
    }
}

impl BuiltInExecutor {
    /// 执行数学表达式
    async fn execute_math(&self, expr: &str, context: &mut CalculationContext) -> Result<()> {
        // 使用简单的表达式解析器
        let result = self.evaluate_expression(expr, context)?;

        // 假设表达式格式为 "output = expression"
        if let Some((output_var, _)) = expr.split_once('=') {
            let output_var = output_var.trim();
            context.outputs.insert(output_var.to_string(), result);
        }

        Ok(())
    }

    /// 执行内置函数
    async fn execute_builtin(
        &self,
        function: &str,
        args: &[String],
        context: &mut CalculationContext,
    ) -> Result<()> {
        match function {
            "sum" => self.builtin_sum(args, context),
            "avg" => self.builtin_avg(args, context),
            "min" => self.builtin_min(args, context),
            "max" => self.builtin_max(args, context),
            "scale" => self.builtin_scale(args, context),
            _ => Err(ModelSrvError::ValidationError(format!(
                "Unknown built-in function: {}",
                function
            ))),
        }
    }

    /// 求和函数
    fn builtin_sum(&self, args: &[String], context: &mut CalculationContext) -> Result<()> {
        let mut sum = 0.0;

        for arg in args.iter().skip(1) {
            if let Some(value) = self.get_variable_value(arg, context) {
                if let Some(num) = value.as_f64() {
                    sum += num;
                }
            }
        }

        if let Some(output_var) = args.first() {
            context
                .outputs
                .insert(output_var.to_string(), serde_json::json!(sum));
        }

        Ok(())
    }

    /// 平均值函数
    fn builtin_avg(&self, args: &[String], context: &mut CalculationContext) -> Result<()> {
        let mut sum = 0.0;
        let mut count = 0;

        for arg in args.iter().skip(1) {
            if let Some(value) = self.get_variable_value(arg, context) {
                if let Some(num) = value.as_f64() {
                    sum += num;
                    count += 1;
                }
            }
        }

        let avg = if count > 0 { sum / count as f64 } else { 0.0 };

        if let Some(output_var) = args.first() {
            context
                .outputs
                .insert(output_var.to_string(), serde_json::json!(avg));
        }

        Ok(())
    }

    /// 最小值函数
    fn builtin_min(&self, args: &[String], context: &mut CalculationContext) -> Result<()> {
        let mut min_value = f64::MAX;
        let mut found = false;

        for arg in args.iter().skip(1) {
            if let Some(value) = self.get_variable_value(arg, context) {
                if let Some(num) = value.as_f64() {
                    min_value = min_value.min(num);
                    found = true;
                }
            }
        }

        if let Some(output_var) = args.first() {
            let result = if found { min_value } else { 0.0 };
            context
                .outputs
                .insert(output_var.to_string(), serde_json::json!(result));
        }

        Ok(())
    }

    /// 最大值函数
    fn builtin_max(&self, args: &[String], context: &mut CalculationContext) -> Result<()> {
        let mut max_value = f64::MIN;
        let mut found = false;

        for arg in args.iter().skip(1) {
            if let Some(value) = self.get_variable_value(arg, context) {
                if let Some(num) = value.as_f64() {
                    max_value = max_value.max(num);
                    found = true;
                }
            }
        }

        if let Some(output_var) = args.first() {
            let result = if found { max_value } else { 0.0 };
            context
                .outputs
                .insert(output_var.to_string(), serde_json::json!(result));
        }

        Ok(())
    }

    /// 缩放函数
    fn builtin_scale(&self, args: &[String], context: &mut CalculationContext) -> Result<()> {
        if args.len() != 4 {
            return Err(ModelSrvError::ValidationError(
                "Scale function requires 4 arguments: output, input, scale, offset".to_string(),
            ));
        }

        let output_var = &args[0];
        let input_var = &args[1];
        let scale = args[2].parse::<f64>().unwrap_or(1.0);
        let offset = args[3].parse::<f64>().unwrap_or(0.0);

        if let Some(value) = self.get_variable_value(input_var, context) {
            if let Some(num) = value.as_f64() {
                let result = num * scale + offset;
                context
                    .outputs
                    .insert(output_var.to_string(), serde_json::json!(result));
            }
        }

        Ok(())
    }

    /// 获取变量值
    fn get_variable_value<'a>(
        &self,
        var_name: &str,
        context: &'a CalculationContext,
    ) -> Option<&'a serde_json::Value> {
        context
            .inputs
            .get(var_name)
            .or_else(|| context.outputs.get(var_name))
            .or_else(|| context.properties.get(var_name))
            .or_else(|| context.variables.get(var_name))
    }

    /// 简单的表达式计算
    fn evaluate_expression(
        &self,
        expr: &str,
        context: &CalculationContext,
    ) -> Result<serde_json::Value> {
        // TODO: 实现完整的表达式解析器
        // 这里只是一个简单的示例

        // 尝试解析为数字
        if let Ok(num) = expr.parse::<f64>() {
            return Ok(serde_json::json!(num));
        }

        // 尝试获取变量值
        if let Some(value) = self.get_variable_value(expr, context) {
            return Ok(value.clone());
        }

        Err(ModelSrvError::ValidationError(format!(
            "Cannot evaluate expression: {}",
            expr
        )))
    }
}

/// 计算引擎
pub struct CalculationEngine {
    /// 执行器映射
    executors: HashMap<String, Arc<dyn CalculationExecutor>>,
    /// 默认执行器
    default_executor: Arc<dyn CalculationExecutor>,
}

impl CalculationEngine {
    /// 创建计算引擎
    pub fn new() -> Self {
        let mut executors = HashMap::new();
        let builtin = Arc::new(BuiltInExecutor) as Arc<dyn CalculationExecutor>;

        executors.insert("builtin".to_string(), builtin.clone());
        executors.insert("math".to_string(), builtin.clone());

        Self {
            executors,
            default_executor: builtin,
        }
    }

    /// 注册执行器
    pub fn register_executor(&mut self, name: String, executor: Arc<dyn CalculationExecutor>) {
        self.executors.insert(name, executor);
    }

    /// 执行模型计算
    pub async fn execute_model_calculations(
        &self,
        model: &DeviceModel,
        instance: &DeviceInstance,
        telemetry_data: &HashMap<String, TelemetryValue>,
    ) -> Result<HashMap<String, CalculationResult>> {
        let mut results = HashMap::new();

        for calc_def in &model.calculations {
            // 检查执行条件
            if let Some(condition) = &calc_def.condition {
                // TODO: 评估条件表达式
                debug!("Checking condition: {}", condition);
            }

            // 准备计算上下文
            let mut context = self.prepare_context(
                &instance.instance_id,
                &instance.properties,
                &calc_def.inputs,
                telemetry_data,
            )?;

            // 选择执行器
            let executor = self.get_executor(&calc_def.expression);

            // 执行计算
            let start = std::time::Instant::now();
            executor.execute(&calc_def.expression, &mut context).await?;
            let duration_ms = start.elapsed().as_millis() as u64;

            // 收集结果
            let result = CalculationResult {
                calculation_id: calc_def.identifier.clone(),
                outputs: context.outputs.clone(),
                timestamp: chrono::Utc::now().timestamp_millis(),
                duration_ms,
            };

            results.insert(calc_def.identifier.clone(), result);

            info!(
                "Executed calculation '{}' for instance '{}' in {}ms",
                calc_def.identifier, instance.instance_id, duration_ms
            );
        }

        Ok(results)
    }

    /// 准备计算上下文
    fn prepare_context(
        &self,
        instance_id: &str,
        properties: &HashMap<String, serde_json::Value>,
        input_vars: &[String],
        telemetry_data: &HashMap<String, TelemetryValue>,
    ) -> Result<CalculationContext> {
        let mut inputs = HashMap::new();

        // 收集输入变量
        for var_name in input_vars {
            if let Some(telemetry) = telemetry_data.get(var_name) {
                inputs.insert(var_name.clone(), telemetry.value.clone());
            } else if let Some(prop_value) = properties.get(var_name) {
                inputs.insert(var_name.clone(), prop_value.clone());
            }
        }

        Ok(CalculationContext {
            instance_id: instance_id.to_string(),
            inputs,
            outputs: HashMap::new(),
            properties: properties.clone(),
            variables: HashMap::new(),
        })
    }

    /// 获取执行器
    fn get_executor(&self, expression: &CalculationExpression) -> Arc<dyn CalculationExecutor> {
        match expression {
            CalculationExpression::Math(_) => self
                .executors
                .get("math")
                .cloned()
                .unwrap_or_else(|| self.default_executor.clone()),
            CalculationExpression::JavaScript(_) => self
                .executors
                .get("javascript")
                .cloned()
                .unwrap_or_else(|| self.default_executor.clone()),
            CalculationExpression::Python(_) => self
                .executors
                .get("python")
                .cloned()
                .unwrap_or_else(|| self.default_executor.clone()),
            CalculationExpression::BuiltIn { .. } => self
                .executors
                .get("builtin")
                .cloned()
                .unwrap_or_else(|| self.default_executor.clone()),
        }
    }
}

impl Default for CalculationEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_builtin_functions() {
        let executor = BuiltInExecutor;
        let mut context = CalculationContext {
            instance_id: "test".to_string(),
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            properties: HashMap::new(),
            variables: HashMap::new(),
        };

        // 测试数据
        context
            .inputs
            .insert("a".to_string(), serde_json::json!(10.0));
        context
            .inputs
            .insert("b".to_string(), serde_json::json!(20.0));
        context
            .inputs
            .insert("c".to_string(), serde_json::json!(30.0));

        // 测试求和
        let sum_expr = CalculationExpression::BuiltIn {
            function: "sum".to_string(),
            args: vec![
                "result".to_string(),
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
            ],
        };

        executor.execute(&sum_expr, &mut context).await.unwrap();
        assert_eq!(
            context.outputs.get("result"),
            Some(&serde_json::json!(60.0))
        );

        // 测试平均值
        let avg_expr = CalculationExpression::BuiltIn {
            function: "avg".to_string(),
            args: vec![
                "average".to_string(),
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
            ],
        };

        executor.execute(&avg_expr, &mut context).await.unwrap();
        assert_eq!(
            context.outputs.get("average"),
            Some(&serde_json::json!(20.0))
        );
    }
}
