use modsrv::api::create_routes;
use modsrv::model::{ModelInstance, ModelManager, ModelMapping};
use modsrv::template::TemplateManager;
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;
use voltage_rs::redis::EdgeRedis;
use warp::test::request;
use warp::Filter;

async fn setup_test_env() -> (TempDir, EdgeRedis, TemplateManager, ModelManager) {
    let temp_dir = TempDir::new().unwrap();
    let template_path = temp_dir.path().to_str().unwrap().to_string();

    // Create test templates
    let templates = vec![
        (
            "power_meter",
            r#"{
                "id": "power_meter",
                "data": {
                    "voltage": "V",
                    "current": "A",
                    "power": "kW",
                    "energy": "kWh"
                },
                "action": {
                    "reset": null,
                    "set_limit": "kW"
                }
            }"#,
        ),
        (
            "diesel_generator",
            r#"{
                "id": "diesel_generator",
                "data": {
                    "power": "kW",
                    "fuel": "%",
                    "temperature": "°C",
                    "status": null
                },
                "action": {
                    "start": null,
                    "stop": null,
                    "set_power": "kW"
                }
            }"#,
        ),
    ];

    for (name, content) in templates {
        let file_path = temp_dir.path().join(format!("{}.json", name));
        fs::write(&file_path, content).unwrap();
    }

    // Initialize components
    let redis = EdgeRedis::new("redis://localhost:6379".to_string())
        .await
        .unwrap();

    let template_manager = TemplateManager::new(template_path);
    template_manager.load_templates().await.unwrap();

    let model_manager = ModelManager::new(redis.clone(), template_manager.clone());

    (temp_dir, redis, template_manager, model_manager)
}

#[tokio::test]
async fn test_list_templates() {
    let (_temp_dir, _redis, template_manager, model_manager) = setup_test_env().await;
    let api = create_routes(model_manager, template_manager);

    let resp = request().method("GET").path("/templates").reply(&api).await;

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
    assert_eq!(body["total"], 2);
    assert_eq!(body["templates"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_get_template() {
    let (_temp_dir, _redis, template_manager, model_manager) = setup_test_env().await;
    let api = create_routes(model_manager, template_manager);

    let resp = request()
        .method("GET")
        .path("/templates/power_meter")
        .reply(&api)
        .await;

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
    assert_eq!(body["id"], "power_meter");
    assert!(body["data"].as_object().unwrap().contains_key("voltage"));
    assert!(body["action"].as_object().unwrap().contains_key("reset"));
}

#[tokio::test]
async fn test_get_template_not_found() {
    let (_temp_dir, _redis, template_manager, model_manager) = setup_test_env().await;
    let api = create_routes(model_manager, template_manager);

    let resp = request()
        .method("GET")
        .path("/templates/nonexistent")
        .reply(&api)
        .await;

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_create_model_from_template() {
    let (_temp_dir, _redis, template_manager, model_manager) = setup_test_env().await;
    let api = create_routes(model_manager, template_manager);

    let mut data_mapping = HashMap::new();
    data_mapping.insert("voltage".to_string(), 1);
    data_mapping.insert("current".to_string(), 2);
    data_mapping.insert("power".to_string(), 3);
    data_mapping.insert("energy".to_string(), 4);

    let mut action_mapping = HashMap::new();
    action_mapping.insert("reset".to_string(), 101);
    action_mapping.insert("set_limit".to_string(), 102);

    let create_request = serde_json::json!({
        "template_id": "power_meter",
        "model_id": "meter_001",
        "model_name": "Main Building Meter",
        "mapping": {
            "channel": 1001,
            "data": data_mapping,
            "action": action_mapping
        }
    });

    let resp = request()
        .method("POST")
        .path("/templates/create-model")
        .json(&create_request)
        .reply(&api)
        .await;

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
    assert_eq!(body["model"]["id"], "meter_001");
    assert_eq!(body["model"]["name"], "Main Building Meter");
    assert_eq!(body["model"]["template"], "power_meter");
}

#[tokio::test]
async fn test_create_model_invalid_template() {
    let (_temp_dir, _redis, template_manager, model_manager) = setup_test_env().await;
    let api = create_routes(model_manager, template_manager);

    let create_request = serde_json::json!({
        "template_id": "nonexistent",
        "model_id": "meter_001",
        "model_name": "Test Meter",
        "mapping": {
            "channel": 1001,
            "data": {},
            "action": {}
        }
    });

    let resp = request()
        .method("POST")
        .path("/templates/create-model")
        .json(&create_request)
        .reply(&api)
        .await;

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_list_models() {
    let (_temp_dir, _redis, template_manager, model_manager) = setup_test_env().await;

    // Create a test model directly
    let mut data_mapping = HashMap::new();
    data_mapping.insert("voltage".to_string(), 1);

    let model = ModelInstance {
        id: "meter_001".to_string(),
        name: "Test Meter".to_string(),
        template: Some("power_meter".to_string()),
        mapping: ModelMapping {
            channel: 1001,
            data: data_mapping,
            action: HashMap::new(),
        },
    };

    model_manager.create_model(model).await.unwrap();

    let api = create_routes(model_manager, template_manager);

    let resp = request().method("GET").path("/models").reply(&api).await;

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
    assert_eq!(body["total"], 1);
    assert_eq!(body["models"][0]["id"], "meter_001");
}

#[tokio::test]
async fn test_get_model() {
    let (_temp_dir, _redis, template_manager, model_manager) = setup_test_env().await;

    // Create a test model
    let model = ModelInstance {
        id: "meter_001".to_string(),
        name: "Test Meter".to_string(),
        template: Some("power_meter".to_string()),
        mapping: ModelMapping {
            channel: 1001,
            data: HashMap::new(),
            action: HashMap::new(),
        },
    };

    model_manager.create_model(model).await.unwrap();

    let api = create_routes(model_manager, template_manager);

    let resp = request()
        .method("GET")
        .path("/models/meter_001")
        .reply(&api)
        .await;

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
    assert_eq!(body["id"], "meter_001");
    assert_eq!(body["name"], "Test Meter");
}

#[tokio::test]
async fn test_get_model_values() {
    let (_temp_dir, redis, template_manager, model_manager) = setup_test_env().await;

    // Create a test model
    let mut data_mapping = HashMap::new();
    data_mapping.insert("voltage".to_string(), 1);
    data_mapping.insert("current".to_string(), 2);

    let model = ModelInstance {
        id: "meter_001".to_string(),
        name: "Test Meter".to_string(),
        template: Some("power_meter".to_string()),
        mapping: ModelMapping {
            channel: 1001,
            data: data_mapping,
            action: HashMap::new(),
        },
    };

    model_manager.create_model(model).await.unwrap();

    // Add test data to Redis
    let channel_key = "comsrv:1001:T";
    redis.hset(channel_key, "1", "220.5").await.unwrap();
    redis.hset(channel_key, "2", "15.3").await.unwrap();

    let api = create_routes(model_manager, template_manager);

    let resp = request()
        .method("GET")
        .path("/models/meter_001/values")
        .reply(&api)
        .await;

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
    assert_eq!(body["model_id"], "meter_001");
    assert_eq!(body["values"]["voltage"], 220.5);
    assert_eq!(body["values"]["current"], 15.3);
}

#[tokio::test]
async fn test_control_command() {
    let (_temp_dir, redis, template_manager, model_manager) = setup_test_env().await;

    // Create a test model
    let mut action_mapping = HashMap::new();
    action_mapping.insert("set_power".to_string(), 101);

    let model = ModelInstance {
        id: "gen_001".to_string(),
        name: "Test Generator".to_string(),
        template: Some("diesel_generator".to_string()),
        mapping: ModelMapping {
            channel: 2001,
            data: HashMap::new(),
            action: action_mapping,
        },
    };

    model_manager.create_model(model).await.unwrap();

    let api = create_routes(model_manager, template_manager);

    let control_request = serde_json::json!({
        "value": 150.0
    });

    let resp = request()
        .method("POST")
        .path("/models/gen_001/control/set_power")
        .json(&control_request)
        .reply(&api)
        .await;

    assert_eq!(resp.status(), 200);

    // Verify the command was written to Redis
    let control_key = "comsrv:2001:C";
    let value: String = redis.hget(control_key, "101").await.unwrap().unwrap();
    assert_eq!(value, "150.000000");
}

#[tokio::test]
async fn test_delete_model() {
    let (_temp_dir, _redis, template_manager, model_manager) = setup_test_env().await;

    // Create a test model
    let model = ModelInstance {
        id: "meter_001".to_string(),
        name: "Test Meter".to_string(),
        template: Some("power_meter".to_string()),
        mapping: ModelMapping {
            channel: 1001,
            data: HashMap::new(),
            action: HashMap::new(),
        },
    };

    model_manager.create_model(model).await.unwrap();

    let api = create_routes(model_manager.clone(), template_manager);

    // Delete the model
    let resp = request()
        .method("DELETE")
        .path("/models/meter_001")
        .reply(&api)
        .await;

    assert_eq!(resp.status(), 200);

    // Verify deletion
    let get_resp = request()
        .method("GET")
        .path("/models/meter_001")
        .reply(&api)
        .await;

    assert_eq!(get_resp.status(), 404);
}

#[tokio::test]
async fn test_health_endpoint() {
    let (_temp_dir, _redis, template_manager, model_manager) = setup_test_env().await;
    let api = create_routes(model_manager, template_manager);

    let resp = request().method("GET").path("/health").reply(&api).await;

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = serde_json::from_slice(resp.body()).unwrap();
    assert_eq!(body["status"], "healthy");
    assert_eq!(body["service"], "modsrv");
}
