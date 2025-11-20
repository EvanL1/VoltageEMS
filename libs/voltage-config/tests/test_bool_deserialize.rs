use voltage_config::comsrv::TelemetryPoint;

#[test]
fn test_reverse_accepts_json_integer() {
    // API scenario: JSON integers
    let json_zero = r#"{
        "point_id": 91,
        "signal_name": "test_int_0",
        "scale": 1.0,
        "offset": 0.0,
        "data_type": "float",
        "reverse": 0,
        "unit": "",
        "description": ""
    }"#;

    let point_zero: TelemetryPoint =
        serde_json::from_str(json_zero).expect("Should deserialize JSON integer 0");
    assert!(!point_zero.reverse);

    // Test integer 1
    let json_one = r#"{
        "point_id": 92,
        "signal_name": "test_int_1",
        "scale": 1.0,
        "offset": 0.0,
        "data_type": "float",
        "reverse": 1,
        "unit": "",
        "description": ""
    }"#;

    let point_one: TelemetryPoint =
        serde_json::from_str(json_one).expect("Should deserialize JSON integer 1");
    assert!(point_one.reverse);
}

#[test]
fn test_reverse_accepts_json_boolean() {
    // API scenario: JSON boolean values
    let json = r#"{
        "point_id": 93,
        "signal_name": "test",
        "scale": 1.0,
        "offset": 0.0,
        "data_type": "float",
        "reverse": false,
        "unit": "",
        "description": ""
    }"#;

    let point: TelemetryPoint =
        serde_json::from_str(json).expect("Should deserialize JSON boolean");
    assert!(!point.reverse);

    // Test true value
    let json_true = r#"{
        "point_id": 94,
        "signal_name": "test2",
        "scale": 1.0,
        "offset": 0.0,
        "data_type": "float",
        "reverse": true,
        "unit": "",
        "description": ""
    }"#;

    let point_true: TelemetryPoint =
        serde_json::from_str(json_true).expect("Should deserialize JSON boolean true");
    assert!(point_true.reverse);
}

#[test]
fn test_reverse_accepts_string_values() {
    // CSV scenario: string values
    let json_str_false = r#"{
        "point_id": 95,
        "signal_name": "test3",
        "scale": 1.0,
        "offset": 0.0,
        "data_type": "float",
        "reverse": "false",
        "unit": "",
        "description": ""
    }"#;

    let point: TelemetryPoint =
        serde_json::from_str(json_str_false).expect("Should deserialize string 'false'");
    assert!(!point.reverse);

    // Test numeric string "1"
    let json_str_one = r#"{
        "point_id": 96,
        "signal_name": "test4",
        "scale": 1.0,
        "offset": 0.0,
        "data_type": "float",
        "reverse": "1",
        "unit": "",
        "description": ""
    }"#;

    let point_one: TelemetryPoint =
        serde_json::from_str(json_str_one).expect("Should deserialize string '1'");
    assert!(point_one.reverse);

    // Test "yes"
    let json_str_yes = r#"{
        "point_id": 97,
        "signal_name": "test5",
        "scale": 1.0,
        "offset": 0.0,
        "data_type": "float",
        "reverse": "yes",
        "unit": "",
        "description": ""
    }"#;

    let point_yes: TelemetryPoint =
        serde_json::from_str(json_str_yes).expect("Should deserialize string 'yes'");
    assert!(point_yes.reverse);
}

#[test]
fn test_reverse_with_serde_json_value() {
    // Simulate API batch operation: parse to Value first, then convert to concrete type
    let json = r#"{
        "point_id": 93,
        "signal_name": "test",
        "scale": 1.0,
        "offset": 0.0,
        "data_type": "float",
        "reverse": false,
        "unit": "",
        "description": ""
    }"#;

    // Step 1: Parse to Value (simulates PointBatchCreateItem.data)
    let value: serde_json::Value = serde_json::from_str(json).expect("Should parse to Value");

    // Step 2: Convert from Value to TelemetryPoint (simulates process_create_operation)
    let point: TelemetryPoint = serde_json::from_value(value)
        .expect("Should deserialize from Value (this is the actual API use case)");

    assert!(!point.reverse);
    assert_eq!(point.base.signal_name, "test");
}
