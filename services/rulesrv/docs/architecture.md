# rulesrv 架构设计

## 概述

rulesrv 采用基于 DAG（有向无环图）的规则引擎架构，专注于从 modsrv 读取计算结果并执行控制逻辑。服务设计强调规则的可组合性、执行效率和实时响应能力。

## 核心架构

```
┌─────────────────────────────────────────────────────────┐
│                      rulesrv                            │
├─────────────────────────────────────────────────────────┤
│                   API Server                            │
│              (Rules/Execution/Stats)                    │
├─────────────────────────────────────────────────────────┤
│                 Rule Manager                            │
│     ┌──────────────┬──────────────┬──────────────┐    │
│     │ Rule Store   │ Rule Parser  │ Rule Cache   │    │
│     │ (Redis)      │ (DAG)        │ (Memory)     │    │
│     └──────────────┴──────────────┴──────────────┘    │
├─────────────────────────────────────────────────────────┤
│                  DAG Engine                             │
│     ┌──────────┬──────────┬──────────┬──────────┐    │
│     │Scheduler │Executor  │Evaluator │Context   │    │
│     └──────────┴──────────┴──────────┴──────────┘    │
├─────────────────────────────────────────────────────────┤
│                 Data Interface                          │
│     ┌──────────────┬──────────────┬──────────────┐    │
│     │ Data Reader  │ Subscription │ Cache        │    │
│     │ (modsrv only)│ Manager      │ Layer        │    │
│     └──────────────┴──────────────┴──────────────┘    │
├─────────────────────────────────────────────────────────┤
│                Action Handlers                          │
│     ┌──────────┬──────────┬──────────┬──────────┐    │
│     │Control   │Alarm     │Notify    │Custom    │    │
│     └──────────┴──────────┴──────────┴──────────┘    │
└─────────────────────────────────────────────────────────┘
```

## 组件说明

### 1. Rule Manager

负责规则的生命周期管理：

```rust
pub struct RuleManager {
    store: Arc<RuleStore>,
    parser: Arc<RuleParser>,
    cache: Arc<RuleCache>,
}

pub struct Rule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub graph: DAG,
    pub metadata: RuleMetadata,
}

impl RuleManager {
    pub async fn create_rule(&self, rule_def: RuleDefinition) -> Result<Rule> {
        // 解析规则定义
        let dag = self.parser.parse(&rule_def)?;
        
        // 验证 DAG 合法性
        self.validate_dag(&dag)?;
        
        // 创建规则对象
        let rule = Rule {
            id: Uuid::new_v4().to_string(),
            name: rule_def.name,
            description: rule_def.description,
            enabled: true,
            graph: dag,
            metadata: RuleMetadata::new(),
        };
        
        // 存储到 Redis
        self.store.save(&rule).await?;
        
        // 更新缓存
        self.cache.insert(&rule.id, rule.clone()).await;
        
        Ok(rule)
    }
}
```

### 2. DAG Engine

DAG 执行引擎是规则系统的核心：

```rust
pub struct DAGEngine {
    scheduler: Arc<Scheduler>,
    executor: Arc<Executor>,
    evaluator: Arc<Evaluator>,
}

pub struct DAG {
    nodes: HashMap<String, Node>,
    edges: Vec<Edge>,
    topology: Vec<String>, // 拓扑排序结果
}

pub enum Node {
    Input(InputNode),
    Transform(TransformNode),
    Condition(ConditionNode),
    Action(ActionNode),
}

impl DAGEngine {
    pub async fn execute(&self, rule: &Rule, context: ExecutionContext) -> Result<ExecutionResult> {
        let mut node_outputs = HashMap::new();
        
        // 按拓扑顺序执行节点
        for node_id in &rule.graph.topology {
            let node = &rule.graph.nodes[node_id];
            
            // 收集输入
            let inputs = self.collect_inputs(&rule.graph, node_id, &node_outputs)?;
            
            // 执行节点
            let output = match node {
                Node::Input(n) => self.execute_input(n, &context).await?,
                Node::Transform(n) => self.execute_transform(n, inputs).await?,
                Node::Condition(n) => self.execute_condition(n, inputs).await?,
                Node::Action(n) => self.execute_action(n, inputs).await?,
            };
            
            node_outputs.insert(node_id.clone(), output);
        }
        
        Ok(ExecutionResult {
            rule_id: rule.id.clone(),
            outputs: node_outputs,
            timestamp: Utc::now(),
        })
    }
}
```

### 3. Data Interface

数据接口专门从 modsrv 读取数据：

```rust
pub struct DataReader {
    redis_client: Arc<RedisClient>,
    cache: Arc<DataCache>,
}

impl DataReader {
    /// 只从 modsrv 读取数据
    pub async fn read_value(
        &self,
        model_name: &str,
        data_type: &str,
        field: &str,
    ) -> Result<StandardFloat> {
        let hash_key = format!("modsrv:{}:{}", model_name, data_type);
        
        // 检查缓存
        if let Some(value) = self.cache.get(&hash_key, field).await {
            return Ok(value);
        }
        
        // 从 Redis 读取
        let value_str: Option<String> = self.redis_client
            .hget(&hash_key, field)
            .await?;
        
        match value_str {
            Some(s) => {
                let value = s.parse::<f64>()?;
                let std_value = StandardFloat::new(value);
                
                // 更新缓存
                self.cache.put(&hash_key, field, std_value).await;
                
                Ok(std_value)
            }
            None => Err(Error::DataNotFound(format!("{}:{}", hash_key, field))),
        }
    }
    
    /// 批量读取
    pub async fn batch_read(
        &self,
        requests: Vec<DataRequest>,
    ) -> Result<HashMap<String, StandardFloat>> {
        // 按 Hash 键分组
        let mut grouped: HashMap<String, Vec<String>> = HashMap::new();
        
        for req in requests {
            let hash_key = format!("modsrv:{}:{}", req.model, req.data_type);
            grouped.entry(hash_key)
                .or_insert_with(Vec::new)
                .push(req.field);
        }
        
        // 批量读取每个 Hash
        let mut results = HashMap::new();
        
        for (hash_key, fields) in grouped {
            let values: Vec<Option<String>> = self.redis_client
                .hmget(&hash_key, &fields)
                .await?;
            
            for (field, value) in fields.iter().zip(values.iter()) {
                if let Some(v) = value {
                    if let Ok(parsed) = v.parse::<f64>() {
                        let key = format!("{}:{}", hash_key, field);
                        results.insert(key, StandardFloat::new(parsed));
                    }
                }
            }
        }
        
        Ok(results)
    }
}
```

### 4. Node Executors

不同类型节点的执行器：

#### Input Node
```rust
pub struct InputNode {
    pub source: String,  // modsrv:model:type
    pub field: String,
    pub default: Option<f64>,
}

async fn execute_input(
    &self,
    node: &InputNode,
    context: &ExecutionContext,
) -> Result<NodeOutput> {
    // 解析数据源
    let parts: Vec<&str> = node.source.split(':').collect();
    if parts.len() != 3 || parts[0] != "modsrv" {
        return Err(Error::InvalidSource("Must read from modsrv"));
    }
    
    let model = parts[1];
    let data_type = parts[2];
    
    // 读取数据
    match self.data_reader.read_value(model, data_type, &node.field).await {
        Ok(value) => Ok(NodeOutput::Value(value)),
        Err(_) => {
            // 使用默认值
            match node.default {
                Some(d) => Ok(NodeOutput::Value(StandardFloat::new(d))),
                None => Err(Error::DataNotFound(node.source.clone())),
            }
        }
    }
}
```

#### Condition Node
```rust
pub struct ConditionNode {
    pub condition_type: ConditionType,
    pub params: serde_json::Value,
}

pub enum ConditionType {
    Threshold {
        operator: ComparisonOperator,
        value: f64,
        duration: Option<Duration>,
    },
    Range {
        min: f64,
        max: f64,
        inside: bool,
    },
    Expression {
        expr: String,
    },
}

async fn execute_condition(
    &self,
    node: &ConditionNode,
    inputs: HashMap<String, NodeOutput>,
) -> Result<NodeOutput> {
    let result = match &node.condition_type {
        ConditionType::Threshold { operator, value, duration } => {
            let input_value = self.get_numeric_input(&inputs, "input")?;
            let meets_condition = self.evaluate_comparison(input_value, *operator, *value);
            
            // 检查持续时间
            if let Some(d) = duration {
                self.check_duration(node, meets_condition, *d).await?
            } else {
                meets_condition
            }
        }
        ConditionType::Expression { expr } => {
            self.evaluate_expression(expr, &inputs)?
        }
        _ => false,
    };
    
    Ok(NodeOutput::Boolean(result))
}
```

#### Action Node
```rust
pub struct ActionNode {
    pub action_type: ActionType,
    pub config: serde_json::Value,
}

pub enum ActionType {
    Control,
    Alarm,
    Notification,
    Custom(String),
}

async fn execute_action(
    &self,
    node: &ActionNode,
    inputs: HashMap<String, NodeOutput>,
) -> Result<NodeOutput> {
    let handler = self.get_action_handler(&node.action_type)?;
    
    // 准备动作参数
    let params = self.prepare_action_params(&node.config, &inputs)?;
    
    // 执行动作
    let result = handler.execute(params).await?;
    
    Ok(NodeOutput::String(result))
}
```

### 5. Action Handlers

动作处理器实现具体的控制逻辑：

```rust
#[async_trait]
pub trait ActionHandler: Send + Sync {
    async fn execute(&self, params: ActionParams) -> Result<String>;
}

pub struct ControlActionHandler {
    redis_client: Arc<RedisClient>,
}

#[async_trait]
impl ActionHandler for ControlActionHandler {
    async fn execute(&self, params: ActionParams) -> Result<String> {
        let channel = params.get_string("channel")?;
        let command = params.get_object("command")?;
        
        // 构建控制命令
        let control_command = ControlCommand {
            point_id: command["point_id"].as_u64().unwrap() as u32,
            value: command["value"].as_f64().unwrap(),
            timestamp: Utc::now().timestamp_millis(),
            source: "rulesrv".to_string(),
        };
        
        // 发布到控制通道
        self.redis_client
            .publish(&channel, serde_json::to_string(&control_command)?)
            .await?;
        
        Ok(format!("Control command sent to {}", channel))
    }
}
```

## 数据流

### 规则触发流程

1. **数据变化监听**
   ```
   modsrv:power_meter:measurement → HSET total_power "1200.500000"
   ```

2. **触发规则评估**
   - 查找监听该数据源的规则
   - 将规则加入执行队列

3. **DAG 执行**
   - 按拓扑顺序执行节点
   - 传递节点间的数据
   - 评估条件分支

4. **动作执行**
   - 根据条件结果执行相应动作
   - 记录执行结果

## 性能优化

### 1. 规则编译缓存

```rust
pub struct CompiledRule {
    pub id: String,
    pub dag: Arc<DAG>,
    pub expression_cache: HashMap<String, CompiledExpression>,
}

pub struct RuleCache {
    compiled: Arc<RwLock<HashMap<String, CompiledRule>>>,
}
```

### 2. 数据预取

```rust
pub async fn prefetch_rule_data(&self, rule: &Rule) -> Result<()> {
    // 分析规则依赖的数据
    let data_deps = self.analyze_data_dependencies(rule)?;
    
    // 批量预取
    let requests: Vec<DataRequest> = data_deps.into_iter()
        .map(|dep| DataRequest {
            model: dep.model,
            data_type: dep.data_type,
            field: dep.field,
        })
        .collect();
    
    self.data_reader.batch_read(requests).await?;
    Ok(())
}
```

### 3. 并行执行

```rust
pub struct ParallelExecutor {
    thread_pool: Arc<ThreadPool>,
    max_parallel: usize,
}

impl ParallelExecutor {
    pub async fn execute_rules(&self, rules: Vec<Rule>) -> Vec<Result<ExecutionResult>> {
        // 分析规则依赖关系
        let groups = self.group_independent_rules(&rules);
        
        let mut all_results = Vec::new();
        
        // 并行执行每组独立规则
        for group in groups {
            let results = futures::future::join_all(
                group.into_iter().map(|rule| {
                    self.execute_single(rule)
                })
            ).await;
            
            all_results.extend(results);
        }
        
        all_results
    }
}
```

## 监控指标

```rust
pub struct Metrics {
    rules_total: IntGauge,
    rules_active: IntGauge,
    executions_total: IntCounter,
    execution_duration: Histogram,
    execution_errors: IntCounter,
    node_execution_duration: HistogramVec,
}
```

## 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum RuleError {
    #[error("Invalid DAG: {0}")]
    InvalidDAG(String),
    
    #[error("Data not found: {0}")]
    DataNotFound(String),
    
    #[error("Expression error: {0}")]
    ExpressionError(String),
    
    #[error("Action failed: {0}")]
    ActionFailed(String),
    
    #[error("Timeout after {0:?}")]
    Timeout(Duration),
}
```

## 扩展性设计

### 添加自定义节点类型

```rust
pub trait NodeExecutor: Send + Sync {
    async fn execute(
        &self,
        node: &Node,
        inputs: HashMap<String, NodeOutput>,
        context: &ExecutionContext,
    ) -> Result<NodeOutput>;
}

// 注册自定义节点
engine.register_node_executor("custom_type", Box::new(CustomExecutor::new()));
```

### 添加自定义动作

```rust
pub struct CustomActionHandler;

#[async_trait]
impl ActionHandler for CustomActionHandler {
    async fn execute(&self, params: ActionParams) -> Result<String> {
        // 实现自定义动作逻辑
        Ok("Custom action executed".to_string())
    }
}

// 注册动作处理器
action_registry.register("custom_action", Box::new(CustomActionHandler));
```