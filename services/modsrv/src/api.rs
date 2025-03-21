use crate::config::Config;
use crate::error::Result;
use crate::storage::DataStore;
use crate::storage::hybrid_store::HybridStore;
use crate::model::{ModelDefinition, ModelWithActions, ControlAction};
use crate::template::TemplateInfo;
use crate::rules::{DagRule, NodeDefinition, EdgeDefinition, NodeType};
use crate::rules_engine::RuleExecutor;
use serde_json::{self, json, Value};
use log::{info, error};
use std::sync::Arc;
use warp::{self, Filter};
use std::convert::Infallible;
use serde::{Serialize, Deserialize};
use warp::http::StatusCode;
use std::collections::HashSet;

/// API module for the model service
/// Provides HTTP REST API for the model service
/// Uses warp for routing and request handling

// Create Instance Request
#[derive(Debug, Deserialize)]
struct CreateInstanceRequest {
    template_id: String,
    instance_id: String,
    config: Value
}

// Execute Operation Request
#[derive(Debug, Deserialize)]
struct ExecuteOperationRequest {
    instance_id: String,
    parameters: Value
}

/// Start the API server
pub async fn start_api_server(config: Config, store: Arc<HybridStore>) -> Result<()> {
    let store_filter = warp::any().map(move || store.clone());
    
    // Health check endpoint
    let health_route = warp::path!("api" / "health")
        .and(warp::get())
        .map(|| {
            warp::reply::json(&json!({
                "status": "ok",
                "version": env!("CARGO_PKG_VERSION")
            }))
        });
    
    // Get model endpoint
    let get_model_route = warp::path!("api" / "models" / String)
        .and(warp::get())
        .and(store_filter.clone())
        .map(|id: String, store: Arc<HybridStore>| {
            let model_key = format!("model:{}", id);
            info!("Getting model with key: {}", model_key);
            
            match store.get_string(&model_key) {
                Ok(model_json) => {
                    warp::reply::json(&json!({
                        "id": id,
                        "model": serde_json::from_str::<Value>(&model_json).unwrap_or(json!({}))
                    }))
                },
                Err(e) => {
                    info!("Error getting model: {:?}", e);
                    warp::reply::json(&json!({
                        "error": format!("Model not found: {}", e)
                    }))
                }
            }
        });
    
    // List models endpoint
    let list_models_route = warp::path!("api" / "models")
        .and(warp::get())
        .and(store_filter.clone())
        .map(|store: Arc<HybridStore>| {
            let model_pattern = "model:*";
            info!("Listing models with pattern: {}", model_pattern);
            
            match store.get_keys(model_pattern) {
                Ok(keys) => {
                    let models = keys.iter()
                        .map(|key| {
                            let id = key.replace("model:", "");
                            match store.get_string(key) {
                                Ok(model_json) => json!({
                                    "id": id,
                                    "model": serde_json::from_str::<Value>(&model_json).unwrap_or(json!({}))
                                }),
                                Err(_) => json!({
                                    "id": id,
                                    "error": "Failed to get model data"
                                })
                            }
                        })
                        .collect::<Vec<_>>();
                    
                    warp::reply::json(&json!({
                        "models": models
                    }))
                },
                Err(e) => {
                    info!("Error listing models: {:?}", e);
                    warp::reply::json(&json!({
                        "error": format!("Failed to list models: {}", e)
                    }))
                }
            }
        });
    
    // List templates endpoint
    let templates_route = warp::path!("api" / "templates")
        .and(warp::get())
        .and(store_filter.clone())
        .map(|store: Arc<HybridStore>| {
            info!("Listing templates");
            // In real implementation, we'd fetch templates from a templates directory
            // For now, return a mock template
            warp::reply::json(&json!({
                "templates": [
                    {
                        "id": "stepper_motor_template",
                        "name": "Stepper Motor Template",
                        "description": "Template for stepper motor control",
                        "file_path": "templates/stepper_motor.json"
                    }
                ]
            }))
        });
    
    // Create instance endpoint
    let create_instance_route = warp::path!("api" / "instances")
        .and(warp::post())
        .and(warp::body::json())
        .and(store_filter.clone())
        .map(|req: CreateInstanceRequest, store: Arc<HybridStore>| {
            info!("Creating instance with ID: {} from template: {}", 
                  req.instance_id, req.template_id);
            
            // In real implementation, we'd instantiate from template
            // For now, just create a basic model instance
            let instance_key = format!("model:{}", req.instance_id);
            
            let instance = json!({
                "id": req.instance_id,
                "name": req.config.get("name").and_then(|v| v.as_str()).unwrap_or("Unnamed Instance"),
                "description": req.config.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                "template_id": req.template_id,
                "parameters": req.config.get("parameters").unwrap_or(&json!({})),
                "enabled": true
            });
            
            match store.set_string(&instance_key, &instance.to_string()) {
                Ok(_) => {
                    warp::reply::with_status(
                        warp::reply::json(&json!({
                            "id": req.instance_id,
                            "status": "created",
                            "instance": instance
                        })),
                        StatusCode::CREATED
                    )
                },
                Err(e) => {
                    error!("Error creating instance: {:?}", e);
                    warp::reply::with_status(
                        warp::reply::json(&json!({
                            "error": format!("Failed to create instance: {}", e)
                        })),
                        StatusCode::INTERNAL_SERVER_ERROR
                    )
                }
            }
        });
    
    // List control operations endpoint
    let list_operations_route = warp::path!("api" / "control" / "operations")
        .and(warp::get())
        .map(|| {
            info!("Listing control operations");
            
            // In real implementation, we'd fetch available operations
            // For now, return mock operations
            warp::reply::json(&json!([
                "start_motor",
                "stop_motor",
                "set_speed"
            ]))
        });
    
    // Execute control operation endpoint
    let execute_operation_route = warp::path!("api" / "control" / "execute" / String)
        .and(warp::post())
        .and(warp::body::json())
        .and(store_filter.clone())
        .map(|operation: String, req: ExecuteOperationRequest, store: Arc<HybridStore>| {
            info!("Executing operation: {} on instance: {} with parameters: {:?}", 
                  operation, req.instance_id, req.parameters);
            
            // In real implementation, we'd execute the actual operation
            // For now, just record the operation attempt
            let operation_key = format!("operation:{}:{}", req.instance_id, operation);
            let operation_data = json!({
                "operation": operation,
                "instance_id": req.instance_id,
                "parameters": req.parameters,
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "status": "executed"
            });
            
            match store.set_string(&operation_key, &operation_data.to_string()) {
                Ok(_) => {
                    warp::reply::json(&json!({
                        "operation": operation,
                        "instance_id": req.instance_id,
                        "status": "success",
                        "message": format!("Operation {} executed successfully", operation)
                    }))
                },
                Err(e) => {
                    error!("Error executing operation: {:?}", e);
                    warp::reply::json(&json!({
                        "error": format!("Failed to execute operation: {}", e)
                    }))
                }
            }
        });
    
    // Rules API endpoints
    
    // List rules endpoint
    let list_rules_route = warp::path!("api" / "rules")
        .and(warp::get())
        .and(store_filter.clone())
        .map(|store: Arc<HybridStore>| {
            info!("Listing rules");
            
            // 首先从Redis重新加载rule数据，确保我们有最新的规则
            if let Err(e) = store.load_from_redis("rule:*") {
                error!("Error loading rules from Redis: {}", e);
            }
            
            // Get rule keys from the store - 使用完整的key模式，不依赖RedisConfig中的key_prefix
            // 使用固定的模式"rule:*"查询Redis
            match store.get_keys("rule:*") {
                Ok(keys) => {
                    let mut rules = Vec::new();
                    
                    info!("Found {} rules with pattern 'rule:*'", keys.len());
                    for key in &keys {
                        info!("Rule key: {}", key);
                        match store.get_string(key) {
                            Ok(rule_json) => {
                                info!("Rule JSON: {}", rule_json);
                                
                                // 尝试解析为DAG规则
                                match serde_json::from_str::<DagRule>(&rule_json) {
                                    Ok(rule) => {
                                        info!("Successfully parsed rule as DAG Rule: {}", rule.id);
                                        rules.push(rule);
                                    },
                                    Err(e) => {
                                        error!("Error parsing rule {} as DAG Rule: {}", key, e);
                                        
                                        // 尝试解析为简单规则格式，然后转换为DAG规则
                                        #[derive(Deserialize)]
                                        struct SimpleRule {
                                            id: String,
                                            name: String,
                                            conditions: Vec<serde_json::Value>,
                                            actions: Vec<serde_json::Value>,
                                            #[serde(default = "default_true")]
                                            enabled: bool,
                                        }
                                        
                                        fn default_true() -> bool {
                                            true
                                        }
                                        
                                        match serde_json::from_str::<SimpleRule>(&rule_json) {
                                            Ok(simple_rule) => {
                                                info!("Successfully parsed rule as Simple Rule: {}", simple_rule.id);
                                                
                                                // 创建条件节点
                                                let condition_node = NodeDefinition {
                                                    id: format!("{}_condition", simple_rule.id),
                                                    name: "条件检查".to_string(),
                                                    node_type: NodeType::Condition,
                                                    config: json!({ "conditions": simple_rule.conditions })
                                                };
                                                
                                                // 创建动作节点
                                                let action_node = NodeDefinition {
                                                    id: format!("{}_action", simple_rule.id),
                                                    name: "执行动作".to_string(),
                                                    node_type: NodeType::Action,
                                                    config: json!({ "actions": simple_rule.actions })
                                                };
                                                
                                                // 创建边
                                                let edge = EdgeDefinition {
                                                    from: condition_node.id.clone(),
                                                    to: action_node.id.clone(),
                                                    condition: None
                                                };
                                                
                                                // 创建DAG规则
                                                let dag_rule = DagRule {
                                                    id: simple_rule.id,
                                                    name: simple_rule.name,
                                                    description: "从简单规则转换而来".to_string(),
                                                    nodes: vec![condition_node, action_node],
                                                    edges: vec![edge],
                                                    enabled: simple_rule.enabled,
                                                    priority: 0
                                                };
                                                
                                                rules.push(dag_rule);
                                            },
                                            Err(e) => {
                                                error!("Error parsing rule {} as Simple Rule: {}", key, e);
                                            }
                                        }
                                    }
                                }
                            },
                            Err(e) => {
                                error!("Error retrieving rule {}: {}", key, e);
                            }
                        }
                    }
                    
                    info!("Returning {} rules", rules.len());
                    warp::reply::json(&json!({
                        "rules": rules,
                        "count": rules.len()
                    }))
                },
                Err(e) => {
                    error!("Error listing rules: {}", e);
                    warp::reply::json(&json!({
                        "error": format!("Failed to list rules: {}", e),
                        "rules": [],
                        "count": 0
                    }))
                }
            }
        });
    
    // Get rule by ID endpoint
    let get_rule_route = warp::path!("api" / "rules" / String)
        .and(warp::get())
        .and(store_filter.clone())
        .map(|rule_id: String, store: Arc<HybridStore>| {
            info!("Getting rule: {}", rule_id);
            
            let rule_key = format!("rule:{}", rule_id);
            
            match store.get_string(&rule_key) {
                Ok(rule_json) => {
                    match serde_json::from_str::<DagRule>(&rule_json) {
                        Ok(rule) => {
                            warp::reply::with_status(
                                warp::reply::json(&json!({
                                    "rule": rule
                                })),
                                StatusCode::OK
                            )
                        },
                        Err(e) => {
                            error!("Error parsing rule {}: {}", rule_id, e);
                            warp::reply::with_status(
                                warp::reply::json(&json!({
                                    "error": format!("Failed to parse rule: {}", e)
                                })),
                                StatusCode::INTERNAL_SERVER_ERROR
                            )
                        }
                    }
                },
                Err(e) => {
                    error!("Error retrieving rule {}: {}", rule_id, e);
                    warp::reply::with_status(
                        warp::reply::json(&json!({
                            "error": format!("Rule not found: {}", rule_id)
                        })),
                        StatusCode::NOT_FOUND
                    )
                }
            }
        });
    
    // Create rule endpoint
    let create_rule_route = warp::path!("api" / "rules")
        .and(warp::post())
        .and(warp::body::json())
        .and(store_filter.clone())
        .map(|rule: DagRule, store: Arc<HybridStore>| {
            info!("Creating rule: {}", rule.id);
            
            let rule_key = format!("rule:{}", rule.id);
            
            // Check if rule already exists
            if let Ok(true) = store.exists(&rule_key) {
                return warp::reply::with_status(
                    warp::reply::json(&json!({
                        "error": format!("Rule with ID {} already exists", rule.id)
                    })),
                    StatusCode::CONFLICT
                );
            }
            
            // Validate the rule structure
            // Check if all edges refer to valid nodes
            let node_ids: HashSet<String> = rule.nodes.iter().map(|n| n.id.clone()).collect();
            for edge in &rule.edges {
                if !node_ids.contains(&edge.from) || !node_ids.contains(&edge.to) {
                    return warp::reply::with_status(
                        warp::reply::json(&json!({
                            "error": format!("Edge ({} -> {}) refers to non-existent nodes", edge.from, edge.to)
                        })),
                        StatusCode::BAD_REQUEST
                    );
                }
            }
            
            // Serialize and store the rule
            match serde_json::to_string(&rule) {
                Ok(rule_json) => {
                    match store.set_string(&rule_key, &rule_json) {
                        Ok(_) => {
                            warp::reply::with_status(
                                warp::reply::json(&json!({
                                    "id": rule.id,
                                    "status": "created",
                                    "rule": rule
                                })),
                                StatusCode::CREATED
                            )
                        },
                        Err(e) => {
                            error!("Error storing rule: {}", e);
                            warp::reply::with_status(
                                warp::reply::json(&json!({
                                    "error": format!("Failed to store rule: {}", e)
                                })),
                                StatusCode::INTERNAL_SERVER_ERROR
                            )
                        }
                    }
                },
                Err(e) => {
                    error!("Error serializing rule: {}", e);
                    warp::reply::with_status(
                        warp::reply::json(&json!({
                            "error": format!("Failed to serialize rule: {}", e)
                        })),
                        StatusCode::INTERNAL_SERVER_ERROR
                    )
                }
            }
        });
    
    // Update rule endpoint
    let update_rule_route = warp::path!("api" / "rules" / String)
        .and(warp::put())
        .and(warp::body::json())
        .and(store_filter.clone())
        .map(|rule_id: String, updated_rule: DagRule, store: Arc<HybridStore>| {
            info!("Updating rule: {}", rule_id);
            
            // Check if IDs match
            if rule_id != updated_rule.id {
                return warp::reply::with_status(
                    warp::reply::json(&json!({
                        "error": "Rule ID in URL does not match rule ID in body"
                    })),
                    StatusCode::BAD_REQUEST
                );
            }
            
            let rule_key = format!("rule:{}", rule_id);
            
            // Check if rule exists
            if let Ok(false) = store.exists(&rule_key) {
                return warp::reply::with_status(
                    warp::reply::json(&json!({
                        "error": format!("Rule with ID {} not found", rule_id)
                    })),
                    StatusCode::NOT_FOUND
                );
            }
            
            // Validate the rule structure
            let node_ids: HashSet<String> = updated_rule.nodes.iter()
                .map(|n| n.id.clone()).collect();
                
            for edge in &updated_rule.edges {
                if !node_ids.contains(&edge.from) || !node_ids.contains(&edge.to) {
                    return warp::reply::with_status(
                        warp::reply::json(&json!({
                            "error": format!("Edge ({} -> {}) refers to non-existent nodes", edge.from, edge.to)
                        })),
                        StatusCode::BAD_REQUEST
                    );
                }
            }
            
            // Serialize and store the updated rule
            match serde_json::to_string(&updated_rule) {
                Ok(rule_json) => {
                    match store.set_string(&rule_key, &rule_json) {
                        Ok(_) => {
                            warp::reply::with_status(
                                warp::reply::json(&json!({
                                    "id": rule_id,
                                    "status": "updated",
                                    "rule": updated_rule
                                })),
                                StatusCode::OK
                            )
                        },
                        Err(e) => {
                            error!("Error updating rule: {}", e);
                            warp::reply::with_status(
                                warp::reply::json(&json!({
                                    "error": format!("Failed to update rule: {}", e)
                                })),
                                StatusCode::INTERNAL_SERVER_ERROR
                            )
                        }
                    }
                },
                Err(e) => {
                    error!("Error serializing rule: {}", e);
                    warp::reply::with_status(
                        warp::reply::json(&json!({
                            "error": format!("Failed to serialize rule: {}", e)
                        })),
                        StatusCode::INTERNAL_SERVER_ERROR
                    )
                }
            }
        });
    
    // Delete rule endpoint
    let delete_rule_route = warp::path!("api" / "rules" / String)
        .and(warp::delete())
        .and(store_filter.clone())
        .map(|rule_id: String, store: Arc<HybridStore>| {
            info!("Deleting rule: {}", rule_id);
            
            let rule_key = format!("rule:{}", rule_id);
            
            // Check if rule exists
            if let Ok(false) = store.exists(&rule_key) {
                return warp::reply::with_status(
                    warp::reply::json(&json!({
                        "error": format!("Rule with ID {} not found", rule_id)
                    })),
                    StatusCode::NOT_FOUND
                );
            }
            
            // Delete the rule
            match store.delete(&rule_key) {
                Ok(_) => {
                    warp::reply::with_status(
                        warp::reply::json(&json!({
                            "id": rule_id,
                            "status": "deleted"
                        })),
                        StatusCode::OK
                    )
                },
                Err(e) => {
                    error!("Error deleting rule: {}", e);
                    warp::reply::with_status(
                        warp::reply::json(&json!({
                            "error": format!("Failed to delete rule: {}", e)
                        })),
                        StatusCode::INTERNAL_SERVER_ERROR
                    )
                }
            }
        });
    
    // Execute rule endpoint
    let execute_rule_route = warp::path!("api" / "rules" / String / "execute")
        .and(warp::post())
        .and(warp::body::json())
        .and(store_filter.clone())
        .and_then(|rule_id: String, context: Option<Value>, store: Arc<HybridStore>| async move {
            info!("Executing rule: {}", rule_id);
            
            let rule_key = format!("rule:{}", rule_id);
            
            // Check if rule exists
            if let Ok(exists) = store.exists(&rule_key) {
                if !exists {
                    return Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&json!({
                            "error": format!("Rule with ID {} not found", rule_id)
                        })),
                        StatusCode::NOT_FOUND
                    ));
                }
            } else {
                return Ok::<_, warp::Rejection>(warp::reply::with_status(
                    warp::reply::json(&json!({
                        "error": "Failed to check rule existence"
                    })),
                    StatusCode::INTERNAL_SERVER_ERROR
                ));
            }
            
            // Get the rule
            let rule_result = store.get_string(&rule_key);
            let rule_json = match rule_result {
                Ok(json) => json,
                Err(e) => {
                    error!("Error retrieving rule {}: {}", rule_id, e);
                    return Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&json!({
                            "error": format!("Failed to retrieve rule: {}", e)
                        })),
                        StatusCode::INTERNAL_SERVER_ERROR
                    ));
                }
            };
            
            // Parse the rule
            let rule: DagRule = match serde_json::from_str(&rule_json) {
                Ok(r) => r,
                Err(e) => {
                    error!("Error parsing rule {}: {}", rule_id, e);
                    return Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&json!({
                            "error": format!("Failed to parse rule: {}", e)
                        })),
                        StatusCode::INTERNAL_SERVER_ERROR
                    ));
                }
            };
            
            // Check if rule is enabled
            if !rule.enabled {
                return Ok::<_, warp::Rejection>(warp::reply::with_status(
                    warp::reply::json(&json!({
                        "error": format!("Rule {} is disabled", rule_id)
                    })),
                    StatusCode::BAD_REQUEST
                ));
            }
            
            // Execute the rule
            let executor = RuleExecutor::new(store.clone());
            match executor.execute_rule(rule, context).await {
                Ok(result) => {
                    Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&json!({
                            "id": rule_id,
                            "status": "executed",
                            "result": result
                        })),
                        StatusCode::OK
                    ))
                },
                Err(e) => {
                    error!("Error executing rule {}: {}", rule_id, e);
                    Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&json!({
                            "error": format!("Failed to execute rule: {}", e)
                        })),
                        StatusCode::INTERNAL_SERVER_ERROR
                    ))
                }
            }
        });
    
    // Combine routes by boxing each filter
    let health_route = health_route.boxed();
    let get_model_route = get_model_route.boxed();
    let list_models_route = list_models_route.boxed();
    let templates_route = templates_route.boxed();
    let create_instance_route = create_instance_route.boxed();
    let list_operations_route = list_operations_route.boxed();
    let execute_operation_route = execute_operation_route.boxed();
    let list_rules_route = list_rules_route.boxed();
    let get_rule_route = get_rule_route.boxed();
    let create_rule_route = create_rule_route.boxed();
    let update_rule_route = update_rule_route.boxed();
    let delete_rule_route = delete_rule_route.boxed();
    let execute_rule_route = execute_rule_route.boxed();
    
    let routes = health_route
        .or(get_model_route)
        .or(list_models_route)
        .or(templates_route)
        .or(create_instance_route)
        .or(list_operations_route)
        .or(execute_operation_route)
        .or(list_rules_route)
        .or(get_rule_route)
        .or(create_rule_route)
        .or(update_rule_route)
        .or(delete_rule_route)
        .or(execute_rule_route)
        .with(warp::cors().allow_any_origin())
        .recover(handle_rejection);
    
    // Parse host and port from config
    let port = config.api.port;
    let host = [0, 0, 0, 0]; // Use default binding to all interfaces
    
    info!("Starting API server at http://{}:{}", 
          host.iter().map(|n| n.to_string()).collect::<Vec<_>>().join("."), 
          port);
    
    warp::serve(routes)
        .run((host, port))
        .await;
    
    Ok(())
}

// Custom rejection handler function
async fn handle_rejection(err: warp::Rejection) -> std::result::Result<impl warp::Reply, Infallible> {
    let code = if err.is_not_found() {
        warp::http::StatusCode::NOT_FOUND
    } else {
        warp::http::StatusCode::INTERNAL_SERVER_ERROR
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&json!({
            "error": "An error occurred",
            "code": code.as_u16()
        })),
        code,
    ))
}