//! Modbus polling engine tests

#[cfg(test)]
mod tests {
    use crate::core::config::types::protocol::TelemetryType;
    use crate::plugins::protocols::modbus::modbus_polling::{
        ModbusPoint, ModbusPollingConfig, ModbusPollingEngine,
    };
    use std::collections::HashMap;

    #[test]
    fn test_polling_config_creation() {
        let config = ModbusPollingConfig {
            default_interval_ms: 1000,
            enable_batch_reading: true,
            max_batch_size: 100,
            read_timeout_ms: 5000,
            slave_configs: HashMap::new(),
        };

        let engine = ModbusPollingEngine::new(config);

        // Basic test to ensure creation works
        let mut engine_mut = engine;
        let points = vec![ModbusPoint {
            point_id: "1001".to_string(),
            telemetry_type: TelemetryType::Telemetry,
            slave_id: 1,
            function_code: 3,
            register_address: 0,
            scale_factor: None,
            data_format: "uint16".to_string(),
            register_count: 1,
            byte_order: None,
        }];

        engine_mut.add_points(points);
    }

    // TODO: Add async polling tests when test infrastructure is ready
}
